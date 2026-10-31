#!/usr/bin/env python3
"""Wrap already-built binaries for local macOS UI verification; no signing or installation."""
import argparse
from pathlib import Path
import plistlib
import shutil
import sys

root = Path(__file__).resolve().parent.parent
if sys.platform != "darwin":
    raise SystemExit("This helper only prepares a local macOS development bundle.")
parser = argparse.ArgumentParser()
parser.add_argument("--name", default="Image Studio")
parser.add_argument("--identifier", default="local.image-studio.foundation.dev")
parser.add_argument(
    "--output",
    type=Path,
    default=root / "target/dev/Image Studio.app",
)
args = parser.parse_args()
bundle = args.output.resolve()
executables = bundle / "Contents/MacOS"
resources = bundle / "Contents/Resources"
executables.mkdir(parents=True, exist_ok=True)
resources.mkdir(parents=True, exist_ok=True)
for name in ["image-studio-desktop", "diagnostic-worker", "inference-supervisor"]:
    source = root / "target/debug" / name
    if not source.is_file():
        raise SystemExit(f"Missing {source}; run cargo build --workspace --locked first.")
    shutil.copy2(source, executables / name)
shutil.copy2(root / "apps/desktop/icons/icon.icns", resources / "AppIcon.icns")
metadata = {
    "CFBundleName": args.name,
    "CFBundleDisplayName": args.name,
    "CFBundleExecutable": "image-studio-desktop",
    "CFBundleIdentifier": args.identifier,
    "CFBundleVersion": "0.1.0",
    "CFBundleShortVersionString": "0.1.0",
    "CFBundleIconFile": "AppIcon",
    "CFBundleIconName": "AppIcon",
    "CFBundlePackageType": "APPL",
    "NSHighResolutionCapable": True,
}
with (bundle / "Contents/Info.plist").open("wb") as output:
    plistlib.dump(metadata, output)
print(bundle)
