"""Re-verify every artifact recorded in the frozen Windows v2 runtime manifest.

Independent check: hashes local files again and compares them with the checked-in
manifest (it does not trust the manifest-builder run), then checks that the
installed runtime contains exactly the pinned distribution versions.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def normalize(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--manifest",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/runtime-manifest.json",
    )
    parser.add_argument("--wheelhouse", type=Path, default=ROOT / "target/parity-runtime/wheelhouse")
    parser.add_argument("--downloads", type=Path, default=ROOT / "target/parity-runtime/downloads")
    parser.add_argument("--runtime", type=Path, default=ROOT / "target/parity-runtime/runtime")
    parser.add_argument("--output", type=Path, default=ROOT / "verification/windows-parity-runtime/manifest-reverify.json")
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    artifacts = [manifest["interpreter"], *manifest["wheels"], *manifest["bootstrap_tools"]]
    results = []
    failures = []
    for record in artifacts:
        if record is manifest["interpreter"]:
            path = args.downloads / record["path"]
        else:
            path = args.wheelhouse / record["path"]
        item = {
            "path": str(path),
            "expected_bytes": record["bytes"],
            "expected_sha256": record["sha256"],
        }
        if not path.is_file():
            item.update({"status": "fail", "error": "missing file"})
            failures.append(item)
        else:
            actual_bytes = path.stat().st_size
            actual_sha = sha256_file(path)
            item.update(
                {
                    "actual_bytes": actual_bytes,
                    "actual_sha256": actual_sha,
                    "status": "pass" if (actual_bytes == record["bytes"] and actual_sha == record["sha256"]) else "fail",
                }
            )
            if item["status"] == "fail":
                failures.append(item)
        results.append(item)

    lock_record = manifest["requirements_lock"]
    lock_path = ROOT / "resources/windows-parity/runtime-v2" / lock_record["path"]
    lock_ok = (
        lock_path.is_file()
        and lock_path.stat().st_size == lock_record["bytes"]
        and sha256_file(lock_path) == lock_record["sha256"]
    )
    if not lock_ok:
        failures.append({"path": str(lock_path), "status": "fail", "error": "requirements.lock mismatch"})

    license_path = ROOT / "resources/windows-parity/runtime-v2" / manifest["license_records"]["path"]
    license_record = manifest["license_records"]
    license_ok = license_path.is_file()
    license_count = 0
    if license_ok:
        payload = json.loads(license_path.read_text(encoding="utf-8"))
        license_count = len(payload["packages"])
        license_ok = license_count == license_record["package_count"]

    installed = {}
    site_packages = args.runtime / "Lib" / "site-packages"
    installed_dists = sorted(path.name[: -len(".dist-info")] for path in site_packages.glob("*.dist-info"))
    for dist in installed_dists:
        parts = dist.split("-")
        for index, part in enumerate(parts):
            if re.fullmatch(r"\d.*", part):
                name = normalize("-".join(parts[:index]))
                installed[name] = "-".join(parts[index:])
                break

    installed_checks = []
    for record in manifest["wheels"]:
        name = normalize(record["package"])
        actual = installed.get(name)
        entry = {
            "package": name,
            "expected": record["version"],
            "actual": actual,
            "status": "pass" if actual == record["version"] else "fail",
        }
        installed_checks.append(entry)
        if entry["status"] == "fail":
            failures.append(entry)

    probe_path = ROOT / manifest["runtime_probe_evidence"]
    probe = json.loads(probe_path.read_text(encoding="utf-8")) if probe_path.is_file() else {}
    evidence = {
        "schema_version": 1,
        "kind": "windows-parity-runtime-manifest-reverify",
        "runtime_id": manifest["id"],
        "status": "pass" if not failures else "fail",
        "checked_artifact_count": len(results),
        "artifact_results": results,
        "requirements_lock": {"path": str(lock_path), "status": "pass" if lock_ok else "fail"},
        "license_records": {"path": str(license_path), "packages": license_count, "status": "pass" if license_ok else "fail"},
        "installed_distribution_checks": installed_checks,
        "installed_distributions_checked": len(installed_checks),
        "probe_status": probe.get("status"),
        "failures": failures,
        "method": "Re-hashed every manifest artifact from local disk and compared against the checked-in manifest; did not trust the builder output.",
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(
        json.dumps(
            {
                "status": evidence["status"],
                "artifacts": len(results),
                "installed_checked": len(installed_checks),
                "failures": len(failures),
                "output": str(args.output),
            }
        )
    )
    return 0 if evidence["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
