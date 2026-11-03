"""Extract the Vulkan device list from captured Real-ESRGAN ncnn verbose logs.

The runner already records nvidia-smi state, but the ncnn device identity lines
are emitted by the frozen executable itself. This helper re-parses the captured
run logs into a standalone evidence file (no GPU work, no process launch).
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import datetime, timezone
from pathlib import Path

DEVICE_LINE = re.compile(r"^\[(\d+)\s+([^\]]+?)\]")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--log", type=Path, action="append", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--case-id", default="G2-R1")
    args = parser.parse_args()

    sources = []
    devices: dict[int, dict[str, object]] = {}
    for log_path in args.log:
        text = log_path.read_text(encoding="utf-8", errors="replace")
        raw_lines = [line.strip() for line in text.splitlines() if DEVICE_LINE.match(line.strip())]
        for line in raw_lines:
            match = DEVICE_LINE.match(line)
            index = int(match.group(1))
            name = match.group(2).strip()
            entry = devices.setdefault(index, {"index": index, "name": name, "raw_lines": []})
            if line not in entry["raw_lines"]:
                entry["raw_lines"].append(line)
        sources.append(
            {
                "path": str(log_path),
                "bytes": log_path.stat().st_size,
                "sha256": sha256(log_path),
                "device_line_count": len(raw_lines),
            }
        )
    ordered = [devices[index] for index in sorted(devices)]
    evidence = {
        "schema_version": 1,
        "kind": "windows-parity-realesrgan-vulkan-device-identity",
        "case_id": args.case_id,
        "extracted_at": datetime.now(timezone.utc).isoformat(),
        "method": "Parsed post-run from the frozen executable's own -v stderr logs; no GPU work performed by this step.",
        "sources": sources,
        "devices": ordered,
        "selected_device": next((item for item in ordered if item["index"] == 0), None),
        "nvidia_device_selected": bool(ordered and ordered[0]["name"].startswith("NVIDIA")),
        "declared_gpu_arg": 0,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"output": str(args.output), "devices": [item["name"] for item in ordered]}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
