#!/usr/bin/env python3
"""Protocol-v2 process fixture. It performs no model inference."""

import hashlib
import json
from pathlib import Path
import sys

FAMILY = "image-studio.generation"
CAPS = [
    "image.text-to-image.flux2-klein-4b.v1",
    "image.edit.flux2-klein-4b.v1",
]
IDENTITY = {
    "build_id": "fixture-worker-v2",
    "runtime_lock_id": "fixture-runtime-v2",
    "model_snapshot_id": "fixture-model-v2",
}


def read():
    return json.loads(sys.stdin.readline())


def send(active, sequence, message):
    envelope = {
        "family": FAMILY,
        "version": 2,
        "session_id": active["session_id"],
        "request_id": active["request_id"],
        "sequence": sequence,
        "execution": active["execution"],
        "message": message,
    }
    sys.stdout.write(json.dumps(envelope, separators=(",", ":")) + "\n")
    sys.stdout.flush()


hello = read()
if (
    hello["message"]["expected_worker"] != IDENTITY
    or hello["message"]["capabilities"] != CAPS
    or hello["message"]["versions"] != {"min": 2, "max": 2}
):
    raise SystemExit(17)
accepted = {
    **hello,
    "version": 2,
    "request_id": hello["request_id"],
    "sequence": 0,
    "execution": None,
    "message": {
        "type": "hello_accepted",
        "versions": {"min": 2, "max": 2},
        "selected_version": 2,
        "identity": IDENTITY,
        "capabilities": CAPS,
    },
}
sys.stdout.write(json.dumps(accepted, separators=(",", ":")) + "\n")
sys.stdout.flush()

execute = read()
request = execute["message"]["request"]
send(
    execute,
    1,
    {
        "type": "progress",
        "stage": "sampling",
        "completed": 4,
        "total": 4,
    },
)
output = Path(request["output_dir"]) / "primary.png"
payload = b"fixture-png-bytes"
output.write_bytes(payload)
send(
    execute,
    2,
    {
        "type": "output_candidate_v2",
        "candidate": {
            "role": "primary",
            "data_type": "image_rgb",
            "relative_path": "primary.png",
            "width": 1024,
            "height": 1024,
            "size_bytes": len(payload),
            "sha256": hashlib.sha256(payload).hexdigest(),
        },
    },
)
