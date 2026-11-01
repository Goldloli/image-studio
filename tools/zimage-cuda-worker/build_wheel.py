"""Reproducible stdlib-only build for the Windows Z-Image worker."""

import base64
import csv
import hashlib
import io
from pathlib import Path
import zipfile

root = Path(__file__).resolve().parent
out = (
    root.parents[1]
    / "resources/windows-foundation/z-image-diffusers"
    / "studio_zimage_cuda_worker-0.1.0-py3-none-any.whl"
)
files = {
    f"studio_zimage_cuda_worker/{path.name}": path.read_bytes()
    for path in sorted((root / "studio_zimage_cuda_worker").glob("*.py"))
}
info = "studio_zimage_cuda_worker-0.1.0.dist-info/"
files[info + "METADATA"] = (
    b"Metadata-Version: 2.1\n"
    b"Name: studio-zimage-cuda-worker\n"
    b"Version: 0.1.0\n"
    b"Requires-Python: ==3.12.*\n"
    b"Requires-Dist: torch==2.10.0+cu126\n"
    b"Requires-Dist: diffusers==0.36.0\n"
    b"Requires-Dist: transformers==4.57.3\n"
    b"Requires-Dist: accelerate==1.12.0\n"
    b"Requires-Dist: bitsandbytes==0.48.2\n"
    b"License: Apache-2.0\n"
)
files[info + "WHEEL"] = (
    b"Wheel-Version: 1.0\n"
    b"Generator: image-studio-stdlib\n"
    b"Root-Is-Purelib: true\n"
    b"Tag: py3-none-any\n"
)
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
writer.writerow((info + "RECORD", "", ""))
files[info + "RECORD"] = record.getvalue().encode()
with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    for name, data in sorted(files.items()):
        item = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
        item.compress_type = zipfile.ZIP_DEFLATED
        item.external_attr = 0o644 << 16
        archive.writestr(item, data)
print(out)
