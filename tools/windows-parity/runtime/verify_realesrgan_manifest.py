"""Independently verify the frozen Real-ESRGAN bundle against its asset manifest.

Re-hashes the downloaded archive, every archive member, the extracted install
files and the pinned license files, and compares all of them with
`resources/windows-parity/realesrgan-ncnn-vulkan/asset-manifest.json`.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=ROOT / "resources/windows-parity/realesrgan-ncnn-vulkan/asset-manifest.json",
    )
    parser.add_argument(
        "--archive",
        type=Path,
        default=ROOT / "target/parity-runtime/downloads/realesrgan-ncnn-vulkan-20220424-windows.zip",
    )
    parser.add_argument("--app-dir", type=Path, default=ROOT / "target/parity-runtime/realesrgan/app")
    parser.add_argument("--bundle-dir", type=Path, default=ROOT / "resources/windows-parity/realesrgan-ncnn-vulkan")
    parser.add_argument(
        "--output", type=Path, default=ROOT / "verification/windows-parity-realesrgan/manifest-reverify.json"
    )
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    failures = []

    archive_record = manifest["archives"][0]
    archive_ok = (
        args.archive.is_file()
        and args.archive.stat().st_size == archive_record["bytes"]
        and sha256_file(args.archive) == archive_record["sha256"]
    )
    if not archive_ok:
        failures.append({"artifact": "archive", "expected": archive_record, "actual": "missing or mismatched"})

    member_results = []
    members_by_path = {}
    with zipfile.ZipFile(args.archive) as archive:
        names = set(archive.namelist())
        for record in manifest["archive_members"]:
            name = record["archive_path"]
            entry = {"archive_path": name, "in_archive": name in names}
            if name in names:
                data = archive.read(name)
                entry.update(
                    {
                        "expected_bytes": record["bytes"],
                        "actual_bytes": len(data),
                        "expected_sha256": record["sha256"],
                        "actual_sha256": sha256_bytes(data),
                    }
                )
                entry["status"] = (
                    "pass"
                    if entry["actual_bytes"] == record["bytes"] and entry["actual_sha256"] == record["sha256"]
                    else "fail"
                )
                members_by_path[name] = entry["actual_sha256"]
            else:
                entry["status"] = "fail"
            if entry["status"] == "fail":
                failures.append({"artifact": f"archive_member:{name}", **entry})
            member_results.append(entry)

    install_results = []
    for record in manifest["install_members"]:
        path = args.app_dir / record["path"]
        entry = {"path": str(path), "expected_sha256": record["sha256"]}
        if path.is_file():
            entry.update(
                {
                    "actual_bytes": path.stat().st_size,
                    "actual_sha256": sha256_file(path),
                }
            )
            entry["status"] = (
                "pass"
                if entry["actual_bytes"] == record["bytes"] and entry["actual_sha256"] == record["sha256"]
                else "fail"
            )
        else:
            entry["status"] = "fail"
            entry["error"] = "missing installed file"
        if entry["status"] == "fail":
            failures.append({"artifact": f"install:{record['path']}", **entry})
        install_results.append(entry)

    license_results = []
    for record in manifest["licenses"]:
        path = args.bundle_dir / record["path"]
        entry = {"path": str(path), "expected_sha256": record["sha256"], "url": record["url"]}
        if path.is_file():
            entry.update({"actual_bytes": path.stat().st_size, "actual_sha256": sha256_file(path)})
            entry["status"] = (
                "pass"
                if entry["actual_bytes"] == record["bytes"] and entry["actual_sha256"] == record["sha256"]
                else "fail"
            )
        else:
            entry["status"] = "fail"
            entry["error"] = "missing license file"
        if entry["status"] == "fail":
            failures.append({"artifact": f"license:{record['path']}", **entry})
        license_results.append(entry)

    evidence = {
        "schema_version": 1,
        "kind": "windows-parity-realesrgan-manifest-reverify",
        "case_id": "G2-R1",
        "status": "pass" if not failures else "fail",
        "bundle_id": manifest["id"],
        "archive": {
            "path": str(args.archive),
            "expected_bytes": archive_record["bytes"],
            "expected_sha256": archive_record["sha256"],
            "actual_bytes": args.archive.stat().st_size if args.archive.is_file() else None,
            "actual_sha256": sha256_file(args.archive) if args.archive.is_file() else None,
            "independent_downloads": archive_record["download_verification"]["independent_downloads"],
            "status": "pass" if archive_ok else "fail",
        },
        "archive_member_checks": member_results,
        "installed_file_checks": install_results,
        "license_checks": license_results,
        "failures": failures,
        "method": (
            "Re-hashed the local official archive, all listed archive members, the extracted install files "
            "and the pinned license files; compared every value with the checked-in asset manifest."
        ),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(
        json.dumps(
            {
                "status": evidence["status"],
                "archive_members": len(member_results),
                "installed_files": len(install_results),
                "licenses": len(license_results),
                "failures": len(failures),
                "output": str(args.output),
            }
        )
    )
    return 0 if evidence["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
