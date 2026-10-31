"""Reproducible stdlib-only build for the independent Qwen/MOSS worker."""

import base64
import csv
import hashlib
import io
from pathlib import Path
import zipfile

root = Path(__file__).resolve().parent
out = (
    root.parents[1]
    / "resources/planned-models/qwen21"
    / "studio_mflux_worker_v3-0.3.0-py3-none-any.whl"
)
files = {
    f"studio_mflux_worker_v3/{path.name}": path.read_bytes()
    for path in sorted((root / "studio_mflux_worker_v3").glob("*.py"))
}
info = "studio_mflux_worker_v3-0.3.0.dist-info/"
files[info + "METADATA"] = (
    b"Metadata-Version: 2.1\n"
    b"Name: studio-mflux-worker-v3\n"
    b"Version: 0.3.0\n"
    b"Requires-Python: ==3.13.*\n"
    b"Requires-Dist: mflux==0.24.0\n"
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
