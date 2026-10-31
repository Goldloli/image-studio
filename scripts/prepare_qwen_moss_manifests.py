#!/usr/bin/env python3
"""Build executable Qwen/MOSS manifests from ModelScope file metadata.

Hugging Face revision, size, LFS SHA-256 and ordinary-file Git blob IDs remain
the frozen content identity. ModelScope supplies the default download source;
only ordinary files are downloaded to prove their Git blob identity.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
import sys
from typing import Any
from urllib.parse import quote, urlencode
import urllib.request


ROOT = Path(__file__).resolve().parents[1]
SOURCE = (
    ROOT
    / "openspec/changes/introduce-qwen21-and-moss-image"
    / "references/model-source-metadata.json"
)
MAX_ORDINARY_FILE_BYTES = 16 * 1024 * 1024
MODELS = (
    {
        "repository": "Qwen/Qwen-Image-2.1",
        "download_repository": "Qwen/Qwen-Image-2.1",
        "directory": "qwen21",
        "id": "qwen-image-2.1-d26bb61231c349cf6b7896fa83353113880e1ba3",
        "license": "qwen-research",
    },
    {
        "repository": "Qwen/Qwen-Image-2.1-Turbo",
        "download_repository": "Qwen/Qwen-Image-2.1-Turbo",
        "directory": "qwen21-turbo",
        "id": "qwen-image-2.1-turbo-fc13ba1986fcb905854dc8de2f52eee395f49b5b",
        "license": "qwen-research",
    },
    {
        "repository": "OpenMOSS-Team/MOSS-Image-SFT",
        "download_repository": "OpenMOSS/MOSS-Image-SFT",
        "directory": "moss-image-sft",
        "id": "moss-image-sft-f972bd4b4b9e3852133cc5da954d87bc32cd8fb9",
        "license": "Apache-2.0",
    },
)
WORKER_WHEEL = (
    ROOT
    / "resources/planned-models/qwen21"
    / "studio_mflux_worker_v3-0.3.0-py3-none-any.whl"
)
RUNTIME_ID = "mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3"


def sha256(value: bytes) -> str:
    return hashlib.sha256(value).hexdigest()


def git_blob_id(value: bytes) -> str:
    digest = hashlib.sha1()
    digest.update(f"blob {len(value)}\0".encode())
    digest.update(value)
    return digest.hexdigest()


def modelscope_api(repository: str, root: str = "") -> str:
    query = urlencode({"Revision": "master", "Root": root})
    return (
        f"https://www.modelscope.cn/api/v1/models/{repository}/repo/files?"
        f"{query}"
    )


def modelscope_url(repository: str, revision: str, path: str) -> str:
    return (
        f"https://www.modelscope.cn/models/{repository}/resolve/{revision}/"
        f"{quote(path, safe='/')}"
    )


def load_modelscope_tree(repository: str) -> dict[str, dict[str, Any]]:
    found: dict[str, dict[str, Any]] = {}
    pending = [""]
    while pending:
        root = pending.pop()
        request = urllib.request.Request(
            modelscope_api(repository, root),
            headers={"User-Agent": "ImageStudio/0.1-model-source-audit"},
        )
        with urllib.request.urlopen(request, timeout=30) as response:
            payload = json.load(response)
        if not payload.get("Success") or not isinstance(
            payload.get("Data", {}).get("Files"), list
        ):
            raise RuntimeError(f"ModelScope inventory failed for {repository}/{root}")
        for item in payload["Data"]["Files"]:
            path = item["Path"]
            if item["Type"] == "tree":
                pending.append(path)
            elif item["Type"] == "blob":
                if path in found:
                    raise RuntimeError(f"duplicate ModelScope file {repository}/{path}")
                found[path] = item
    return found


def fetch(url: str, expected_bytes: int) -> bytes:
    completed = subprocess.run(
        [
            "curl",
            "--fail",
            "--location",
            "--silent",
            "--show-error",
            "--max-time",
            "90",
            url,
        ],
        check=False,
        capture_output=True,
    )
    if completed.returncode != 0:
        detail = completed.stderr.decode("utf-8", "replace").strip()
        raise RuntimeError(f"could not fetch {url}: {detail}")
    data = completed.stdout
    if len(data) != expected_bytes:
        raise RuntimeError(
            f"{url} returned {len(data)} bytes, expected {expected_bytes}"
        )
    return data


def load_source() -> dict[str, Any]:
    return json.loads(SOURCE.read_text(encoding="utf-8"))


def build_manifest(model: dict[str, Any], source: dict[str, Any]) -> dict[str, Any]:
    repository = model["repository"]
    source_model = next(
        item for item in source["models"] if item["repository"] == repository
    )
    metadata = {item["path"]: item for item in source_model["files"]}
    selected = list(source_model["candidate_component_files"])
    if "LICENSE" in metadata:
        selected.append("LICENSE")

    revision = source_model["revision"]
    download_repository = model["download_repository"]
    download_files = load_modelscope_tree(download_repository)
    files = []
    documents: dict[str, bytes] = {}
    for path in selected:
        item = metadata.get(path)
        if item is None:
            raise RuntimeError(f"{repository} metadata is missing {path}")
        download = download_files.get(path)
        if download is None:
            raise RuntimeError(f"ModelScope {download_repository} is missing {path}")
        if download["Size"] != item["bytes"]:
            raise RuntimeError(
                f"{download_repository}/{path} size changed: "
                f"{download['Size']} != {item['bytes']}"
            )
        content_sha256 = download["Sha256"].lower()
        if len(content_sha256) != 64:
            raise RuntimeError(f"{download_repository}/{path} has no SHA-256")
        if item.get("sha256") is not None and content_sha256 != item["sha256"]:
            raise RuntimeError(f"{download_repository}/{path} SHA-256 changed")
        download_revision = download["Revision"]
        if len(download_revision) != 40:
            raise RuntimeError(
                f"{download_repository}/{path} has no pinned file revision"
            )
        url = modelscope_url(download_repository, download_revision, path)
        if item.get("sha256") is None:
            if item["bytes"] > MAX_ORDINARY_FILE_BYTES:
                raise RuntimeError(f"{repository}/{path} is unexpectedly non-LFS")
            data = fetch(url, item["bytes"])
            if sha256(data) != content_sha256:
                raise RuntimeError(f"{download_repository}/{path} SHA-256 changed")
            if git_blob_id(data) != item.get("git_blob_id"):
                raise RuntimeError(
                    f"{download_repository}/{path} Git blob identity changed"
                )
            if path.endswith(".json"):
                documents[path] = data
        print(f"verified {repository}/{path}: {item['bytes']} bytes", flush=True)
        files.append(
            {
                "path": path,
                "url": url,
                "bytes": item["bytes"],
                "sha256": content_sha256,
                "license": model["license"],
                "source_revision": download_revision,
            }
        )

    selected_paths = set(selected)
    for path, data in documents.items():
        if not path.endswith(".safetensors.index.json"):
            continue
        try:
            index = json.loads(data)
            shards = set(index["weight_map"].values())
        except (KeyError, TypeError, ValueError) as error:
            raise RuntimeError(f"invalid weight index {repository}/{path}: {error}") from error
        parent = Path(path).parent
        missing = [
            str(parent / shard) for shard in sorted(shards) if str(parent / shard) not in selected_paths
        ]
        if missing:
            raise RuntimeError(
                f"{repository}/{path} references missing manifest shards: {missing}"
            )

    component_bytes = sum(
        item["bytes"] for item in files if item["path"] != "README.md"
    )
    return {
        "schema_version": 1,
        "id": model["id"],
        "repository": repository,
        "revision": revision,
        "source_url": source_model["source_url"],
        "download_source": {
            "provider": "modelscope",
            "repository": download_repository,
            "url": f"https://www.modelscope.cn/models/{download_repository}",
            "revision_policy": "per-file",
        },
        "license": model["license"],
        "component_bytes": component_bytes,
        "files": files,
    }


def build_readme(model: dict[str, Any], manifest: dict[str, Any]) -> str:
    paths = "\n".join(f"- `{item['path']}`" for item in manifest["files"])
    return (
        f"# {model['repository']}\n\n"
        f"- Frozen Hugging Face revision: `{manifest['revision']}`\n"
        f"- Default download source: ModelScope `{model['download_repository']}`\n"
        f"- License: `{manifest['license']}`\n"
        f"- Component bytes: `{manifest['component_bytes']}`\n"
        "- Runtime: `mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3`\n\n"
        "The executable manifest fixes every ModelScope file revision and "
        "download URL. File size and SHA-256 must match the frozen Hugging Face "
        "snapshot; ordinary files additionally match its Git blob IDs.\n\n"
        "## Files\n\n"
        f"{paths}\n"
    )


def build_runtime() -> None:
    source_path = ROOT / "resources/planned-models/klein/runtime-manifest.json"
    runtime = json.loads(source_path.read_text(encoding="utf-8"))
    runtime["id"] = RUNTIME_ID
    runtime["worker_build_id"] = "studio-mflux-worker-0.3.0"
    runtime["worker_module"] = "studio_mflux_worker_v3"
    wheels = [
        item
        for item in runtime["wheels"]
        if item.get("package") != "studio-mflux-worker-v2"
    ]
    wheel_data = WORKER_WHEEL.read_bytes()
    wheel_sha256 = sha256(wheel_data)
    wheels.append(
        {
            "path": WORKER_WHEEL.name,
            "url": "bundled:worker-wheel",
            "bytes": len(wheel_data),
            "sha256": wheel_sha256,
            "package": "studio-mflux-worker-v3",
            "version": "0.3.0",
            "license": "Apache-2.0",
        }
    )
    runtime["wheels"] = sorted(wheels, key=lambda value: value["package"].lower())
    packages = {
        item["package"]: item["version"] for item in runtime["wheels"]
    }
    runtime["packages"] = packages

    directory = ROOT / "resources/planned-models/qwen21"
    (directory / "runtime-manifest.json").write_text(
        json.dumps(runtime, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    lock = "".join(
        f"{item['package']}=={item['version']} "
        f"--hash=sha256:{item['sha256']}\n"
        for item in runtime["wheels"]
    )
    (directory / "requirements.lock").write_text(lock, encoding="utf-8")
    print(
        f"runtime {RUNTIME_ID}: {len(wheels)} wheels, "
        f"worker sha256={wheel_sha256}"
    )


def main() -> None:
    arguments = sys.argv[1:]
    if arguments == ["--runtime-only"]:
        build_runtime()
        return
    if arguments not in ([], ["--check"]):
        raise SystemExit(
            "usage: prepare_qwen_moss_manifests.py [--runtime-only|--check]"
        )
    source = load_source()
    for model in MODELS:
        manifest = build_manifest(model, source)
        directory = ROOT / "resources/planned-models" / model["directory"]
        if arguments == ["--check"]:
            current = json.loads(
                (directory / "model-manifest.json").read_text(encoding="utf-8")
            )
            if current != manifest:
                raise SystemExit(
                    f"{model['repository']} manifest differs from ModelScope metadata"
                )
            print(
                f"checked {model['repository']}: {len(manifest['files'])} files, "
                f"{manifest['component_bytes']} bytes"
            )
            continue
        directory.mkdir(parents=True, exist_ok=True)
        (directory / "model-manifest.json").write_text(
            json.dumps(manifest, indent=2, ensure_ascii=False) + "\n",
            encoding="utf-8",
        )
        (directory / "README.md").write_text(
            build_readme(model, manifest), encoding="utf-8"
        )
        print(
            f"{model['repository']}: {len(manifest['files'])} files, "
            f"{manifest['component_bytes']} bytes"
        )
    build_runtime()


if __name__ == "__main__":
    main()
