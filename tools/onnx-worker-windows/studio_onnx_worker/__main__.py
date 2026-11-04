"""BiRefNet Windows ONNX Runtime CPU worker, generation v2 JSONL.

Forked from ``tools/onnx-worker/studio_onnx_worker`` (the portable macOS v2
shell); only the frozen Windows identity, the probe device and the
``--model-snapshot-id`` installer argument differ. Import of numpy/Pillow/
onnxruntime happens only after stdout has been isolated to the protocol pipe.
"""

import hashlib
import json
import os
import platform
import sys
import threading
from pathlib import Path

from . import (
    CAPABILITIES,
    CAPABILITY,
    MODEL_ID,
    MODEL_SNAPSHOT_ID,
    RECIPE_ID,
    RUNTIME_LOCK_ID,
    expected_parameters,
    worker_identity,
)

MAX_FRAME = 64 * 1024
FAMILY = "image-studio.generation"
MAX_BYTES = 32 * 1024 * 1024
DEVICE = "windows-cpu"


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


def validate_request(request, identity):
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
        request["model_id"] != MODEL_ID
        or request["capability_id"] != CAPABILITY
        or request["recipe_id"] != RECIPE_ID
        or request["recipe_version"] != 1
        or request["runtime_lock_id"] != RUNTIME_LOCK_ID
        or request["model_snapshot_id"] != identity["model_snapshot_id"]
        or request["instruction"] != ""
        or request["seed"] is not None
        or request["sampling_steps"] is not None
        or request["parameters"] != expected_parameters()
    ):
        raise ValueError("BiRefNet model, capability, recipe, or parameters differ.")
    for name in ("model_path", "output_dir"):
        if not _safe_directory(request[name]):
            raise ValueError("BiRefNet paths must be normalized local directories.")
    model_path = Path(request["model_path"])
    output_dir = Path(request["output_dir"])
    if model_path == output_dir or model_path in output_dir.parents:
        raise ValueError("output directory cannot be inside the model")
    inputs = request["inputs"]
    if len(inputs) != 1:
        raise ValueError("BiRefNet requires exactly one source image.")
    source = inputs[0]
    fields(source, ("role", "path", "width", "height", "sha256"))
    if (
        source["role"] != "source"
        or type(source["width"]) is not int
        or type(source["height"]) is not int
        or source["width"] <= 0
        or source["height"] <= 0
        or source["width"] > 4096
        or source["height"] > 4096
        or source["width"] * source["height"] > 4 * 1024 * 1024
        or len(source["sha256"]) != 64
    ):
        raise ValueError("invalid BiRefNet source descriptor.")
    source_path = Path(source["path"])
    if (
        not source_path.is_absolute()
        or source_path.is_symlink()
        or not source_path.is_file()
        or source_path.resolve() != source_path
        or source_path.parent != output_dir
    ):
        raise ValueError("BiRefNet source must be an Attempt-local regular file.")
    with source_path.open("rb") as stream:
        if hashlib.file_digest(stream, "sha256").hexdigest() != source["sha256"]:
            raise ValueError("BiRefNet source hash mismatch.")
    outputs = request["outputs"]
    if len(outputs) != 2:
        raise ValueError("BiRefNet requires primary and mask outputs.")
    expected = {
        "primary": {
            "role": "primary",
            "data_type": "image_rgba",
            "width": source["width"],
            "height": source["height"],
            "max_bytes": MAX_BYTES,
        },
        "mask": {
            "role": "mask",
            "data_type": "mask",
            "width": source["width"],
            "height": source["height"],
            "max_bytes": MAX_BYTES,
        },
    }
    found = {}
    for output in outputs:
        fields(output, ("role", "data_type", "width", "height", "max_bytes"))
        if output["role"] in found:
            raise ValueError("duplicate BiRefNet output role")
        found[output["role"]] = output
    if found != expected:
        raise ValueError("BiRefNet outputs differ from the frozen recipe.")


def probe_identity_device():
    """CPU device identity reported to the installer (no inference loaded here)."""
    processor = platform.processor().strip()
    if not processor:
        processor = os.environ.get("PROCESSOR_IDENTIFIER", "").strip()
    return processor or "Windows CPU"


def probe(model_path, identity):
    import importlib.metadata

    import onnxruntime as ort

    from .backend import model_file

    # The portable backend path guards expect strings, exactly like the macOS worker.
    model_path = str(model_path)
    if not _safe_directory(model_path):
        raise ValueError("BiRefNet probe requires a normalized model directory")
    path = model_file(model_path)
    options = ort.SessionOptions()
    options.intra_op_num_threads = 4
    options.inter_op_num_threads = 1
    options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
    session = ort.InferenceSession(
        str(path),
        sess_options=options,
        providers=["CPUExecutionProvider"],
    )
    if session.get_providers() != ["CPUExecutionProvider"]:
        raise RuntimeError("CPUExecutionProvider is unavailable")
    inputs = session.get_inputs()
    outputs = session.get_outputs()
    if len(inputs) != 1 or len(outputs) < 1 or inputs[0].type != "tensor(float)":
        raise RuntimeError("BiRefNet graph contract is unsupported")
    packages = {}
    for name in ("onnxruntime", "numpy", "pillow"):
        try:
            packages[name] = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            packages[name] = None
    return dict(
        identity=identity,
        python=platform.python_version(),
        device=DEVICE,
        device_name=probe_identity_device(),
        packages=packages,
        graph=dict(
            model_file=path.name,
            input=dict(name=inputs[0].name, type=inputs[0].type, shape=inputs[0].shape),
            outputs=[
                dict(name=output.name, type=output.type, shape=output.shape)
                for output in outputs
            ],
            providers=session.get_providers(),
        ),
    )


def run(protocol, stream, identity):
    from .backend import Cancelled, generate

    capabilities = list(CAPABILITIES)
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
        or msg["expected_worker"] != identity
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
            identity=identity,
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
        validate_request(request, identity)

        def progress(stage, completed=None, total=None):
            send(
                dict(
                    type="progress",
                    stage=stage,
                    completed=completed,
                    total=total,
                )
            )

        candidates = generate(request, progress, cancel)
        if cancel.is_set():
            raise Cancelled()
        for candidate in candidates:
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
    snapshot_id = MODEL_SNAPSHOT_ID
    seen_snapshot = False
    probe_mode = False
    model_path = None
    index = 0
    while index < len(argv):
        value = argv[index]
        if value == "--model-snapshot-id":
            if seen_snapshot or index + 1 >= len(argv):
                raise ValueError("invalid or duplicate --model-snapshot-id")
            snapshot_id = argv[index + 1]
            seen_snapshot = True
            index += 2
        elif value == "--probe" and not probe_mode:
            probe_mode = True
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
    worker_identity(snapshot_id)  # fail fast on an unsupported install identity
    if probe_mode and model_path is None:
        raise ValueError("ONNX probe requires a model directory")
    return {"snapshot_id": snapshot_id, "probe": probe_mode, "model_path": model_path}


def main():
    protocol = isolate_stdout()
    for key in (
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_HUB_DISABLE_TELEMETRY",
        "DO_NOT_TRACK",
    ):
        os.environ[key] = "1"
    arguments = parse_arguments(sys.argv[1:])
    identity = worker_identity(arguments["snapshot_id"])
    if arguments["probe"]:
        if arguments["model_path"] is None:
            raise ValueError("ONNX probe requires a model directory")
        protocol.write(json.dumps(probe(arguments["model_path"], identity)) + "\n")
        protocol.flush()
        return 0
    control = os.fdopen(os.dup(0), "rb", buffering=0)
    return run(protocol, control, identity)


if __name__ == "__main__":
    try:
        sys.exit(main())
    except Exception as exc:
        print(
            f"worker startup failed: {type(exc).__name__}: {str(exc)[:1024]}",
            file=sys.stderr,
        )
        sys.exit(70)
