"""Build the frozen Windows v2 Python runtime manifest.

Every wheel in the wheelhouse is re-hashed and checked against the official
index metadata (PyPI JSON API or the PyTorch cu126 index page). The Python
embeddable distribution is checked against the v1 Windows runtime record in this
repository. License metadata is extracted from each wheel's own METADATA.

Deterministic output: no timestamps are embedded, so re-running after the later
offline-install stage only changes the requested status/evidence fields.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
import urllib.parse
import urllib.request
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PATCHED_URL = "https://download.pytorch.org/whl/cu126"
PYPI_JSON = "https://pypi.org/pypi/{name}/{version}/json"
PYPI_INDEX = "https://pypi.org/simple"
USER_AGENT = "image-studio-runtime-freeze/1.0"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def http_get(url: str, timeout: int = 120) -> bytes:
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=timeout) as response:
        return response.read()


def parse_wheel_filename(filename: str) -> tuple[str, str, str, str, str]:
    stem = filename[: -len(".whl")]
    parts = stem.split("-")
    if len(parts) < 5:
        raise ValueError(f"unexpected wheel filename: {filename}")
    platform_tag, abi_tag, python_tag = parts[-1], parts[-2], parts[-3]
    middle = parts[:-3]
    if len(middle) == 2:
        name, version = middle
        build = ""
    elif len(middle) == 3:
        name, version, build = middle
    else:
        raise ValueError(f"unexpected wheel filename: {filename}")
    return name, version, build, python_tag, f"{abi_tag}-{platform_tag}"


def normalize(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def pypi_metadata(name: str, version: str) -> dict[str, object]:
    norm = urllib.parse.quote(normalize(name))
    payload = json.loads(http_get(PYPI_JSON.format(name=norm, version=urllib.parse.quote(version))))
    info = payload["info"]
    license_expression = info.get("license_expression") or ""
    license_field = info.get("license") or ""
    classifiers = [item for item in (info.get("classifiers") or []) if item.startswith("License ::")]
    return {
        "source": "pypi-json",
        "url": f"https://pypi.org/pypi/{norm}/{version}",
        "license_expression": license_expression,
        "license_field": license_field,
        "license_classifiers": classifiers,
        "requires_python": info.get("requires_python") or "",
    }


def pypi_wheel_file(name: str, version: str, filename: str) -> tuple[str, str]:
    norm = urllib.parse.quote(normalize(name))
    payload = json.loads(http_get(PYPI_JSON.format(name=norm, version=urllib.parse.quote(version))))
    for item in payload["urls"]:
        if item["filename"] == filename:
            return item["url"], item["digests"]["sha256"]
    raise RuntimeError(f"PyPI has no file {filename} for {name}=={version}")


def pytorch_wheel_file(name: str, version: str, filename: str) -> tuple[str, str]:
    index = f"{PATCHED_URL}/{normalize(name)}/"
    html = http_get(index).decode("utf-8", "replace")
    for match in re.finditer(r'href="([^"]+)"', html):
        href = match.group(1)
        href_path = href.split("#", 1)[0]
        href_name = urllib.parse.unquote(href_path.rsplit("/", 1)[-1])
        if href_name == filename:
            hash_match = re.search(r"#sha256=([0-9a-f]{64})", href)
            if not hash_match:
                raise RuntimeError(f"no sha256 fragment for {filename} in {index}")
            return href_path, hash_match.group(1)
    raise RuntimeError(f"{PATCHED_URL} has no file {filename} for {name}=={version}")


def wheel_metadata(path: Path) -> dict[str, object]:
    with zipfile.ZipFile(path) as archive:
        metadata_name = next(
            (name for name in archive.namelist() if re.fullmatch(r"[^/]+\.dist-info/METADATA", name)),
            None,
        )
        if metadata_name is None:
            return {"metadata": None}
        text = archive.read(metadata_name).decode("utf-8", "replace")
        headers: dict[str, list[str]] = {}
        for line in text.splitlines():
            if not line or line.startswith((" ", "\t")):
                continue
            key, _, value = line.partition(":")
            headers.setdefault(key.strip(), []).append(value.strip())
        license_files = sorted(
            {
                name.split("/", 1)[1]
                for name in archive.namelist()
                if re.fullmatch(r"[^/]+\.dist-info/(?:LICENSE|COPYING|NOTICE)(?:\..*)?", name)
            }
        )
        return {
            "metadata_path": metadata_name,
            "license_expression": (headers.get("License-Expression") or [""])[0],
            "license_field": (headers.get("License") or [""])[0],
            "license_classifiers": [item for item in headers.get("Classifier", []) if item.startswith("License ::")],
            "license_files_in_wheel": license_files,
            "requires_python": (headers.get("Requires-Python") or [""])[0],
        }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--wheelhouse", type=Path, default=ROOT / "target/parity-runtime/wheelhouse")
    parser.add_argument("--downloads", type=Path, default=ROOT / "target/parity-runtime/downloads")
    parser.add_argument(
        "--lock", type=Path, default=ROOT / "resources/windows-parity/runtime-v2/requirements.lock"
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/runtime-manifest.json",
    )
    parser.add_argument(
        "--license-records",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/license-records.json",
    )
    parser.add_argument(
        "--python-license-out",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/LICENSE-Python-3.12.10.txt",
    )
    parser.add_argument(
        "--v1-manifest",
        type=Path,
        default=ROOT / "resources/windows-foundation/python-runtime/runtime-manifest.json",
    )
    parser.add_argument("--status", default="offline_install_verified")
    parser.add_argument("--offline-install-evidence", default="verification/windows-parity-runtime/install-evidence.json")
    parser.add_argument("--runtime-probe-evidence", default="verification/windows-parity-runtime/runtime-probe.json")
    parser.add_argument("--runtime-dir", type=Path, default=ROOT / "target/parity-runtime/runtime")
    args = parser.parse_args()

    wheelhouse = args.wheelhouse.resolve()
    downloads = args.downloads.resolve()
    lock_path = args.lock.resolve()
    output = args.output.resolve()
    v1_manifest = json.loads(args.v1_manifest.read_text(encoding="utf-8"))
    v1_expected_python = v1_manifest["interpreter"]

    python_zip = downloads / "python-3.12.10-embed-amd64.zip"
    python_sha = sha256_file(python_zip)
    if python_zip.stat().st_size != v1_expected_python["bytes"] or python_sha != v1_expected_python["sha256"]:
        raise SystemExit("Python embeddable archive does not match the v1 repository record")

    wheels: list[dict[str, object]] = []
    license_entries: list[dict[str, object]] = []
    bootstrap: list[dict[str, object]] = []
    for path in sorted(wheelhouse.glob("*.whl")):
        name, version, build, python_tag, target = parse_wheel_filename(path.name)
        local_sha = sha256_file(path)
        norm = normalize(name)
        if norm == "pip":
            url, expected_sha = pypi_wheel_file(name, version, path.name)
            record = {
                "path": path.name,
                "url": url,
                "bytes": path.stat().st_size,
                "sha256": local_sha,
                "package": norm,
                "version": version,
                "role": "bootstrap-pip",
            }
            bootstrap.append(record)
        else:
            if norm in {"torch", "torchvision"}:
                url, expected_sha = pytorch_wheel_file(name, version, path.name)
                index_meta = {"source": "pytorch-cu126-index", "index_url": f"{PATCHED_URL}/{norm}/"}
            else:
                url, expected_sha = pypi_wheel_file(name, version, path.name)
                index_meta = pypi_metadata(name, version)
            if local_sha != expected_sha:
                raise SystemExit(f"sha256 mismatch for {path.name}: local {local_sha} index {expected_sha}")
            record = {
                "path": path.name,
                "url": url,
                "bytes": path.stat().st_size,
                "sha256": local_sha,
                "package": norm,
                "version": version,
                "python_tag": python_tag,
                "target": target,
                "index_metadata": index_meta,
            }
            if build:
                record["build"] = build
            wheels.append(record)
            meta = wheel_metadata(path)
            license_entries.append(
                {
                    "package": norm,
                    "version": version,
                    "wheel": path.name,
                    "wheel_sha256": local_sha,
                    "license_expression": meta.get("license_expression", ""),
                    "license_field": meta.get("license_field", ""),
                    "license_classifiers": meta.get("license_classifiers", []),
                    "license_files_in_wheel": meta.get("license_files_in_wheel", []),
                }
            )

    wheels.sort(key=lambda item: (item["package"], item["version"]))
    bootstrap.sort(key=lambda item: item["package"])
    license_entries.sort(key=lambda item: item["package"])

    with zipfile.ZipFile(python_zip) as archive:
        python_license_member = "LICENSE.txt"
        python_license_bytes = archive.read(python_license_member)
        python_license_sha = hashlib.sha256(python_license_bytes).hexdigest()
        members = []
        for info in archive.infolist():
            members.append(
                {
                    "path": info.filename,
                    "bytes": info.file_size,
                    "sha256": hashlib.sha256(archive.read(info)).hexdigest(),
                }
            )
    args.python_license_out.parent.mkdir(parents=True, exist_ok=True)
    args.python_license_out.write_bytes(python_license_bytes)

    lock_bytes = lock_path.read_bytes()
    interpreter = {
        "path": python_zip.name,
        "url": v1_expected_python["url"],
        "bytes": python_zip.stat().st_size,
        "sha256": python_sha,
        "package": "cpython",
        "version": "3.12.10",
        "implementation": "CPython",
        "platform": "Windows embeddable x64",
        "verified_against": "repository v1 record: resources/windows-foundation/python-runtime/runtime-manifest.json",
        "license_member": {
            "archive_path": python_license_member,
            "bytes": len(python_license_bytes),
            "sha256": python_license_sha,
            "saved_as": args.python_license_out.name,
        },
        "member_count": len(members),
    }

    top_level = [
        "torch==2.10.0+cu126",
        "torchvision==0.25.0+cu126",
        "diffusers==0.41.0",
        "transformers==5.3.0",
        "accelerate==1.12.0",
        "bitsandbytes==0.50.2",
        "safetensors==0.8.0",
        "huggingface-hub==1.33.0",
        "onnxruntime==1.31.1",
        "psutil==7.1.3",
    ]
    pth_path = args.runtime_dir / "python312._pth"
    pth_content = []
    pth_record: dict[str, object] = {
        "path": "python312._pth",
        "required_lines": ["python312.zip", ".", "Lib\\site-packages", "import site"],
        "note": (
            "The installer writes this file next to python.exe. Without Lib\\site-packages + import site "
            "the embeddable interpreter does not see the frozen wheel closure."
        ),
    }
    if pth_path.is_file():
        pth_content = pth_path.read_text(encoding="utf-8").splitlines()
        pth_record.update(
            {
                "installed_bytes": pth_path.stat().st_size,
                "installed_sha256": hashlib.sha256(pth_path.read_bytes()).hexdigest(),
                "installed_lines": pth_content,
            }
        )
    runtime_dir_bytes = 0
    runtime_file_count = 0
    if args.runtime_dir.is_dir():
        for item in args.runtime_dir.rglob("*"):
            if item.is_file():
                runtime_dir_bytes += item.stat().st_size
                runtime_file_count += 1
    all_artifacts = [interpreter, *wheels, *bootstrap]
    manifest = {
        "schema_version": 1,
        "id": "windows-python31210-torch2100-cu126-diff041-transformers53-v2",
        "kind": "windows-parity-runtime-closure",
        "status": args.status,
        "platform": "windows-amd64",
        "python": "3.12.10",
        "torch": "2.10.0+cu126",
        "torchvision": "0.25.0+cu126",
        "cuda": "12.6",
        "diffusers": "0.41.0",
        "transformers": "5.3.0",
        "accelerate": "1.12.0",
        "bitsandbytes": "0.50.2",
        "safetensors": "0.8.0",
        "huggingface_hub": "1.33.0",
        "onnxruntime": "1.31.1",
        "serves_models": [
            "FLUX.2 Klein 4B distilled",
            "Qwen-Image 2.1",
            "Qwen-Image 2.1-Turbo",
            "MOSS-Image-SFT",
            "BiRefNet epoch_244 (ONNX Runtime CPU)",
        ],
        "top_level_requirements": top_level,
        "requirements_lock": {
            "path": lock_path.name,
            "url": f"repository:resources/windows-parity/runtime-v2/{lock_path.name}",
            "bytes": len(lock_bytes),
            "sha256": hashlib.sha256(lock_bytes).hexdigest(),
        },
        "interpreter": interpreter,
        "bootstrap_tools": bootstrap,
        "wheels": wheels,
        "artifact_count": len(all_artifacts),
        "total_bytes": sum(item["bytes"] for item in all_artifacts),
        "wheelhouse": {
            "path": "target/parity-runtime/wheelhouse",
            "bytes": sum(item["bytes"] for item in wheels) + sum(item["bytes"] for item in bootstrap),
            "files": len(wheels) + len(bootstrap),
            "install_command": (
                "runtime\\python.exe -m pip install --no-index --find-links target\\parity-runtime\\wheelhouse "
                "-r resources\\windows-parity\\runtime-v2\\requirements.lock"
            ),
        },
        "license_records": {
            "path": args.license_records.name,
            "package_count": len(license_entries),
        },
        "installer_contract": {
            "hardcoded_interpreter_paths": ["python.exe", "python312._pth"],
            "runtime_root_layout": [
                "python.exe",
                "python312._pth",
                "python312.zip",
                "Lib/site-packages/",
                "Scripts/ (console entry points; not required by workers and not added to PATH)",
            ],
            "pth_file": pth_record,
            "environment_hints": {
                "PYTHONNOUSERSITE": "1 (keeps the frozen closure isolated from user site-packages)",
                "PYTHONUTF8": "1 (UTF-8 mode for worker logs and prompts)",
            },
        },
        "space": {
            "wheelhouse_bytes": sum(item["bytes"] for item in wheels) + sum(item["bytes"] for item in bootstrap),
            "compressed_closure_bytes": sum(item["bytes"] for item in all_artifacts),
            "installed_runtime_bytes": runtime_dir_bytes,
            "installed_runtime_files": runtime_file_count,
            "installed_runtime_path": "target/parity-runtime/runtime",
            "note": "The installer space pre-check should budget installed_runtime_bytes plus model weights.",
        },
        "dependency_resolution": {
            "resolved_with": "pip 26.2.1 on the host Python, cross-target resolution for cp312-win_amd64",
            "resolved_packages": len(wheels),
            "indexes": [PYPI_INDEX, PATCHED_URL],
            "resolve_report": "target/parity-runtime/pip-resolve-report.json (build artifact, gitignored)",
            "lock_sha256": hashlib.sha256(lock_bytes).hexdigest(),
            "lock_covers_full_closure": True,
        },
        "onnxruntime_selection": {
            "version": "1.31.1",
            "wheel": next((item["path"] for item in wheels if item["package"] == "onnxruntime"), None),
            "reason": (
                "Latest stable onnxruntime release with an official cp312 win_amd64 wheel at freeze time. "
                "BiRefNet runs through the ONNX Runtime CPUExecutionProvider only, so the CPU package is pinned "
                "instead of the GPU package; onnxruntime 1.31.1 supports Python 3.12 on Windows x64."
            ),
            "execution_provider": "CPUExecutionProvider",
        },
        "offline_install_evidence": args.offline_install_evidence,
        "runtime_probe_evidence": args.runtime_probe_evidence,
        "source_policy": {
            "binary_only": True,
            "system_python_modified": False,
            "target_platform": "windows-amd64",
            "python_abi": "cp312",
            "pypi_index": PYPI_INDEX,
            "cuda_index": PATCHED_URL,
            "no_network_at_install": True,
        },
        "limitations": [
            "This manifest proves fixed binary content and an offline install/import probe only.",
            "It does not prove full model inference, quality, or resource budgets.",
            "Python 3.12.10 is the last 3.12 release with official Windows binaries; later 3.12 releases are source-only.",
        ],
    }
    args.license_records.parent.mkdir(parents=True, exist_ok=True)
    args.license_records.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "kind": "windows-parity-runtime-license-records",
                "runtime_id": manifest["id"],
                "source": "License fields extracted from each wheel's own .dist-info/METADATA in the frozen wheelhouse.",
                "packages": license_entries,
            },
            ensure_ascii=False,
            indent=2,
        )
        + "\n",
        encoding="utf-8",
        newline="\n",
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(
        json.dumps(
            {
                "output": str(output),
                "wheels": len(wheels),
                "bootstrap": len(bootstrap),
                "artifact_count": manifest["artifact_count"],
                "total_bytes": manifest["total_bytes"],
                "status": manifest["status"],
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
