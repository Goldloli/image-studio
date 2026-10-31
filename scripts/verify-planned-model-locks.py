#!/usr/bin/env python3
"""Verify the planned-model resource inventories without network or inference."""

import hashlib
import json
from pathlib import Path
import re
import zipfile


def verify_lock(directory, source_directory, worker_module, worker_package, worker_version):
    model = json.loads((directory / "model-manifest.json").read_text())
    runtime = json.loads((directory / "runtime-manifest.json").read_text())

    assert model["schema_version"] == 1
    assert sum(file["bytes"] for file in model["files"]) == model["component_bytes"]
    assert runtime["schema_version"] == 1
    worker = next(
        wheel for wheel in runtime["wheels"] if wheel["url"] == "bundled:worker-wheel"
    )
    assert worker["path"] == (
        f"{worker_module}-{worker_version}-py3-none-any.whl"
    )
    assert runtime["packages"][worker_package] == worker_version

    for file in [*model["files"], runtime["interpreter"], *runtime["wheels"]]:
        path = Path(file["path"])
        assert not path.is_absolute() and ".." not in path.parts
        assert file["bytes"] > 0
        assert re.fullmatch("[0-9a-f]{64}", file["sha256"])
        assert file["url"].startswith("https://") or file["url"] == "bundled:worker-wheel"

    lines = [
        line
        for line in (directory / "requirements.lock").read_text().splitlines()
        if line and not line.startswith("#")
    ]
    assert lines == [
        f"{wheel['package']}=={wheel['version']} --hash=sha256:{wheel['sha256']}"
        for wheel in runtime["wheels"]
    ]

    wheel_path = directory / worker["path"]
    assert wheel_path.stat().st_size == worker["bytes"]
    assert hashlib.sha256(wheel_path.read_bytes()).hexdigest() == worker["sha256"]
    with zipfile.ZipFile(wheel_path) as archive:
        for source in (root / "tools" / source_directory / worker_module).glob("*.py"):
            assert (
                source.read_bytes()
                == archive.read(f"{worker_module}/{source.name}")
            )
    return model, runtime, worker


root = Path(__file__).resolve().parent.parent
klein = root / "resources/planned-models/klein"
klein_model, klein_runtime, klein_worker = verify_lock(
    klein,
    "mflux-worker-v2",
    "studio_mflux_worker_v2",
    "studio-mflux-worker-v2",
    "0.2.0",
)
assert klein_model["repository"] == "black-forest-labs/FLUX.2-klein-4B"
assert klein_model["revision"] == "e7b7dc27f91deacad38e78976d1f2b499d76a294"
assert len(klein_model["files"]) == 18
assert klein_model["component_bytes"] == 15_980_131_745
assert klein_runtime["id"] == "mflux-0.24.0-py3.13.12-macos-arm64-recipes-v2"
assert (
    klein_runtime["mflux_source"]["revision"]
    == "8a7fd0a486635f670ef90e02efe1c051ed3f133c"
)

birefnet = root / "resources/planned-models/birefnet"
birefnet_model, birefnet_runtime, birefnet_worker = verify_lock(
    birefnet,
    "onnx-worker",
    "studio_onnx_worker",
    "studio-onnx-worker",
    "0.1.0",
)
assert birefnet_model["repository"] == "ZhengPeng7/BiRefNet"
assert len(birefnet_model["files"]) == 1
assert birefnet_model["component_bytes"] == 972_666_916
assert (
    birefnet_model["files"][0]["sha256"]
    == "58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c"
)
assert (
    birefnet_runtime["id"]
    == "onnxruntime-1.31.1-py3.13.12-macos-arm64-cpu-v1"
)

print(
    json.dumps(
        {
            "locks": [
                {
                    "model_components": len(klein_model["files"]),
                    "component_bytes": klein_model["component_bytes"],
                    "runtime_lock_id": klein_runtime["id"],
                    "worker_sha256": klein_worker["sha256"],
                },
                {
                    "model_components": len(birefnet_model["files"]),
                    "component_bytes": birefnet_model["component_bytes"],
                    "runtime_lock_id": birefnet_runtime["id"],
                    "worker_sha256": birefnet_worker["sha256"],
                },
            ],
            "worker_sources_match_wheels": True,
            "network_accessed": False,
        },
        indent=2,
    )
)
