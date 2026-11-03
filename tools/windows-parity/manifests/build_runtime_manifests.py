"""Build the installer-facing Windows v2 product runtime manifest and the
Real-ESRGAN native runtime manifest.

The product manifest derives from the frozen runtime closure
(resources/windows-parity/runtime-v2/runtime-manifest.json) and the runtime that
was actually installed offline; `packages` is taken from the installed
distribution metadata exactly as the installer probe reads it
(`metadata['Name'].lower().replace('_','-') -> version`).

The native manifest describes the Real-ESRGAN ncnn Windows package frozen in
task 1.3: only members that really exist in the official zip are listed as
archive members; the two license texts are bundled application resources and
are recorded in the non-breaking `licenses` field.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]

REALESRGAN_ARCHIVE = {
    "path": "realesrgan-ncnn-vulkan-20220424-windows.zip",
    "url": (
        "https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/"
        "realesrgan-ncnn-vulkan-20220424-windows.zip"
    ),
    "bytes": 45474481,
    "sha256": "abc02804e17982a3be33675e4d471e91ea374e65b70167abc09e31acb412802d",
    "members": [
        {
            "archive_path": "realesrgan-ncnn-vulkan.exe",
            "path": "realesrgan-ncnn-vulkan.exe",
            "bytes": 6161408,
            "sha256": "07e49f7cbb4ede01ae4dd4c399d3a7e5846e3d2085c3128eff881e55cb7b1a0c",
            "executable": True,
        },
        {
            "archive_path": "vcomp140.dll",
            "path": "vcomp140.dll",
            "bytes": 182704,
            "sha256": "8f72ef2e483465444b2059fc6744d6cb22cd8d8a27f6fa56befd2a42dcd0f78b",
            "executable": False,
        },
        {
            "archive_path": "README_windows.md",
            "path": "README_windows.md",
            "bytes": 1859,
            "sha256": "2b882314a0c7a9f17a347b9b04f909939f6274aa96cf5b2ebba2f7ab75987480",
            "executable": False,
        },
    ],
}

REALESRGAN_LICENSES = [
    {
        "path": "LICENSE-RealESRGAN.txt",
        "bytes": 1519,
        "sha256": "4a699ec4863d96a91fc265948a0c90033f7e8735d515524dcf3444736406e0c2",
        "source": "resources/windows-parity/realesrgan-ncnn-vulkan/LICENSE-RealESRGAN.txt",
        "source_url": "https://raw.githubusercontent.com/xinntao/Real-ESRGAN/v0.2.5.0/LICENSE",
        "note": "The official Windows zip does not contain license files; the text is bundled as an application resource.",
    },
    {
        "path": "LICENSE-ncnn-app.txt",
        "bytes": 2228,
        "sha256": "5abb941454de437b0e90d78dcb72e3688f74e14bcd4e24393273cb5cd0e9c937",
        "source": "resources/windows-parity/realesrgan-ncnn-vulkan/LICENSE-ncnn-app.txt",
        "source_url": "https://raw.githubusercontent.com/xinntao/Real-ESRGAN-ncnn-vulkan/v0.2.0/LICENSE",
        "note": "MIT license of the ncnn application, bundled as an application resource.",
    },
]

WORKER_WHEELS_PENDING = {
    "status": "pending",
    "expected_wheels": [
        {
            "package": "studio-diffusers-worker",
            "version": "0.1.0",
            "worker_build": "studio-diffusers-worker-windows-0.1.0",
            "serves": ["Klein text-to-image", "Klein single-image edit", "Qwen base", "Qwen Turbo", "MOSS"],
        },
        {
            "package": "studio-onnx-worker",
            "version": "0.1.0",
            "worker_build": "studio-onnx-worker-windows-0.1.0",
            "serves": ["BiRefNet"],
        },
    ],
    "note": (
        "The Windows worker wheels are produced by the worker implementation streams. They will be appended to "
        "`wheels` as Artifacts (path/url as 'bundled:worker-wheel' or a frozen local artifact, bytes and sha256 "
        "from the built wheel) when available, and this field will be removed then. Until that happens this "
        "runtime manifest is explicitly incomplete on the worker-wheel field and must not be used to activate an "
        "installation."
    ),
}


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalize(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def parse_lock(path: Path) -> dict[str, str]:
    pins: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        name, version = line.split("==", 1)
        pins[normalize(name)] = version
    return pins


def installed_versions(site_packages: Path) -> dict[str, str]:
    """Mirror the installer probe: metadata Name lowercased, underscores -> dashes."""
    versions: dict[str, str] = {}
    for metadata_path in site_packages.glob("*.dist-info/METADATA"):
        name = None
        version = None
        for line in metadata_path.read_text(encoding="utf-8", errors="replace").splitlines():
            if line.startswith("Name: ") and name is None:
                name = line[len("Name: ") :].strip()
            elif line.startswith("Version: ") and version is None:
                version = line[len("Version: ") :].strip()
            if name is not None and version is not None:
                break
        if name and version:
            versions[name.lower().replace("_", "-")] = version
    return versions


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--frozen-runtime",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/runtime-manifest.json",
    )
    parser.add_argument(
        "--lock",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/requirements.lock",
    )
    parser.add_argument("--runtime-dir", type=Path, default=ROOT / "target/parity-runtime/runtime")
    parser.add_argument(
        "--product-output",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/product-runtime-manifest.json",
    )
    parser.add_argument(
        "--native-output",
        type=Path,
        default=ROOT / "resources/windows-parity/realesrgan-ncnn-vulkan/runtime-manifest.json",
    )
    args = parser.parse_args()

    frozen = json.loads(args.frozen_runtime.read_text(encoding="utf-8"))
    pins = parse_lock(args.lock)
    versions = installed_versions(args.runtime_dir / "Lib" / "site-packages")
    missing = sorted(set(pins) - set(versions))
    mismatched = {
        name: {"locked": pins[name], "installed": versions[name]}
        for name in sorted(set(pins) & set(versions))
        if versions[name] != pins[name]
    }
    if missing or mismatched:
        raise SystemExit(f"installed runtime does not match the lock: missing={missing} mismatched={mismatched}")

    packages = {name: versions[name] for name in sorted(pins)}
    wheels = []
    for wheel in frozen["wheels"]:
        wheels.append(
            {
                "path": wheel["path"],
                "url": wheel["url"],
                "bytes": wheel["bytes"],
                "sha256": wheel["sha256"],
                "package": wheel["package"],
                "version": wheel["version"],
            }
        )
    interpreter = frozen["interpreter"]
    product = {
        "schema_version": 1,
        "kind": "python-embeddable-windows",
        "id": frozen["id"],
        "python": frozen["python"],
        "platform": frozen["platform"],
        "interpreter": {
            "path": interpreter["path"],
            "url": interpreter["url"],
            "bytes": interpreter["bytes"],
            "sha256": interpreter["sha256"],
        },
        "wheels": wheels,
        "packages": packages,
        "archives": [],
        "install_layout": {
            "interpreter_root": "python.exe",
            "site_packages": "Lib/site-packages",
            "path_file": "python312._pth",
        },
        "worker_wheels_pending": WORKER_WHEELS_PENDING,
        "frozen_runtime_manifest": {
            "path": "resources/windows-parity/runtime-v2/runtime-manifest.json",
            "sha256": sha256_file(args.frozen_runtime),
        },
        "requirements_lock": {
            "path": "resources/windows-parity/runtime-v2/requirements.lock",
            "sha256": sha256_file(args.lock),
        },
        "packages_source": (
            "importlib.metadata-equivalent read of the installed runtime's .dist-info METADATA, normalized "
            "exactly like the installer probe (Name.lower().replace('_','-')); every value equals requirements.lock"
        ),
        "notes": [
            "This manifest is the installer RuntimeManifest view of the frozen runtime; the frozen evidence "
            "closure with per-file index metadata and licenses stays in runtime-manifest.json.",
            "worker_wheels_pending marks the manifest as incomplete for product activation until the worker "
            "wheels are appended.",
        ],
    }
    write_json(args.product_output, product)

    native = {
        "schema_version": 1,
        "kind": "native",
        "id": "realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1",
        "platform": "windows-amd64",
        "executable": "realesrgan-ncnn-vulkan.exe",
        "archives": [
            {
                "path": REALESRGAN_ARCHIVE["path"],
                "url": REALESRGAN_ARCHIVE["url"],
                "bytes": REALESRGAN_ARCHIVE["bytes"],
                "sha256": REALESRGAN_ARCHIVE["sha256"],
                "members": REALESRGAN_ARCHIVE["members"],
            }
        ],
        "licenses": REALESRGAN_LICENSES,
        "install_layout": {
            "root": ".",
            "executable": "realesrgan-ncnn-vulkan.exe",
            "models_dir": "models",
        },
        "fixed_parameters": {
            "model": "realesrgan-x4plus",
            "scale": 4,
            "tile": 128,
            "gpu_ids": [0],
            "threads": "1:1:1",
            "tta": False,
        },
        "notes": [
            "The official Windows zip contains the executable, vcomp140.dll, the model weights, README_windows.md "
            "and sample assets, but no license files; the two license texts are bundled application resources "
            "recorded in `licenses` and must ship with the installed files.",
            "The same archive is referenced by the Real-ESRGAN model manifest; the installer download cache is "
            "keyed by SHA-256, so the 45,474,481-byte zip is stored once. Install-space math that counts both "
            "manifest downloads is a conservative upper bound.",
            "The platform gate requires platform == 'windows-amd64' after the planned native-platform change.",
        ],
    }
    write_json(args.native_output, native)
    print(
        json.dumps(
            {
                "product": str(args.product_output),
                "product_wheels": len(wheels),
                "product_packages": len(packages),
                "native": str(args.native_output),
                "native_members": len(native["archives"][0]["members"]),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
