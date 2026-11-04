"""Run one real Windows BiRefNet ONNX worker generation-v2 protocol exchange.

The frozen Windows v2 Python runtime is an embeddable distribution whose
``python312._pth`` disables ``PYTHONPATH``, so the harness launches the worker
through a tiny ``-c`` bootstrap that puts the wheel installation directory on
``sys.path`` and then calls the package's real ``main()``. Production launches
the same package from the runtime site-packages with ``-I -m studio_onnx_worker``;
only the path bootstrap differs.

The exchange is the exact v2 frame sequence the desktop host sends (hello,
hello_accepted, execute_v2, progress, output_candidate_v2 x2), then verifies the
dual RGBA/mask transaction, budget and PNG identity, and writes evidence JSON.
"""

import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import threading
import time
from datetime import datetime, timezone

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from studio_onnx_worker import (  # noqa: E402
    CAPABILITIES,
    CAPABILITY,
    MODEL_ID,
    MODEL_SNAPSHOT_ID,
    RECIPE_ID,
    RUNTIME_LOCK_ID,
    expected_parameters,
    worker_identity,
)

FAMILY = "image-studio.generation"
MAX_OUTPUT_BYTES = 32 * 1024 * 1024
EXPECTED_MODEL_SHA256 = (
    "58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c"
)
EXPECTED_MODEL_BYTES = 972666916


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


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


class _MemoryCounters(ctypes.Structure):
    _fields_ = [
        ("cb", ctypes.c_ulong),
        ("PageFaultCount", ctypes.c_ulong),
        ("PeakWorkingSetSize", ctypes.c_size_t),
        ("WorkingSetSize", ctypes.c_size_t),
        ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPagedPoolUsage", ctypes.c_size_t),
        ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
        ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
        ("PagefileUsage", ctypes.c_size_t),
        ("PeakPagefileUsage", ctypes.c_size_t),
    ]


def peak_working_set_mib(process_id):
    """Best-effort worker peak working set via psapi (stdlib only, Windows)."""
    if not sys.platform.startswith("win"):
        return None
    try:
        kernel32 = ctypes.windll.kernel32
        psapi = ctypes.windll.psapi
        handle = kernel32.OpenProcess(0x1000, False, process_id)
        if not handle:
            return None
        try:
            counters = _MemoryCounters()
            counters.cb = ctypes.sizeof(counters)
            if not psapi.GetProcessMemoryInfo(
                handle, ctypes.byref(counters), counters.cb
            ):
                return None
            return round(counters.PeakWorkingSetSize / (1024 * 1024), 1)
        finally:
            kernel32.CloseHandle(handle)
    except Exception:  # noqa: BLE001 - diagnostics are best effort
        return None


class ResourceSampler(threading.Thread):
    def __init__(self, process_id):
        super().__init__(daemon=True)
        self.process_id = process_id
        self.peak_mib = None
        self.samples = 0
        # Thread already uses the private ``_stop`` attribute internally.
        self._stop_event = threading.Event()

    def run(self):
        while not self._stop_event.is_set():
            value = peak_working_set_mib(self.process_id)
            self.samples += 1
            if value is not None:
                self.peak_mib = max(self.peak_mib or 0.0, value)
            self._stop_event.wait(0.1)

    def stop(self):
        self._stop_event.set()
        self.join(timeout=2)


def utc_now():
    return datetime.now(timezone.utc).isoformat(timespec="seconds")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--python", type=Path, required=True)
    parser.add_argument("--package-root", type=Path, required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-image", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--evidence-dir", type=Path)
    parser.add_argument("--wheel", type=Path)
    parser.add_argument("--model-snapshot-id", default=MODEL_SNAPSHOT_ID)
    args = parser.parse_args()

    project_root = Path(__file__).resolve().parents[3]
    package_root = args.package_root.resolve()
    python = args.python.resolve()
    model = args.model.resolve()
    output = args.output.resolve()
    report_path = args.report.resolve()
    report_path.parent.mkdir(parents=True, exist_ok=True)
    evidence_dir = args.evidence_dir.resolve() if args.evidence_dir else None

    started = time.monotonic()
    started_at = utc_now()
    identity = worker_identity(args.model_snapshot_id)
    parameters = expected_parameters()
    failures = []

    def display(path):
        try:
            return path.resolve().relative_to(project_root).as_posix()
        except ValueError:
            return path.name

    model_files = sorted(model.glob("*.onnx"))
    if len(model_files) != 1:
        raise SystemExit("model directory must contain exactly one ONNX file")
    model_file = model_files[0]
    model_bytes = model_file.stat().st_size
    model_sha256 = sha256_file(model_file)
    model_matches_frozen = (
        model_sha256 == EXPECTED_MODEL_SHA256 and model_bytes == EXPECTED_MODEL_BYTES
    )

    output.mkdir(parents=True, exist_ok=True)
    for candidate_name in ("primary.png", "mask.png"):
        stale = output / candidate_name
        if stale.exists() or stale.is_symlink():
            stale.unlink()
    source = output / "input.png"
    shutil.copyfile(args.source_image.resolve(), source)
    source_sha256 = sha256_file(source)
    with Image.open(source) as opened:
        opened.load()
        source_mode = opened.mode
        source_width, source_height = opened.size
        source_rgba = np.asarray(opened.convert("RGBA"), dtype=np.uint8).copy()

    bootstrap = (
        f"import sys; sys.path.insert(0, {str(package_root)!r}); "
        "from studio_onnx_worker.__main__ import main; raise SystemExit(main())"
    )
    command = [
        str(python),
        "-I",
        "-c",
        bootstrap,
        "--model-snapshot-id",
        args.model_snapshot_id,
    ]
    environment = os.environ.copy()
    for key in (
        "HF_HUB_OFFLINE",
        "TRANSFORMERS_OFFLINE",
        "HF_HUB_DISABLE_TELEMETRY",
        "DO_NOT_TRACK",
    ):
        environment[key] = "1"
    execute_started = time.monotonic()
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
    stderr_lines = []
    stderr_bytes = [0]
    stderr_thread = threading.Thread(
        target=drain_stderr,
        args=(process.stderr, stderr_lines, stderr_bytes),
        daemon=True,
    )
    stderr_thread.start()
    sampler = ResourceSampler(process.pid)
    sampler.start()
    progress = []
    candidates = []
    hello_seconds = None
    terminal = None
    try:
        write_frame(
            process.stdin,
            {
                "family": FAMILY,
                "version": None,
                "session_id": "session:onnx-birefnet-windows",
                "request_id": "request:hello",
                "sequence": 0,
                "execution": None,
                "message": {
                    "type": "hello",
                    "versions": {"min": 2, "max": 2},
                    "build_id": "studio-host-0.1.0",
                    "expected_worker": identity,
                    "capabilities": list(CAPABILITIES),
                },
            },
        )
        accepted = read_frame(process.stdout)
        hello_seconds = time.monotonic() - execute_started
        if accepted["message"] != {
            "type": "hello_accepted",
            "versions": {"min": 2, "max": 2},
            "selected_version": 2,
            "identity": identity,
            "capabilities": list(CAPABILITIES),
        }:
            raise RuntimeError("worker handshake mismatch")
        write_frame(
            process.stdin,
            {
                "family": FAMILY,
                "version": 2,
                "session_id": "session:onnx-birefnet-windows",
                "request_id": "request:execute",
                "sequence": 1,
                "execution": {
                    "job_id": "job:onnx-birefnet-windows",
                    "attempt_id": "attempt:onnx-birefnet-windows",
                    "execution_token": "token:onnx-birefnet-windows",
                },
                "message": {
                    "type": "execute_v2",
                    "request": {
                        "model_id": MODEL_ID,
                        "capability_id": CAPABILITY,
                        "recipe_id": RECIPE_ID,
                        "recipe_version": 1,
                        "runtime_lock_id": RUNTIME_LOCK_ID,
                        "model_snapshot_id": identity["model_snapshot_id"],
                        "instruction": "",
                        "seed": None,
                        "sampling_steps": None,
                        "parameters": parameters,
                        "inputs": [
                            {
                                "role": "source",
                                "path": str(source),
                                "width": source_width,
                                "height": source_height,
                                "sha256": source_sha256,
                            }
                        ],
                        "outputs": [
                            {
                                "role": "primary",
                                "data_type": "image_rgba",
                                "width": source_width,
                                "height": source_height,
                                "max_bytes": MAX_OUTPUT_BYTES,
                            },
                            {
                                "role": "mask",
                                "data_type": "mask",
                                "width": source_width,
                                "height": source_height,
                                "max_bytes": MAX_OUTPUT_BYTES,
                            },
                        ],
                        "model_path": str(model),
                        "output_dir": str(output),
                    },
                },
            },
        )
        execute_at = time.monotonic()
        while terminal is None:
            frame = read_frame(process.stdout)
            message = frame["message"]
            if message["type"] == "progress":
                progress.append(
                    {
                        "stage": message["stage"],
                        "completed": message["completed"],
                        "total": message["total"],
                        "seconds_since_execute": round(time.monotonic() - execute_at, 3),
                        "seconds_since_start": round(time.monotonic() - started, 3),
                    }
                )
            elif message["type"] == "output_candidate_v2":
                candidates.append(message["candidate"])
                # A successful worker sends both candidates then exits without a
                # separate terminal success frame (v2 contract).
                if len(candidates) == 2:
                    break
            elif message["type"] in {"error", "cancelled"}:
                terminal = message
        status = process.wait(timeout=120)
        sampler.stop()
        if stderr_thread.is_alive():
            stderr_thread.join(timeout=2)
        if terminal is not None:
            raise RuntimeError(f"worker failed: {terminal}; exit={status}")
        if status != 0:
            raise RuntimeError(f"worker exited with {status}")
        if len(candidates) != 2:
            raise RuntimeError(f"expected two candidates, got {len(candidates)}")
        by_role = {candidate["role"]: candidate for candidate in candidates}
        if set(by_role) != {"primary", "mask"}:
            raise RuntimeError("candidate roles differ from the frozen contract")
        if [candidate["role"] for candidate in candidates] != ["primary", "mask"]:
            failures.append("candidate emission order is not primary then mask")

        expected_specs = {
            "primary": ("image_rgba", "primary.png"),
            "mask": ("mask", "mask.png"),
        }
        for role, (data_type, relative_path) in expected_specs.items():
            candidate = by_role[role]
            path = output / candidate["relative_path"]
            if candidate["relative_path"] != relative_path:
                failures.append(f"{role} relative path differs")
            if candidate["data_type"] != data_type:
                failures.append(f"{role} data type differs")
            if (candidate["width"], candidate["height"]) != (
                source_width,
                source_height,
            ):
                failures.append(f"{role} dimensions differ from source")
            if not 0 < candidate["size_bytes"] <= MAX_OUTPUT_BYTES:
                failures.append(f"{role} exceeds the 32 MiB candidate budget")
            if candidate["size_bytes"] != path.stat().st_size:
                failures.append(f"{role} size differs from the written file")
            if sha256_file(path) != candidate["sha256"]:
                failures.append(f"{role} sha256 differs from the written file")

        with Image.open(output / "primary.png") as opened:
            opened.load()
            primary_mode = opened.mode
            primary_size = opened.size
            primary = np.asarray(opened, dtype=np.uint8).copy()
        with Image.open(output / "mask.png") as opened:
            opened.load()
            mask_mode = opened.mode
            mask_size = opened.size
            mask = np.asarray(opened, dtype=np.uint8).copy()
        checks = {
            "primary_mode_rgba": primary_mode == "RGBA",
            "mask_mode_l": mask_mode == "L",
            "primary_dimensions_equal_source": primary_size
            == (source_width, source_height),
            "mask_dimensions_equal_source": mask_size == (source_width, source_height),
            "primary_alpha_equals_mask": bool(np.array_equal(primary[..., 3], mask)),
            "primary_rgb_equals_source_rgb_straight_alpha": bool(
                np.array_equal(primary[..., :3], source_rgba[..., :3])
            ),
            "mask_has_partial_alpha": bool(
                np.any((mask > 0) & (mask < 255))
            ),
            "mask_spatial_variation": bool(np.unique(mask).size > 1),
            "mask_fully_transparent_pixels": int(np.count_nonzero(mask == 0)),
            "mask_fully_opaque_pixels": int(np.count_nonzero(mask == 255)),
            "mask_partial_pixels": int(np.count_nonzero((mask > 0) & (mask < 255))),
            "mask_mean": round(float(mask.mean()), 3),
        }
        for name, passed in checks.items():
            if isinstance(passed, bool) and not passed:
                failures.append(f"check failed: {name}")
        if not model_matches_frozen:
            failures.append("model file is not the frozen BiRefNet epoch_244 asset")

        evidence = {}
        if evidence_dir is not None:
            evidence_dir.mkdir(parents=True, exist_ok=True)
            for role, name in (
                ("primary", "onnx-birefnet-primary.png"),
                ("mask", "onnx-birefnet-mask.png"),
            ):
                target = evidence_dir / name
                shutil.copyfile(output / by_role[role]["relative_path"], target)
                evidence[name] = {
                    "path": display(target),
                    "bytes": target.stat().st_size,
                    "sha256": sha256_file(target),
                }

        stage_seconds = {}
        for item in progress:
            stage_seconds.setdefault(item["stage"], item["seconds_since_start"])
        report = {
            "schema_version": 1,
            "kind": "windows-parity-onnx-birefnet-v2-protocol",
            "case_id": "G2-B2",
            "status": "passed" if not failures else "failed",
            "started_at_utc": started_at,
            "finished_at_utc": utc_now(),
            "machine": {
                "os": platform.platform(),
                "python": platform.python_version(),
                "machine": platform.machine(),
                "processor": platform.processor(),
                "logical_cores": os.cpu_count(),
            },
            "identity": identity,
            "runtime_lock_id": RUNTIME_LOCK_ID,
            "worker_wheel": (
                {
                    "path": display(args.wheel.resolve()),
                    "bytes": args.wheel.resolve().stat().st_size,
                    "sha256": sha256_file(args.wheel.resolve()),
                    "installed_site": display(package_root),
                }
                if args.wheel is not None
                else None
            ),
            "model": {
                "path": display(model_file),
                "bytes": model_bytes,
                "sha256": model_sha256,
                "expected_bytes": EXPECTED_MODEL_BYTES,
                "expected_sha256": EXPECTED_MODEL_SHA256,
                "matches_frozen_asset": model_matches_frozen,
            },
            "input": {
                "path": display(source),
                "source_path": display(args.source_image.resolve()),
                "mode": source_mode,
                "width": source_width,
                "height": source_height,
                "bytes": source.stat().st_size,
                "sha256": source_sha256,
            },
            "request": {
                "model_id": MODEL_ID,
                "capability_id": CAPABILITY,
                "recipe_id": RECIPE_ID,
                "recipe_version": 1,
                "instruction": "",
                "seed": None,
                "sampling_steps": None,
                "parameters": parameters,
                "inputs": 1,
                "outputs": ["primary(image_rgba)", "mask(mask)"],
            },
            "protocol": {
                "family": FAMILY,
                "version": 2,
                "hello_accepted": True,
                "progress_frames": len(progress),
                "candidate_frames": len(candidates),
            },
            "progress": progress,
            "timings": {
                "total_seconds": round(time.monotonic() - started, 3),
                "handshake_seconds": round(hello_seconds, 3),
                "execute_to_terminal_seconds": round(time.monotonic() - execute_at, 3),
                "first_observed_stage_seconds": stage_seconds,
            },
            "resources": {
                "worker_peak_working_set_mib": sampler.peak_mib,
                "sample_count": sampler.samples,
                "sample_interval_seconds": 0.1,
            },
            "candidates": candidates,
            "checks": checks,
            "evidence": evidence,
            "worker_stderr_tail": "".join(stderr_lines)[-8192:].replace(
                str(project_root), "<repo>"
            ),
            "failures": failures,
        }
        temporary = report_path.with_suffix(report_path.suffix + ".pending")
        temporary.write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
            newline="\n",
        )
        os.replace(temporary, report_path)
        print(
            json.dumps(
                {
                    "status": report["status"],
                    "report": str(report_path),
                    "failures": failures,
                }
            )
        )
        return 0 if not failures else 1
    except Exception as error:  # noqa: BLE001 - report a failed evidence record
        sampler.stop()
        try:
            process.kill()
        except OSError:
            pass
        process.wait(timeout=30)
        report = {
            "schema_version": 1,
            "kind": "windows-parity-onnx-birefnet-v2-protocol",
            "case_id": "G2-B2",
            "status": "failed",
            "started_at_utc": started_at,
            "finished_at_utc": utc_now(),
            "identity": identity,
            "progress": progress,
            "candidates": candidates,
            "worker_stderr_tail": "".join(stderr_lines)[-8192:].replace(
                str(project_root), "<repo>"
            ),
            "failures": [*failures, f"{type(error).__name__}: {error}"],
        }
        report_path.write_text(
            json.dumps(report, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
            newline="\n",
        )
        print(json.dumps({"status": "failed", "report": str(report_path)}))
        return 1
    finally:
        if process.poll() is None:
            process.kill()
            process.wait()


if __name__ == "__main__":
    raise SystemExit(main())
