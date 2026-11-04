#!/usr/bin/env python3
"""Wrap already-built binaries for local macOS UI verification; no signing or installation."""
import argparse
import json
from pathlib import Path
import plistlib
import shutil
import sys

ROOT = Path(__file__).resolve().parent.parent

BINARIES = [
    "image-studio-desktop",
    "diagnostic-worker",
    "inference-supervisor",
    "realesrgan-worker",
]


def workspace_version() -> str:
    config = json.loads((ROOT / "apps/desktop/tauri.conf.json").read_text(encoding="utf-8"))
    return str(config["version"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", default="Image Studio")
    parser.add_argument("--identifier", default="local.image-studio.foundation.dev")
    parser.add_argument(
        "--profile",
        choices=("debug", "release"),
        default="debug",
        help="Cargo profile the binaries were built with (default: debug).",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=ROOT / "target/dev/Image Studio.app",
    )
    args = parser.parse_args()
    if sys.platform != "darwin":
        raise SystemExit("This helper only prepares a macOS application bundle.")
    bundle = args.output.resolve()
    executables = bundle / "Contents/MacOS"
    resources = bundle / "Contents/Resources"
    executables.mkdir(parents=True, exist_ok=True)
    resources.mkdir(parents=True, exist_ok=True)
    built = ROOT / "target" / args.profile
    for name in BINARIES:
        source = built / name
        if not source.is_file():
            raise SystemExit(f"Missing {source}; run cargo build --profile {args.profile} --workspace --locked first.")
        shutil.copy2(source, executables / name)
    shutil.copy2(ROOT / "apps/desktop/icons/icon.icns", resources / "AppIcon.icns")
    version = workspace_version()
    metadata = {
        "CFBundleName": args.name,
        "CFBundleDisplayName": args.name,
        "CFBundleExecutable": "image-studio-desktop",
        "CFBundleIdentifier": args.identifier,
        "CFBundleVersion": version,
        "CFBundleShortVersionString": version,
        "CFBundleIconFile": "AppIcon",
        "CFBundleIconName": "AppIcon",
        "CFBundlePackageType": "APPL",
        "NSHighResolutionCapable": True,
    }
    with (bundle / "Contents/Info.plist").open("wb") as output:
        plistlib.dump(metadata, output)
    print(bundle)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
