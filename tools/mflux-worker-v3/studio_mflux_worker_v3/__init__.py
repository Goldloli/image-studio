"""Managed, offline MFLUX worker for Qwen-Image 2.1 and MOSS-Image-SFT."""

BUILD_ID = "studio-mflux-worker-0.3.0"
RUNTIME_LOCK_ID = "mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3"

QWEN_BASE_SNAPSHOT_ID = (
    "qwen-image-2.1-d26bb61231c349cf6b7896fa83353113880e1ba3"
)
QWEN_TURBO_SNAPSHOT_ID = (
    "qwen-image-2.1-turbo-fc13ba1986fcb905854dc8de2f52eee395f49b5b"
)
MOSS_SNAPSHOT_ID = (
    "moss-image-sft-f972bd4b4b9e3852133cc5da954d87bc32cd8fb9"
)

QWEN_BASE_MODEL_ID = "qwen-image-2.1-mflux-v1"
QWEN_TURBO_MODEL_ID = "qwen-image-2.1-turbo-mflux-v1"
MOSS_MODEL_ID = "moss-image-sft-mflux-v1"

QWEN_BASE_CAPABILITY = "image.text-to-image.qwen-image-2.1.v1"
QWEN_TURBO_CAPABILITY = "image.text-to-image.qwen-image-2.1-turbo.v1"
MOSS_CAPABILITY = "image.text-to-image.moss-image-sft.v1"

MODEL_SPECS = {
    QWEN_BASE_SNAPSHOT_ID: dict(
        model_id=QWEN_BASE_MODEL_ID,
        capability=QWEN_BASE_CAPABILITY,
        recipe="qwen21-txt2img-q8-40-v1",
        family="qwen21",
        steps=40,
        guidance=1.0,
        scheduler="linear",
        token_budget=2048,
        quantized_components=("transformer",),
        text_encoder_precision="bfloat16",
        vae_precision="float32",
    ),
    QWEN_TURBO_SNAPSHOT_ID: dict(
        model_id=QWEN_TURBO_MODEL_ID,
        capability=QWEN_TURBO_CAPABILITY,
        recipe="qwen21-turbo-txt2img-q8-8-v1",
        family="qwen21-turbo",
        steps=8,
        guidance=1.0,
        scheduler="linear",
        token_budget=2048,
        quantized_components=("transformer",),
        text_encoder_precision="bfloat16",
        vae_precision="float32",
        sampling_schedule="qwen-turbo-model-index-v1",
    ),
    MOSS_SNAPSHOT_ID: dict(
        model_id=MOSS_MODEL_ID,
        capability=MOSS_CAPABILITY,
        recipe="moss-image-sft-txt2img-q8-50-v1",
        family="moss-image",
        steps=50,
        guidance=3.0,
        scheduler="moss_image",
        token_budget=4096,
        quantized_components=("transformer", "text_encoder"),
        text_encoder_precision="q8",
        vae_precision="bfloat16",
        thinking=True,
        negative_prompt="",
        uncond_skip_layers=(22,),
    ),
}


def model_spec(snapshot_id):
    try:
        return MODEL_SPECS[snapshot_id]
    except KeyError as exc:
        raise ValueError(f"unsupported model snapshot: {snapshot_id}") from exc


def identity(snapshot_id):
    model_spec(snapshot_id)
    return dict(
        build_id=BUILD_ID,
        runtime_lock_id=RUNTIME_LOCK_ID,
        model_snapshot_id=snapshot_id,
    )


def expected_parameters(snapshot_id, seed):
    spec = model_spec(snapshot_id)
    result = {
        "seed": seed,
        "steps": spec["steps"],
        "quantization": 8,
        "guidance": spec["guidance"],
        "scheduler": spec["scheduler"],
        "recipe": spec["recipe"],
        "recipe_version": 1,
        "token_budget": spec["token_budget"],
        "truncation": False,
        "quantized_components": list(spec["quantized_components"]),
        "text_encoder_precision": spec["text_encoder_precision"],
        "vae_precision": spec["vae_precision"],
    }
    for key in ("sampling_schedule", "thinking", "negative_prompt", "uncond_skip_layers"):
        if key in spec:
            value = spec[key]
            result[key] = list(value) if isinstance(value, tuple) else value
    return result
