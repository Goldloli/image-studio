"""Run the Real-ESRGAN ncnn Vulkan Gate 0 case (G2-R1) and record evidence.

The command surface is fixed: model realesrgan-x4plus, scale 4, tile 128,
single GPU 0, load:proc:save 1:1:1, TTA disabled. The script verifies the output
is exactly 4x the input dimensions, hashes input/output, captures the Vulkan
device identity printed by the ncnn program under -v, and records nvidia-smi
state before/after. Stdlib only; runs on the host Python.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import struct
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

DEVICE_LINE = re.compile(r"^\[(\d+)\s+([^\]]+?)\]")


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def png_dimensions(path: Path) -> tuple[int, int]:
    with path.open("rb") as handle:
        header = handle.read(24)
    if len(header) < 24 or header[:8] != b"\x89PNG\r\n\x1a\n" or header[12:16] != b"IHDR":
        raise ValueError(f"not a PNG file: {path}")
    width, height = struct.unpack(">II", header[16:24])
    return int(width), int(height)


def nvidia_smi() -> dict[str, object]:
    command = [
        "nvidia-smi",
        "--query-gpu=name,uuid,driver_version,memory.total,memory.used,utilization.gpu",
        "--format=csv,noheader",
    ]
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=30)
    except OSError as error:  # pragma: no cover - capability dependent
        return {"available": False, "error": str(error)}
    if result.returncode != 0:
        return {"available": False, "error": result.stderr.strip()}
    rows = [line.strip() for line in result.stdout.splitlines() if line.strip()]
    return {"available": True, "command": command, "rows": rows, "raw": result.stdout.strip()}


def run(command: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(command, cwd=str(cwd), capture_output=True, text=True, timeout=900)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--app-dir", type=Path, required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--label", required=True)
    parser.add_argument("--verify-device-index", type=int, default=0)
    args = parser.parse_args()

    app_dir = args.app_dir.resolve()
    executable = app_dir / "realesrgan-ncnn-vulkan.exe"
    input_path = args.input.resolve()
    output_path = args.output.resolve()
    evidence_path = args.evidence.resolve()
    output_path.parent.mkdir(parents=True, exist_ok=True)
    output_path.unlink(missing_ok=True)

    if not executable.is_file():
        raise SystemExit(f"missing executable: {executable}")
    input_dims = png_dimensions(input_path)
    input_record = {
        "path": str(input_path),
        "dims": list(input_dims),
        "bytes": input_path.stat().st_size,
        "sha256": sha256(input_path),
    }

    command = [
        str(executable),
        "-i",
        str(input_path),
        "-o",
        str(output_path),
        "-n",
        "realesrgan-x4plus",
        "-s",
        "4",
        "-t",
        "128",
        "-g",
        "0",
        "-j",
        "1:1:1",
        "-f",
        "png",
        "-v",
    ]
    gpu_before = nvidia_smi()
    started_at = datetime.now(timezone.utc).isoformat()
    started = time.perf_counter()
    result = run(command, app_dir)
    wall_seconds = time.perf_counter() - started
    finished_at = datetime.now(timezone.utc).isoformat()
    gpu_after = nvidia_smi()

    log_lines = (result.stdout + "\n" + result.stderr).splitlines()
    device_map: dict[int, str] = {}
    device_raw_lines: list[str] = []
    for line in log_lines:
        match = DEVICE_LINE.match(line.strip())
        if match:
            index = int(match.group(1))
            device_map[index] = match.group(2).strip()
            device_raw_lines.append(line.strip())
    devices = [
        {"index": index, "name": name, "source": "ncnn verbose log"}
        for index, name in sorted(device_map.items())
    ]
    stdout = result.stdout
    stderr = result.stderr

    status = "fail"
    failure = None
    output_record: dict[str, object] | None = None
    exact_4x = False
    if result.returncode != 0:
        failure = f"process exited with {result.returncode}"
    elif not output_path.is_file():
        failure = "output file missing"
    else:
        output_dims = png_dimensions(output_path)
        output_record = {
            "path": str(output_path),
            "dims": list(output_dims),
            "bytes": output_path.stat().st_size,
            "sha256": sha256(output_path),
        }
        exact_4x = output_dims == (input_dims[0] * 4, input_dims[1] * 4)
        if not exact_4x:
            failure = f"dimensions {output_dims} are not 4x {input_dims}"
        else:
            status = "pass"

    evidence = {
        "schema_version": 1,
        "kind": "windows-parity-realesrgan-gate0-run",
        "case_id": "G2-R1",
        "label": args.label,
        "status": status,
        "failure": failure,
        "started_at": started_at,
        "finished_at": finished_at,
        "wall_seconds": round(wall_seconds, 3),
        "host": {
            "platform": sys.platform,
            "system": os.environ.get("OS", ""),
            "python": sys.version.split()[0],
            "machine": os.environ.get("PROCESSOR_IDENTIFIER", ""),
        },
        "command": command,
        "cwd": str(app_dir),
        "exit_code": result.returncode,
        "fixed_parameters": {
            "model": "realesrgan-x4plus",
            "scale": 4,
            "tile": 128,
            "gpu_ids": [0],
            "single_gpu": True,
            "threads": "1:1:1",
            "tta": False,
            "format": "png",
            "executable_sha256": sha256(executable),
        },
        "input": input_record,
        "output": output_record,
        "exact_4x": exact_4x,
        "device_identity": {
            "ncnn_verbose_devices": devices,
            "ncnn_verbose_device_lines": device_raw_lines,
            "nvidia_smi_before": gpu_before,
            "nvidia_smi_after": gpu_after,
            "device_index_check": args.verify_device_index,
        },
        "log_files": {
            "stdout": str(evidence_path.with_suffix(".stdout.log")),
            "stderr": str(evidence_path.with_suffix(".stderr.log")),
        },
        "limitations": [
            "Gate 0 single-run evidence for one input; not a throughput benchmark.",
            "ncnn tiling may produce block-boundary differences from the PyTorch implementation.",
        ],
    }
    evidence_path.parent.mkdir(parents=True, exist_ok=True)
    evidence_path.with_suffix(".stdout.log").write_text(stdout, encoding="utf-8", newline="\n")
    evidence_path.with_suffix(".stderr.log").write_text(stderr, encoding="utf-8", newline="\n")
    evidence_path.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"status": status, "wall_seconds": evidence["wall_seconds"], "evidence": str(evidence_path), "failure": failure}))
    return 0 if status == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
