#!/usr/bin/env python3
"""Verify an installed Klein snapshot against the embedded pinned inventories."""

import argparse
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parent.parent
KLEIN = ROOT / "resources/planned-models/klein"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(8 * 1024 * 1024):
            digest.update(chunk)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--data-root", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    data = args.data_root.resolve()
    model = json.loads((KLEIN / "model-manifest.json").read_text())
    runtime = json.loads((KLEIN / "runtime-manifest.json").read_text())
    activation_path = data / "installations/models/flux2-klein-4b-distilled.json"
    activation = json.loads(activation_path.read_text())
    assert activation["model_snapshot_id"] == model["id"]
    assert activation["runtime_lock_id"] == runtime["id"]
    assert activation["probe"]["identity"]["build_id"] == "studio-mflux-worker-0.2.0"
    assert activation["probe"]["identity"]["runtime_lock_id"] == runtime["id"]
    assert activation["probe"]["identity"]["model_snapshot_id"] == model["id"]
    assert activation["probe"]["device"] == "gpu"
    assert activation["probe"]["device_name"]

    files = []
    model_root = data / "models" / model["id"]
    for expected in model["files"]:
        path = model_root / expected["path"]
        actual_bytes = path.stat().st_size
        actual_sha256 = sha256(path)
        assert actual_bytes == expected["bytes"], expected["path"]
        assert actual_sha256 == expected["sha256"], expected["path"]
        files.append(
            {
                "path": expected["path"],
                "bytes": actual_bytes,
                "sha256": actual_sha256,
            }
        )

    report = {
        "schemaVersion": 1,
        "configurationKey": "K-T/K-E",
        "evidenceKind": "device_probe_and_inventory",
        "status": "passed",
        "modelId": "flux2-klein-4b-distilled",
        "modelSnapshotId": model["id"],
        "runtimeLockId": runtime["id"],
        "workerBuildId": activation["probe"]["identity"]["build_id"],
        "device": activation["probe"]["device_name"],
        "python": activation["probe"]["python"],
        "componentFiles": len(files),
        "componentBytes": sum(file["bytes"] for file in files),
        "files": files,
        "activationPath": str(activation_path),
    }
    encoded = json.dumps(report, indent=2, ensure_ascii=False) + "\n"
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.output.with_suffix(args.output.suffix + ".tmp")
        temporary.write_text(encoded)
        temporary.replace(args.output)
    print(encoded, end="")


if __name__ == "__main__":
    main()
