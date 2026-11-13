#!/usr/bin/env python3
"""Verify a macOS DMG and the Image Studio application bundle it contains."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import platform
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parent.parent
BINARIES = (
    "image-studio-desktop",
    "diagnostic-worker",
    "inference-supervisor",
    "realesrgan-worker",
)
RESOURCES = (
    "LICENSE",
    "NOTICE",
    "THIRD_PARTY_NOTICES.md",
    "OFL.txt",
    "PROMPT-LIBRARY-LICENSE.txt",
)


def run(command: list[str], *, capture: bool = False) -> str:
    result = subprocess.run(
        command,
        check=True,
        text=True,
        capture_output=capture,
    )
    return result.stdout if capture else ""


def try_detach(mountpoint: Path) -> bool:
    result = subprocess.run(
        ["hdiutil", "detach", str(mountpoint)],
        text=True,
        capture_output=True,
    )
    if result.returncode == 0:
        return True
    result = subprocess.run(
        ["hdiutil", "detach", "-force", str(mountpoint)],
        text=True,
        capture_output=True,
    )
    return result.returncode == 0


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_config() -> dict:
    return json.loads(
        (ROOT / "apps/desktop/tauri.conf.json").read_text(encoding="utf-8")
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dmg", type=Path, required=True)
    parser.add_argument("--app-name", default=None)
    parser.add_argument("--expected-version", default=None)
    parser.add_argument("--expected-identifier", default=None)
    args = parser.parse_args()

    if sys.platform != "darwin":
        raise SystemExit("DMG verification is only supported on macOS.")
    if platform.machine() != "arm64":
        raise SystemExit(f"Expected an arm64 macOS runner, got {platform.machine()}")

    dmg = args.dmg.resolve()
    if not dmg.is_file() or dmg.suffix != ".dmg":
        raise SystemExit(f"Missing DMG: {dmg}")

    config = read_config()
    app_name = args.app_name or str(config["productName"])
    expected_version = args.expected_version or str(config["version"])
    expected_identifier = args.expected_identifier or str(config["identifier"])

    run(["hdiutil", "verify", str(dmg)])

    mount_parent = Path(tempfile.mkdtemp(prefix="image-studio-dmg-check-"))
    mountpoint = mount_parent / "volume"
    mountpoint.mkdir()
    attached = False
    detach_error = None
    try:
        run(
            [
                "hdiutil",
                "attach",
                "-nobrowse",
                "-readonly",
                "-mountpoint",
                str(mountpoint),
                str(dmg),
            ]
        )
        attached = True
        app = mountpoint / f"{app_name}.app"
        if not app.is_dir():
            raise SystemExit(f"Application bundle missing from DMG: {app}")
        applications_link = mountpoint / "Applications"
        if not applications_link.is_symlink():
            raise SystemExit("DMG is missing the Applications shortcut")
        if os.readlink(applications_link) != "/Applications":
            raise SystemExit(
                f"Unexpected Applications link target: {os.readlink(applications_link)}"
            )

        run(["plutil", "-lint", str(app / "Contents/Info.plist")])
        with (app / "Contents/Info.plist").open("rb") as source:
            metadata = plistlib.load(source)
        expected = {
            "CFBundleIdentifier": expected_identifier,
            "CFBundleShortVersionString": expected_version,
            "CFBundleVersion": expected_version,
            "LSMinimumSystemVersion": "11.0",
        }
        for key, value in expected.items():
            if metadata.get(key) != value:
                raise SystemExit(
                    f"Info.plist {key} mismatch: {metadata.get(key)!r} != {value!r}"
                )

        architectures = {}
        for binary in BINARIES:
            path = app / "Contents/MacOS" / binary
            if not path.is_file():
                raise SystemExit(f"Missing executable: {path}")
            description = run(["file", str(path)], capture=True)
            if "arm64" not in description:
                raise SystemExit(f"Executable is not arm64: {path}: {description.strip()}")
            architectures[binary] = "arm64"

        for resource in RESOURCES:
            path = app / "Contents/Resources" / resource
            if not path.is_file():
                raise SystemExit(f"Missing resource: {path}")

        unexpected_zips = sorted(
            path.name for path in mountpoint.iterdir() if path.suffix == ".zip"
        )
        if unexpected_zips:
            raise SystemExit(f"DMG unexpectedly contains zip entries: {unexpected_zips}")

        result = {
            "dmg": str(dmg),
            "bytes": dmg.stat().st_size,
            "sha256": sha256(dmg),
            "app": app_name,
            "identifier": metadata["CFBundleIdentifier"],
            "version": metadata["CFBundleShortVersionString"],
            "minimum_macos": metadata["LSMinimumSystemVersion"],
            "applications_link": os.readlink(applications_link),
            "architectures": architectures,
            "resources": list(RESOURCES),
            "unexpected_zips": [],
        }
        print(json.dumps(result, ensure_ascii=False, indent=2))
        return 0
    finally:
        if attached and not try_detach(mountpoint):
            detach_error = f"Failed to detach DMG mountpoint: {mountpoint}"
        if detach_error is None:
            shutil.rmtree(mount_parent, ignore_errors=True)
        elif sys.exc_info()[0] is None:
            raise SystemExit(detach_error)


if __name__ == "__main__":
    raise SystemExit(main())
