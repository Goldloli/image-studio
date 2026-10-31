#!/usr/bin/env python3
"""Verify the planned-model resource inventories without network or inference."""

import hashlib
import json
from pathlib import Path
import re
import zipfile


root = Path(__file__).resolve().parent.parent
klein = root / "resources/planned-models/klein"
model = json.loads((klein / "model-manifest.json").read_text())
runtime = json.loads((klein / "runtime-manifest.json").read_text())

assert model["schema_version"] == 1
assert model["repository"] == "black-forest-labs/FLUX.2-klein-4B"
assert model["revision"] == "e7b7dc27f91deacad38e78976d1f2b499d76a294"
assert len(model["files"]) == 18
assert sum(file["bytes"] for file in model["files"]) == model["component_bytes"]
assert model["component_bytes"] == 15_980_131_745

assert runtime["schema_version"] == 1
assert runtime["id"] == "mflux-0.24.0-py3.13.12-macos-arm64-recipes-v2"
assert runtime["mflux_source"]["revision"] == "8a7fd0a486635f670ef90e02efe1c051ed3f133c"
worker = next(wheel for wheel in runtime["wheels"] if wheel["url"] == "bundled:worker-wheel")
assert worker["path"] == "studio_mflux_worker_v2-0.2.0-py3-none-any.whl"
assert runtime["packages"]["studio-mflux-worker-v2"] == "0.2.0"

for file in [*model["files"], runtime["interpreter"], *runtime["wheels"]]:
    path = Path(file["path"])
    assert not path.is_absolute() and ".." not in path.parts
    assert file["bytes"] > 0
    assert re.fullmatch("[0-9a-f]{64}", file["sha256"])
    assert file["url"].startswith("https://") or file["url"] == "bundled:worker-wheel"

lines = [
    line
    for line in (klein / "requirements.lock").read_text().splitlines()
    if line and not line.startswith("#")
]
assert lines == [
    f"{wheel['package']}=={wheel['version']} --hash=sha256:{wheel['sha256']}"
    for wheel in runtime["wheels"]
]

wheel_path = klein / worker["path"]
assert wheel_path.stat().st_size == worker["bytes"]
assert hashlib.sha256(wheel_path.read_bytes()).hexdigest() == worker["sha256"]
with zipfile.ZipFile(wheel_path) as archive:
    for source in (
        root / "tools/mflux-worker-v2/studio_mflux_worker_v2"
    ).glob("*.py"):
        assert (
            source.read_bytes()
            == archive.read(f"studio_mflux_worker_v2/{source.name}")
        )

print(
    json.dumps(
        {
            "model_components": len(model["files"]),
            "component_bytes": model["component_bytes"],
            "runtime_lock_id": runtime["id"],
            "worker_sha256": worker["sha256"],
            "worker_source_matches_wheel": True,
            "network_accessed": False,
        },
        indent=2,
    )
)
