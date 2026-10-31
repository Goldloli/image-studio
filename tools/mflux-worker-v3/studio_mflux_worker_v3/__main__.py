"""Qwen/MOSS generation protocol v2 JSONL; no ML import before stdout isolation."""

import hashlib
import json
import os
import sys
import threading
from pathlib import Path

from . import identity, model_spec

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


def validate_request(request, worker_identity, spec):
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
        or request["runtime_lock_id"] != worker_identity["runtime_lock_id"]
        or request["model_snapshot_id"] != worker_identity["model_snapshot_id"]
        or request["sampling_steps"] != spec["steps"]
        or type(request["seed"]) is not int
        or not 0 <= request["seed"] <= 4294967295
    ):
        raise ValueError("model, runtime, recipe, or seed mismatch.")
    prompt = request["instruction"]
    if (
        not isinstance(prompt, str)
        or not prompt.strip()
        or "\0" in prompt
        or len(prompt.encode()) > 16384
    ):
        raise ValueError("prompt is empty or exceeds 16 KiB.")
    from . import expected_parameters

    if request["parameters"] != expected_parameters(
        worker_identity["model_snapshot_id"], request["seed"]
    ):
        raise ValueError("effective parameters differ from the frozen recipe.")
    for name in ("model_path", "output_dir"):
        if not _safe_directory(request[name]):
            raise ValueError("model and output paths must be normalized directories.")
    model_path = Path(request["model_path"])
    output_dir = Path(request["output_dir"])
    if model_path.name != worker_identity["model_snapshot_id"]:
        raise ValueError("model path does not match the selected snapshot.")
    if model_path == output_dir or model_path in output_dir.parents:
        raise ValueError("output directory cannot be inside the model.")
    if request["inputs"] != []:
        raise ValueError("Qwen/MOSS text-to-image accepts no source image.")
    if request["outputs"] != [
        {
            "role": "primary",
            "data_type": "image_rgb",
            "width": 1024,
            "height": 1024,
            "max_bytes": 32 * 1024 * 1024,
        }
    ]:
        raise ValueError("output differs from the frozen recipe.")


def probe(worker_identity):
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
        identity=worker_identity,
        python=platform.python_version(),
        device="gpu",
        device_name=mx.metal.device_info()["device_name"],
        packages={
            name: importlib.metadata.version(name)
            for name in ("mflux", "mlx", "torch", "transformers")
        },
    )


def run(protocol, stream, worker_identity, spec):
    from .backend import Cancelled, generate

    capability = spec["capability"]
    capabilities = [capability]
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
        or msg["expected_worker"] != worker_identity
        or msg["capabilities"] != capabilities
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
            identity=worker_identity,
            capabilities=capabilities,
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
        validate_request(request, worker_identity, spec)

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


def parse_arguments(argv):
    snapshot_id = None
    probe_mode = False
    index = 0
    while index < len(argv):
        value = argv[index]
        if value == "--model-snapshot-id":
            if snapshot_id is not None or index + 1 >= len(argv):
                raise ValueError("invalid or duplicate --model-snapshot-id")
            snapshot_id = argv[index + 1]
            index += 2
        elif value == "--probe" and not probe_mode:
            probe_mode = True
            index += 1
        else:
            raise ValueError("unsupported worker arguments")
    if snapshot_id is None:
        raise ValueError("--model-snapshot-id is required")
    model_spec(snapshot_id)
    return snapshot_id, probe_mode


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
    snapshot_id, probe_mode = parse_arguments(sys.argv[1:])
    worker_identity = identity(snapshot_id)
    spec = model_spec(snapshot_id)
    if probe_mode:
        protocol.write(json.dumps(probe(worker_identity)) + "\n")
        return 0
    control = os.fdopen(os.dup(0), "rb", buffering=0)
    return run(protocol, control, worker_identity, spec)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(
            f"worker startup failed: {type(exc).__name__}: {str(exc)[:1024]}",
            file=sys.stderr,
        )
        sys.exit(70)
