#!/usr/bin/env python3
"""Verify the frozen MiniMax-H3-Image macOS model and native runtime inventory."""

import hashlib
import json
import re
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parents[1]
RESOURCE = ROOT / "resources/minimax-h3-image/macos-v1"
MODEL = json.loads((RESOURCE / "model-manifest.json").read_text(encoding="utf-8"))
RUNTIME = json.loads((RESOURCE / "runtime-manifest.json").read_text(encoding="utf-8"))
WINDOWS_RESOURCE = ROOT / "resources/minimax-h3-image/windows-v1"
WINDOWS_MODEL = json.loads(
    (WINDOWS_RESOURCE / "model-manifest.json").read_text(encoding="utf-8")
)
WINDOWS_RUNTIME = json.loads(
    (WINDOWS_RESOURCE / "runtime-manifest.json").read_text(encoding="utf-8")
)


def digest(path: Path) -> str:
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


assert MODEL["id"] == "minimax-h3-image-q3ks-sdcpp-macos-v1"
assert MODEL["component_bytes"] == 23_045_953_427
assert MODEL["license"] == "minimax-h3-image-community-license-agreement"
assert sum(file["bytes"] for file in MODEL["files"]) == MODEL["component_bytes"]
assert [file["role"] for file in MODEL["files"]] == [
    "transformer",
    "text_encoder",
    "vae",
    "tokenizer",
]
assert MODEL["files"][0]["bytes"] == 8_768_254_688
assert (
    MODEL["files"][0]["sha256"]
    == "cbf7e586952da45de3fcc6bdb51a9c5fef1dbb42a9abec943c577664be090ccd"
)
assert MODEL["files"][1]["bytes"] == 13_102_161_024
assert (
    MODEL["files"][1]["sha256"]
    == "a8ccadccd57ef34c838ffb8a7da8368bb554721b2760274a1d3b0df63960b997"
)
assert MODEL["files"][2]["bytes"] == 1_168_505_312
assert (
    MODEL["files"][2]["sha256"]
    == "d80e9c2e5cfc9844869961ff5a58c9a9e52182df6bfb30b26018dd7f348721af"
)
assert MODEL["files"][3]["bytes"] == 7_032_403
assert (
    MODEL["files"][3]["sha256"]
    == "a5d85b6dcc535e6b93115a9ef287e6132fdbf30270da6218194ba742261173c7"
)

for file in MODEL["files"]:
    path = PurePosixPath(file["path"])
    assert path.parts and not path.is_absolute() and ".." not in path.parts
    assert file["bytes"] > 0
    assert re.fullmatch(r"[0-9a-f]{64}", file["sha256"])
    assert "/resolve/master/" not in file["url"]
    assert re.search(r"/resolve/[0-9a-f]{40}/", file["url"])
    assert file["source_revision"] in file["url"]

assert WINDOWS_MODEL["id"] == "minimax-h3-image-q3ks-sdcpp-windows-v1"
assert WINDOWS_MODEL["component_bytes"] == MODEL["component_bytes"]
assert WINDOWS_MODEL["files"] == MODEL["files"]

assert RUNTIME["kind"] == "native"
assert RUNTIME["id"] == "minimax-h3-sdcpp-3b12781-macos-arm64-metal-v1"
assert RUNTIME["platform"] == "macos-arm64"
assert RUNTIME["executable"] == "bin/sd-cli"
assert len(RUNTIME["archives"]) == 1
archive = RUNTIME["archives"][0]
assert archive["bytes"] == 39_644_163
assert (
    archive["sha256"]
    == "efb501ec271393e43b08d25eed37ab26c154134fa495501923a5c17417b8782a"
)
assert archive["url"].endswith("/master-1122-3b12781/" + archive["path"])
members = {member["archive_path"]: member for member in archive["members"]}
assert members["sd-cli"]["path"] == "bin/sd-cli"
assert members["sd-cli"]["executable"] is True
assert members["libstable-diffusion.dylib"]["path"] == "bin/libstable-diffusion.dylib"

assert WINDOWS_RUNTIME["kind"] == "native"
assert WINDOWS_RUNTIME["id"] == "minimax-h3-sdcpp-3b12781-windows-cuda12-v1"
assert WINDOWS_RUNTIME["platform"] == "windows-amd64"
assert WINDOWS_RUNTIME["executable"] == "bin/sd-cli.exe"
assert len(WINDOWS_RUNTIME["archives"]) == 1
windows_archive = WINDOWS_RUNTIME["archives"][0]
assert windows_archive["bytes"] == 413_082_709
assert (
    windows_archive["sha256"]
    == "34169b12181ae75e520d662bdccefd89095ba02d79c3ccb9568531afea1c3dd1"
)
assert windows_archive["url"].endswith(
    "/master-1122-3b12781/" + windows_archive["path"]
)
windows_members = {
    member["archive_path"]: member for member in windows_archive["members"]
}
assert windows_members["sd-cli.exe"]["path"] == "bin/sd-cli.exe"
assert windows_members["sd-cli.exe"]["executable"] is True
assert windows_members["stable-diffusion.dll"]["path"] == "bin/stable-diffusion.dll"
assert windows_members["ggml-cuda.dll"]["bytes"] == 382_631_424

license_files = {
    "LICENSE": (
        18_208,
        "0ec3bb5e6d9e885fa2aaf15e84689e50fd51f42b53fa24d3a64ed9d8c76d423b",
    ),
    "NOTICE": (
        386,
        "f828a0087e386efe5e1c3039bee87bff734c375ad52fb75f35110957f16de9c1",
    ),
    "LICENSE-stable-diffusion.cpp.txt": (
        1_062,
        "b53fa08f515cb5a6fff7b9fd8fcd0961a4b80df29d74372d25e1f9171aa042ee",
    ),
    "LICENSE-ggml.txt": (
        1_078,
        "94f29bbed6a22c35b992c5c6ebf0e7c92f13b836b90f36f461c9cf2f0f1d010d",
    ),
}
for name, (expected_bytes, expected_sha) in license_files.items():
    path = RESOURCE / name
    assert path.stat().st_size == expected_bytes
    assert digest(path) == expected_sha

print(
    json.dumps(
        {
            "model_components": len(MODEL["files"]),
            "component_bytes": MODEL["component_bytes"],
            "runtime_members": len(archive["members"]),
            "runtime_id": RUNTIME["id"],
            "windows_runtime_members": len(windows_archive["members"]),
            "windows_runtime_id": WINDOWS_RUNTIME["id"],
            "licenses": sorted(license_files),
            "network_accessed": False,
        },
        indent=2,
    )
)
