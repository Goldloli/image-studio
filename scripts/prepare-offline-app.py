#!/usr/bin/env python3
"""Prepare a local GUI app wrapper that runs the existing app under a sandbox profile."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import plistlib
import shutil
import subprocess


ROOT = Path(__file__).resolve().parent.parent


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        default=str(ROOT / "target/dev/Image Studio.app"),
    )
    parser.add_argument(
        "--destination",
        default=str(ROOT / "target/model-usability-offline/Image Studio Offline.app"),
    )
    parser.add_argument(
        "--sandbox-profile",
        default=str(ROOT / "verification/deny-network.sb"),
    )
    parser.add_argument(
        "--inject-source",
        default=str(ROOT / "tools/offline-sandbox-inject.c"),
    )
    parser.add_argument(
        "--bundle-id",
        default="local.image-studio.offline.dev",
        help="bundle identifier exposed to LaunchServices",
    )
    args = parser.parse_args()

    source = Path(args.source).expanduser().resolve()
    destination = Path(args.destination).expanduser().resolve()
    profile = Path(args.sandbox_profile).expanduser().resolve()
    inject_source = Path(args.inject_source).expanduser().resolve()
    if destination.exists():
        raise SystemExit(f"destination already exists: {destination}")
    if (
        not (source / "Contents/Info.plist").is_file()
        or not profile.is_file()
        or not inject_source.is_file()
    ):
        raise SystemExit("source app bundle, sandbox profile or inject source is missing")
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copytree(source, destination, symlinks=True)

    info_path = destination / "Contents/Info.plist"
    with info_path.open("rb") as stream:
        info = plistlib.load(stream)
    executable_name = info["CFBundleExecutable"]
    executable = destination / "Contents/MacOS" / executable_name
    if not executable.is_file():
        raise SystemExit("source app executable is missing")
    inject_library = destination / "Contents/MacOS/offline-sandbox.dylib"
    profile_header = destination / "Contents/MacOS/offline-profile.h"
    profile_header.write_text(
        "#pragma once\n"
        f"static const char sandbox_profile[] = {json.dumps(profile.read_text(), ensure_ascii=True)};\n",
        encoding="utf-8",
    )

    compile_result = subprocess.run(
        [
            "/usr/bin/xcrun",
            "clang",
            "-O2",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-Wno-deprecated-declarations",
            "-include",
            str(profile_header),
            "-dynamiclib",
            str(inject_source),
            "-o",
            str(inject_library),
            "-lsandbox",
        ],
        cwd=ROOT,
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        check=False,
    )
    if compile_result.returncode != 0:
        raise SystemExit(
            "cannot compile offline sandbox injection library: "
            + (compile_result.stderr.strip() or compile_result.stdout.strip())
        )
    info["CFBundleName"] = "Image Studio Offline"
    info["CFBundleDisplayName"] = "Image Studio Offline"
    info["CFBundleIdentifier"] = args.bundle_id
    info["LSEnvironment"] = {
        "DYLD_INSERT_LIBRARIES": str(inject_library),
    }
    with info_path.open("wb") as stream:
        plistlib.dump(info, stream)
    sign_result = subprocess.run(
        ["/usr/bin/codesign", "--force", "--deep", "--sign", "-", str(destination)],
        stdin=subprocess.DEVNULL,
        capture_output=True,
        text=True,
        check=False,
    )
    if sign_result.returncode != 0:
        raise SystemExit(
            "cannot ad-hoc sign offline app: "
            + (sign_result.stderr.strip() or sign_result.stdout.strip())
        )

    receipt = {
        "schema_version": 1,
        "source": str(source),
        "destination": str(destination),
        "sandbox_profile": str(profile),
        "app_executable": str(executable),
        "injection_library": str(inject_library),
        "bundle_identifier": info["CFBundleIdentifier"],
        "network_policy": "constructor applies network-denial profile before app startup and descendants inherit it",
    }
    receipt_path = destination.parent / "offline-app-receipt.json"
    receipt_path.write_text(json.dumps(receipt, ensure_ascii=False, indent=2) + "\n")
    print(receipt_path)
    print(destination)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
