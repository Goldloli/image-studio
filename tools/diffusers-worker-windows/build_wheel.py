"""Reproducible stdlib-only wheel build for the Windows diffusers worker."""

from __future__ import annotations

import base64
import csv
import hashlib
import io
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PACKAGE = "studio_diffusers_worker"
OUT = (
    ROOT.parents[1]
    / "resources/windows-parity/workers"
    / "studio_diffusers_worker-0.1.0-py3-none-any.whl"
)
FIXED_TIMESTAMP = (2026, 1, 1, 0, 0, 0)

METADATA = (
    "Metadata-Version: 2.1\n"
    "Name: studio-diffusers-worker\n"
    "Version: 0.1.0\n"
    "Requires-Python: ==3.12.*\n"
    "Requires-Dist: torch==2.10.0+cu126\n"
    "Requires-Dist: diffusers==0.41.0\n"
    "Requires-Dist: transformers==5.3.0\n"
    "Requires-Dist: accelerate==1.12.0\n"
    "Requires-Dist: bitsandbytes==0.50.2\n"
    "Requires-Dist: safetensors==0.8.0\n"
    "Requires-Dist: huggingface-hub==1.33.0\n"
    "Requires-Dist: numpy==2.5.4\n"
    "Requires-Dist: pillow==13.0.0\n"
    "License: Apache-2.0\n"
)

WHEEL = (
    "Wheel-Version: 1.0\n"
    "Generator: image-studio-stdlib\n"
    "Root-Is-Purelib: true\n"
    "Tag: py3-none-any\n"
)


def collect_files() -> dict[str, bytes]:
    files: dict[str, bytes] = {}
    for path in sorted(ROOT.joinpath(PACKAGE).rglob("*")):
        if not path.is_file():
            continue
        if "__pycache__" in path.parts:
            continue
        relative = path.relative_to(ROOT).as_posix()
        files[relative] = path.read_bytes()
    info = "studio_diffusers_worker-0.1.0.dist-info/"
    files[info + "METADATA"] = METADATA.encode()
    files[info + "WHEEL"] = WHEEL.encode()
    return files


def build() -> Path:
    files = collect_files()
    record = io.StringIO(newline="")
    writer = csv.writer(record, lineterminator="\n")
    for name, data in sorted(files.items()):
        digest = hashlib.sha256(data).digest()
        writer.writerow(
            (
                name,
                "sha256=" + base64.urlsafe_b64encode(digest).decode().rstrip("="),
                len(data),
            )
        )
    info = "studio_diffusers_worker-0.1.0.dist-info/"
    writer.writerow((info + "RECORD", "", ""))
    files[info + "RECORD"] = record.getvalue().encode()
    OUT.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(OUT, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        for name, data in sorted(files.items()):
            item = zipfile.ZipInfo(name, FIXED_TIMESTAMP)
            item.compress_type = zipfile.ZIP_DEFLATED
            item.external_attr = 0o644 << 16
            archive.writestr(item, data)
    return OUT


if __name__ == "__main__":
    path = build()
    data = path.read_bytes()
    print(path)
    print(f"bytes={len(data)}")
    print(f"sha256={hashlib.sha256(data).hexdigest()}")
