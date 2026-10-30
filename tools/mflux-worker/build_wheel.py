"""Reproducible stdlib-only build; never executed by the production installer."""
import base64
import csv
import hashlib
import io
from pathlib import Path
import zipfile

root = Path(__file__).resolve().parent
out = root.parents[1] / 'resources/local-generation/studio_mflux_worker-0.1.0-py3-none-any.whl'
files = {f'studio_mflux_worker/{p.name}': p.read_bytes() for p in sorted((root / 'studio_mflux_worker').glob('*.py'))}
info = 'studio_mflux_worker-0.1.0.dist-info/'
files[info + 'METADATA'] = b'Metadata-Version: 2.1\nName: studio-mflux-worker\nVersion: 0.1.0\nRequires-Python: ==3.13.*\nRequires-Dist: mflux==0.24.0\nLicense: Apache-2.0\n'
files[info + 'WHEEL'] = b'Wheel-Version: 1.0\nGenerator: image-studio-stdlib\nRoot-Is-Purelib: true\nTag: py3-none-any\n'
record = io.StringIO(newline='')
writer = csv.writer(record, lineterminator='\n')
for name, data in sorted(files.items()):
    writer.writerow((name, 'sha256=' + base64.urlsafe_b64encode(hashlib.sha256(data).digest()).decode().rstrip('='), len(data)))
writer.writerow((info + 'RECORD', '', ''))
files[info + 'RECORD'] = record.getvalue().encode()
with zipfile.ZipFile(out, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
    for name, data in sorted(files.items()):
        item = zipfile.ZipInfo(name, (2026, 1, 1, 0, 0, 0)); item.compress_type = zipfile.ZIP_DEFLATED
        item.external_attr = 0o644 << 16
        archive.writestr(item, data)
print(out)
