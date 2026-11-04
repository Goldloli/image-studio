"""Run real Windows diffusers worker exchanges (probe/prepare/run/cancel).

The harness spawns the worker exactly like the product supervisor will: the
runtime ``python.exe`` with the installed worker wheel on ``sys.path``, stdin/
stdout as the v2 JSONL control channel, offline environment, and host-side
resource sampling for the evidence JSON. It is not product code; it proves the
worker protocol and inference path on the target machine.

Run with the frozen experiment interpreter (it carries torch/diffusers/PIL):

    target\\parity-gate0\\py312\\python.exe tools\\diffusers-worker-windows\\tests\\run_real_protocol.py \
        --python target\\parity-gate0\\py312\\python.exe \
        --worker-site target\\parity-worker\\worker-site \
        --model target\\parity-worker\\models\\<snapshot> \
        --snapshot <snapshot> --output target\\parity-worker\\attempts\\... \
        --report verification\\windows-parity-worker\\...json
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

WORKER_ROOT = Path(__file__).resolve().parents[1]
if str(WORKER_ROOT) not in sys.path:
    sys.path.insert(0, str(WORKER_ROOT))

from studio_diffusers_worker import (  # noqa: E402
    expected_parameters,
    model_spec,
    output_specs,
    recipe_for,
    worker_identity,
)

FAMILY = "image-studio.generation"
PROJECT_ROOT = Path(__file__).resolve().parents[3]


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
        remaining = 512 * 1024 - counter[0]
        if remaining > 0:
            output.append(chunk[:remaining])
            counter[0] += min(len(chunk), remaining)


def display_path(path: Path) -> str:
    try:
        return path.resolve().relative_to(PROJECT_ROOT).as_posix()
    except ValueError:
        return path.name


class HostSampler:
    """Sample worker process-tree commit, available RAM and nvidia-smi usage."""

    def __init__(self, process, interval_seconds: float = 0.5) -> None:
        import psutil

        self.psutil = psutil
        self.process = process
        self.interval = interval_seconds
        self.stop_event = threading.Event()
        self.thread = threading.Thread(target=self._loop, daemon=True)
        self.private_peak = 0
        self.available_ram_min = 1 << 62
        self.nvidia_peak_mib = 0
        self.samples = 0

    def _sample(self) -> None:
        try:
            root = self.psutil.Process(self.process.pid)
            total = 0
            for item in [root, *root.children(recursive=True)]:
                try:
                    total += int(item.memory_info().private)
                except (self.psutil.NoSuchProcess, self.psutil.AccessDenied):
                    pass
            self.private_peak = max(self.private_peak, total)
            memory = self.psutil.virtual_memory()
            self.available_ram_min = min(self.available_ram_min, int(memory.available))
            if self.samples % 2 == 0:
                import subprocess as sp

                output = sp.run(
                    [
                        "nvidia-smi",
                        "--query-gpu=memory.used",
                        "--format=csv,noheader,nounits",
                    ],
                    check=True,
                    capture_output=True,
                    text=True,
                    timeout=10,
                ).stdout.strip()
                self.nvidia_peak_mib = max(
                    self.nvidia_peak_mib, int(output.splitlines()[0])
                )
            self.samples += 1
        except Exception:  # noqa: BLE001 - sampling is best effort
            pass

    def _loop(self) -> None:
        while not self.stop_event.wait(self.interval):
            self._sample()

    def start(self) -> None:
        self.thread.start()

    def stop(self) -> None:
        self.stop_event.set()
        self.thread.join(timeout=10)

    def snapshot(self) -> dict:
        return {
            "process_tree_private_commit_peak_gib": round(
                self.private_peak / 1024**3, 3
            ),
            "system_available_ram_min_gib": round(
                self.available_ram_min / 1024**3, 3
            ),
            "nvidia_used_peak_mib": self.nvidia_peak_mib,
            "sample_interval_seconds": self.interval,
        }


def worker_bootstrap(import_path: Path) -> str:
    return (
        "import sys; sys.path.insert(0, "
        + json.dumps(str(import_path.resolve()))
        + "); from studio_diffusers_worker.__main__ import main; "
        "raise SystemExit(main())"
    )


def worker_command(args, import_path: Path, extra: list[str], snapshot_id: str):
    bootstrap = worker_bootstrap(import_path)
    arguments = ["-I", "-c", bootstrap, "--model-snapshot-id", snapshot_id]
    arguments.extend(extra)
    if args.supervisor:
        raise RuntimeError("supervisor mode is not implemented for this harness yet")
    return [str(args.python.resolve()), *arguments]


def spawn_worker(args, import_path: Path, extra: list[str], snapshot_id: str):
    environment = os.environ.copy()
    for key in (
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_HUB_DISABLE_TELEMETRY",
        "DO_NOT_TRACK",
    ):
        environment[key] = "1"
    return subprocess.Popen(
        worker_command(args, import_path, extra, snapshot_id),
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
        bufsize=1,
        env=environment,
        cwd=str(PROJECT_ROOT),
    )


def prepare_attempt_output(output: Path) -> None:
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)


def source_descriptor(source_image: Path, output: Path) -> dict:
    from PIL import Image

    target = output / "source.png"
    shutil.copyfile(source_image, target)
    digest = hashlib.sha256(target.read_bytes()).hexdigest()
    with Image.open(target) as opened:
        opened.load()
        width, height = opened.size
    return {
        "role": "source",
        "path": str(target),
        "width": width,
        "height": height,
        "sha256": digest,
    }


def write_report(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".pending")
    temporary.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, path)


def run_probe(args, snapshot_id: str, import_path: Path) -> int:
    identity = worker_identity(snapshot_id)
    extra = ["--probe"]
    if args.model is not None:
        extra.append(str(args.model.resolve()))
    process = spawn_worker(args, import_path, extra, snapshot_id)
    assert process.stdout is not None
    assert process.stderr is not None
    started = time.monotonic()
    line = process.stdout.readline()
    status = process.wait(timeout=180)
    stderr = process.stderr.read()
    if not line:
        raise RuntimeError(f"probe produced no output (exit {status}): {stderr}")
    probe = json.loads(line)
    report = {
        "schema_version": 1,
        "kind": "windows-diffusers-worker-probe",
        "status": "passed",
        "model_snapshot_id": snapshot_id,
        "identity": probe["identity"],
        "python": probe["python"],
        "device": probe["device"],
        "device_name": probe["device_name"],
        "compute_capability": probe.get("compute_capability"),
        "packages": probe.get("packages"),
        "elapsed_seconds": round(time.monotonic() - started, 2),
        "worker_exit_code": status,
        "worker_stderr": stderr[-32768:],
    }
    if probe["identity"] != identity or probe["device"] != "gpu" or status != 0:
        report["status"] = "failed"
    write_report(args.report, report)
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] == "passed" else 1


def run_prepare(args, snapshot_id: str, import_path: Path) -> int:
    if args.model is None or args.prepared_dir is None:
        raise SystemExit("--prepare requires --model-dir and --prepared-dir")
    extra = [
        "--prepare",
        "--model-dir",
        str(args.model.resolve()),
        "--prepared-dir",
        str(args.prepared_dir.resolve()),
    ]
    if args.rebuild_prepared:
        extra.append("--no-reuse-prepared")
    process = spawn_worker(args, import_path, extra, snapshot_id)
    assert process.stdout is not None
    assert process.stderr is not None
    stderr = []
    stderr_bytes = [0]
    thread = threading.Thread(
        target=drain_stderr,
        args=(process.stderr, stderr, stderr_bytes),
        daemon=True,
    )
    thread.start()
    started = time.monotonic()
    line = process.stdout.readline()
    status = process.wait(timeout=1800)
    if thread.is_alive():
        thread.join(timeout=2)
    if not line:
        raise RuntimeError(f"prepare produced no output (exit {status})")
    result = json.loads(line)
    report = {
        "schema_version": 1,
        "kind": "windows-diffusers-worker-prepare",
        "status": result.get("status"),
        "model_snapshot_id": snapshot_id,
        "elapsed_seconds": round(time.monotonic() - started, 2),
        "worker_exit_code": status,
        "prepare": result,
        "worker_stderr": "".join(stderr),
    }
    write_report(args.report, report)
    print(json.dumps({"status": report["status"], "report": str(args.report)}))
    return 0 if report["status"] in ("prepared", "reused", "not_required") and status == 0 else 1


def run_protocol(args, snapshot_id: str, import_path: Path) -> int:
    if args.model is None:
        raise SystemExit("run mode requires --model-dir")
    model = args.model.resolve()
    output = args.report_output.resolve() if args.report_output else (PROJECT_ROOT / "target/parity-worker/attempts" / f"{snapshot_id}-{int(time.time())}")
    prepare_attempt_output(output)
    capability = args.capability or model_spec(snapshot_id)["capabilities"][0]
    recipe_id, steps = recipe_for(snapshot_id, capability)
    seed = args.seed
    inputs = []
    if args.source_image is not None:
        inputs = [source_descriptor(args.source_image.resolve(), output)]
    request = {
        "model_id": model_spec(snapshot_id)["model_id"],
        "capability_id": capability,
        "recipe_id": recipe_id,
        "recipe_version": 1,
        "runtime_lock_id": worker_identity(snapshot_id)["runtime_lock_id"],
        "model_snapshot_id": snapshot_id,
        "instruction": args.prompt,
        "seed": seed,
        "sampling_steps": steps,
        "parameters": expected_parameters(snapshot_id, capability, seed),
        "inputs": inputs,
        "outputs": output_specs(snapshot_id),
        "model_path": str(model),
        "output_dir": str(output),
    }
    process = spawn_worker(args, import_path, [], snapshot_id)
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
    sampler = HostSampler(process)
    sampler.start()
    identity = worker_identity(snapshot_id)
    session_id = "session:real-windows-parity"
    execution = {
        "job_id": "job:real-windows-parity",
        "attempt_id": "attempt:real-windows-parity",
        "execution_token": "token:real-windows-parity",
    }
    frames = []
    started = time.monotonic()
    terminal = None
    try:
        write_frame(
            process.stdin,
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
                    "expected_worker": identity,
                    "capabilities": model_spec(snapshot_id)["capabilities"],
                },
            },
        )
        accepted = read_frame(process.stdout)
        frames.append(accepted)
        if accepted["message"].get("type") != "hello_accepted":
            raise RuntimeError(f"unexpected handshake: {accepted}")
        write_frame(
            process.stdin,
            {
                "family": FAMILY,
                "version": 2,
                "session_id": session_id,
                "request_id": "request:execute",
                "sequence": 1,
                "execution": execution,
                "message": {"type": "execute_v2", "request": request},
            },
        )
        if args.cancel_immediately:
            write_frame(
                process.stdin,
                {
                    "family": FAMILY,
                    "version": 2,
                    "session_id": session_id,
                    "request_id": "request:execute",
                    "sequence": 2,
                    "execution": execution,
                    "message": {"type": "cancel"},
                },
            )
        while terminal is None:
            frame = read_frame(process.stdout)
            frames.append(frame)
            message = frame["message"]
            if message["type"] == "progress":
                print(
                    f"progress {message['stage']} "
                    f"{message.get('completed')}/{message.get('total')}",
                    flush=True,
                )
            elif message["type"] in {
                "output_candidate_v2",
                "error",
                "cancelled",
            }:
                terminal = message
        status = process.wait(timeout=60)
    finally:
        sampler.stop()
        if process.poll() is None:
            process.kill()
            process.wait()
        if stderr_thread.is_alive():
            stderr_thread.join(timeout=2)
    elapsed = time.monotonic() - started
    resources = sampler.snapshot()
    candidate = (terminal or {}).get("candidate") if terminal else None
    if candidate:
        image_path = output / candidate["relative_path"]
        payload = image_path.read_bytes()
        if hashlib.sha256(payload).hexdigest() != candidate["sha256"]:
            raise RuntimeError("candidate hash mismatch")
        image = str(image_path)
    else:
        image = None
    worker_stderr = "".join(stderr)
    worker_resources = None
    for line in worker_stderr.splitlines():
        marker = "resources="
        if marker in line:
            try:
                worker_resources = json.loads(line.split(marker, 1)[1])
            except json.JSONDecodeError:
                continue
    report = {
        "schema_version": 1,
        "kind": "windows-diffusers-worker-real-protocol",
        "status": "passed",
        "protocol_version": 2,
        "mode": "cancel" if args.cancel_immediately else "run",
        "model_snapshot_id": snapshot_id,
        "identity": identity,
        "capability": capability,
        "request": {
            "prompt": args.prompt,
            "seed": seed,
            "steps": steps,
            "model_path": display_path(model),
            "output_dir": display_path(output),
            "prepared_dir": str(model) + ".prepared",
            "inputs": [
                {
                    "role": item["role"],
                    "width": item["width"],
                    "height": item["height"],
                    "sha256": item["sha256"],
                }
                for item in inputs
            ],
        },
        "handshake": frames[0]["message"] if frames else None,
        "progress": [
            frame["message"]
            for frame in frames
            if frame["message"]["type"] == "progress"
        ],
        "terminal": terminal,
        "worker_exit_code": status,
        "elapsed_seconds": round(elapsed, 2),
        "host_sampled_resources": resources,
        "worker_reported_resources": worker_resources,
        "worker_stderr": worker_stderr.replace(str(PROJECT_ROOT), "<repo>")[
            -131072:
        ],
        "output_image": image,
    }
    if args.cancel_immediately:
        report["status"] = "passed" if terminal and terminal["type"] == "cancelled" else "failed"
        report["cancel_received"] = bool(terminal and terminal["type"] == "cancelled")
    elif not candidate or status != 0:
        report["status"] = "failed"
    write_report(args.report, report)
    print(
        json.dumps(
            {
                "status": report["status"],
                "report": display_path(args.report),
                "image": display_path(Path(image)) if image else None,
            },
            ensure_ascii=False,
        )
    )
    return 0 if report["status"] == "passed" else 1


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--mode", choices=["run", "probe", "prepare"], default="run")
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--worker-site", type=Path, required=True)
    parser.add_argument("--snapshot", required=True)
    parser.add_argument("--model", type=Path)
    parser.add_argument("--prepared-dir", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--report-output", type=Path)
    parser.add_argument("--capability")
    parser.add_argument("--source-image", type=Path)
    parser.add_argument("--prompt", default="雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光")
    parser.add_argument("--seed", type=int, default=3148150817)
    parser.add_argument("--cancel-immediately", action="store_true")
    parser.add_argument("--rebuild-prepared", action="store_true")
    parser.add_argument("--supervisor", type=Path)
    args = parser.parse_args()
    model_spec(args.snapshot)  # fail fast on an unknown snapshot
    if args.mode == "probe":
        return run_probe(args, args.snapshot, args.worker_site)
    if args.mode == "prepare":
        return run_prepare(args, args.snapshot, args.worker_site)
    return run_protocol(args, args.snapshot, args.worker_site)


if __name__ == "__main__":
    raise SystemExit(main())
