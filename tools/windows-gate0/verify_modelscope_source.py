"""Prove that ModelScope transfers the exact fixed Gate 0 model closure."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import urllib.parse
import urllib.request
from pathlib import Path


USER_AGENT = "ImageStudio-Gate0/1"


def get(url: str) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=90) as response:
        return response.read()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--manifest", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    source = manifest["download_source"]
    repository = source["repository"]
    revision = source["revision"]
    api_url = f"https://www.modelscope.cn/api/v1/models/{repository}"
    api = json.loads(get(api_url))
    if not api.get("Success"):
        raise RuntimeError(f"ModelScope API failed: {api.get('Message')}")
    model = api["Data"]
    if model["Name"] != repository.rsplit("/", 1)[-1]:
        raise RuntimeError("ModelScope repository identity mismatch")
    if model["Path"] + "/" + model["Name"] != repository:
        raise RuntimeError("ModelScope namespace identity mismatch")
    if model["Revision"] != revision:
        raise RuntimeError("ModelScope revision identity mismatch")

    safetensor_files = {
        file["name"]: file for file in model["ModelInfos"]["safetensor"]["files"]
    }
    verified = []
    for artifact in manifest["files"]:
        path = artifact["path"]
        if path in safetensor_files:
            remote = safetensor_files[path]
            actual_size = int(remote["size"])
            actual_sha = remote["sha256"]
            method = "modelscope_api_sha256"
        else:
            encoded_path = urllib.parse.quote(path, safe="/")
            url = (
                f"https://www.modelscope.cn/models/{repository}/resolve/"
                f"{revision}/{encoded_path}"
            )
            content = get(url)
            actual_size = len(content)
            actual_sha = hashlib.sha256(content).hexdigest()
            method = "downloaded_and_hashed"
        if actual_size != artifact["bytes"] or actual_sha != artifact["sha256"]:
            raise RuntimeError(f"ModelScope content mismatch for {path}")
        verified.append(
            {
                "path": path,
                "bytes": actual_size,
                "sha256": actual_sha,
                "verification": method,
            }
        )

    report = {
        "schema_version": 1,
        "kind": "windows-gate0-modelscope-equivalence",
        "status": "passed",
        "provider": source["provider"],
        "repository": repository,
        "source_revision": revision,
        "upstream_huggingface_revision": manifest["revision"],
        "verified_file_count": len(verified),
        "component_bytes": manifest["component_bytes"],
        "files": verified,
        "limitations": [
            "The mutable master alias is not the model identity; every byte and SHA-256 is pinned by the existing manifest.",
            "Seven large safetensors files use ModelScope API hashes; all remaining files were downloaded and hashed.",
        ],
    }
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(output.suffix + ".pending")
    temporary.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, output)
    print(
        json.dumps(
            {
                "status": "passed",
                "verified_files": len(verified),
                "output": str(output),
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
