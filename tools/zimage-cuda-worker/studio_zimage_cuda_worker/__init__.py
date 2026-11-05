"""Frozen identity and protocol parameters for the Windows Z-Image worker."""

BUILD_ID = "studio-zimage-cuda-worker-0.1.0"
RUNTIME_ID = "windows-python31210-torch2100-cu126-bnb0482-v1"
MODEL_ID = "z-image-turbo-diffusers-cuda-nf4-v1"
CAPABILITY = "image.text-to-image.z-image-turbo-diffusers-cuda-nf4.v1"
RECIPE_ID = "z-image-turbo-nf4-cuda-v1"
COMPACT_RECIPE_ID = "z-image-turbo-nf4-cuda-compact-v1"


def worker_identity(snapshot_id=MODEL_ID):
    if snapshot_id != MODEL_ID:
        raise ValueError("unsupported Windows Z-Image snapshot")
    return {
        "build_id": BUILD_ID,
        "runtime_lock_id": RUNTIME_ID,
        "model_snapshot_id": MODEL_ID,
    }


def model_spec(snapshot_id=MODEL_ID):
    if snapshot_id != MODEL_ID:
        raise ValueError("unsupported Windows Z-Image snapshot")
    return {
        "model_id": MODEL_ID,
        "capability": CAPABILITY,
        "recipe": RECIPE_ID,
        "steps": 9,
    }


def is_compact_recipe(recipe_id):
    return isinstance(recipe_id, str) and recipe_id == COMPACT_RECIPE_ID


def expected_parameters(seed, recipe_id=None):
    compact = is_compact_recipe(recipe_id)
    parameters = {
        "seed": seed,
        "steps": 9,
        "quantization": "nf4",
        "guidance": 0.0,
        "scheduler": "FlowMatchEulerDiscreteScheduler",
        "recipe": COMPACT_RECIPE_ID if compact else RECIPE_ID,
        "recipe_version": 1,
        "token_budget": 512,
        "truncation": False,
        "thinking": True,
        "quantized_components": ["transformer", "text_encoder"],
        "compute_dtype": "float16",
        "vae_precision": "float32",
        "attention": "_native_efficient",
        "offload": "model_cpu",
    }
    if compact:
        parameters["resolution"] = 512
    return parameters
