"""Reproducible stdlib-only build for the Windows BiRefNet ONNX worker wheel."""

import base64
import csv
import hashlib
import io
from pathlib import Path
import zipfile

root = Path(__file__).resolve().parent
out = (
    root.parents[1]
    / "resources/windows-parity/workers"
    / "studio_onnx_worker-0.1.0-py3-none-any.whl"
)
files = {
    f"studio_onnx_worker/{path.name}": path.read_bytes()
    for path in sorted((root / "studio_onnx_worker").glob("*.py"))
}
info = "studio_onnx_worker-0.1.0.dist-info/"
files[info + "METADATA"] = (
    b"Metadata-Version: 2.1\n"
    b"Name: studio-onnx-worker\n"
    b"Version: 0.1.0\n"
    # The frozen Windows v2 runtime is Python 3.12.10; the macOS wheel pins 3.13.
    b"Requires-Python: ==3.12.*\n"
    b"Requires-Dist: onnxruntime==1.31.1\n"
    b"Requires-Dist: numpy==2.5.4\n"
    b"Requires-Dist: pillow==13.0.0\n"
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
out.parent.mkdir(parents=True, exist_ok=True)
with zipfile.ZipFile(out, "w", compression=zipfile.ZIP_DEFLATED) as archive:
    for name, data in sorted(files.items()):
        item = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0))
        item.compress_type = zipfile.ZIP_DEFLATED
        item.external_attr = 0o644 << 16
        archive.writestr(item, data)
print(out)
