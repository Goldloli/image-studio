"""Windows diffusers generation protocol v2 JSONL.

The shell (frame format, hello handshake, cancel control channel, output
candidate framing, bounded error summaries, offline environment) is forked from
``tools/zimage-cuda-worker``. It serves one frozen model snapshot per process;
the snapshot id is passed by the host (``--model-snapshot-id``), exactly like
the installer/supervisor worker arguments. No torch/diffusers import happens
before stdout isolation and request validation.
"""

import importlib
import json
import os
import sys
import threading
from pathlib import Path

from . import (
    CAP_KLEIN_EDIT,
    model_spec,
    output_specs,
    recipe_for,
    worker_identity,
)

MAX_FRAME = 64 * 1024
FAMILY = "image-studio.generation"
MAX_PROMPT_BYTES = 16384
MAX_SOURCE_PIXELS = 4 * 1024 * 1024

REQUEST_FIELDS = (
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
)

PROBE_PACKAGES = (
    "torch",
    "diffusers",
    "transformers",
    "accelerate",
    "bitsandbytes",
    "safetensors",
    "huggingface-hub",
    "tokenizers",
    "numpy",
    "pillow",
)


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


def _sha256_file(path: Path) -> str:
    import hashlib

    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def validate_request(request, snapshot_id):
    """Validate one v2 execute request against the frozen Windows recipe."""
    fields(request, REQUEST_FIELDS)
    identity = worker_identity(snapshot_id)
    spec = model_spec(snapshot_id)
    capability = request["capability_id"]
    if capability not in spec["capabilities"]:
        raise ValueError("capability is not part of the selected model snapshot")
    recipe_id, steps = recipe_for(snapshot_id, capability)
    if (
        request["model_id"] != spec["model_id"]
        or request["recipe_id"] != recipe_id
        or request["recipe_version"] != 1
        or request["runtime_lock_id"] != identity["runtime_lock_id"]
        or request["model_snapshot_id"] != identity["model_snapshot_id"]
        or request["sampling_steps"] != steps
    ):
        raise ValueError("model, runtime, recipe, or step count mismatch")
    seed = request["seed"]
    if type(seed) is not int or not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError("seed must be a resolved 32-bit unsigned integer")
    from . import expected_parameters

    if request["parameters"] != expected_parameters(snapshot_id, capability, seed):
        raise ValueError("effective parameters differ from the frozen recipe")
    prompt = request["instruction"]
    if (
        not isinstance(prompt, str)
        or not prompt.strip()
        or "\0" in prompt
        or len(prompt.encode()) > MAX_PROMPT_BYTES
    ):
        raise ValueError("instruction is empty or exceeds 16 KiB")
    for name in ("model_path", "output_dir"):
        if not safe_directory(request[name]):
            raise ValueError("model and output paths must be normalized directories")
    model_path = Path(request["model_path"])
    output_dir = Path(request["output_dir"])
    if model_path == output_dir or model_path in output_dir.parents:
        raise ValueError("output directory cannot be inside the model")
    if not (model_path / "model_index.json").is_file():
        raise ValueError("model snapshot directory is incomplete")
    inputs = request["inputs"]
    if capability == CAP_KLEIN_EDIT:
        if len(inputs) != 1:
            raise ValueError("Klein edit requires exactly one source image")
        source = inputs[0]
        fields(source, ("role", "path", "width", "height", "sha256"))
        if (
            source["role"] != "source"
            or type(source["width"]) is not int
            or type(source["height"]) is not int
            or source["width"] < 16
            or source["height"] < 16
            or source["width"] * source["height"] > MAX_SOURCE_PIXELS
            or not isinstance(source["sha256"], str)
            or len(source["sha256"]) != 64
        ):
            raise ValueError("invalid source asset descriptor")
        source_path = Path(source["path"])
        if (
            not source_path.is_absolute()
            or source_path.is_symlink()
            or not source_path.is_file()
            or source_path.resolve() != source_path
            or source_path.parent != output_dir
        ):
            raise ValueError("source image must be an Attempt-local regular file")
        if _sha256_file(source_path) != source["sha256"]:
            raise ValueError("source image hash mismatch")
    elif inputs:
        raise ValueError("this recipe accepts no source image")
    if request["outputs"] != output_specs(snapshot_id):
        raise ValueError("output specification differs from the frozen recipe")


def probe(snapshot_id, model_path=None):
    import importlib.metadata
    import platform

    import torch

    expected = {
        "torch": "2.10.0+cu126",
        "diffusers": "0.41.0",
        "transformers": "5.3.0",
        "accelerate": "1.12.0",
        "bitsandbytes": "0.50.2",
    }
    packages = {}
    for name in PROBE_PACKAGES:
        try:
            packages[name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            continue
    for name, version in expected.items():
        if packages.get(name) != version:
            raise RuntimeError(f"private package identity mismatch: {name}")
    from .backends.common import select_device

    index, properties = select_device()
    torch.cuda.set_device(index)
    left = torch.randn((32, 32), device="cuda", dtype=torch.bfloat16)
    product = left @ left
    if product.dtype != torch.bfloat16 or not torch.isfinite(product).all():
        raise RuntimeError("BF16 probe failed")
    from bitsandbytes.nn import Linear4bit

    layer = Linear4bit(32, 16, quant_type="nf4", compute_dtype=torch.bfloat16).cuda()
    output = layer(torch.randn((2, 32), device="cuda", dtype=torch.bfloat16))
    if output.shape != (2, 16) or not torch.isfinite(output).all():
        raise RuntimeError("NF4 probe failed")
    if torch.cuda.is_available():
        torch.cuda.empty_cache()
    if model_path is not None:
        model = Path(model_path)
        if not model.is_dir() or not (model / "model_index.json").is_file():
            raise RuntimeError("model closure is missing model_index.json")
    return {
        "identity": worker_identity(snapshot_id),
        "python": platform.python_version(),
        "device": "gpu",
        "device_name": properties.name,
        "compute_capability": f"{properties.major}.{properties.minor}",
        "packages": packages,
    }


def run_prepare(snapshot_id, model_dir, prepared_dir, reuse):
    from .backends.prepare import prepare_cache

    return prepare_cache(snapshot_id, model_dir, prepared_dir, reuse)


def run(protocol, stream, snapshot_id):
    from .backends import common

    identity = worker_identity(snapshot_id)
    spec = model_spec(snapshot_id)
    capabilities = spec["capabilities"]
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
        except Exception as error:  # noqa: BLE001 - report through terminal frame
            control_error.append(error)
            cancel.set()

    try:
        request = active["message"]["request"]
        validate_request(request, snapshot_id)
        common.warm_imports()
        print("worker request validated", file=sys.stderr, flush=True)
        threading.Thread(target=control, daemon=True).start()

        def progress(stage, completed=None, total=None):
            send(
                {
                    "type": "progress",
                    "stage": stage,
                    "completed": completed,
                    "total": total,
                }
            )

        module = importlib.import_module(f".backends.{spec['kind']}", __package__)
        candidate = module.generate(request, progress, cancel)
        if cancel.is_set():
            raise common.Cancelled()
        send({"type": "output_candidate_v2", "candidate": candidate})
    except common.Cancelled:
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
    except Exception as error:  # noqa: BLE001 - terminal error frame
        code = "validation_error" if isinstance(error, ValueError) else "inference_error"
        send(
            {
                "type": "error",
                "code": code,
                "summary": (str(error) or type(error).__name__)[:1024],
            }
        )
        return 1
    return 0


def parse_arguments(argv):
    snapshot_id = None
    probe_mode = False
    prepare_mode = False
    model_path = None
    model_dir = None
    prepared_dir = None
    reuse = True
    index = 0
    while index < len(argv):
        value = argv[index]
        if value == "--model-snapshot-id" and index + 1 < len(argv):
            snapshot_id = argv[index + 1]
            index += 2
        elif value == "--probe" and not probe_mode and not prepare_mode:
            probe_mode = True
            index += 1
        elif value == "--prepare" and not probe_mode and not prepare_mode:
            prepare_mode = True
            index += 1
        elif value == "--model-dir" and index + 1 < len(argv):
            model_dir = Path(argv[index + 1])
            index += 2
        elif value == "--prepared-dir" and index + 1 < len(argv):
            prepared_dir = Path(argv[index + 1])
            index += 2
        elif value == "--no-reuse-prepared":
            reuse = False
            index += 1
        elif (
            probe_mode
            and model_path is None
            and not value.startswith("--")
            and Path(value).is_absolute()
        ):
            model_path = Path(value)
            index += 1
        else:
            raise ValueError("unsupported worker arguments")
    if snapshot_id is None:
        raise ValueError("--model-snapshot-id is required")
    model_spec(snapshot_id)  # fail fast on an unknown snapshot id
    return {
        "snapshot_id": snapshot_id,
        "probe": probe_mode,
        "prepare": prepare_mode,
        "model_path": model_path,
        "model_dir": model_dir,
        "prepared_dir": prepared_dir,
        "reuse": reuse,
    }


def main():
    protocol = isolate_stdout()
    for key in (
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_HUB_DISABLE_TELEMETRY",
        "DO_NOT_TRACK",
    ):
        os.environ[key] = "1"
    os.environ["TOKENIZERS_PARALLELISM"] = "false"
    arguments = parse_arguments(sys.argv[1:])
    snapshot_id = arguments["snapshot_id"]
    if arguments["probe"]:
        protocol.write(
            json.dumps(probe(snapshot_id, arguments["model_path"]), ensure_ascii=False)
            + "\n"
        )
        protocol.flush()
        return 0
    if arguments["prepare"]:
        if arguments["model_dir"] is None or arguments["prepared_dir"] is None:
            raise ValueError("--prepare requires --model-dir and --prepared-dir")
        report = run_prepare(
            snapshot_id,
            arguments["model_dir"],
            arguments["prepared_dir"],
            arguments["reuse"],
        )
        protocol.write(json.dumps(report, ensure_ascii=False) + "\n")
        protocol.flush()
        return 0 if report.get("status") in ("prepared", "reused", "not_required") else 1
    control = os.fdopen(os.dup(0), "rb", buffering=0)
    return run(protocol, control, snapshot_id)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:  # noqa: BLE001 - startup failure channel
        print(
            f"worker startup failed: {type(exc).__name__}: {str(exc)[:1024]}",
            file=sys.stderr,
        )
        sys.exit(70)
