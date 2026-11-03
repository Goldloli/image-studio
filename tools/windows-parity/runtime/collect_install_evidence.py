"""Collect the offline-install evidence for the frozen Windows v2 runtime.

Facts are read from the real install logs/timings and re-measured on disk; the
script fails if the offline install log shows network activity or a non-zero
exit code, or if the runtime probe did not pass.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import shutil
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def directory_bytes(path: Path) -> int:
    return sum(item.stat().st_size for item in path.rglob("*") if item.is_file())


def read_text_auto(path: Path) -> str:
    """Read a text file regardless of the UTF-8/UTF-16 BOM left by shell redirects."""
    data = path.read_bytes()
    if data[:2] == b"\xff\xfe":
        return data.decode("utf-16-le").lstrip("\ufeff")
    if data[:2] == b"\xfe\xff":
        return data.decode("utf-16-be").lstrip("\ufeff")
    if data[:3] == b"\xef\xbb\xbf":
        return data[3:].decode("utf-8", "replace")
    return data.decode("utf-8", "replace")


def read_ram_bytes() -> int | None:
    try:
        import ctypes

        class MemoryStatus(ctypes.Structure):
            _fields_ = [
                ("dwLength", ctypes.c_ulong),
                ("dwMemoryLoad", ctypes.c_ulong),
                ("ullTotalPhys", ctypes.c_ulonglong),
                ("ullAvailPhys", ctypes.c_ulonglong),
                ("ullTotalPageFile", ctypes.c_ulonglong),
                ("ullAvailPageFile", ctypes.c_ulonglong),
                ("ullTotalVirtual", ctypes.c_ulonglong),
                ("ullAvailVirtual", ctypes.c_ulonglong),
                ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
            ]

        status = MemoryStatus()
        status.dwLength = ctypes.sizeof(MemoryStatus)
        if ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
            return int(status.ullTotalPhys)
    except (OSError, AttributeError):
        return None
    return None


def parse_timing(path: Path) -> dict[str, object]:
    text = read_text_auto(path)
    result: dict[str, object] = {"raw": text.strip()}
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("START "):
            result["started_at"] = line[len("START ") :]
        elif line.startswith("END   "):
            result["finished_at"] = line[len("END   ") :]
        elif line.startswith("EXITCODE "):
            result["exit_code"] = int(line[len("EXITCODE ") :])
        elif line.startswith("ELAPSED_SECONDS "):
            result["elapsed_seconds"] = float(line[len("ELAPSED_SECONDS ") :])
    return result


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=ROOT / "verification/windows-parity-runtime/install-evidence.json")
    parser.add_argument("--runtime", type=Path, default=ROOT / "target/parity-runtime/runtime")
    parser.add_argument("--wheelhouse", type=Path, default=ROOT / "target/parity-runtime/wheelhouse")
    parser.add_argument("--downloads", type=Path, default=ROOT / "target/parity-runtime/downloads")
    parser.add_argument("--install-log", type=Path, default=ROOT / "target/parity-runtime/offline-install.log")
    parser.add_argument("--timing", type=Path, default=ROOT / "target/parity-runtime/offline-install-timing.txt")
    parser.add_argument("--resolve-log", type=Path, default=ROOT / "target/parity-runtime/pip-resolve.log")
    parser.add_argument("--resolve-report", type=Path, default=ROOT / "target/parity-runtime/pip-resolve-report.json")
    parser.add_argument("--probe", type=Path, default=ROOT / "verification/windows-parity-runtime/runtime-probe.json")
    parser.add_argument(
        "--manifest",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/runtime-manifest.json",
    )
    parser.add_argument(
        "--lock",
        type=Path,
        default=ROOT / "resources/windows-parity/runtime-v2/requirements.lock",
    )
    args = parser.parse_args()

    timing = parse_timing(args.timing)
    install_log = read_text_auto(args.install_log)
    resolve_log = read_text_auto(args.resolve_log)
    report = json.loads(args.resolve_report.read_text(encoding="utf-8"))
    probe = json.loads(args.probe.read_text(encoding="utf-8"))
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    lock_text = args.lock.read_text(encoding="utf-8")

    download_lines = [line for line in install_log.splitlines() if line.strip().startswith("Downloading ")]
    http_lines = [line for line in install_log.splitlines() if "http://" in line or "https://" in line]
    lookup_lines = [line for line in install_log.splitlines() if "Looking in links" in line]
    no_index_lines = [line for line in install_log.splitlines() if "--no-index" in line or "Looking in indexes" in line]
    warnings = [line.strip() for line in install_log.splitlines() if line.strip().startswith(("WARNING:", "ERROR:"))]

    installed_dists = []
    site_packages = args.runtime / "Lib" / "site-packages"
    for dist_info in sorted(site_packages.glob("*.dist-info")):
        installed_dists.append(dist_info.name[: -len(".dist-info")])

    report_pip = report.get("pip_version")
    disk_usage = shutil.disk_usage(ROOT)
    space = {
        "wheelhouse": {
            "path": "target/parity-runtime/wheelhouse",
            "files": sum(1 for _ in args.wheelhouse.glob("*.whl")),
            "bytes": directory_bytes(args.wheelhouse),
        },
        "downloads": {"path": "target/parity-runtime/downloads", "bytes": directory_bytes(args.downloads)},
        "installed_runtime": {
            "path": "target/parity-runtime/runtime",
            "bytes": directory_bytes(args.runtime),
            "files": sum(1 for item in args.runtime.rglob("*") if item.is_file()),
        },
        "disk": {"total": disk_usage.total, "used": disk_usage.used, "free": disk_usage.free},
    }

    offline_checks = {
        "install_uses_no_index": any("Looking in links" in line for line in lookup_lines)
        and not any("Looking in indexes" in line for line in no_index_lines),
        "zero_download_lines_in_install_log": len(download_lines) == 0,
        "zero_http_urls_in_install_log": len(http_lines) == 0,
        "exit_code_zero": timing.get("exit_code") == 0,
        "probe_passed": probe.get("status") == "pass",
    }
    status = "pass" if all(offline_checks.values()) else "fail"

    evidence = {
        "schema_version": 1,
        "kind": "windows-parity-runtime-install-evidence",
        "case_id": "1.2",
        "runtime_id": manifest["id"],
        "status": status,
        "started_at": timing.get("started_at"),
        "finished_at": timing.get("finished_at"),
        "install_elapsed_seconds": timing.get("elapsed_seconds"),
        "host": {
            "hostname": platform.node(),
            "os": f"{platform.system()} {platform.release()} {platform.version()}",
            "python_used_for_resolution": platform.python_version(),
            "python_used_for_probe": probe.get("platform", {}).get("python_version"),
            "cpu": platform.processor(),
            "cpu_count": os.cpu_count(),
            "ram_bytes": read_ram_bytes(),
        },
        "commands": {
            "resolve": (
                "python -m pip install --dry-run --ignore-installed --report target/parity-runtime/pip-resolve-report.json "
                "--target target/parity-runtime/_resolve_probe -r target/parity-runtime/top-level-requirements.txt "
                "--only-binary=:all: --platform win_amd64 --implementation cp --python-version 3.12 --abi cp312 "
                "--index-url https://pypi.org/simple --extra-index-url https://download.pytorch.org/whl/cu126"
            ),
            "download": (
                "python -m pip download --no-deps -r resources/windows-parity/runtime-v2/requirements.lock "
                "--only-binary=:all: --platform win_amd64 --implementation cp --python-version 3.12 --abi cp312 "
                "-d target/parity-runtime/wheelhouse (PyPI for all but torch/torchvision; "
                "https://download.pytorch.org/whl/cu126 for torch/torchvision)"
            ),
            "offline_install": (
                "target/parity-runtime/runtime/python.exe -I -m pip install --no-index "
                "--find-links target/parity-runtime/wheelhouse -r resources/windows-parity/runtime-v2/requirements.lock "
                "--only-binary=:all: --no-cache-dir --disable-pip-version-check -v"
            ),
            "probe": (
                "target/parity-runtime/runtime/python.exe -I tools/windows-parity/runtime/probe_runtime_v2.py "
                "--output verification/windows-parity-runtime/runtime-probe.json"
            ),
        },
        "offline_checks": offline_checks,
        "install": {
            "exit_code": timing.get("exit_code"),
            "log": {
                "path": "target/parity-runtime/offline-install.log",
                "bytes": args.install_log.stat().st_size,
                "sha256": sha256_file(args.install_log),
            },
            "download_lines": download_lines[:5],
            "http_url_lines": http_lines[:5],
            "lookup_lines": lookup_lines[:5],
            "warnings": warnings[:20],
        },
        "resolution": {
            "pip_version": report_pip,
            "resolved_packages": len(report.get("install", [])),
            "resolve_log": {
                "path": "target/parity-runtime/pip-resolve.log",
                "bytes": args.resolve_log.stat().st_size,
                "sha256": sha256_file(args.resolve_log),
            },
            "indexes": ["https://pypi.org/simple", "https://download.pytorch.org/whl/cu126"],
        },
        "space": space,
        "locked_packages": len([line for line in lock_text.splitlines() if line and not line.startswith("#")]),
        "installed_distributions": len(installed_dists),
        "probe": {
            "path": "verification/windows-parity-runtime/runtime-probe.json",
            "sha256": sha256_file(args.probe),
            "status": probe.get("status"),
            "checks": [
                {
                    "check": item["check"],
                    "status": item["status"],
                    **({"imports": item.get("imports")} if item.get("imports") else {}),
                    **({"result": item.get("result")} if item.get("result") else {}),
                }
                for item in probe.get("checks", [])
            ],
        },
        "artifacts": {
            "runtime_manifest": {
                "path": "resources/windows-parity/runtime-v2/runtime-manifest.json",
                "sha256": sha256_file(args.manifest),
            },
            "requirements_lock": {
                "path": "resources/windows-parity/runtime-v2/requirements.lock",
                "sha256": sha256_file(args.lock),
            },
        },
        "onnxruntime_choice": {
            "version": "1.31.1",
            "wheel": "onnxruntime-1.31.1-cp312-cp312-win_amd64.whl",
            "reason": (
                "Latest stable onnxruntime release at freeze time with an official cp312 win_amd64 wheel; "
                "the CPU package is used only through the ONNX Runtime CPUExecutionProvider for BiRefNet, "
                "so GPU/other execution providers are not part of this runtime closure."
            ),
            "provider_used": "CPUExecutionProvider (planned BiRefNet worker)",
            "wheel_sha256": next(
                (item["sha256"] for item in manifest["wheels"] if item["package"] == "onnxruntime"),
                None,
            ),
        },
        "generated_at": datetime.now(timezone.utc).isoformat(),
        "limitations": [
            "The probe imports libraries and queries CUDA at driver level; it does not run model inference.",
            "The offline proof covers the install step: pip ran with --no-index and only the local wheelhouse.",
            "Python 3.12.10 is the last 3.12 release with official Windows binaries; later 3.12 releases are source-only.",
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    output = json.dumps(evidence, ensure_ascii=False, indent=2) + "\n"
    args.output.write_text(output, encoding="utf-8", newline="\n")
    print(json.dumps({"output": str(args.output), "status": status, "checks": offline_checks}))
    return 0 if status == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
