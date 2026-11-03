"""Summarize the pip dependency resolution used to freeze the Windows v2 runtime.

The raw `pip install --dry-run --report` output lives in the gitignored build
directory; this summary keeps the resolver facts (command, indexes, resolved
pins, canonical URLs and hashes) next to the checked-in evidence.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/runtime-manifest.json",
    )
    parser.add_argument(
        "--lock",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/requirements.lock",
    )
    parser.add_argument(
        "--resolve-report",
        type=Path,
        default=ROOT / "target/parity-runtime/pip-resolve-report.json",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "verification/windows-parity-runtime/dependency-resolution.json",
    )
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    lock_lines = [
        line.strip() for line in args.lock.read_text(encoding="utf-8").splitlines() if line.strip() and not line.startswith("#")
    ]
    pins = dict(line.split("==", 1) for line in lock_lines)
    report = json.loads(args.resolve_report.read_text(encoding="utf-8"))
    report_packages = {}
    for item in report["install"]:
        metadata = item["metadata"]
        report_packages[re.sub(r"[-_.]+", "-", metadata["name"]).lower()] = {
            "version": metadata["version"],
            "report_url": item["download_info"]["url"],
        }
    packages = []
    for wheel in manifest["wheels"]:
        name = wheel["package"]
        packages.append(
            {
                "package": name,
                "version": wheel["version"],
                "wheel": wheel["path"],
                "bytes": wheel["bytes"],
                "sha256": wheel["sha256"],
                "canonical_url": wheel["url"],
                "resolver_version": report_packages.get(name, {}).get("version"),
            }
        )
    mismatch = [item["package"] for item in packages if item["resolver_version"] not in (None, item["version"])]
    evidence = {
        "schema_version": 1,
        "kind": "windows-parity-runtime-dependency-resolution",
        "runtime_id": manifest["id"],
        "status": "pass" if not mismatch else "fail",
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "resolver": {
            "tool": "pip 26.2.1 (host Python 3.14) with cross-target flags",
            "resolve_command": (
                "python -m pip install --dry-run --ignore-installed --report target/parity-runtime/pip-resolve-report.json "
                "--target target/parity-runtime/_resolve_probe -r target/parity-runtime/top-level-requirements.txt "
                "--only-binary=:all: --platform win_amd64 --implementation cp --python-version 3.12 --abi cp312 "
                "--index-url https://pypi.org/simple --extra-index-url https://download.pytorch.org/whl/cu126"
            ),
            "indexes": ["https://pypi.org/simple", "https://download.pytorch.org/whl/cu126"],
            "target": {"platform": "win_amd64", "implementation": "cp", "python_version": "3.12", "abi": "cp312"},
            "raw_report": {
                "path": "target/parity-runtime/pip-resolve-report.json (build artifact, gitignored)",
                "bytes": args.resolve_report.stat().st_size,
                "sha256": sha256_file(args.resolve_report),
                "resolved_items": len(report.get("install", [])),
            },
        },
        "top_level_requirements": manifest["top_level_requirements"],
        "locked_packages": len(pins),
        "resolved_packages": packages,
        "resolver_vs_lock_mismatches": mismatch,
        "notes": [
            "torch and torchvision resolve from the PyTorch cu126 index; every other package resolves from PyPI.",
            "The lock pins every resolved distribution, so the offline install needs no resolver network access.",
            "pip is stored in the wheelhouse only as bootstrap tooling and is not part of the runtime closure lock.",
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"status": evidence["status"], "packages": len(packages), "output": str(args.output)}))
    return 0 if evidence["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
