"""Windows Z-Image generation protocol v2 JSONL."""

import faulthandler
import json
import os
import sys
import threading
from pathlib import Path

from . import model_spec, worker_identity

MAX_FRAME = 64 * 1024
FAMILY = "image-studio.generation"


def isolate_stdout():
    sys.stdout.flush()
    protocol = os.fdopen(os.dup(1), "w", encoding="utf-8", buffering=1)
    os.dup2(2, 1)
    sys.stdout = sys.stderr
    return protocol


def read_frame(stream):
    line = stream.readline(MAX_FRAME + 2)
    if not line:
        raise EOFError("control channel closed")
    if not line.endswith(b"\n") or len(line.rstrip(b"\r\n")) > MAX_FRAME:
        raise ValueError("control frame exceeds 64 KiB or lacks newline")
    return json.loads(line)


def fields(value, expected):
    if not isinstance(value, dict) or set(value) != set(expected):
        raise ValueError("unknown or missing protocol fields")


def valid_identifier(value):
    return (
        isinstance(value, str)
        and 0 < len(value.encode()) <= 256
        and all(ord(char) >= 32 for char in value)
    )


def validate_envelope(value):
    fields(
        value,
        (
            "family",
            "version",
            "session_id",
            "request_id",
            "sequence",
            "execution",
            "message",
        ),
    )
    if (
        value["family"] != FAMILY
        or type(value["sequence"]) is not int
        or value["sequence"] < 0
        or not valid_identifier(value["session_id"])
        or not valid_identifier(value["request_id"])
    ):
        raise ValueError("wrong generation family, sequence, or identity")


def safe_directory(value):
    path = Path(value)
    return (
        isinstance(value, str)
        and path.is_absolute()
        and not path.is_symlink()
        and path.is_dir()
        and path.resolve() == path
    )


def validate_request(request, identity, spec):
    fields(
        request,
        (
            "model_id",
            "capability_id",
            "recipe_id",
            "recipe_version",
            "runtime_lock_id",
            "model_snapshot_id",
            "instruction",
            "seed",
            "sampling_steps",
            "parameters",
            "inputs",
            "outputs",
            "model_path",
            "output_dir",
        ),
    )
    if (
        request["model_id"] != spec["model_id"]
        or request["capability_id"] != spec["capability"]
        or request["recipe_id"] != spec["recipe"]
        or request["recipe_version"] != 1
        or request["runtime_lock_id"] != identity["runtime_lock_id"]
        or request["model_snapshot_id"] != identity["model_snapshot_id"]
        or request["sampling_steps"] != spec["steps"]
        or type(request["seed"]) is not int
        or not 0 <= request["seed"] <= 4294967295
    ):
        raise ValueError("model, runtime, recipe, or seed mismatch")
    prompt = request["instruction"]
    if (
        not isinstance(prompt, str)
        or not prompt.strip()
        or "\0" in prompt
        or len(prompt.encode()) > 16384
    ):
        raise ValueError("prompt is empty or exceeds 16 KiB")
    from . import expected_parameters

    if request["parameters"] != expected_parameters(request["seed"]):
        raise ValueError("effective parameters differ from the frozen recipe")
    for name in ("model_path", "output_dir"):
        if not safe_directory(request[name]):
            raise ValueError("model and output paths must be normalized directories")
    model_path = Path(request["model_path"])
    output_dir = Path(request["output_dir"])
    if model_path.name != identity["model_snapshot_id"]:
        raise ValueError("model path does not match the selected snapshot")
    if model_path == output_dir or model_path in output_dir.parents:
        raise ValueError("output directory cannot be inside the model")
    if request["inputs"] != []:
        raise ValueError("Windows Z-Image accepts no source image")
    if request["outputs"] != [
        {
            "role": "primary",
            "data_type": "image_rgb",
            "width": 1024,
            "height": 1024,
            "max_bytes": 32 * 1024 * 1024,
        }
    ]:
        raise ValueError("output differs from the frozen recipe")


def probe(identity, model_path=None):
    import importlib.metadata
    import platform

    import bitsandbytes
    import diffusers
    import torch
    import transformers
    from bitsandbytes.nn import Linear4bit

    packages = {}
    for name in (
        "torch",
        "diffusers",
        "transformers",
        "accelerate",
        "bitsandbytes",
        "numpy",
        "safetensors",
        "tokenizers",
    ):
        packages[name] = importlib.metadata.version(name)
    if (
        packages["torch"] != "2.10.0+cu126"
        or packages["diffusers"] != "0.36.0"
        or packages["transformers"] != "4.57.3"
        or packages["accelerate"] != "1.12.0"
        or packages["bitsandbytes"] != "0.48.2"
    ):
        raise RuntimeError("private package identity mismatch")
    if not torch.cuda.is_available() or torch.version.cuda != "12.6":
        raise RuntimeError("CUDA 12.6 is unavailable")
    selected = None
    for index in range(torch.cuda.device_count()):
        properties = torch.cuda.get_device_properties(index)
        if (
            "NVIDIA" in properties.name.upper()
            and (properties.major, properties.minor) == (8, 6)
            and properties.total_memory >= 10 * 1024**3
        ):
            selected = index
            break
    if selected is None:
        raise RuntimeError("required NVIDIA sm86 device was not found")
    torch.cuda.set_device(selected)
    left = torch.randn((32, 32), device="cuda", dtype=torch.bfloat16)
    product = left @ left
    if product.dtype != torch.bfloat16 or not torch.isfinite(product).all():
        raise RuntimeError("BF16 probe failed")
    layer = Linear4bit(32, 16, quant_type="nf4", compute_dtype=torch.bfloat16).cuda()
    output = layer(torch.randn((2, 32), device="cuda", dtype=torch.bfloat16))
    if output.shape != (2, 16) or not torch.isfinite(output).all():
        raise RuntimeError("NF4 probe failed")
    if model_path is not None:
        model = Path(model_path)
        if not model.is_dir() or not (model / "model_index.json").is_file():
            raise RuntimeError("model closure is missing model_index.json")
    properties = torch.cuda.get_device_properties(selected)
    return {
        "identity": identity,
        "python": platform.python_version(),
        "device": "gpu",
        "device_name": properties.name,
        "compute_capability": f"{properties.major}.{properties.minor}",
        "packages": packages,
        "imports": {
            "diffusers": diffusers.__version__,
            "transformers": transformers.__version__,
            "bitsandbytes": bitsandbytes.__version__,
        },
    }


def run(protocol, stream, identity, spec):
    from .backend import Cancelled, generate, warm_imports

    capabilities = [spec["capability"]]
    hello = read_frame(stream)
    validate_envelope(hello)
    message = hello["message"]
    fields(
        message,
        ("type", "versions", "build_id", "expected_worker", "capabilities"),
    )
    if (
        message["type"] != "hello"
        or hello["sequence"] != 0
        or hello["version"] is not None
        or hello["execution"] is not None
        or message["expected_worker"] != identity
        or message["capabilities"] != capabilities
        or message["versions"] != {"min": 2, "max": 2}
    ):
        raise ValueError("generation handshake identity mismatch")
    sequence = 0
    active = None

    def send(message):
        nonlocal sequence
        source = active or hello
        envelope = {
            "family": FAMILY,
            "version": 2,
            "session_id": hello["session_id"],
            "request_id": source["request_id"],
            "sequence": sequence,
            "execution": source["execution"],
            "message": message,
        }
        data = json.dumps(envelope, ensure_ascii=False, separators=(",", ":"))
        if len(data.encode()) > MAX_FRAME:
            raise ValueError("outgoing frame too large")
        protocol.write(data + "\n")
        protocol.flush()
        sequence += 1

    send(
        {
            "type": "hello_accepted",
            "versions": {"min": 2, "max": 2},
            "selected_version": 2,
            "identity": identity,
            "capabilities": capabilities,
        }
    )
    active = read_frame(stream)
    validate_envelope(active)
    fields(active["message"], ("type", "request"))
    if (
        active["session_id"] != hello["session_id"]
        or active["sequence"] != 1
        or active["version"] != 2
        or active["message"]["type"] != "execute_v2"
    ):
        raise ValueError("execute identity mismatch")
    fields(active["execution"], ("job_id", "attempt_id", "execution_token"))
    cancel = threading.Event()
    control_error = []

    def control():
        try:
            value = read_frame(stream)
            validate_envelope(value)
            if (
                value["session_id"] != active["session_id"]
                or value["request_id"] != active["request_id"]
                or value["execution"] != active["execution"]
                or value["sequence"] != 2
                or value["version"] != 2
                or value["message"] != {"type": "cancel"}
            ):
                raise ValueError("invalid cancel")
            cancel.set()
        except EOFError:
            os._exit(70)
        except Exception as exc:
            control_error.append(exc)
            cancel.set()

    try:
        request = active["message"]["request"]
        validate_request(request, identity, spec)
        warm_imports()
        print("worker request validated", file=sys.stderr)

        def progress(stage, completed=None, total=None):
            send(
                {
                    "type": "progress",
                    "stage": stage,
                    "completed": completed,
                    "total": total,
                }
            )

        threading.Thread(target=control, daemon=True).start()
        candidate = generate(request, progress, cancel)
        if cancel.is_set():
            raise Cancelled()
        send({"type": "output_candidate_v2", "candidate": candidate})
    except Cancelled:
        if control_error:
            send(
                {
                    "type": "error",
                    "code": "protocol_error",
                    "summary": str(control_error[0])[:1024],
                }
            )
        else:
            send({"type": "cancelled"})
    except Exception as exc:
        code = "validation_error" if isinstance(exc, ValueError) else "inference_error"
        send(
            {
                "type": "error",
                "code": code,
                "summary": (str(exc) or type(exc).__name__)[:1024],
            }
        )
        return 1
    return 0


def parse_arguments(argv):
    probe_mode = False
    model_path = None
    index = 0
    while index < len(argv):
        value = argv[index]
        if value == "--probe" and not probe_mode:
            probe_mode = True
            index += 1
        elif probe_mode and model_path is None:
            model_path = value
            index += 1
        else:
            raise ValueError("unsupported worker arguments")
    return probe_mode, model_path


def main():
    protocol = isolate_stdout()
    if os.environ.get("STUDIO_WORKER_DEBUG") == "1":
        faulthandler.dump_traceback_later(20, repeat=True)
    for key in (
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_HUB_DISABLE_TELEMETRY",
        "DO_NOT_TRACK",
    ):
        os.environ[key] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"
    probe_mode, model_path = parse_arguments(sys.argv[1:])
    identity = worker_identity()
    spec = model_spec()
    if probe_mode:
        protocol.write(
            json.dumps(probe(identity, model_path), ensure_ascii=False) + "\n"
        )
        protocol.flush()
        return 0
    control = os.fdopen(os.dup(0), "rb", buffering=0)
    return run(protocol, control, identity, spec)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(
            f"worker startup failed: {type(exc).__name__}: {str(exc)[:1024]}",
            file=sys.stderr,
        )
        sys.exit(70)
