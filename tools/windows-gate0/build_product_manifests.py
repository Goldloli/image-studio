"""Build installer manifests without rewriting the Gate 0 evidence manifests."""

import hashlib
import json
import os
import urllib.parse
from pathlib import Path

root = Path(__file__).resolve().parents[2]
runtime_dir = root / "resources/windows-foundation/python-runtime"
model_dir = root / "resources/windows-foundation/z-image-diffusers"


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write(path, value):
    temporary = path.with_suffix(path.suffix + ".pending")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, path)


gate_runtime = read(runtime_dir / "runtime-manifest.json")
worker = model_dir / "studio_zimage_cuda_worker-0.1.0-py3-none-any.whl"
worker_record = {
    "path": worker.name,
    "url": "bundled:worker-wheel",
    "bytes": worker.stat().st_size,
    "sha256": digest(worker),
    "package": "studio-zimage-cuda-worker",
    "version": "0.1.0",
}
wheels = [*gate_runtime["wheels"], worker_record]
packages = {
    record["package"]: record["version"]
    for record in wheels
    if record["package"]
    in {
        "torch",
        "diffusers",
        "transformers",
        "accelerate",
        "bitsandbytes",
        "numpy",
        "safetensors",
        "tokenizers",
        "studio-zimage-cuda-worker",
    }
}
runtime = {
    "schema_version": 1,
    "id": "windows-python31210-torch2100-cu126-bnb0482-v1",
    "kind": "python-embeddable-windows",
    "platform": "windows-amd64",
    "python": "3.12.10",
    "interpreter": gate_runtime["interpreter"],
    "wheels": wheels,
    "packages": packages,
    "requirements_lock": gate_runtime["requirements_lock"],
    "gate0_manifest_sha256": digest(runtime_dir / "runtime-manifest.json"),
    "install_layout": {
        "interpreter_root": "python.exe",
        "site_packages": "Lib/site-packages",
        "path_file": "python312._pth",
    },
}
write(runtime_dir / "product-runtime-manifest.json", runtime)

gate_model = read(model_dir / "model-manifest.json")
source = gate_model["download_source"]
template = source["url_template"]
files = []
for artifact in gate_model["files"]:
    files.append(
        {
            **artifact,
            "url": template.format(path=urllib.parse.quote(artifact["path"], safe="/")),
            "upstream_url": artifact["url"],
        }
    )
model = {
    **gate_model,
    "files": files,
    "gate0_manifest_sha256": digest(model_dir / "model-manifest.json"),
}
write(model_dir / "product-model-manifest.json", model)

print(
    json.dumps(
        {
            "runtime": str(runtime_dir / "product-runtime-manifest.json"),
            "runtime_artifacts": 1 + len(wheels),
            "model": str(model_dir / "product-model-manifest.json"),
            "model_files": len(files),
        }
    )
)
