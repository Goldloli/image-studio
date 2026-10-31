#!/usr/bin/env python3
"""Build executable Qwen/MOSS manifests from pinned Hugging Face metadata.

Only ordinary files are downloaded here. LFS object hashes come from the pinned
Hugging Face API metadata; no model weights are downloaded.
"""

from __future__ import annotations

import hashlib
import json
from pathlib import Path
import subprocess
from typing import Any
from urllib.parse import quote


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
        "directory": "qwen21",
        "id": "qwen-image-2.1-d26bb61231c349cf6b7896fa83353113880e1ba3",
        "license": "qwen-research",
    },
    {
        "repository": "Qwen/Qwen-Image-2.1-Turbo",
        "directory": "qwen21-turbo",
        "id": "qwen-image-2.1-turbo-fc13ba1986fcb905854dc8de2f52eee395f49b5b",
        "license": "qwen-research",
    },
    {
        "repository": "OpenMOSS-Team/MOSS-Image-SFT",
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
    files = []
    for path in selected:
        item = metadata.get(path)
        if item is None:
            raise RuntimeError(f"{repository} metadata is missing {path}")
        url = (
            f"https://huggingface.co/{repository}/resolve/{revision}/"
            f"{quote(path, safe='/')}"
        )
        content_sha256 = item.get("sha256")
        if content_sha256 is None:
            if item["bytes"] > MAX_ORDINARY_FILE_BYTES:
                raise RuntimeError(
                    f"{repository}/{path} has no LFS SHA-256 and is too large to fetch"
                )
            data = fetch(url, item["bytes"])
            content_sha256 = sha256(data)
        print(f"verified {repository}/{path}: {item['bytes']} bytes", flush=True)
        files.append(
            {
                "path": path,
                "url": url,
                "bytes": item["bytes"],
                "sha256": content_sha256,
                "license": model["license"],
            }
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
        "license": model["license"],
        "component_bytes": component_bytes,
        "files": files,
    }


def build_readme(model: dict[str, Any], manifest: dict[str, Any]) -> str:
    paths = "\n".join(f"- `{item['path']}`" for item in manifest["files"])
    return (
        f"# {model['repository']}\n\n"
        f"- Revision: `{manifest['revision']}`\n"
        f"- License: `{manifest['license']}`\n"
        f"- Component bytes: `{manifest['component_bytes']}`\n"
        "- Runtime: `mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3`\n\n"
        "The executable manifest fixes every download URL, byte count, and "
        "content SHA-256. Ordinary-file hashes were derived from the pinned "
        "files; LFS hashes come from the Hugging Face blob metadata.\n\n"
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
    runtime["wheels"] = wheels
    packages = {
        item["package"]: item["version"]
        for item in wheels
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
        for item in sorted(wheels, key=lambda value: value["package"].lower())
    )
    (directory / "requirements.lock").write_text(lock, encoding="utf-8")
    print(
        f"runtime {RUNTIME_ID}: {len(wheels)} wheels, "
        f"worker sha256={wheel_sha256}"
    )


def main() -> None:
    source = load_source()
    for model in MODELS:
        manifest = build_manifest(model, source)
        directory = ROOT / "resources/planned-models" / model["directory"]
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
