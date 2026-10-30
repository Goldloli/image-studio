#!/usr/bin/env python3
"""Check the embedded fixed-source inventory without executing/downloading artifacts."""
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import zipfile

root = Path(__file__).resolve().parent.parent
directory = root / 'resources/local-generation'
model = json.loads((directory / 'model-manifest.json').read_text())
runtime = json.loads((directory / 'runtime-manifest.json').read_text())
assert model['repository'] == 'Tongyi-MAI/Z-Image-Turbo'
assert model['revision'] == 'f332072aa78be7aecdf3ee76d5c247082da564a6'
components = [file for file in model['files'] if file['path'] != 'README.md']
assert len(components) == 17
assert sum(file['bytes'] for file in components) == model['component_bytes'] == 32_848_304_893
assert runtime['platform'] == 'macos-arm64' and runtime['python'] == '3.13.12'
assert runtime['mflux_source']['revision'] == '8a7fd0a486635f670ef90e02efe1c051ed3f133c'
assert len({wheel['package'] for wheel in runtime['wheels']}) == len(runtime['wheels'])
for file in [*model['files'], runtime['interpreter'], *runtime['wheels']]:
    assert not PurePosixPath(file['path']).is_absolute() and '..' not in PurePosixPath(file['path']).parts
    assert file['bytes'] > 0 and re.fullmatch('[0-9a-f]{64}', file['sha256'])
    assert file['url'].startswith('https://') or file['url'] == 'bundled:worker-wheel'
for wheel in runtime['wheels']:
    assert wheel['path'].endswith('.whl') and 'license' in wheel
    assert runtime['packages'][wheel['package']] == wheel['version']
lines = [line for line in (directory / 'requirements.lock').read_text().splitlines() if line and not line.startswith('#')]
assert lines == [f"{wheel['package']}=={wheel['version']} --hash=sha256:{wheel['sha256']}" for wheel in runtime['wheels']]
worker = next(wheel for wheel in runtime['wheels'] if wheel['package'] == 'studio-mflux-worker')
wheel_path = directory / worker['path']
assert wheel_path.stat().st_size == worker['bytes']
assert hashlib.sha256(wheel_path.read_bytes()).hexdigest() == worker['sha256']
with zipfile.ZipFile(wheel_path) as archive:
    for source in (root / 'tools/mflux-worker/studio_mflux_worker').glob('*.py'):
        assert source.read_bytes() == archive.read('studio_mflux_worker/' + source.name), source
print(json.dumps({'model_components': len(components), 'component_bytes': model['component_bytes'], 'runtime_wheels': len(runtime['wheels']), 'worker_source_matches_wheel': True, 'runtime_lock': runtime['id'], 'network_accessed': False}, indent=2))
