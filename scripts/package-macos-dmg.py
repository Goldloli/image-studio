#!/usr/bin/env python3
"""Create a drag-to-install macOS disk image from an existing application bundle."""
import argparse
import os
from pathlib import Path
import subprocess
import sys
import tempfile


def run(command: list[str]) -> None:
    subprocess.run(command, check=True)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--app", type=Path, required=True, help="Existing .app bundle")
    parser.add_argument("--output", type=Path, required=True, help="Destination .dmg")
    parser.add_argument("--volume-name", default="Image Studio")
    args = parser.parse_args()

    if sys.platform != "darwin":
        raise SystemExit("DMG packaging is only supported on macOS.")

    app = args.app.resolve()
    output = args.output.resolve()
    if app.suffix != ".app" or not app.is_dir():
        raise SystemExit(f"Input must be an existing .app bundle: {app}")
    if output.suffix != ".dmg":
        raise SystemExit(f"Output must end with .dmg: {output}")
    output.parent.mkdir(parents=True, exist_ok=True)

    try:
        with tempfile.TemporaryDirectory(prefix=".image-studio-dmg-", dir=output.parent) as root:
            stage = Path(root) / "volume"
            stage.mkdir()
            run(
                [
                    "ditto",
                    "--norsrc",
                    "--noextattr",
                    str(app),
                    str(stage / app.name),
                ]
            )
            os.symlink("/Applications", stage / "Applications")
            output.unlink(missing_ok=True)
            run(
                [
                    "hdiutil",
                    "create",
                    "-volname",
                    args.volume_name,
                    "-srcfolder",
                    str(stage),
                    "-ov",
                    "-format",
                    "UDZO",
                    str(output),
                ]
            )
            run(["hdiutil", "verify", str(output)])
    except BaseException:
        output.unlink(missing_ok=True)
        raise

    print(output)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
