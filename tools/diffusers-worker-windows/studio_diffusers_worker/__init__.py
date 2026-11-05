"""Frozen Windows diffusers worker identities and per-snapshot model specs.

The Windows model-parity identities are frozen in
``openspec/changes/achieve-windows-model-parity/design.md`` section 11. A
worker process serves exactly one model snapshot; the host passes
``--model-snapshot-id`` (the installer profile sets ``worker_model_argument``)
and the worker validates the whole v2 request against the frozen spec.

This module is deliberately stdlib-only: the protocol shell, probe and tests
import it without touching torch/diffusers.
"""

BUILD_ID = "studio-diffusers-worker-windows-0.1.0"
RUNTIME_ID = "windows-python31210-torch2100-cu126-diff041-transformers53-v2"

KLEIN_SNAPSHOT = (
    "flux2-klein-4b-windows-e7b7dc27f91deacad38e78976d1f2b499d76a294"
)
QWEN_SNAPSHOT = (
    "qwen-image-2.1-windows-staged-d26bb61231c349cf6b7896fa83353113880e1ba3"
)
QWEN_TURBO_SNAPSHOT = (
    "qwen-image-2.1-turbo-windows-staged-fc13ba1986fcb905854dc8de2f52eee395f49b5b"
)
MOSS_SNAPSHOT = (
    "moss-image-sft-windows-staged-f972bd4b4b9e3852133cc5da954d87bc32cd8fb9"
)

CAP_KLEIN_TEXT = "image.text-to-image.flux2-klein-4b.windows-v1"
CAP_KLEIN_EDIT = "image.edit.flux2-klein-4b.windows-v1"
CAP_QWEN = "image.text-to-image.qwen-image-2.1.windows-v1"
CAP_QWEN_TURBO = "image.text-to-image.qwen-image-2.1-turbo.windows-v1"
CAP_MOSS = "image.text-to-image.moss-image-sft.windows-v1"

RECIPE_KLEIN_TEXT = "klein4b-text-windows-v1"
RECIPE_KLEIN_EDIT = "klein4b-edit-windows-v1"
RECIPE_QWEN = "qwen21-windows-txt2img-nf4-40-v1"
RECIPE_QWEN_TURBO = "qwen21-turbo-windows-txt2img-nf4-8-v1"
RECIPE_MOSS = "moss-image-sft-windows-txt2img-nf4-50-v1"

SNAPSHOTS: dict[str, dict] = {
    KLEIN_SNAPSHOT: {
        "kind": "klein",
        "model_id": "flux2-klein-4b-distilled-windows-v1",
        "capabilities": [CAP_KLEIN_TEXT, CAP_KLEIN_EDIT],
        "recipes": {
            CAP_KLEIN_TEXT: {"recipe_id": RECIPE_KLEIN_TEXT, "steps": 4},
            CAP_KLEIN_EDIT: {"recipe_id": RECIPE_KLEIN_EDIT, "steps": 4},
        },
    },
    QWEN_SNAPSHOT: {
        "kind": "qwen",
        "model_id": "qwen-image-2.1-windows-v1",
        "capabilities": [CAP_QWEN],
        "recipes": {
            CAP_QWEN: {"recipe_id": RECIPE_QWEN, "steps": 40},
        },
    },
    QWEN_TURBO_SNAPSHOT: {
        "kind": "qwen",
        "model_id": "qwen-image-2.1-turbo-windows-v1",
        "capabilities": [CAP_QWEN_TURBO],
        "recipes": {
            CAP_QWEN_TURBO: {"recipe_id": RECIPE_QWEN_TURBO, "steps": 8},
        },
    },
    MOSS_SNAPSHOT: {
        "kind": "moss",
        "model_id": "moss-image-sft-windows-v1",
        "capabilities": [CAP_MOSS],
        "recipes": {
            CAP_MOSS: {"recipe_id": RECIPE_MOSS, "steps": 50},
        },
    },
}


def _require_snapshot(snapshot_id):
    if not isinstance(snapshot_id, str) or snapshot_id not in SNAPSHOTS:
        raise ValueError(f"unsupported Windows model snapshot: {snapshot_id!r}")
    return SNAPSHOTS[snapshot_id]


def worker_identity(snapshot_id):
    """Return the frozen worker identity for one model snapshot."""
    _require_snapshot(snapshot_id)
    return {
        "build_id": BUILD_ID,
        "runtime_lock_id": RUNTIME_ID,
        "model_snapshot_id": snapshot_id,
    }


def model_spec(snapshot_id):
    """Return the frozen model/capability/recipe spec for one snapshot."""
    entry = _require_snapshot(snapshot_id)
    return {
        "snapshot_id": snapshot_id,
        "kind": entry["kind"],
        "model_id": entry["model_id"],
        "capabilities": list(entry["capabilities"]),
        "recipes": {
            capability: dict(recipe)
            for capability, recipe in entry["recipes"].items()
        },
    }


def recipe_for(snapshot_id, capability):
    """Return (recipe_id, steps) for one capability of a snapshot."""
    spec = model_spec(snapshot_id)
    if capability not in spec["recipes"]:
        raise ValueError(
            f"capability {capability!r} is not part of snapshot {snapshot_id!r}"
        )
    recipe = spec["recipes"][capability]
    return recipe["recipe_id"], int(recipe["steps"])


COMPACT_SUFFIX = "-compact-v1"


def is_compact_recipe(recipe_id) -> bool:
    return isinstance(recipe_id, str) and recipe_id.endswith(COMPACT_SUFFIX)


def compact_recipe_id(standard_recipe_id: str) -> str:
    return standard_recipe_id[: -len("-v1")] + COMPACT_SUFFIX


def expected_parameters(snapshot_id, capability, seed, recipe_id=None):
    """Return the exact frozen effective-parameter map for a request.

    ``recipe_id`` selects the compact (8 GB) map when it ends with ``-compact-v1``; the
    standard map is returned otherwise, including when no recipe id is supplied.
    """
    standard_recipe_id, steps = recipe_for(snapshot_id, capability)
    compact = is_compact_recipe(recipe_id)
    if compact and recipe_id != compact_recipe_id(standard_recipe_id):
        raise ValueError(f"unsupported compact recipe: {recipe_id!r}")
    recipe = recipe_id if compact else standard_recipe_id
    kind = SNAPSHOTS[snapshot_id]["kind"]
    if kind == "klein":
        parameters = {
            "recipe": recipe,
            "recipe_version": 1,
            "seed": seed,
            "steps": 4,
            "guidance": 1.0,
            "scheduler": "flow_match_euler_discrete",
            "token_budget": 512,
            "thinking": False,
            "truncation": False,
            "quantization": "nf4",
            "load_strategy": (
                "nf4-sequential-offload-v1" if compact else "nf4-component-offload-v1"
            ),
        }
        if compact:
            parameters["resolution"] = 768
            parameters["compute_dtype"] = "float16"
            parameters["vae_precision"] = "float16"
        return parameters
    if kind == "qwen":
        parameters = {
            "recipe": recipe,
            "recipe_version": 1,
            "seed": seed,
            "steps": steps,
            "guidance": 1.0,
            "sampling": "linear",
            "token_budget": 2048,
            "quantization": "nf4",
            "load_strategy": "staged-nf4-sequential-v1" if compact else "staged-nf4-v1",
        }
        if steps == 8:
            parameters["sampling_schedule"] = "qwen-turbo-model-index-v1"
        if compact:
            parameters["resolution"] = 768
            parameters["compute_dtype"] = "float16"
        return parameters
    if kind == "moss":
        parameters = {
            "recipe": recipe,
            "recipe_version": 1,
            "seed": seed,
            "steps": 50,
            "guidance": 3.0,
            "sampling": "moss_image",
            "token_budget": 4096,
            "thinking": True,
            "negative_prompt": "",
            "uncond_skip_layers": [22],
            "quantized_components": ["transformer", "text_encoder"],
            "text_encoder_precision": "nf4",
            "vae_precision": "float32",
            "load_strategy": "staged-nf4-sequential-v1" if compact else "staged-nf4-v1",
        }
        if compact:
            parameters["resolution"] = 768
            parameters["compute_dtype"] = "float16"
        return parameters
    raise ValueError(f"unsupported worker kind: {kind}")


def output_specs(snapshot_id, resolution=1024):
    """Return the frozen output specification list for one snapshot."""
    kind = SNAPSHOTS[snapshot_id]["kind"]
    if kind in ("klein", "qwen", "moss"):
        return [
            {
                "role": "primary",
                "data_type": "image_rgb",
                "width": resolution,
                "height": resolution,
                "max_bytes": 32 * 1024 * 1024,
            }
        ]
    raise ValueError(f"unsupported worker kind: {kind}")

__all__ = [
    "BUILD_ID",
    "RUNTIME_ID",
    "KLEIN_SNAPSHOT",
    "QWEN_SNAPSHOT",
    "QWEN_TURBO_SNAPSHOT",
    "MOSS_SNAPSHOT",
    "CAP_KLEIN_TEXT",
    "CAP_KLEIN_EDIT",
    "CAP_QWEN",
    "CAP_QWEN_TURBO",
    "CAP_MOSS",
    "RECIPE_KLEIN_TEXT",
    "RECIPE_KLEIN_EDIT",
    "RECIPE_QWEN",
    "RECIPE_QWEN_TURBO",
    "RECIPE_MOSS",
    "SNAPSHOTS",
    "worker_identity",
    "model_spec",
    "recipe_for",
    "expected_parameters",
    "output_specs",
]
