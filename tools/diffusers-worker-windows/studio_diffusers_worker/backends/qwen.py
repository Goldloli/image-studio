"""Qwen-Image 2.1 / Turbo staged backend over a prepared NF4 cache.

Stage order (only one large component owns the GPU at a time, no
``enable_model_cpu_offload``): text encode -> release -> DiT sample -> release ->
tiled VAE decode -> release. The prepared cache is a derived local artifact
built by ``--prepare`` (``models/<snapshot>.prepared``); it is deterministically
rebuilt from the fixed snapshot and never replaces it.

Measured Gate 0 budgets for this path (verification/windows-parity-gate0/
staged-loader-qwen21-turbo.md): CUDA reserved 6.00 GiB, private commit
13.95 GiB, available RAM 17.32 GiB, 8-step sampling 10.66 s.
"""

from __future__ import annotations

import gc
import json
import time
from pathlib import Path

from ..resources import (
    CUDA_RESERVED_LIMIT_GIB,
    BudgetExceeded,
    CudaPeaks,
    ResourceSampler,
    enforce_budget,
    require_available_ram,
    resource_report,
)
from .common import checkpoint, log, save_primary_image, select_device

SYS_PROMPT = "Comprehend and analyze the provided prompt."
PROMPT_TEMPLATE_T2I = (
    f"<|im_start|>system\n{SYS_PROMPT}<|im_end|>\n"
    f"<|im_start|>user\n{{}}<|im_end|>\n"
    f"<|im_start|>assistant\n"
)
IMAGE_PAD_TOKEN = "<|image_pad|>"
LOAD_LIMIT_SECONDS = 600.0
# The compact tier must fit an 8 GB card that also runs the desktop compositor.
COMPACT_CUDA_LIMIT_GIB = 6.0
CLEANUP_RESERVED_LIMIT_GIB = 2.5
CLEANUP_ALLOCATED_LIMIT_GIB = 0.25


def compute_torch_dtype(name: str | None):
    """Map the frozen `compute_dtype` parameter to a torch dtype.

    The compact tier runs on 8 GB Turing cards, which have no native BF16 path, so the
    protocol freezes `float16` there; the standard tier keeps `bfloat16`.
    """
    import torch

    return torch.float16 if name == "float16" else torch.bfloat16


def quant_config(compute_dtype=None):
    import torch
    from transformers import BitsAndBytesConfig

    return BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=compute_dtype or torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )


def load_processor(model_dir: Path):
    from transformers import Qwen3VLProcessor

    return Qwen3VLProcessor.from_pretrained(str(model_dir), subfolder="processor")


def load_text_encoder(prepared_dir: Path, compute_dtype=None, sequential: bool = False):
    import torch
    from transformers import Qwen3VLForConditionalGeneration

    if not (prepared_dir / "config.json").is_file():
        raise ValueError(
            "prepared NF4 text encoder cache is missing; run --prepare first"
        )
    dtype = compute_dtype or torch.bfloat16
    if sequential:
        # Compact measurement path: keep the whole NF4 encoder on the card and rely on the
        # 768-square sampling stage plus staged release to stay inside the 8 GB budget.
        encoder = Qwen3VLForConditionalGeneration.from_pretrained(
            str(prepared_dir),
            torch_dtype=dtype,
            device_map="cpu",
            low_cpu_mem_usage=True,
        )
        encoder = encoder.to("cuda:0")
        log("qwen text encoder moved to the card as one component (compact)")
    else:
        # Standard tier: use the same CPU staging as the compact branch. Loading
        # without ``device_map`` and then moving the NF4 modules to the card left
        # ~5.83 GiB of CUDA reserved memory behind after the text forward even
        # after ``empty_cache`` (the staged release check then failed); staging
        # through CPU frees the reserved blocks as expected.
        encoder = Qwen3VLForConditionalGeneration.from_pretrained(
            str(prepared_dir),
            torch_dtype=dtype,
            device_map="cpu",
            low_cpu_mem_usage=True,
        )
        encoder = encoder.to("cuda:0")
    encoder.eval()
    return encoder

def encode_prompt(processor, text_encoder, prompt: str):
    """Mirror of QwenImage21Pipeline._get_qwen_prompt_embeds for text-to-image."""
    import torch

    sys_message = [
        {"role": "system", "content": [{"type": "text", "text": SYS_PROMPT}]}
    ]
    sys_tokens = processor.apply_chat_template(
        sys_message, tokenize=True, return_dict=False
    )
    drop_idx = len(sys_tokens[0])
    img_token_id = processor.tokenizer.encode(IMAGE_PAD_TOKEN)[0]
    template = PROMPT_TEMPLATE_T2I.replace("{}", prompt if prompt else " ")
    model_inputs = processor(
        text=[template],
        padding=True,
        padding_side="left",
        return_tensors="pt",
    ).to("cuda:0")
    forward_kwargs = {
        "input_ids": model_inputs.input_ids,
        "attention_mask": model_inputs.attention_mask,
        "output_hidden_states": True,
    }
    text_model = getattr(text_encoder.model, "language_model", text_encoder.model)
    handle = text_model.norm.register_forward_hook(
        lambda module, args, output: args[0]
    )
    try:
        with torch.no_grad():
            outputs = text_encoder(**forward_kwargs)
    finally:
        handle.remove()
    hidden_states = outputs.hidden_states[-1]
    bool_mask = model_inputs.attention_mask.bool()
    valid_lengths = bool_mask.sum(dim=1)
    selected = hidden_states[bool_mask]
    split_hidden_states = [
        row[drop_idx:] for row in torch.split(selected, valid_lengths.tolist(), dim=0)
    ]
    image_pad_mask = [
        (sample_ids[sample_mask.bool()] == img_token_id)[drop_idx:]
        for sample_ids, sample_mask in zip(
            model_inputs.input_ids, model_inputs.attention_mask
        )
    ]
    max_seq_len = max(row.size(0) for row in split_hidden_states)
    prompt_embeds = torch.stack(
        [
            torch.cat([row, row.new_zeros(max_seq_len - row.size(0), row.size(1))])
            for row in split_hidden_states
        ]
    )
    encoder_attention_mask = torch.stack(
        [
            torch.cat(
                [
                    torch.ones(row.size(0), dtype=torch.long, device=row.device),
                    row.new_zeros(max_seq_len - row.size(0)),
                ]
            )
            for row in split_hidden_states
        ]
    )
    image_pad_mask = torch.stack(
        [
            torch.cat([row, row.new_zeros(max_seq_len - row.size(0))])
            for row in image_pad_mask
        ]
    )
    return prompt_embeds.cpu(), encoder_attention_mask.cpu(), image_pad_mask.cpu()


def load_transformer(prepared_dir: Path, compute_dtype=None):
    import torch
    from diffusers import QwenImage21Transformer2DModel

    if not (prepared_dir / "config.json").is_file():
        raise ValueError(
            "prepared NF4 transformer cache is missing; run --prepare first"
        )
    transformer = QwenImage21Transformer2DModel.from_pretrained(
        str(prepared_dir),
        torch_dtype=compute_dtype or torch.bfloat16,
        device_map="cpu",
        low_cpu_mem_usage=True,
    )
    transformer = transformer.to("cuda:0")
    transformer.eval()
    return transformer


def _release() -> None:
    import torch

    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.synchronize()


def _cleanup_check(phase: str) -> None:
    import torch

    reserved_gib = torch.cuda.memory_reserved() / 1024**3
    allocated_gib = torch.cuda.memory_allocated() / 1024**3
    log(
        f"qwen {phase}_reserved_after_release_gib={reserved_gib:.3f} "
        f"allocated_after_release_gib={allocated_gib:.3f}"
    )
    if allocated_gib > CLEANUP_ALLOCATED_LIMIT_GIB:
        raise RuntimeError(
            f"cleanup failed after {phase}: {allocated_gib:.2f} GiB still "
            f"allocated (limit {CLEANUP_ALLOCATED_LIMIT_GIB} GiB)"
        )
    if reserved_gib > CLEANUP_RESERVED_LIMIT_GIB:
        raise RuntimeError(
            f"cleanup failed after {phase}: CUDA reserved {reserved_gib:.2f} GiB "
            f"> {CLEANUP_RESERVED_LIMIT_GIB} GiB"
        )


def generate(request, progress, cancel):
    import torch

    started = time.monotonic()
    sampler = ResourceSampler()
    sampler.start()
    cuda = CudaPeaks()
    prepared = Path(request["model_path"] + ".prepared")
    model_dir = Path(request["model_path"])
    steps = int(request["parameters"]["steps"])
    token_budget = int(request["parameters"]["token_budget"])
    resolution = int(request["parameters"]["resolution"] if "resolution" in request["parameters"] else 1024)
    compute_dtype = compute_torch_dtype(request["parameters"].get("compute_dtype"))
    sequential = request["parameters"].get("load_strategy") == "staged-nf4-sequential-v1"
    try:
        require_available_ram("qwen text stage")
        device_index, properties = select_device(
            8 if sequential else 10, (7, 5) if sequential else (8, 6)
        )
        torch.cuda.set_device(device_index)
        torch.cuda.empty_cache()
        log(
            f"qwen device={properties.name} steps={steps} "
            f"prepared={prepared.name}"
        )

        # stage 1: text encoder
        progress("loading")
        cuda.reset_phase()
        stage_started = time.perf_counter()
        processor = load_processor(model_dir)
        checkpoint(cancel)
        text_encoder = load_text_encoder(
            prepared / "text_encoder", compute_dtype, sequential
        )
        text_load_seconds = time.perf_counter() - stage_started
        if text_load_seconds > LOAD_LIMIT_SECONDS:
            raise BudgetExceeded(
                f"qwen text encoder load {text_load_seconds:.1f} s > "
                f"{LOAD_LIMIT_SECONDS:.0f} s"
            )
        cuda.record_phase()
        progress("encoding")
        encode_started = time.perf_counter()
        embeds, embeds_mask, image_pad_mask = encode_prompt(
            processor, text_encoder, request["instruction"]
        )
        prompt_tokens = int(embeds.shape[1])
        log(
            f"qwen prompt_tokens={prompt_tokens} token_budget={token_budget} "
            f"encode_seconds={time.perf_counter() - encode_started:.2f}"
        )
        if prompt_tokens > token_budget:
            raise ValueError(
                f"prompt uses {prompt_tokens} tokens, over the {token_budget}-token budget"
            )
        checkpoint(cancel)
        del text_encoder
        del processor
        _release()
        _cleanup_check("text")

        # stage 2: transformer sampling
        progress("loading")
        require_available_ram("qwen sampling stage")
        cuda.reset_phase()
        stage_started = time.perf_counter()
        scheduler = _load_scheduler(model_dir)
        transformer = load_transformer(prepared / "transformer", compute_dtype)
        transformer_load_seconds = time.perf_counter() - stage_started
        if transformer_load_seconds > LOAD_LIMIT_SECONDS:
            raise BudgetExceeded(
                f"qwen transformer load {transformer_load_seconds:.1f} s > "
                f"{LOAD_LIMIT_SECONDS:.0f} s"
            )
        cuda.record_phase()
        checkpoint(cancel)
        latents = _sample(
            transformer,
            scheduler,
            embeds,
            embeds_mask,
            image_pad_mask,
            request,
            steps,
            progress,
            cancel,
            cuda,
            resolution,
            compute_dtype,
        )
        checkpoint(cancel)
        del transformer
        del scheduler
        _release()
        _cleanup_check("transformer")

        # stage 3: tiled VAE decode
        progress("decoding")
        require_available_ram("qwen decode stage")
        cuda.reset_phase()
        stage_started = time.perf_counter()
        from diffusers import AutoencoderKLQwenImage21

        vae = AutoencoderKLQwenImage21.from_pretrained(
            str(model_dir), subfolder="vae", torch_dtype=torch.bfloat16
        ).to("cuda:0")
        vae.eval()
        if hasattr(vae, "enable_tiling"):
            vae.enable_tiling()
        vae_load_seconds = time.perf_counter() - stage_started
        if vae_load_seconds > LOAD_LIMIT_SECONDS:
            raise BudgetExceeded(
                f"qwen VAE load {vae_load_seconds:.1f} s > {LOAD_LIMIT_SECONDS:.0f} s"
            )
        cuda.record_phase()
        image = _decode_vae(vae, latents, resolution, resolution)
        del vae
        _release()
        _cleanup_check("vae")
        progress("writing")
        candidate = save_primary_image(
            Path(request["output_dir"]), image, width=resolution, height=resolution
        )
        elapsed_seconds = time.monotonic() - started
        resources = resource_report(sampler, cuda, elapsed_seconds)
        log("qwen resources=" + json.dumps(resources, ensure_ascii=False))
        enforce_budget(
            sampler,
            cuda,
            elapsed_seconds,
            COMPACT_CUDA_LIMIT_GIB if sequential else CUDA_RESERVED_LIMIT_GIB,
        )
        return candidate
    finally:
        sampler.stop()
        gc.collect()
        try:
            import torch

            torch.cuda.empty_cache()
        except Exception:  # noqa: BLE001 - best effort cleanup
            pass


def _load_scheduler(model_dir: Path):
    from diffusers import FlowMatchEulerDiscreteScheduler

    return FlowMatchEulerDiscreteScheduler.from_pretrained(
        str(model_dir), subfolder="scheduler"
    )


def _sample(
    transformer,
    scheduler,
    embeds,
    embeds_mask,
    image_pad_mask,
    request,
    steps: int,
    progress,
    cancel,
    cuda: CudaPeaks,
    resolution: int = 1024,
    dtype=None,
):
    import json as json_module

    import torch
    from diffusers.pipelines.qwenimage21.pipeline_qwenimage21 import (
        QwenImage21KVCache,
        calculate_shift,
        randn_tensor,
        retrieve_timesteps,
    )

    height = resolution
    width = resolution
    dtype = dtype or torch.bfloat16
    embeds = embeds.to("cuda:0")
    embeds_mask = embeds_mask.to("cuda:0") if embeds_mask is not None else None
    if embeds_mask is not None and embeds_mask.all():
        embeds_mask = None
    image_pad_mask = image_pad_mask.to("cuda:0")
    checkpoint(cancel)
    height_latent = height // 16
    width_latent = width // 16
    num_channels = transformer.config.in_channels
    generator = torch.Generator(device="cpu").manual_seed(request["seed"])
    latents = randn_tensor(
        (1, 1, num_channels, height_latent, width_latent),
        generator=generator,
        device="cuda:0",
        dtype=dtype,
    )
    latents = latents.view(1, num_channels, height_latent * width_latent).transpose(1, 2)
    model_index = json_module.loads(
        (Path(request["model_path"]) / "model_index.json").read_text(encoding="utf-8")
    )
    sample_sigmas = model_index.get("sample_sigmas")
    mu = calculate_shift(
        latents.shape[1],
        scheduler.config.get("base_image_seq_len", 256),
        scheduler.config.get("max_image_seq_len", 4096),
        scheduler.config.get("base_shift", 0.5),
        scheduler.config.get("max_shift", 1.15),
    )
    timesteps, num_inference_steps = retrieve_timesteps(
        scheduler, steps, "cuda:0", sigmas=sample_sigmas, mu=mu
    )
    img_shapes = [[(1, height_latent, width_latent)]]
    image_pad_mask = torch.cat(
        [
            image_pad_mask,
            image_pad_mask.new_ones(
                image_pad_mask.shape[0], latents.shape[1] // 4
            ),
        ],
        dim=1,
    )
    num_blocks = len(transformer.transformer_blocks)
    cond_cache = QwenImage21KVCache(num_blocks)
    scheduler.set_begin_index(0)
    sample_started = time.perf_counter()
    with torch.no_grad():
        for index, timestep in enumerate(timesteps):
            checkpoint(cancel)
            progress("sampling", index + 1, len(timesteps))
            kv_mode = "extract" if index == 0 else "cached"
            timestep_tensor = timestep.expand(latents.shape[0]).to(latents.dtype)
            with transformer.cache_context("cond"):
                noise_pred = transformer(
                    hidden_states=latents,
                    timestep=timestep_tensor / 1000,
                    encoder_hidden_states=embeds,
                    encoder_hidden_states_mask=embeds_mask,
                    img_shapes=img_shapes,
                    img_mask=image_pad_mask,
                    attention_kwargs={},
                    kv_cache=cond_cache,
                    kv_cache_mode=kv_mode,
                    return_dict=False,
                )[0]
            noise_pred = noise_pred[:, -latents.size(1):]
            latents = scheduler.step(
                noise_pred, timestep, latents, return_dict=False
            )[0]
    torch.cuda.synchronize()
    cuda.record_phase()
    log(
        f"qwen sample_seconds={time.perf_counter() - sample_started:.2f} "
        f"steps={len(timesteps)}"
    )
    return latents


def _decode_vae(vae, latents, height: int, width: int):
    import torch
    from diffusers.image_processor import VaeImageProcessor

    latent_height = 2 * (height // 32)
    latent_width = 2 * (width // 32)
    unpacked = latents.transpose(1, 2).reshape(
        1, vae.config.z_dim, 1, latent_height, latent_width
    )
    unpacked = unpacked.to("cuda:0", dtype=vae.dtype)
    latents_mean = (
        torch.tensor(vae.config.latents_mean)
        .view(1, vae.config.z_dim, 1, 1, 1)
        .to(unpacked.device, unpacked.dtype)
    )
    latents_std = (
        torch.tensor(vae.config.latents_std)
        .view(1, vae.config.z_dim, 1, 1, 1)
        .to(unpacked.device, unpacked.dtype)
    )
    unpacked = unpacked * latents_std + latents_mean
    started = time.perf_counter()
    with torch.no_grad():
        image = vae.decode(unpacked, return_dict=False)[0][:, :, 0]
    torch.cuda.synchronize()
    log(f"qwen vae_decode_seconds={time.perf_counter() - started:.2f}")
    processor = VaeImageProcessor(
        vae_scale_factor=16, vae_latent_channels=vae.config.z_dim
    )
    return processor.postprocess(image, output_type="pil")[0]
