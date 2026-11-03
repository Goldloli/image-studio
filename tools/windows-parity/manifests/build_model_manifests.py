"""Build installer-schema Windows model manifests for the parity models.

Source of truth:
  - existing planned-model manifests (fixed HF content hashes / per-file ModelScope revisions)
  - verification/windows-parity-gate0/transfer-klein.json (ModelScope mirror URLs actually used)
  - the frozen Real-ESRGAN Windows release archive metadata from task 1.3

Output: resources/windows-parity/models/<model_variant_id>/model-manifest.json,
using the installer schema from apps/desktop/src/adapters/installation.rs
(ModelManifest: id/revision/component_bytes/files/archives). Extra fields are
non-breaking because the installer deserializes unknown fields away.
"""

from __future__ import annotations

import argparse
import json
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
            "archive_path": "models/realesrgan-x4plus.bin",
            "path": "realesrgan-x4plus.bin",
            "bytes": 33424520,
            "sha256": "713ee713b0353afaa27976f0563a64a5043bd70b9bd8936c2e26e25ebcdbcddf",
            "executable": False,
        },
        {
            "archive_path": "models/realesrgan-x4plus.param",
            "path": "realesrgan-x4plus.param",
            "bytes": 116029,
            "sha256": "35330ececcea33b6c397a72548e788d5d53becee4734c50b7fada36e89f10a86",
            "executable": False,
        },
    ],
}

KLEIN_LICENSE_MD = {
    "path": "LICENSE.md",
    "url": (
        "https://www.modelscope.cn/models/black-forest-labs/FLUX.2-klein-4B/"
        "resolve/a6e056113f18cb00eb3d9639b5c67fe60c32dd80/LICENSE.md"
    ),
    "bytes": 9584,
    "sha256": "ca02bc51900ab07789d1b70283329e7137f5af98f5161c23a1c81fc38a4af1fe",
    "license": "Apache-2.0",
    "source_revision": "a6e056113f18cb00eb3d9639b5c67fe60c32dd80",
}

BIREFNET_LICENSE = {
    "path": "LICENSE-BiRefNet.txt",
    "bytes": 1087,
    "sha256": "d7f5aa6ca5087c02da51a178073fe13df4480dfc2171e0c014bf39a7a57dd6a4",
    "source": "resources/planned-models/birefnet/LICENSE-BiRefNet.txt",
    "source_repository": "https://github.com/ZhengPeng7/BiRefNet",
    "note": (
        "Bundled as an application resource: the GitHub raw URL redirects to raw.githubusercontent.com, "
        "which is outside the installer URL allowlist, and the v1 release has no license asset."
    ),
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


def load(path: Path) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")


def artifact(path: str, url: str, entry: dict, extra: dict | None = None) -> dict:
    result = {
        "path": path,
        "url": url,
        "bytes": entry["bytes"],
        "sha256": entry["sha256"],
    }
    if extra:
        result.update(extra)
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--models-dir", type=Path, default=ROOT / "resources/windows-parity/models")
    parser.add_argument(
        "--planned-root", type=Path, default=ROOT / "resources/planned-models"
    )
    parser.add_argument(
        "--transfer-klein",
        type=Path,
        default=ROOT / "verification/windows-parity-gate0/transfer-klein.json",
    )
    parser.add_argument("--output-report", type=Path, default=ROOT / "target/parity-runtime/manifests-aux/build-report.json")
    args = parser.parse_args()

    planned_root = args.planned_root
    klein_planned = load(planned_root / "klein/model-manifest.json")
    birefnet_planned = load(planned_root / "birefnet/model-manifest.json")
    qwen_planned = load(planned_root / "qwen21/model-manifest.json")
    qwen_turbo_planned = load(planned_root / "qwen21-turbo/model-manifest.json")
    moss_planned = load(planned_root / "moss-image-sft/model-manifest.json")
    klein_transfer = load(args.transfer_klein)
    klein_sources = {item["path"]: item for item in klein_transfer["files"]}

    built: list[dict] = []

    def add(model: dict, manifest: dict) -> None:
        output = args.models_dir / model["id"] / "model-manifest.json"
        write_json(output, manifest)
        built.append(
            {
                "model": model["model"],
                "id": model["id"],
                "path": str(output.relative_to(ROOT)),
                "revision": manifest["revision"],
                "component_bytes": manifest["component_bytes"],
                "files": len(manifest.get("files", [])),
                "archives": len(manifest.get("archives", [])),
            }
        )

    # 1. FLUX.2 Klein 4B distilled -- ModelScope transport URLs, fixed HF content hashes.
    klein_files = []
    for entry in klein_planned["files"]:
        source = klein_sources.get(entry["path"])
        if source is None:
            raise SystemExit(f"transfer-klein.json has no URL for {entry['path']}")
        klein_files.append(
            artifact(
                entry["path"],
                source["source_url"],
                entry,
                {
                    "license": "Apache-2.0",
                    "declared_url": source["declared_url"],
                    "declared_revision": klein_transfer["revision"],
                    "source_revision": source["source_revision"],
                    "source_kind": source["source_kind"],
                },
            )
        )
    klein_files.append(
        artifact(
            KLEIN_LICENSE_MD["path"],
            KLEIN_LICENSE_MD["url"],
            KLEIN_LICENSE_MD,
            {
                "license": "Apache-2.0",
                "declared_url": (
                    "https://huggingface.co/black-forest-labs/FLUX.2-klein-4B/resolve/"
                    "e7b7dc27f91deacad38e78976d1f2b499d76a294/LICENSE.md"
                ),
                "declared_revision": klein_transfer["revision"],
                "source_revision": KLEIN_LICENSE_MD["source_revision"],
                "source_kind": "modelscope_mirror",
            },
        )
    )
    add(
        {"model": "flux2-klein-4b-distilled", "id": "flux2-klein-4b-distilled-windows-v1"},
        {
            "schema_version": 1,
            "id": "flux2-klein-4b-distilled-windows-v1",
            "repository": "black-forest-labs/FLUX.2-klein-4B",
            "revision": "e7b7dc27f91deacad38e78976d1f2b499d76a294",
            "license": "Apache-2.0",
            "component_bytes": sum(item["bytes"] for item in klein_files if item["path"] != "README.md"),
            "download_source": {
                "provider": "modelscope",
                "repository": "black-forest-labs/FLUX.2-klein-4B",
                "mirror_listing_revision": "master",
                "transport_revision": "a6e056113f18cb00eb3d9639b5c67fe60c32dd80",
                "content_identity": "SHA-256 fixed from the upstream Hugging Face blob manifest; the ModelScope mirror is transport only and every file hash/size was proven equal by transfer-klein.json.",
                "hf_declared_revision": "e7b7dc27f91deacad38e78976d1f2b499d76a294",
            },
            "files": klein_files,
            "archives": [],
        },
    )

    # 2. BiRefNet general epoch_244 ONNX (GitHub release; bundled MIT license record).
    birefnet_files = [
        artifact(
            birefnet_planned["files"][0]["path"],
            birefnet_planned["files"][0]["url"],
            birefnet_planned["files"][0],
            {"license": "MIT"},
        )
    ]
    add(
        {"model": "birefnet-general-epoch-244", "id": "birefnet-general-epoch-244-windows-v1"},
        {
            "schema_version": 1,
            "id": "birefnet-general-epoch-244-windows-v1",
            "repository": "ZhengPeng7/BiRefNet",
            "revision": "v1 / release asset 186740004",
            "license": "MIT",
            "component_bytes": sum(item["bytes"] for item in birefnet_files if item["path"] != "README.md"),
            "download_source": {
                "provider": "github-release",
                "repository": "ZhengPeng7/BiRefNet",
                "release": "v1",
                "asset_id": 186740004,
                "content_identity": "fixed release asset bytes and SHA-256",
            },
            "files": birefnet_files,
            "archives": [],
            "licenses": [BIREFNET_LICENSE],
        },
    )

    def modelscope_manifest(
        model_name: str,
        variant_id: str,
        planned: dict,
        license_name: str,
        notes: list[str] | None = None,
    ) -> None:
        files = []
        for entry in planned["files"]:
            extra = {
                "license": entry.get("license", license_name),
                "source_revision": entry.get("source_revision"),
                "upstream_url": (
                    f"https://huggingface.co/{planned['repository']}/resolve/{planned['revision']}/{entry['path']}"
                ),
            }
            files.append(artifact(entry["path"], entry["url"], entry, extra))
        manifest = {
            "schema_version": 1,
            "id": variant_id,
            "repository": planned["repository"],
            "revision": planned["revision"],
            "license": license_name,
            "component_bytes": sum(item["bytes"] for item in files if item["path"] != "README.md"),
            "download_source": planned.get("download_source", {}),
            "content_identity": "SHA-256 fixed from the upstream HF blob manifest; ModelScope per-file revisions are transport and were hash-verified.",
            "files": files,
            "archives": [],
        }
        if notes:
            manifest["notes"] = notes
        add({"model": model_name, "id": variant_id}, manifest)

    # 3. Qwen-Image 2.1 base.
    modelscope_manifest("qwen-image-2.1", "qwen-image-2.1-windows-v1", qwen_planned, "qwen-research")

    # 4. Qwen-Image 2.1 Turbo.
    modelscope_manifest(
        "qwen-image-2.1-turbo",
        "qwen-image-2.1-turbo-windows-v1",
        qwen_turbo_planned,
        "qwen-research",
    )

    # 5. MOSS-Image-SFT (the custom pipeline code is vendored in the worker, not a model file).
    modelscope_manifest(
        "moss-image-sft",
        "moss-image-sft-windows-v1",
        moss_planned,
        "Apache-2.0",
        notes=[
            "pipeline_moss_image.py is vendored in the Windows worker (fixed upstream revision + reviewed hash); it is not installed from the model repository.",
            "MOSS DiT runs with the documented bnb dtype shim from tools/windows-parity-staged/moss_bnb_shim.py.",
        ],
    )

    # 6. Real-ESRGAN x4plus weights extracted from the official Windows release zip.
    archive = {
        "path": REALESRGAN_ARCHIVE["path"],
        "url": REALESRGAN_ARCHIVE["url"],
        "bytes": REALESRGAN_ARCHIVE["bytes"],
        "sha256": REALESRGAN_ARCHIVE["sha256"],
        "members": REALESRGAN_ARCHIVE["members"],
    }
    add(
        {"model": "realesrgan-x4plus", "id": "realesrgan-x4plus-windows-v1"},
        {
            "schema_version": 1,
            "id": "realesrgan-x4plus-windows-v1",
            "repository": "xinntao/Real-ESRGAN",
            "revision": "v0.2.5.0 / realesrgan-ncnn-vulkan-20220424 / x4plus",
            "license": "BSD-3-Clause (model) / MIT (ncnn app)",
            "component_bytes": sum(member["bytes"] for member in archive["members"]),
            "download_source": {
                "provider": "github-release",
                "repository": "xinntao/Real-ESRGAN",
                "release": "v0.2.5.0",
                "content_identity": "fixed official release asset bytes and SHA-256",
                "cache_note": (
                    "The native runtime manifest references the same zip; the installer cache is keyed by SHA-256, "
                    "so the archive is stored and downloaded once. Space math counting both manifests is a "
                    "conservative upper bound."
                ),
            },
            "files": [],
            "archives": [archive],
            "licenses": REALESRGAN_LICENSES,
        },
    )

    args.output_report.parent.mkdir(parents=True, exist_ok=True)
    args.output_report.write_text(json.dumps({"built": built}, indent=2) + "\n", encoding="utf-8", newline="\n")
    for item in built:
        print(
            f"{item['model']:>24} {item['id']:<42} files={item['files']:<3} "
            f"archives={item['archives']} component_bytes={item['component_bytes']}"
        )
    print(json.dumps({"built": len(built), "report": str(args.output_report)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
