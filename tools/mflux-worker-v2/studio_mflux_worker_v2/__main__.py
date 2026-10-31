"""Klein generation protocol v2 JSONL; no ML import before stdout isolation."""

import json
import hashlib
import os
import sys
import threading
from pathlib import Path

from . import CAPABILITIES, IDENTITY, TEXT_CAPABILITY

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


def _safe_directory(value):
    path = Path(value)
    return (
        isinstance(value, str)
        and path.is_absolute()
        and not path.is_symlink()
        and path.is_dir()
        and path.resolve() == path
    )


def validate_request(request):
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
    pair = (request["recipe_id"], request["capability_id"])
    if pair not in (
        ("klein4b-text-v1", TEXT_CAPABILITY),
        ("klein4b-edit-v1", "image.edit.flux2-klein-4b.v1"),
    ):
        raise ValueError("unsupported Klein recipe.")
    if (
        request["model_id"] != "flux2-klein-4b-distilled"
        or request["recipe_version"] != 1
        or request["runtime_lock_id"] != IDENTITY["runtime_lock_id"]
        or request["model_snapshot_id"] != IDENTITY["model_snapshot_id"]
        or request["sampling_steps"] != 4
        or type(request["seed"]) is not int
        or not 0 <= request["seed"] <= 4294967295
    ):
        raise ValueError("Klein model, runtime, recipe, or seed mismatch.")
    prompt = request["instruction"]
    if (
        not isinstance(prompt, str)
        or not prompt.strip()
        or "\0" in prompt
        or len(prompt.encode()) > 16384
    ):
        raise ValueError("Klein instruction is empty or exceeds 16 KiB.")
    parameters = request["parameters"]
    expected_parameters = {
        "seed": request["seed"],
        "steps": 4,
        "quantization": 4,
        "guidance": 1.0,
        "scheduler": "flow_match_euler_discrete",
        "recipe": request["recipe_id"],
        "recipe_version": 1,
        "token_budget": 512,
        "thinking": False,
        "truncation": False,
    }
    if parameters != expected_parameters:
        raise ValueError("Klein effective parameters differ from the frozen recipe.")
    for name in ("model_path", "output_dir"):
        if not _safe_directory(request[name]):
            raise ValueError("Klein paths must be normalized local directories.")
    model_path = Path(request["model_path"])
    output_dir = Path(request["output_dir"])
    if model_path == output_dir or model_path in output_dir.parents:
        raise ValueError("output directory cannot be inside the model")
    inputs = request["inputs"]
    if request["recipe_id"] == "klein4b-text-v1":
        if inputs != []:
            raise ValueError("Klein text-to-image accepts no source image.")
    else:
        if len(inputs) != 1:
            raise ValueError("Klein edit requires exactly one source image.")
        source = inputs[0]
        fields(source, ("role", "path", "width", "height", "sha256"))
        if (
            source["role"] != "source"
            or type(source["width"]) is not int
            or type(source["height"]) is not int
            or source["width"] < 16
            or source["height"] < 16
            or source["width"] * source["height"] > 4 * 1024 * 1024
            or len(source["sha256"]) != 64
        ):
            raise ValueError("invalid Klein source descriptor.")
        source_path = Path(source["path"])
        if (
            not source_path.is_absolute()
            or source_path.is_symlink()
            or not source_path.is_file()
            or source_path.resolve() != source_path
            or source_path.parent != output_dir
        ):
            raise ValueError("Klein source must be an Attempt-local regular file.")
        with source_path.open("rb") as stream:
            if hashlib.file_digest(stream, "sha256").hexdigest() != source["sha256"]:
                raise ValueError("Klein source hash mismatch.")
    outputs = request["outputs"]
    if len(outputs) != 1:
        raise ValueError("Klein requires exactly one output.")
    output = outputs[0]
    fields(output, ("role", "data_type", "width", "height", "max_bytes"))
    if output != {
        "role": "primary",
        "data_type": "image_rgb",
        "width": 1024,
        "height": 1024,
        "max_bytes": 32 * 1024 * 1024,
    }:
        raise ValueError("Klein output differs from the frozen recipe.")


def probe():
    import importlib.metadata
    import platform

    import mlx.core as mx
    import mflux  # noqa: F401
    import torch  # noqa: F401
    import transformers  # noqa: F401

    mx.set_default_device(mx.gpu)
    result = mx.array([1.0, 2.0]) + mx.array([2.0, 3.0])
    mx.eval(result)
    if result.tolist() != [3.0, 5.0] or platform.machine() != "arm64":
        raise RuntimeError("MLX device probe failed")
    return dict(
        identity=IDENTITY,
        python=platform.python_version(),
        device="gpu",
        device_name=mx.metal.device_info()["device_name"],
        packages={
            name: importlib.metadata.version(name)
            for name in ("mflux", "mlx", "torch", "transformers")
        },
    )


def run(protocol, stream):
    from .backend import Cancelled, generate

    hello = read_frame(stream)
    validate_envelope(hello)
    msg = hello["message"]
    fields(
        msg,
        ("type", "versions", "build_id", "expected_worker", "capabilities"),
    )
    if (
        msg["type"] != "hello"
        or hello["sequence"] != 0
        or hello["version"] is not None
        or hello["execution"] is not None
        or msg["expected_worker"] != IDENTITY
        or msg["capabilities"] != CAPABILITIES
        or msg["versions"] != {"min": 2, "max": 2}
    ):
        raise ValueError("generation handshake identity mismatch")
    sequence = 0
    active = None

    def send(message):
        nonlocal sequence
        source = active or hello
        envelope = dict(
            family=FAMILY,
            version=2,
            session_id=hello["session_id"],
            request_id=source["request_id"],
            sequence=sequence,
            execution=source["execution"],
            message=message,
        )
        data = json.dumps(envelope, ensure_ascii=False, separators=(",", ":"))
        if len(data.encode()) > MAX_FRAME:
            raise ValueError("outgoing frame too large")
        protocol.write(data + "\n")
        protocol.flush()
        sequence += 1

    send(
        dict(
            type="hello_accepted",
            versions=dict(min=2, max=2),
            selected_version=2,
            identity=IDENTITY,
            capabilities=CAPABILITIES,
        )
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

    threading.Thread(target=control, daemon=True).start()
    try:
        request = active["message"]["request"]
        validate_request(request)

        def progress(stage, completed=None, total=None):
            send(
                dict(
                    type="progress",
                    stage=stage,
                    completed=completed,
                    total=total,
                )
            )

        candidate = generate(request, progress, cancel)
        if cancel.is_set():
            raise Cancelled()
        send(dict(type="output_candidate_v2", candidate=candidate))
    except Cancelled:
        if control_error:
            send(
                dict(
                    type="error",
                    code="protocol_error",
                    summary=str(control_error[0])[:1024],
                )
            )
        else:
            send(dict(type="cancelled"))
    except Exception as exc:
        code = "validation_error" if isinstance(exc, ValueError) else "inference_error"
        send(
            dict(
                type="error",
                code=code,
                summary=(str(exc) or type(exc).__name__)[:1024],
            )
        )
        return 1
    return 0


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
    if sys.argv[1:] == ["--probe"]:
        protocol.write(json.dumps(probe()) + "\n")
        return 0
    if sys.argv[1:]:
        raise ValueError("unsupported worker arguments")
    control = os.fdopen(os.dup(0), "rb", buffering=0)
    return run(protocol, control)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(
            f"worker startup failed: {type(exc).__name__}: {str(exc)[:1024]}",
            file=sys.stderr,
        )
        sys.exit(70)
