#!/usr/bin/env python3
"""Wrap already-built binaries for local macOS UI verification; no signing or installation."""
import argparse
import json
import os
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

LEGAL_RESOURCES = [
    ("LICENSE", "LICENSE"),
    ("NOTICE", "NOTICE"),
    ("THIRD_PARTY_NOTICES.md", "THIRD_PARTY_NOTICES.md"),
    ("resources/fonts/OFL.txt", "OFL.txt"),
    ("resources/prompt-library/LICENSE", "PROMPT-LIBRARY-LICENSE.txt"),
]


def workspace_version() -> str:
    config = json.loads((ROOT / "apps/desktop/tauri.conf.json").read_text(encoding="utf-8"))
    return str(config["version"])


def bundle_identifier() -> str:
    config = json.loads((ROOT / "apps/desktop/tauri.conf.json").read_text(encoding="utf-8"))
    return str(config["identifier"])


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--name", default="Image Studio")
    parser.add_argument(
        "--identifier",
        default=None,
        help="Override the bundle identifier from apps/desktop/tauri.conf.json.",
    )
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
    if bundle.suffix != ".app":
        raise SystemExit(f"Output must end with .app: {bundle}")
    built = ROOT / "target" / args.profile
    binary_sources = []
    for name in BINARIES:
        source = built / name
        if not source.is_file():
            raise SystemExit(f"Missing {source}; run cargo build --profile {args.profile} --workspace --locked first.")
        binary_sources.append((source, name))
    resource_sources = [
        (ROOT / "apps/desktop/icons/icon.icns", "AppIcon.icns"),
        *[(ROOT / source, destination) for source, destination in LEGAL_RESOURCES],
    ]
    for source, _ in resource_sources:
        if not source.is_file():
            raise SystemExit(f"Missing application resource: {source}")

    staging = bundle.with_name(f".{bundle.name}.staging-{os.getpid()}")
    if staging.exists():
        shutil.rmtree(staging)
    executables = staging / "Contents/MacOS"
    resources = staging / "Contents/Resources"
    executables.mkdir(parents=True, exist_ok=True)
    resources.mkdir(parents=True, exist_ok=True)
    for source, name in binary_sources:
        shutil.copy2(source, executables / name)
    for source, name in resource_sources:
        shutil.copy2(source, resources / name)
    version = workspace_version()
    metadata = {
        "CFBundleName": args.name,
        "CFBundleDisplayName": args.name,
        "CFBundleExecutable": "image-studio-desktop",
        "CFBundleIdentifier": args.identifier or bundle_identifier(),
        "CFBundleVersion": version,
        "CFBundleShortVersionString": version,
        "CFBundleIconFile": "AppIcon",
        "CFBundleIconName": "AppIcon",
        "CFBundlePackageType": "APPL",
        "CFBundleDevelopmentRegion": "zh_CN",
        "LSApplicationCategoryType": "public.app-category.graphics-design",
        "LSMinimumSystemVersion": "11.0",
        "NSHumanReadableCopyright": "Copyright 2026 Goldloli. Licensed under Apache-2.0.",
        "NSHighResolutionCapable": True,
        "NSPrincipalClass": "NSApplication",
    }
    with (staging / "Contents/Info.plist").open("wb") as output:
        plistlib.dump(metadata, output)
    if bundle.exists():
        shutil.rmtree(bundle)
    staging.rename(bundle)
    print(bundle)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
