"""Windows Z-Image inference after stdout isolation."""

import hashlib
import os
from pathlib import Path
import sys


class Cancelled(Exception):
    pass


def warm_imports():
    import bitsandbytes  # noqa: F401
    import diffusers  # noqa: F401
    import numpy  # noqa: F401
    import PIL  # noqa: F401
    import torch  # noqa: F401
    import transformers  # noqa: F401


def checkpoint(cancel):
    if cancel.is_set():
        raise Cancelled()


def select_device(minimum_memory_gib: int = 10, minimum_capability=(8, 6)):
    import torch

    if not torch.cuda.is_available() or torch.version.cuda != "12.6":
        raise RuntimeError("CUDA 12.6 is unavailable")
    for index in range(torch.cuda.device_count()):
        properties = torch.cuda.get_device_properties(index)
        if (
            "NVIDIA" in properties.name.upper()
            and (properties.major, properties.minor) >= minimum_capability
            and round(properties.total_memory / 1024**3) >= minimum_memory_gib
        ):
            return index, properties
    raise RuntimeError(
        f"required NVIDIA compute capability {minimum_capability} with "
        f">= {minimum_memory_gib} GiB was not found"
    )


def token_count(tokenizer, prompt):
    templated = tokenizer.apply_chat_template(
        [{"role": "user", "content": prompt}],
        tokenize=False,
        add_generation_prompt=True,
        enable_thinking=True,
    )
    encoded = tokenizer(templated, truncation=False, return_tensors="pt")
    return int(encoded.input_ids.shape[1])


def quant_config(module, compute_dtype):
    from diffusers import BitsAndBytesConfig as DiffusersBnB
    from transformers import BitsAndBytesConfig as TransformersBnB

    kwargs = {
        "load_in_4bit": True,
        "bnb_4bit_quant_type": "nf4",
        "bnb_4bit_compute_dtype": compute_dtype,
        "bnb_4bit_use_double_quant": True,
        "llm_int8_skip_modules": ["proj_out"],
    }
    return (
        TransformersBnB(**kwargs)
        if module == "text_encoder"
        else DiffusersBnB(**kwargs)
    )


def load_pipeline(
    model_path, device_index, cancel, progress, compute_dtype=None, vae_dtype=None
):
    import torch
    from diffusers import (
        AutoencoderKL,
        FlowMatchEulerDiscreteScheduler,
        ZImagePipeline,
        ZImageTransformer2DModel,
    )
    from transformers import Qwen2Tokenizer, Qwen3Model

    import torch as _torch

    dtype = compute_dtype or _torch.bfloat16
    model_path = Path(model_path)
    device = f"cuda:{device_index}"
    checkpoint(cancel)
    tokenizer = Qwen2Tokenizer.from_pretrained(
        model_path, subfolder="tokenizer", local_files_only=True
    )
    checkpoint(cancel)
    progress("loading")
    transformer = ZImageTransformer2DModel.from_pretrained(
        model_path,
        subfolder="transformer",
        local_files_only=True,
        torch_dtype=dtype,
        low_cpu_mem_usage=True,
        quantization_config=quant_config("transformer", dtype),
        device_map={"": device},
    )
    transformer = transformer.to("cpu")
    transformer.set_attention_backend("_native_efficient")
    transformer.eval()
    checkpoint(cancel)
    text_encoder = Qwen3Model.from_pretrained(
        model_path,
        subfolder="text_encoder",
        local_files_only=True,
        torch_dtype=dtype,
        low_cpu_mem_usage=True,
        quantization_config=quant_config("text_encoder", dtype),
        device_map={"": device},
    )
    text_encoder = text_encoder.to("cpu")
    text_encoder.eval()
    checkpoint(cancel)
    vae = AutoencoderKL.from_pretrained(
        model_path,
        subfolder="vae",
        local_files_only=True,
        torch_dtype=vae_dtype or dtype,
        low_cpu_mem_usage=True,
    )
    vae.eval()
    vae.enable_tiling()
    scheduler = FlowMatchEulerDiscreteScheduler.from_pretrained(
        model_path, subfolder="scheduler", local_files_only=True
    )
    pipeline = ZImagePipeline(
        scheduler=scheduler,
        vae=vae,
        text_encoder=text_encoder,
        tokenizer=tokenizer,
        transformer=transformer,
    )
    pipeline.enable_model_cpu_offload(gpu_id=device_index)
    return pipeline, tokenizer, device


def generate(request, progress, cancel):
    import torch

    print("backend generation started", file=sys.stderr)
    checkpoint(cancel)
    resolution = int(request["parameters"].get("resolution", 1024))
    compute_dtype = (
        torch.float16
        if request["parameters"].get("compute_dtype") == "float16"
        else torch.bfloat16
    )
    compact = resolution != 1024
    vae_dtype = (
        torch.float32
        if request["parameters"].get("vae_precision") == "float32"
        else compute_dtype
    )
    device_index, properties = select_device(
        8 if compact else 10, (7, 5) if compact else (8, 6)
    )
    torch.cuda.set_device(device_index)
    print(
        f"selected_cuda={device_index} name={properties.name} cc={properties.major}.{properties.minor}",
        file=sys.stderr,
    )
    pipeline, tokenizer, device = load_pipeline(
        request["model_path"], device_index, cancel, progress, compute_dtype, vae_dtype
    )
    print("pipeline loaded", file=sys.stderr)
    actual_tokens = token_count(tokenizer, request["instruction"])
    if actual_tokens > 512:
        raise ValueError(
            f"提示词含模板共 {actual_tokens} token，超过 512 token 上限；请缩短提示词。"
        )
    print(f"token_count={actual_tokens} token_budget=512", file=sys.stderr)
    checkpoint(cancel)

    progress("encoding")
    prompt_embeds, _ = pipeline.encode_prompt(
        prompt=request["instruction"],
        device=device,
        do_classifier_free_guidance=False,
        max_sequence_length=512,
    )
    checkpoint(cancel)

    original_decode = pipeline.vae.decode

    def measured_decode(*args, **kwargs):
        checkpoint(cancel)
        progress("decoding")
        result = original_decode(*args, **kwargs)
        checkpoint(cancel)
        return result

    pipeline.vae.decode = measured_decode

    def callback(_pipe, step, _timestep, callback_kwargs):
        checkpoint(cancel)
        progress("sampling", step + 1, 9)
        return callback_kwargs

    progress("sampling", 0, 9)
    generator = torch.Generator(device=device).manual_seed(request["seed"])
    result = pipeline(
        prompt=None,
        prompt_embeds=prompt_embeds,
        height=resolution,
        width=resolution,
        num_inference_steps=9,
        guidance_scale=0.0,
        generator=generator,
        output_type="pil",
        return_dict=True,
        callback_on_step_end=callback,
        callback_on_step_end_tensor_inputs=["latents"],
        max_sequence_length=512,
    )
    checkpoint(cancel)
    progress("writing")
    if len(result.images) != 1:
        raise ValueError("pipeline did not return exactly one image")
    image = result.images[0]
    if image.size != (resolution, resolution) or image.mode != "RGB":
        raise ValueError(
            f"pipeline did not return one {resolution}x{resolution} RGB image"
        )
    output = Path(request["output_dir"]) / "primary.png"
    if output.exists() or output.is_symlink():
        raise ValueError("Attempt output already exists")
    image.save(output, format="PNG")
    size = output.stat().st_size
    if not 0 < size <= 32 * 1024 * 1024:
        raise ValueError("PNG candidate exceeds 32 MiB")
    with output.open("rb+") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        stream.flush()
        os.fsync(stream.fileno())
    return {
        "role": "primary",
        "data_type": "image_rgb",
        "relative_path": "primary.png",
        "width": resolution,
        "height": resolution,
        "size_bytes": size,
        "sha256": digest,
    }
