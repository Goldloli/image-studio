"""Extract the resolved runtime closure from a `pip install --dry-run --report` file.

Writes:
  - resources/windows-parity/runtime-v2/requirements.lock (sorted exact pins)
  - target/parity-runtime/resolved-closure.json (name/version/url summary)

The report URL is recorded for provenance only; final artifact URLs and hashes
are re-derived from the official indexes by build_runtime_manifest_v2.py.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def main() -> int:
    report_path = Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "target/parity-runtime/pip-resolve-report.json")
    lock_out = Path(
        sys.argv[2]
        if len(sys.argv) > 2
        else ROOT / "resources/windows-parity/runtime-v2/requirements.lock"
    )
    summary_out = Path(
        sys.argv[3] if len(sys.argv) > 3 else ROOT / "target/parity-runtime/resolved-closure.json"
    )
    report = json.loads(report_path.read_text(encoding="utf-8"))
    rows = []
    for item in report["install"]:
        meta = item["metadata"]
        download = item["download_info"]
        archive = download.get("archive_info", {})
        hashes = archive.get("hashes", {})
        sha256 = hashes.get("sha256")
        if sha256 is None and "hash" in archive:
            sha256 = archive["hash"].removeprefix("sha256=")
        rows.append(
            {
                "package": meta["name"],
                "version": meta["version"],
                "report_url": download["url"],
                "report_sha256": sha256,
            }
        )
    rows.sort(key=lambda row: row["package"].lower())
    lines = [
        "# Windows v2 Python runtime closure (resolved with pip for cp312-win_amd64).",
        "# Top-level requirements: torch 2.10.0+cu126, torchvision 0.25.0+cu126,",
        "# diffusers 0.41.0, transformers 5.3.0, accelerate 1.12.0,",
        "# bitsandbytes 0.50.2, safetensors 0.8.0, huggingface-hub 1.33.0,",
        "# onnxruntime 1.31.1 (CPU), psutil 7.1.3.",
        "# Install offline with: pip install --no-index --find-links <wheelhouse> -r requirements.lock",
    ]
    for row in rows:
        lines.append(f"{row['package']}=={row['version']}")
    lock_out.parent.mkdir(parents=True, exist_ok=True)
    lock_out.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    summary_out.parent.mkdir(parents=True, exist_ok=True)
    summary_out.write_text(
        json.dumps({"schema_version": 1, "count": len(rows), "packages": rows}, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    print(json.dumps({"lock": str(lock_out), "summary": str(summary_out), "count": len(rows)}, indent=2))
    missing = [row["package"] for row in rows if not row["report_sha256"]]
    print(f"report entries without sha256 (re-derived later): {missing}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
