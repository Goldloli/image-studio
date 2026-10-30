#!/usr/bin/env python3
"""Wrap already-built binaries for local macOS UI verification; no signing or installation."""
from pathlib import Path
import plistlib
import shutil
import sys

root = Path(__file__).resolve().parent.parent
if sys.platform != "darwin":
    raise SystemExit("This helper only prepares a local macOS development bundle.")
bundle = root / "target/dev/Image Studio.app"
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
    "CFBundleName": "Image Studio",
    "CFBundleDisplayName": "Image Studio",
    "CFBundleExecutable": "image-studio-desktop",
    "CFBundleIdentifier": "local.image-studio.foundation.dev",
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
