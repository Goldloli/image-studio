"""Run one real Windows Real-ESRGAN worker (native ncnn wrapper) v2 exchange.

The native worker is a Rust binary (`tools/realesrgan-worker`) that speaks the
same generation v2 JSONL protocol and wraps the frozen official
`realesrgan-ncnn-vulkan.exe` with the fixed recipe (-s 4 -t 128 -g 0 -j 1:1:1,
no TTA). This harness spawns the Windows release binary with
`--ncnn-binary <absolute exe>`, drives the hello/execute/cancel frames, samples
host resources, and records the candidate plus the exact-4x check and the raw
ncnn stderr. It is experiment/verification tooling, not product code.

Usage (frozen experiment interpreter, which has PIL):

    target\\parity-gate0\\py312\\python.exe tools\\diffusers-worker-windows\\tests\\run_realesrgan_protocol.py \
        --worker-exe target\\release\\realesrgan-worker.exe \
        --ncnn-binary target\\parity-runtime\\realesrgan\\app\\realesrgan-ncnn-vulkan.exe \
        --model-dir target\\parity-worker\\realesrgan\\model \
        --input <png> --output target\\parity-worker\\attempts\\realesrgan-... \
        --report verification\\windows-parity-worker\\real-protocol-realesrgan.json
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

FAMILY = "image-studio.generation"
MAX_OUTPUT_BYTES = 128 * 1024 * 1024

IDENTITY = {
    "build_id": "studio-realesrgan-worker-windows-0.1.0",
    "runtime_lock_id": "realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1",
    "model_snapshot_id": "realesrgan-x4plus-ncnn-windows-v1",
}
MODEL_ID = "realesrgan-x4plus-windows-v1"
CAPABILITY = "image.upscale.realesrgan-x4plus.windows-v1"
RECIPE_ID = "realesrgan-x4plus-windows-t128-v1"
PARAMETERS = {
    "recipe": RECIPE_ID,
    "recipe_version": 1,
    "model": "realesrgan-x4plus",
    "scale": 4,
    "tile": 128,
    "threads": "1:1:1",
    "tta": False,
}


def write_frame(stream, value):
    stream.write(json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n")
    stream.flush()


def read_frame(stream, timeout_seconds):
    line = stream.readline()
    if not line:
        raise EOFError("worker closed stdout")
    return json.loads(line)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_png_dimensions(path: Path) -> tuple[int, int]:
    from PIL import Image

    with Image.open(path) as opened:
        opened.load()
        return opened.width, opened.height


def nvidia_snapshot() -> dict:
    import subprocess as sp

    used = sp.run(
        ["nvidia-smi", "--query-gpu=name,driver_version,memory.used,memory.total",
         "--format=csv,noheader,nounits"],
        check=True, capture_output=True, text=True, timeout=15,
    ).stdout.strip()
    name, driver, mem_used, mem_total = [item.strip() for item in used.split(",")]
    uuid = sp.run(
        ["nvidia-smi", "--query-gpu=uuid", "--format=csv,noheader"],
        check=True, capture_output=True, text=True, timeout=15,
    ).stdout.strip().splitlines()[0].strip()
    return {
        "name": name,
        "driver": driver,
        "memory_used_mib": int(mem_used),
        "memory_total_mib": int(mem_total),
        "uuid": uuid,
    }


def drain_stderr(stream, output, counter):
    while True:
        raw = stream.read1(8192) if hasattr(stream, "read1") else stream.read(8192)
        if not raw:
            return
        chunk = raw.decode("utf-8", errors="replace") if isinstance(raw, bytes) else raw
        print(f"[worker stderr] {chunk}", end="", file=sys.stderr, flush=True)
        remaining = 256 * 1024 - counter[0]
        if remaining > 0:
            output.append(chunk[:remaining])
            counter[0] += min(len(chunk), remaining)


def parse_vulkan_devices(stderr: str) -> list[str]:
    import re

    pattern = re.compile(r"^\[(\d+)\s+([^\]]+)\]")
    devices = []
    for line in stderr.splitlines():
        match = pattern.match(line.strip())
        if match:
            devices.append(
                {"index": int(match.group(1)), "name": match.group(2).strip()}
            )
    return devices


def verbatim(path: Path) -> str:
    """Windows extended-length form.

    The committed native worker compares ``Path::canonicalize() == path``; on
    Windows ``canonicalize`` returns an extended-length ``\\\\?\\C:\\...`` path, so
    a plain absolute path is always rejected. The worker defect is recorded in
    the evidence README; passing verbatim paths keeps the frozen binary usable
    without modifying its source.
    """
    text = str(path)
    if text.startswith("\\\\?\\"):
        return text
    absolute = os.path.abspath(text)
    if absolute.startswith("\\\\"):
        return "\\\\?\\UNC\\" + absolute[2:]
    return "\\\\?\\" + absolute


def display_path(path: Path) -> str:
    project_root = Path(__file__).resolve().parents[3]
    try:
        return path.resolve().relative_to(project_root).as_posix()
    except ValueError:
        return path.name


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker-exe", type=Path, required=True)
    parser.add_argument("--ncnn-binary", type=Path, required=True)
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--expected-output-sha256")
    parser.add_argument("--expected-output-bytes", type=int)
    parser.add_argument("--cancel-immediately", action="store_true")
    parser.add_argument(
        "--no-verbatim",
        action="store_true",
        help="reproduce the committed worker's Windows canonicalize rejection",
    )
    parser.add_argument("--timeout-seconds", type=float, default=300.0)
    args = parser.parse_args()

    worker_exe = args.worker_exe.resolve()
    ncnn_binary = args.ncnn_binary.resolve()
    model_dir = args.model_dir.resolve()
    if not worker_exe.is_file():
        raise SystemExit(f"worker binary missing: {worker_exe}")
    if not ncnn_binary.is_file():
        raise SystemExit(f"ncnn binary missing: {ncnn_binary}")
    if not (model_dir / "models" / "realesrgan-x4plus.bin").is_file():
        raise SystemExit(f"ncnn weight layout missing under {model_dir}")

    output = args.output.resolve()
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    source_target = output / "source.png"
    shutil.copyfile(args.input.resolve(), source_target)
    width, height = read_png_dimensions(source_target)
    output_verbatim = verbatim(output) if not args.no_verbatim else str(output)
    source_verbatim = (
        output_verbatim + "\\source.png"
        if not args.no_verbatim
        else str(source_target)
    )
    source = {
        "role": "source",
        "path": source_verbatim,
        "width": width,
        "height": height,
        "sha256": sha256_file(source_target),
    }
    request = {
        "model_id": MODEL_ID,
        "capability_id": CAPABILITY,
        "recipe_id": RECIPE_ID,
        "recipe_version": 1,
        "runtime_lock_id": IDENTITY["runtime_lock_id"],
        "model_snapshot_id": IDENTITY["model_snapshot_id"],
        "instruction": "",
        "seed": None,
        "sampling_steps": None,
        "parameters": PARAMETERS,
        "inputs": [source],
        "outputs": [
            {
                "role": "primary",
                "data_type": "image_rgb",
                "width": width * 4,
                "height": height * 4,
                "max_bytes": MAX_OUTPUT_BYTES,
            }
        ],
        "model_path": verbatim(model_dir) if not args.no_verbatim else str(model_dir),
        "output_dir": output_verbatim,
    }
    gpu_before = nvidia_snapshot()
    command = [
        str(worker_exe),
        "--ncnn-binary",
        verbatim(ncnn_binary) if not args.no_verbatim else str(ncnn_binary),
    ]
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=False,
        bufsize=0,
    )
    assert process.stdin is not None
    assert process.stdout is not None
    assert process.stderr is not None
    stdin = process.stdin
    stdout = process.stdout
    stderr_lines: list[str] = []
    stderr_bytes = [0]
    stderr_thread = threading.Thread(
        target=drain_stderr, args=(process.stderr, stderr_lines, stderr_bytes), daemon=True
    )
    stderr_thread.start()
    frames = []
    started = time.monotonic()
    session_id = "session:real-realesrgan-windows"
    execution = {
        "job_id": "job:real-realesrgan",
        "attempt_id": "attempt:real-realesrgan",
        "execution_token": "token:real-realesrgan",
    }

    def send(value):
        stdin.write(
            (json.dumps(value, ensure_ascii=False, separators=(",", ":")) + "\n").encode()
        )
        stdin.flush()

    terminal = None
    error_message = None
    try:
        send(
            {
                "family": FAMILY,
                "version": None,
                "session_id": session_id,
                "request_id": "request:hello",
                "sequence": 0,
                "execution": None,
                "message": {
                    "type": "hello",
                    "versions": {"min": 2, "max": 2},
                    "build_id": "studio-host-0.1.0",
                    "expected_worker": IDENTITY,
                    "capabilities": [CAPABILITY],
                },
            }
        )
        first = stdout.readline()
        if not first:
            raise RuntimeError("worker closed stdout during handshake")
        frames.append(json.loads(first))
        send(
            {
                "family": FAMILY,
                "version": 2,
                "session_id": session_id,
                "request_id": "request:execute",
                "sequence": 1,
                "execution": execution,
                "message": {"type": "execute_v2", "request": request},
            }
        )
        if args.cancel_immediately:
            send(
                {
                    "family": FAMILY,
                    "version": 2,
                    "session_id": session_id,
                    "request_id": "request:execute",
                    "sequence": 2,
                    "execution": execution,
                    "message": {"type": "cancel"},
                }
            )
        deadline = started + args.timeout_seconds
        while terminal is None:
            if time.monotonic() > deadline:
                raise RuntimeError("protocol timeout")
            line = stdout.readline()
            if not line:
                raise RuntimeError("worker closed stdout before a terminal frame")
            frame = json.loads(line)
            frames.append(frame)
            message = frame["message"]
            if message["type"] == "progress":
                print(f"progress {message.get('stage')}", flush=True)
            elif message["type"] in {"output_candidate_v2", "error", "cancelled"}:
                terminal = message
        exit_code = process.wait(timeout=60)
    except (RuntimeError, EOFError, OSError) as error:
        error_message = f"{type(error).__name__}: {error}"
        try:
            exit_code = process.wait(timeout=30)
        except Exception:  # noqa: BLE001 - startup failure evidence
            exit_code = None
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()
        if stderr_thread.is_alive():
            stderr_thread.join(timeout=2)
    elapsed = time.monotonic() - started
    gpu_after = nvidia_snapshot()
    stderr_text = "".join(stderr_lines)
    files = sorted(item.name for item in output.iterdir())
    report: dict = {
        "schema_version": 1,
        "kind": "windows-realesrgan-worker-real-protocol",
        "status": "failed",
        "mode": "cancel" if args.cancel_immediately else "run",
        "identity": IDENTITY,
        "worker_binary": {
            "path": display_path(worker_exe),
            "bytes": worker_exe.stat().st_size,
            "sha256": sha256_file(worker_exe),
        },
        "ncnn_binary": {
            "path": display_path(ncnn_binary),
            "bytes": ncnn_binary.stat().st_size,
            "sha256": sha256_file(ncnn_binary),
        },
        "model_dir": display_path(model_dir),
        "request": {
            "input": {
                "path": display_path(source_target),
                "width": width,
                "height": height,
                "sha256": source["sha256"],
            },
            "parameters": PARAMETERS,
            "output_spec": request["outputs"][0],
        },
        "handshake": frames[0]["message"] if frames else None,
        "progress": [f["message"] for f in frames if f["message"]["type"] == "progress"],
        "terminal": terminal,
        "worker_exit_code": exit_code,
        "elapsed_seconds": round(elapsed, 3),
        "gpu_before": gpu_before,
        "gpu_after": gpu_after,
        "vulkan_devices_from_stderr": parse_vulkan_devices(stderr_text),
        "worker_stderr": stderr_text[-131072:],
        "attempt_files": files,
        "failure": error_message,
    }
    if args.cancel_immediately:
        report["status"] = "passed" if terminal and terminal["type"] == "cancelled" else "failed"
        report["cancel_received"] = bool(terminal and terminal["type"] == "cancelled")
        report["no_candidate_file"] = "primary.png" not in files
        if report["status"] == "passed" and not report["no_candidate_file"]:
            report["status"] = "failed"
    elif terminal and terminal["type"] == "output_candidate_v2" and exit_code == 0:
        candidate = terminal["candidate"]
        image = output / candidate["relative_path"]
        actual_sha = sha256_file(image)
        actual_width, actual_height = read_png_dimensions(image)
        report["output"] = {
            "path": display_path(image),
            "bytes": image.stat().st_size,
            "sha256": actual_sha,
            "width": actual_width,
            "height": actual_height,
        }
        checks = {
            "candidate_sha256_matches_file": actual_sha == candidate["sha256"],
            "candidate_bytes_match_file": image.stat().st_size == candidate["size_bytes"],
            "candidate_dims_match_file": (actual_width, actual_height)
            == (candidate["width"], candidate["height"]),
            "exact_4x_dimensions": (actual_width, actual_height)
            == (width * 4, height * 4),
            "expected_output_sha256": (
                args.expected_output_sha256 is None
                or actual_sha == args.expected_output_sha256.lower()
            ),
            "expected_output_bytes": (
                args.expected_output_bytes is None
                or image.stat().st_size == args.expected_output_bytes
            ),
        }
        report["checks"] = checks
        report["status"] = "passed" if all(checks.values()) else "failed"
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.report.with_suffix(args.report.suffix + ".pending")
        temporary.write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
            newline="\n",
        )
        os.replace(temporary, args.report)
    print(
        json.dumps(
            {
                "status": report["status"],
                "report": display_path(args.report),
                "output": report.get("output"),
            },
            ensure_ascii=False,
        )
    )
    return 0 if report["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
