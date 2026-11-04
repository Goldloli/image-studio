"""Frozen identity and v2 protocol contract for the Windows BiRefNet ONNX worker.

The Windows model-parity identities are frozen in
``openspec/changes/achieve-windows-model-parity/design.md`` section 11. The
worker serves exactly one installed model; the installer may pass
``--model-snapshot-id`` (the profile sets ``worker_model_argument``) and the
worker validates the whole generation v2 request against the frozen spec.

This module is deliberately stdlib-only so the protocol shell, probe and tests
import it without loading onnxruntime.

The desktop installer currently announces the model manifest id, while design
section 11 freezes a distinct snapshot id. Both strings name the same installed
BiRefNet release, so the worker accepts either launch id and echoes exactly the
id the host passed; the v2 request is then validated against that same identity
(probe ``identity.model_snapshot_id``, ``ReadyPaths.model_snapshot_id`` and
``ExecuteRequestV2.model_snapshot_id`` all agree). The canonical section 11
snapshot id is the default when no argument is passed.
"""

BUILD_ID = "studio-onnx-worker-windows-0.1.0"
RUNTIME_LOCK_ID = "windows-python31210-torch2100-cu126-diff041-transformers53-v2"
MODEL_ID = "birefnet-general-epoch-244-windows-v1"
MODEL_SNAPSHOT_ID = "birefnet-general-epoch-244-onnx-windows-v1"
CAPABILITY = "image.remove-background.birefnet-general-epoch-244.windows-v1"
RECIPE_ID = "birefnet-general-cpu-windows-v1"
CAPABILITIES = [CAPABILITY]

# Model-variant id (installer manifest id) and the frozen design section 11 snapshot id.
ACCEPTED_MODEL_IDS = (MODEL_SNAPSHOT_ID, MODEL_ID)


def _identity_snapshot(snapshot_id):
    if snapshot_id not in ACCEPTED_MODEL_IDS:
        raise ValueError(f"unsupported Windows BiRefNet snapshot: {snapshot_id!r}")
    return snapshot_id


def worker_identity(snapshot_id=MODEL_SNAPSHOT_ID):
    """Return the frozen worker identity for one accepted install identity."""
    return {
        "build_id": BUILD_ID,
        "runtime_lock_id": RUNTIME_LOCK_ID,
        "model_snapshot_id": _identity_snapshot(snapshot_id),
    }


def model_spec(snapshot_id=MODEL_SNAPSHOT_ID):
    """Return the frozen model/capability/recipe spec for one accepted identity."""
    _identity_snapshot(snapshot_id)
    return {
        "snapshot_id": snapshot_id,
        "model_id": MODEL_ID,
        "capability": CAPABILITY,
        "recipe": RECIPE_ID,
    }


def expected_parameters():
    """Return the exact frozen effective-parameter map (design.md section 11)."""
    return {
        "recipe": RECIPE_ID,
        "recipe_version": 1,
        "provider": "CPUExecutionProvider",
        "precision": "float32",
        "preprocessing": "birefnet-1024-rgb-v1",
        "postprocessing": "birefnet-logits-sigmoid-v1",
        "intra_op_threads": 4,
        "inter_op_threads": 1,
        "execution_mode": "sequential",
    }


__all__ = [
    "BUILD_ID",
    "RUNTIME_LOCK_ID",
    "MODEL_ID",
    "MODEL_SNAPSHOT_ID",
    "CAPABILITY",
    "CAPABILITIES",
    "RECIPE_ID",
    "ACCEPTED_MODEL_IDS",
    "worker_identity",
    "model_spec",
    "expected_parameters",
]
