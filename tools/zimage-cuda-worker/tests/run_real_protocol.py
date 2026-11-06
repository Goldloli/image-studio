"""Run one real Windows Z-Image worker protocol exchange."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import threading
import time

from studio_zimage_cuda_worker import (
    COMPACT_RECIPE_ID,
    MODEL_ID,
    expected_parameters,
    model_spec,
    worker_identity,
)


FAMILY = "image-studio.generation"


def write_frame(stream, value):
    data = json.dumps(value, ensure_ascii=False, separators=(",", ":"))
    stream.write(data + "\n")
    stream.flush()


def read_frame(stream):
    line = stream.readline()
    if not line:
        raise EOFError("worker closed stdout")
    return json.loads(line)


def drain_stderr(stream, output, counter):
    while True:
        raw = stream.buffer.read1(8192)
        if not raw:
            return
        chunk = raw.decode("utf-8", errors="replace")
        print(f"[worker stderr] {chunk}", end="", file=sys.stderr, flush=True)
        remaining = 128 * 1024 - counter[0]
        if remaining > 0:
            output.append(chunk[:remaining])
            counter[0] += min(len(chunk), remaining)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--compact", action="store_true")
    parser.add_argument("--supervisor", type=Path)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--prompt", required=True)
    parser.add_argument("--seed", type=int, default=20261101)
    args = parser.parse_args()

    project_root = Path(__file__).resolve().parents[3]

    def display_path(path):
        try:
            return path.resolve().relative_to(project_root).as_posix()
        except ValueError:
            return path.name

    model = args.model.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    report_path = args.report.resolve()
    report_path.parent.mkdir(parents=True, exist_ok=True)
    environment = os.environ.copy()
    # Make the worker package importable regardless of the caller's PYTHONPATH.
    environment["PYTHONPATH"] = str(Path(__file__).resolve().parents[1]) + os.pathsep + environment.get("PYTHONPATH", "")
    for key in (
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_HUB_DISABLE_TELEMETRY",
        "DO_NOT_TRACK",
    ):
        environment[key] = "1"
    started = time.monotonic()
    identity = worker_identity()
    spec = model_spec()
    if args.supervisor:
        config_path = output / "supervisor.json"
        config = {
            "python_program": str(args.python.resolve()),
            "worker_args": ["-I", "-m", "studio_zimage_cuda_worker"],
            "working_dir": str(output),
            "execution_lock": str(output / "execution.lock"),
            "log_path": str(output / "worker.log"),
            "expected_worker": identity,
            "protocol_version": 2,
            "expected_capabilities": [spec["capability"]],
            "timeouts": {
                "handshake_ms": 10_000,
                "loading_ms": 600_000,
                "generation_ms": 900_000,
                "cancel_grace_ms": 3_000,
                "cleanup_deadline_ms": 5_000,
            },
        }
        config_path.write_text(
            json.dumps(config, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
            newline="\n",
        )
        command = [str(args.supervisor.resolve()), "--config", str(config_path)]
    else:
        command = [
            str(args.python.resolve()),
            "-I",
            "-m",
            "studio_zimage_cuda_worker",
        ]
    process = subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        bufsize=1,
        env=environment,
    )
    assert process.stdin is not None
    assert process.stdout is not None
    assert process.stderr is not None
    stderr = []
    stderr_bytes = [0]
    stderr_thread = threading.Thread(
        target=drain_stderr,
        args=(process.stderr, stderr, stderr_bytes),
        daemon=True,
    )
    stderr_thread.start()
    progress = []
    try:
        write_frame(
            process.stdin,
            {
                "family": FAMILY,
                "version": None,
                "session_id": "session:real-windows",
                "request_id": "request:hello",
                "sequence": 0,
                "execution": None,
                "message": {
                    "type": "hello",
                    "versions": {"min": 2, "max": 2},
                    "build_id": "studio-host-0.1.0",
                    "expected_worker": identity,
                    "capabilities": [spec["capability"]],
                },
            },
        )
        print("sent hello", flush=True)
        accepted = read_frame(process.stdout)
        print("received handshake", flush=True)
        if accepted["message"] != {
            "type": "hello_accepted",
            "versions": {"min": 2, "max": 2},
            "selected_version": 2,
            "identity": identity,
            "capabilities": [spec["capability"]],
        }:
            raise RuntimeError("worker handshake mismatch")
        execution = {
            "job_id": "job:real-windows",
            "attempt_id": "attempt:real-windows",
            "execution_token": "token:real-windows",
        }
        write_frame(
            process.stdin,
            {
                "family": FAMILY,
                "version": 2,
                "session_id": "session:real-windows",
                "request_id": "request:execute",
                "sequence": 1,
                "execution": execution,
                "message": {
                    "type": "execute_v2",
                    "request": {
                        "model_id": MODEL_ID,
                        "capability_id": spec["capability"],
                        "recipe_id": COMPACT_RECIPE_ID if args.compact else spec["recipe"],
                        "recipe_version": 1,
                        "runtime_lock_id": identity["runtime_lock_id"],
                        "model_snapshot_id": MODEL_ID,
                        "instruction": args.prompt,
                        "seed": args.seed,
                        "sampling_steps": 9,
                        "parameters": expected_parameters(
                            args.seed,
                            COMPACT_RECIPE_ID if args.compact else spec["recipe"],
                        ),
                        "inputs": [],
                        "outputs": [
                            {
                                "role": "primary",
                                "data_type": "image_rgb",
                                "width": (
                                    expected_parameters(
                                        args.seed,
                                        COMPACT_RECIPE_ID if args.compact else spec["recipe"],
                                    ).get("resolution", 1024)
                                ),
                                "height": (
                                    expected_parameters(
                                        args.seed,
                                        COMPACT_RECIPE_ID if args.compact else spec["recipe"],
                                    ).get("resolution", 1024)
                                ),
                                "max_bytes": 32 * 1024 * 1024,
                            }
                        ],
                        "model_path": str(model),
                        "output_dir": str(output),
                    },
                },
            },
        )
        print("sent execute", flush=True)
        terminal = None
        while terminal is None:
            frame = read_frame(process.stdout)
            message = frame["message"]
            if message["type"] == "progress":
                progress.append(message)
                print(
                    f"progress {message['stage']} {message['completed']}/{message['total']}",
                    flush=True,
                )
            elif message["type"] in {
                "output_candidate_v2",
                "error",
                "cancelled",
            }:
                terminal = message
        status = process.wait(timeout=30)
        if stderr_thread.is_alive():
            stderr_thread.join(timeout=2)
        candidate = terminal.get("candidate")
        if terminal["type"] != "output_candidate_v2" or status != 0:
            raise RuntimeError(f"worker failed: {terminal}; exit={status}")
        image = output / candidate["relative_path"]
        payload = image.read_bytes()
        if hashlib.sha256(payload).hexdigest() != candidate["sha256"]:
            raise RuntimeError("candidate hash mismatch")
        report = {
            "schema_version": 1,
            "kind": "windows-zimage-real-worker-protocol",
            "status": "passed",
            "supervisor": args.supervisor is not None,
            "protocol_version": 2,
            "identity": identity,
            "elapsed_seconds": time.monotonic() - started,
            "request": {
                "prompt": args.prompt,
                "seed": args.seed,
                "model_path": display_path(model),
                "output_dir": display_path(output),
            },
            "progress": progress,
            "candidate": candidate,
            "worker_stderr": "".join(stderr)
            .replace(str(project_root), "<repo>")
            .replace(str(Path.home()), "<home>"),
        }
        if args.supervisor:
            lifecycle = output / "compute-lifecycle.json"
            if not lifecycle.is_file():
                raise RuntimeError("supervisor lifecycle evidence is missing")
            report["lifecycle"] = json.loads(lifecycle.read_text(encoding="utf-8"))
            report["worker_log_bytes"] = (output / "worker.log").stat().st_size
        temporary = report_path.with_suffix(report_path.suffix + ".pending")
        temporary.write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
            newline="\n",
        )
        os.replace(temporary, report_path)
        print(json.dumps({"status": "passed", "report": str(report_path)}))
        return 0
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


if __name__ == "__main__":
    raise SystemExit(main())
