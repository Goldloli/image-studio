"""Verify installer manifests against the real artifacts and the installer rules.

Mirrors the acceptance rules in apps/desktop/src/adapters/installation.rs:
  * ModelManifest / RuntimeManifest JSON shapes
  * Artifact: safe relative path, non-zero bytes, 64-hex sha256, allowlisted https URL
  * ArchiveArtifact: path ends .zip, members non-empty, unique safe targets, valid members
  * Model component_bytes == sum(installed files except README.md) + archive member bytes
  * Python runtime: python + interpreter present, no native archives
  * Native runtime: executable present, no interpreter/wheels

Then re-hashes the real local files:
  * model files from target/parity-gate0/models/<model dir>/
  * runtime interpreter/wheels from target/parity-runtime/{downloads,wheelhouse}/
  * native archive members from the downloaded official zip

Missing local files are reported honestly as `missing`, never faked.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]

ALLOWED_HOSTS_EXACT = {
    "huggingface.co",
    "modelscope.cn",
    "github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
    "files.pythonhosted.org",
    "www.python.org",
    "download.pytorch.org",
    "download-r2.pytorch.org",
}
ALLOWED_HOST_SUFFIXES = (".cdn.hf.co", ".xethub.hf.co", ".modelscope.cn")
HEX64 = re.compile(r"[0-9a-f]{64}")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def safe_relative(value: str) -> bool:
    if not value:
        return False
    parts = Path(value).parts
    if not parts:
        return False
    return all(part not in {"..", ".", ""} for part in parts) and not Path(value).is_absolute()


def allowed_url(url: str) -> tuple[bool, str]:
    if not url.startswith("https://"):
        return False, "not https"
    rest = url[len("https://") :]
    host_part = rest.split("/", 1)[0]
    if "@" in host_part:
        return False, "credentials in URL"
    host = host_part.split(":", 1)[0].lower()
    if ":" in host_part and host_part.rsplit(":", 1)[1] not in {"443", ""}:
        return False, "non-default port"
    if host in ALLOWED_HOSTS_EXACT or host.endswith(ALLOWED_HOST_SUFFIXES):
        return True, host
    return False, f"host {host} not allowlisted"


def validate_artifact(artifact: dict) -> list[str]:
    errors = []
    if not isinstance(artifact.get("path"), str) or not safe_relative(artifact["path"]):
        errors.append("artifact path is not a safe relative path")
    if not isinstance(artifact.get("bytes"), int) or artifact["bytes"] <= 0:
        errors.append("artifact bytes must be a positive integer")
    sha = artifact.get("sha256")
    if not isinstance(sha, str) or not HEX64.fullmatch(sha):
        errors.append("artifact sha256 must be 64 lowercase hex characters")
    url = artifact.get("url")
    if not isinstance(url, str):
        errors.append("artifact url missing")
    elif url != "bundled:worker-wheel":
        ok, reason = allowed_url(url)
        if not ok:
            errors.append(f"artifact url rejected: {reason}")
    return errors


def validate_archive(archive: dict) -> list[str]:
    errors = []
    path = archive.get("path")
    if not isinstance(path, str) or not safe_relative(path):
        errors.append("archive path is not a safe relative path")
    elif not path.endswith(".zip"):
        errors.append("archive path must end with .zip")
    for error in validate_artifact(
        {
            "path": archive.get("path", ""),
            "url": archive.get("url", ""),
            "bytes": archive.get("bytes", 0),
            "sha256": archive.get("sha256", ""),
        }
    ):
        errors.append(f"archive: {error}")
    members = archive.get("members")
    if not isinstance(members, list) or not members:
        errors.append("archive members must be a non-empty list")
        return errors
    targets = set()
    for member in members:
        if not isinstance(member.get("archive_path"), str) or not safe_relative(member["archive_path"]):
            errors.append("archive member archive_path is not a safe relative path")
        if not isinstance(member.get("path"), str) or not safe_relative(member["path"]):
            errors.append("archive member path is not a safe relative path")
        elif member["path"] in targets:
            errors.append(f"duplicate archive member target: {member['path']}")
        else:
            targets.add(member["path"])
        if not isinstance(member.get("bytes"), int) or member["bytes"] <= 0:
            errors.append("archive member bytes must be positive")
        if not isinstance(member.get("sha256"), str) or not HEX64.fullmatch(member["sha256"]):
            errors.append("archive member sha256 must be 64 lowercase hex characters")
    return errors


def check_local_file(record: dict, local_path: Path) -> dict:
    result = {
        "path": record["path"],
        "expected_bytes": record["bytes"],
        "expected_sha256": record["sha256"],
        "local_path": str(local_path),
    }
    if not local_path.is_file():
        result.update({"status": "missing"})
        return result
    actual_bytes = local_path.stat().st_size
    actual_sha = sha256_file(local_path)
    result.update(
        {
            "actual_bytes": actual_bytes,
            "actual_sha256": actual_sha,
            "status": "pass" if (actual_bytes == record["bytes"] and actual_sha == record["sha256"]) else "fail",
        }
    )
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--models-dir", type=Path, default=ROOT / "resources/windows-parity/models")
    parser.add_argument(
        "--product-runtime",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/product-runtime-manifest.json",
    )
    parser.add_argument(
        "--native-runtime",
        type=Path,
        default=ROOT / "resources/windows-parity/realesrgan-ncnn-vulkan/runtime-manifest.json",
    )
    parser.add_argument("--frozen-runtime", type=Path, default=ROOT / "resources/windows-parity/runtime-v2/runtime-manifest.json")
    parser.add_argument("--lock", type=Path, default=ROOT / "resources/windows-parity/runtime-v2/requirements.lock")
    parser.add_argument("--model-root", type=Path, default=ROOT / "target/parity-gate0/models")
    parser.add_argument("--wheelhouse", type=Path, default=ROOT / "target/parity-runtime/wheelhouse")
    parser.add_argument("--downloads", type=Path, default=ROOT / "target/parity-runtime/downloads")
    parser.add_argument(
        "--aux-dir", type=Path, default=ROOT / "target/parity-runtime/manifests-aux"
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "verification/windows-parity-manifests/installer-manifests-verification.json",
    )
    args = parser.parse_args()

    model_dirs = {
        "flux2-klein-4b-distilled-windows-v1": args.model_root / "klein",
        "birefnet-general-epoch-244-windows-v1": args.model_root / "birefnet",
        "qwen-image-2.1-windows-v1": args.model_root / "qwen-image-2.1",
        "qwen-image-2.1-turbo-windows-v1": args.model_root / "qwen-image-2.1-turbo",
        "moss-image-sft-windows-v1": args.model_root / "moss-image-sft",
    }
    aux_overrides = {
        ("flux2-klein-4b-distilled-windows-v1", "LICENSE.md"): args.aux_dir / "klein-LICENSE.md",
    }

    evidence: dict = {
        "schema_version": 1,
        "kind": "windows-parity-installer-manifest-verification",
        "status": "pending",
        "method": (
            "Re-implements the installer's manifest validation rules in Python, then re-hashes every "
            "manifest artifact against the local transferred/downloaded files."
        ),
        "allowlist": {
            "exact_hosts": sorted(ALLOWED_HOSTS_EXACT),
            "host_suffixes": list(ALLOWED_HOST_SUFFIXES),
            "scheme": "https",
            "port": 443,
        },
        "manifests": [],
        "failures": [],
        "notes": [],
    }

    # ---- model manifests -------------------------------------------------
    model_manifests = sorted(args.models_dir.glob("*/model-manifest.json"))
    for manifest_path in model_manifests:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        entry = {
            "kind": "model-manifest",
            "path": str(manifest_path.relative_to(ROOT)),
            "id": manifest.get("id"),
            "revision": manifest.get("revision"),
            "schema_errors": [],
            "url_hosts": [],
            "file_checks": [],
            "license_checks": [],
            "component_bytes": {
                "declared": manifest.get("component_bytes"),
                "computed_installed_bytes": None,
                "status": "pending",
            },
            "status": "pending",
        }
        for required in ("id", "revision", "component_bytes"):
            if required not in manifest:
                entry["schema_errors"].append(f"missing required field {required}")
        if not isinstance(manifest.get("id"), str) or not manifest.get("id"):
            entry["schema_errors"].append("id must be a non-empty string")
        if not isinstance(manifest.get("revision"), str) or not manifest.get("revision"):
            entry["schema_errors"].append("revision must be a non-empty string")
        if not isinstance(manifest.get("component_bytes"), int) or manifest["component_bytes"] <= 0:
            entry["schema_errors"].append("component_bytes must be a positive integer")
        files = manifest.get("files") or []
        archives = manifest.get("archives") or []
        if not isinstance(files, list) or not isinstance(archives, list):
            entry["schema_errors"].append("files/archives must be lists")
            files, archives = [], []
        installed_bytes = 0
        local_root = model_dirs.get(manifest.get("id"))
        if local_root is None and files:
            entry["schema_errors"].append("no local model directory mapping for this id")
        for record in files:
            for error in validate_artifact(record):
                entry["schema_errors"].append(f"{record.get('path')}: {error}")
            ok, host = allowed_url(record.get("url", ""))
            if ok and host not in entry["url_hosts"]:
                entry["url_hosts"].append(host)
            if record.get("path") == "README.md":
                continue
            installed_bytes += record.get("bytes", 0)
            local = aux_overrides.get((manifest.get("id"), record.get("path")))
            if local is None and local_root is not None:
                local = local_root / record["path"]
            if local is None:
                entry["file_checks"].append({"path": record.get("path"), "status": "missing", "reason": "no local mapping"})
                continue
            entry["file_checks"].append(check_local_file(record, local))
        member_checks = []
        for archive in archives:
            for error in validate_archive(archive):
                entry["schema_errors"].append(f"archive {archive.get('path')}: {error}")
            local_archive = args.downloads / archive.get("path", "")
            archive_record = {
                "path": archive.get("path"),
                "bytes": archive.get("bytes"),
                "sha256": archive.get("sha256"),
                "url": archive.get("url"),
            }
            archive_check = check_local_file(archive_record, local_archive)
            archive_check["archive_path"] = archive.get("path")
            entry["file_checks"].append(archive_check)
            if local_archive.is_file():
                try:
                    with zipfile.ZipFile(local_archive) as handle:
                        names = set(handle.namelist())
                        for member in archive.get("members", []):
                            item = {
                                "archive_path": member.get("archive_path"),
                                "path": member.get("path"),
                                "expected_bytes": member.get("bytes"),
                                "expected_sha256": member.get("sha256"),
                                "in_archive": member.get("archive_path") in names,
                            }
                            if item["in_archive"]:
                                data = handle.read(member["archive_path"])
                                item["actual_bytes"] = len(data)
                                item["actual_sha256"] = hashlib.sha256(data).hexdigest()
                                item["status"] = (
                                    "pass"
                                    if item["actual_bytes"] == member["bytes"]
                                    and item["actual_sha256"] == member["sha256"]
                                    else "fail"
                                )
                            else:
                                item["status"] = "missing"
                            member_checks.append(item)
                            installed_bytes += member.get("bytes", 0)
                except zipfile.BadZipFile:
                    entry["schema_errors"].append("local archive is not a readable zip")
        entry["archive_member_checks"] = member_checks
        for record in manifest.get("licenses", []) or []:
            local = ROOT / record["source"]
            entry["license_checks"].append(check_local_file(record, local))
        entry["component_bytes"]["computed_installed_bytes"] = installed_bytes
        component_ok = installed_bytes == manifest.get("component_bytes")
        entry["component_bytes"]["status"] = "pass" if component_ok else "fail"
        file_failures = [item for item in entry["file_checks"] if item["status"] != "pass"]
        member_failures = [item for item in member_checks if item["status"] != "pass"]
        license_failures = [item for item in entry["license_checks"] if item["status"] != "pass"]
        entry["status"] = (
            "pass"
            if not entry["schema_errors"] and component_ok and not file_failures and not member_failures and not license_failures
            else "fail"
        )
        if entry["status"] != "pass":
            evidence["failures"].append(
                {
                    "manifest": entry["path"],
                    "schema_errors": entry["schema_errors"],
                    "file_failures": file_failures,
                    "member_failures": member_failures,
                    "license_failures": license_failures,
                }
            )
        evidence["manifests"].append(entry)

    # ---- product runtime manifest (if present) ---------------------------
    if args.product_runtime.is_file():
        runtime = json.loads(args.product_runtime.read_text(encoding="utf-8"))
        entry = {
            "kind": "product-runtime-manifest",
            "path": str(args.product_runtime.relative_to(ROOT)),
            "id": runtime.get("id"),
            "schema_errors": [],
            "file_checks": [],
            "packages_check": {},
            "status": "pending",
        }
        for required in ("id", "python", "interpreter", "wheels"):
            if required not in runtime:
                entry["schema_errors"].append(f"missing required field {required}")
        if runtime.get("kind") != "python-embeddable-windows":
            entry["schema_errors"].append("kind must be python-embeddable-windows")
        if runtime.get("platform") != "windows-amd64":
            entry["schema_errors"].append("platform must be windows-amd64")
        if runtime.get("archives"):
            entry["schema_errors"].append("python runtime must not contain native archives")
        interpreter = runtime.get("interpreter")
        if isinstance(interpreter, dict):
            for error in validate_artifact(interpreter):
                entry["schema_errors"].append(f"interpreter: {error}")
            entry["file_checks"].append(check_local_file(interpreter, args.downloads / interpreter.get("path", "")))
        wheels = runtime.get("wheels") or []
        if not isinstance(wheels, list):
            entry["schema_errors"].append("wheels must be a list")
            wheels = []
        for wheel in wheels:
            for error in validate_artifact(wheel):
                entry["schema_errors"].append(f"{wheel.get('path')}: {error}")
            entry["file_checks"].append(check_local_file(wheel, args.wheelhouse / wheel.get("path", "")))
        packages = runtime.get("packages") or {}
        lock_pins = {}
        for line in args.lock.read_text(encoding="utf-8").splitlines():
            if "==" in line and not line.startswith("#"):
                name, version = line.split("==", 1)
                lock_pins[re.sub(r"[-_.]+", "-", name).lower()] = version
        package_errors = []
        for name, version in lock_pins.items():
            if packages.get(name) != version:
                package_errors.append({"package": name, "expected": version, "actual": packages.get(name)})
        extra = sorted(set(packages) - set(lock_pins))
        entry["packages_check"] = {
            "lock_pins": len(lock_pins),
            "manifest_packages": len(packages),
            "missing_or_mismatched": package_errors,
            "extra_packages": extra,
            "status": "pass" if not package_errors else "fail",
        }
        wheel_failures = [item for item in entry["file_checks"] if item["status"] != "pass"]
        entry["status"] = (
            "pass"
            if not entry["schema_errors"] and not wheel_failures and not package_errors
            else "fail"
        )
        if entry["status"] != "pass":
            evidence["failures"].append({"manifest": entry["path"], "schema_errors": entry["schema_errors"], "file_failures": wheel_failures, "package_errors": package_errors})
        evidence["manifests"].append(entry)
    else:
        evidence["notes"].append("product-runtime-manifest.json not present yet; runtime manifest checks skipped.")

    # ---- native runtime manifest (if present) ---------------------------
    if args.native_runtime.is_file():
        runtime = json.loads(args.native_runtime.read_text(encoding="utf-8"))
        entry = {
            "kind": "native-runtime-manifest",
            "path": str(args.native_runtime.relative_to(ROOT)),
            "id": runtime.get("id"),
            "schema_errors": [],
            "archive_checks": [],
            "member_checks": [],
            "license_checks": [],
            "status": "pending",
        }
        if runtime.get("id") != "realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1":
            entry["schema_errors"].append("unexpected native runtime id")
        if runtime.get("platform") != "windows-amd64":
            entry["schema_errors"].append("native runtime platform must be windows-amd64")
        if runtime.get("executable") != "realesrgan-ncnn-vulkan.exe":
            entry["schema_errors"].append("native runtime executable must be realesrgan-ncnn-vulkan.exe")
        if runtime.get("interpreter") is not None:
            entry["schema_errors"].append("native runtime must not carry an interpreter")
        if runtime.get("wheels"):
            entry["schema_errors"].append("native runtime must not carry wheels")
        if runtime.get("kind") != "native":
            entry["schema_errors"].append("native runtime kind must be native")
        for archive in runtime.get("archives") or []:
            for error in validate_archive(archive):
                entry["schema_errors"].append(f"archive {archive.get('path')}: {error}")
            local_archive = args.downloads / archive.get("path", "")
            archive_record = {
                "path": archive.get("path"),
                "bytes": archive.get("bytes"),
                "sha256": archive.get("sha256"),
                "url": archive.get("url"),
            }
            archive_check = check_local_file(archive_record, local_archive)
            archive_check["archive_path"] = archive.get("path")
            entry["archive_checks"].append(archive_check)
            if local_archive.is_file():
                with zipfile.ZipFile(local_archive) as handle:
                    names = set(handle.namelist())
                    for member in archive.get("members", []):
                        item = {
                            "archive_path": member.get("archive_path"),
                            "path": member.get("path"),
                            "expected_bytes": member.get("bytes"),
                            "expected_sha256": member.get("sha256"),
                            "in_archive": member.get("archive_path") in names,
                        }
                        if item["in_archive"]:
                            data = handle.read(member["archive_path"])
                            item["actual_bytes"] = len(data)
                            item["actual_sha256"] = hashlib.sha256(data).hexdigest()
                            item["status"] = (
                                "pass"
                                if item["actual_bytes"] == member["bytes"]
                                and item["actual_sha256"] == member["sha256"]
                                else "fail"
                            )
                        else:
                            item["status"] = "missing"
                        entry["member_checks"].append(item)
        for record in runtime.get("licenses", []) or []:
            local = ROOT / record["source"]
            entry["license_checks"].append(check_local_file(record, local))
        archive_failures = [item for item in entry["archive_checks"] if item["status"] != "pass"]
        member_failures = [item for item in entry["member_checks"] if item["status"] != "pass"]
        license_failures = [item for item in entry["license_checks"] if item["status"] != "pass"]
        entry["status"] = (
            "pass"
            if not entry["schema_errors"] and not archive_failures and not member_failures and not license_failures
            else "fail"
        )
        if entry["status"] != "pass":
            evidence["failures"].append({"manifest": entry["path"], "schema_errors": entry["schema_errors"], "archive_failures": archive_failures, "member_failures": member_failures, "license_failures": license_failures})
        evidence["manifests"].append(entry)
    else:
        evidence["notes"].append("realesrgan native runtime-manifest.json not present yet; native runtime checks skipped.")

    evidence["status"] = "pass" if not evidence["failures"] and evidence["manifests"] else "fail"
    evidence["summary"] = {
        "manifests_checked": len(evidence["manifests"]),
        "model_manifests": sum(1 for item in evidence["manifests"] if item["kind"] == "model-manifest"),
        "product_runtime": any(item["kind"] == "product-runtime-manifest" for item in evidence["manifests"]),
        "native_runtime": any(item["kind"] == "native-runtime-manifest" for item in evidence["manifests"]),
        "file_checks": sum(len(item.get("file_checks", [])) for item in evidence["manifests"]),
        "failures": len(evidence["failures"]),
    }

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"status": evidence["status"], "summary": evidence["summary"], "output": str(args.output)}))
    return 0 if evidence["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
