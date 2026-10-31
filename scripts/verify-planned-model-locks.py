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


def verify_native_lock(directory):
    model = json.loads((directory / "model-manifest.json").read_text())
    runtime = json.loads((directory / "runtime-manifest.json").read_text())
    assert model["schema_version"] == 1
    assert model["files"] == []
    assert (
        sum(member["bytes"] for archive in model["archives"] for member in archive["members"])
        == model["component_bytes"]
    )
    assert runtime["schema_version"] == 1
    assert runtime["kind"] == "native"
    assert runtime["platform"] == "macos-universal2"
    for archive in [*model["archives"], *runtime["archives"]]:
        path = Path(archive["path"])
        assert not path.is_absolute() and ".." not in path.parts
        assert archive["bytes"] > 0
        assert re.fullmatch("[0-9a-f]{64}", archive["sha256"])
        assert archive["url"].startswith("https://")
        assert archive["members"]
        for member in archive["members"]:
            assert all(
                not Path(member[key]).is_absolute()
                and ".." not in Path(member[key]).parts
                for key in ("archive_path", "path")
            )
            assert member["bytes"] > 0
            assert re.fullmatch("[0-9a-f]{64}", member["sha256"])
    executable = runtime["executable"]
    assert any(
        member["path"] == executable and member["executable"]
        for member in runtime["archives"][0]["members"]
    )
    return model, runtime


def verify_text_models(shared_directory, model_directories):
    runtime = json.loads(
        (shared_directory / "runtime-manifest.json").read_text()
    )
    assert runtime["schema_version"] == 1
    assert runtime["id"] == "mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3"
    assert (
        runtime["mflux_source"]["revision"]
        == "8a7fd0a486635f670ef90e02efe1c051ed3f133c"
    )
    worker = next(
        wheel for wheel in runtime["wheels"] if wheel["url"] == "bundled:worker-wheel"
    )
    assert worker["path"] == "studio_mflux_worker_v3-0.3.0-py3-none-any.whl"
    assert runtime["packages"]["studio-mflux-worker-v3"] == "0.3.0"
    for file in [runtime["interpreter"], *runtime["wheels"]]:
        path = Path(file["path"])
        assert not path.is_absolute() and ".." not in path.parts
        assert file["bytes"] > 0
        assert re.fullmatch("[0-9a-f]{64}", file["sha256"])
        assert file["url"].startswith("https://") or file["url"] == "bundled:worker-wheel"
    lines = [
        line
        for line in (shared_directory / "requirements.lock")
        .read_text()
        .splitlines()
        if line and not line.startswith("#")
    ]
    assert lines == [
        f"{wheel['package']}=={wheel['version']} --hash=sha256:{wheel['sha256']}"
        for wheel in runtime["wheels"]
    ]
    wheel_path = shared_directory / worker["path"]
    assert wheel_path.stat().st_size == worker["bytes"]
    assert hashlib.sha256(wheel_path.read_bytes()).hexdigest() == worker["sha256"]
    with zipfile.ZipFile(wheel_path) as archive:
        for source in (root / "tools/mflux-worker-v3/studio_mflux_worker_v3").glob(
            "*.py"
        ):
            assert (
                source.read_bytes()
                == archive.read(f"studio_mflux_worker_v3/{source.name}")
            )

    expected = {
        "qwen21": (
            "Qwen/Qwen-Image-2.1",
            "Qwen/Qwen-Image-2.1",
            "d26bb61231c349cf6b7896fa83353113880e1ba3",
            33_131_609_424,
            25,
        ),
        "qwen21-turbo": (
            "Qwen/Qwen-Image-2.1-Turbo",
            "Qwen/Qwen-Image-2.1-Turbo",
            "fc13ba1986fcb905854dc8de2f52eee395f49b5b",
            32_451_607_098,
            16,
        ),
        "moss-image-sft": (
            "OpenMOSS-Team/MOSS-Image-SFT",
            "OpenMOSS/MOSS-Image-SFT",
            "f972bd4b4b9e3852133cc5da954d87bc32cd8fb9",
            27_219_783_878,
            19,
        ),
    }
    source_metadata = json.loads(
        (
            root
            / "openspec/changes/introduce-qwen21-and-moss-image"
            / "references/model-source-metadata.json"
        ).read_text()
    )
    source_by_repository = {
        item["repository"]: item for item in source_metadata["models"]
    }
    verified = []
    for directory in model_directories:
        model = json.loads((directory / "model-manifest.json").read_text())
        repository, download_repository, revision, component_bytes, file_count = expected[
            directory.name
        ]
        source_model = source_by_repository[repository]
        source_files = {item["path"]: item for item in source_model["files"]}
        assert model["schema_version"] == 1
        assert model["repository"] == repository
        assert model["revision"] == revision
        assert model["download_source"]["provider"] == "modelscope"
        assert model["download_source"]["repository"] == download_repository
        assert model["download_source"]["revision_policy"] == "per-file"
        assert source_model["revision"] == revision
        assert model["component_bytes"] == component_bytes
        assert len(model["files"]) == file_count
        assert sum(file["bytes"] for file in model["files"]) == component_bytes
        for file in model["files"]:
            path = Path(file["path"])
            assert not path.is_absolute() and ".." not in path.parts
            assert file["bytes"] > 0
            assert re.fullmatch("[0-9a-f]{64}", file["sha256"])
            source_revision = file["source_revision"]
            assert re.fullmatch("[0-9a-f]{40}", source_revision)
            assert file["url"] == (
                f"https://www.modelscope.cn/models/{download_repository}/resolve/"
                f"{source_revision}/{file['path']}"
            )
            assert "/master/" not in file["url"]
            assert file["license"]
            source_file = source_files[file["path"]]
            assert file["bytes"] == source_file["bytes"]
            if source_file["sha256"] is not None:
                assert file["sha256"] == source_file["sha256"]
        selected = set(source_model["candidate_component_files"])
        selected.add("LICENSE")
        assert {file["path"] for file in model["files"]} == selected
        verified.append(
            {
                "repository": repository,
                "model_components": len(model["files"]),
                "component_bytes": model["component_bytes"],
                "runtime_lock_id": runtime["id"],
                "worker_sha256": worker["sha256"],
            }
        )
    return runtime, worker, verified


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

qwen_runtime, qwen_worker, text_locks = verify_text_models(
    root / "resources/planned-models/qwen21",
    [
        root / "resources/planned-models/qwen21",
        root / "resources/planned-models/qwen21-turbo",
        root / "resources/planned-models/moss-image-sft",
    ],
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

realesrgan = root / "resources/planned-models/realesrgan"
realesrgan_model, realesrgan_runtime = verify_native_lock(realesrgan)
assert realesrgan_model["id"] == "realesrgan-x4plus-ncnn"
assert realesrgan_model["component_bytes"] == 33_540_549
assert realesrgan_runtime["id"] == "realesrgan-x4plus-ncnn-macos-v0.2.0-v1"
assert (
    realesrgan_runtime["archives"][0]["sha256"]
    == "f1cc31f13398126ddec25a186b9e50c7ab3e4f2bc011d0222331b43ce2037d3b"
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
                {
                    "model_components": sum(
                        len(archive["members"]) for archive in realesrgan_model["archives"]
                    ),
                    "component_bytes": realesrgan_model["component_bytes"],
                    "runtime_lock_id": realesrgan_runtime["id"],
                    "worker_build_id": "studio-realesrgan-worker-0.1.0",
                    "archive_count": len(
                        [*realesrgan_model["archives"], *realesrgan_runtime["archives"]]
                    ),
                },
                *text_locks,
            ],
            "worker_sources_match_wheels": True,
            "network_accessed": False,
        },
        indent=2,
    )
)
