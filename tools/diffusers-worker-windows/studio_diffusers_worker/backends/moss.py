"""MOSS-Image-SFT staged backend.

Stage order: NF4 text encoder (positive + empty negative prompt) -> release ->
NF4 DiT with 50 CFG steps, guidance 3.0 and negative branch skipping layer 22 ->
release -> fp32 FLUX.2 VAE decode -> release. The upstream custom pipeline module
is vendored inside this package (fixed bytes + SHA-256 checked at load time),
not read from the model directory, and the documented bitsandbytes dtype shim
supplies the explicit bf16 compute dtype the custom forward requires.

Measured Gate 0 budgets (verification/windows-parity-gate0/
staged-loader-moss-image-sft.md): CUDA reserved 5.84 GiB, private commit
9.20 GiB, available RAM 21.47 GiB, 50-step sampling 102.58 s.
"""

from __future__ import annotations

import gc
import json
import time
from pathlib import Path

from ..resources import (
    COMPACT_CUDA_LIMIT_GIB,
    CUDA_RESERVED_LIMIT_GIB,
    compute_torch_dtype,
    BudgetExceeded,
    CudaPeaks,
    ResourceSampler,
    enforce_budget,
    require_available_ram,
    resource_report,
)
from ..vendored import load_moss_pipeline, moss_bnb_shim
from .common import checkpoint, log, save_primary_image, select_device

LOAD_LIMIT_SECONDS = 600.0
CLEANUP_RESERVED_LIMIT_GIB = 2.5
CLEANUP_ALLOCATED_LIMIT_GIB = 0.25
HEIGHT = 1024
WIDTH = 1024


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
        f"moss {phase}_reserved_after_release_gib={reserved_gib:.3f} "
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


def bnb_config():
    import torch
    from transformers import BitsAndBytesConfig

    return BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )


def load_tokenizer(model_dir: Path):
    from transformers import AutoTokenizer

    return AutoTokenizer.from_pretrained(str(model_dir), subfolder="tokenizer")


def load_text_encoder(model_dir: Path, prepared_dir: Path, compute_dtype=None):
    import torch
    from transformers import AutoModel

    if not (prepared_dir / "config.json").is_file():
        raise ValueError(
            "prepared NF4 text encoder cache is missing; run --prepare first"
        )
    encoder = AutoModel.from_pretrained(
        str(prepared_dir),
        dtype=compute_dtype or torch.bfloat16,
        device_map="cpu",
        low_cpu_mem_usage=True,
    )
    encoder = encoder.to("cuda:0")
    encoder.eval()
    return encoder


def encode_prompt(tokenizer, text_encoder, prompt: str, max_sequence_length: int):
    import torch

    formatted = tokenizer.apply_chat_template(
        [{"role": "user", "content": prompt}],
        tokenize=False,
        add_generation_prompt=True,
        enable_thinking=True,
    )
    inputs = tokenizer(
        formatted,
        return_tensors="pt",
        truncation=True,
        max_length=max_sequence_length,
    ).to("cuda:0")
    with torch.no_grad():
        output = text_encoder(**inputs, use_cache=False, return_dict=True)
    row = output.last_hidden_state[0]
    return row.cpu(), int(row.shape[0])


def load_transformer(module, prepared_dir: Path, compute_dtype=None):
    import torch

    if not (prepared_dir / "config.json").is_file():
        raise ValueError("prepared NF4 DiT cache is missing; run --prepare first")
    transformer = module.MOSSImageTransformer.from_pretrained(
        str(prepared_dir),
        torch_dtype=compute_dtype or torch.bfloat16,
        low_cpu_mem_usage=True,
    )
    transformer = transformer.to("cuda:0")
    transformer.eval()
    return transformer


def _pad_prompt_rows(positive, negative):
    import torch

    max_length = max(positive.shape[0], negative.shape[0])

    def pad(row):
        padded = row.new_zeros(max_length, row.shape[-1])
        padded[: row.shape[0]] = row
        mask = torch.zeros(max_length, dtype=torch.bool)
        mask[: row.shape[0]] = True
        return padded.unsqueeze(0), mask.unsqueeze(0)

    return pad(positive) + pad(negative)


def _sample(module, transformer, positive, positive_mask, negative, negative_mask, request, progress, cancel, resolution=1024):
    import torch
    from diffusers.utils.torch_utils import randn_tensor

    steps = int(request["parameters"]["steps"])
    guidance = float(request["parameters"]["guidance"])
    skip_layers = tuple(int(item) for item in request["parameters"]["uncond_skip_layers"])
    scheduler = module.MOSSImageScheduler()
    scheduler.set_timesteps(steps, device="cuda:0")
    generator = torch.Generator(device="cpu").manual_seed(request["seed"])
    latents = randn_tensor(
        (1, transformer.config.latent_channels, resolution // 16, resolution // 16),
        generator=generator,
        device="cuda:0",
        dtype=torch.float32,
    )
    progress("sampling", 0, len(scheduler.timesteps))
    sample_started = time.perf_counter()
    with torch.no_grad():
        for index, timestep in enumerate(scheduler.timesteps):
            checkpoint(cancel)
            prediction = transformer(
                latents, timestep, positive, positive_mask
            ).sample.float()
            uncond = transformer(
                latents,
                timestep,
                negative,
                negative_mask,
                skip_layers=skip_layers,
            ).sample.float()
            prediction = uncond + guidance * (prediction - uncond)
            latents = scheduler.step(
                prediction, timestep, latents, return_dict=False
            )[0]
            progress("sampling", index + 1, len(scheduler.timesteps))
    torch.cuda.synchronize()
    log(
        f"moss sample_seconds={time.perf_counter() - sample_started:.2f} "
        f"steps={len(scheduler.timesteps)}"
    )
    return latents, scheduler


def _decode_vae(vae, latents):
    import torch
    from diffusers.image_processor import VaeImageProcessor

    started = time.perf_counter()
    with torch.no_grad():
        z = latents.to("cuda:0", dtype=vae.dtype)
        bn = vae.bn
        eps = vae.config.batch_norm_eps
        z = z * (bn.running_var + eps).sqrt().to(z).view(1, -1, 1, 1) + (
            bn.running_mean.to(z).view(1, -1, 1, 1)
        )
        batch, channels, height, width = z.shape
        z = (
            z.reshape(batch, channels // 4, 2, 2, height, width)
            .permute(0, 1, 4, 2, 5, 3)
            .reshape(batch, channels // 4, 2 * height, 2 * width)
        )
        images = vae.decode(z, return_dict=False)[0].clamp(-1, 1)
    torch.cuda.synchronize()
    log(f"moss vae_decode_seconds={time.perf_counter() - started:.2f}")
    processor = VaeImageProcessor(vae_scale_factor=16)
    return processor.postprocess(images, output_type="pil")[0]


def generate(request, progress, cancel):
    import torch
    from diffusers import AutoencoderKLFlux2

    started = time.monotonic()
    sampler = ResourceSampler()
    sampler.start()
    cuda = CudaPeaks()
    model_dir = Path(request["model_path"])
    prepared = model_dir.with_name(model_dir.name + ".prepared")
    steps = int(request["parameters"]["steps"])
    token_budget = int(request["parameters"]["token_budget"])
    resolution = int(request["parameters"].get("resolution", 1024))
    compute_dtype = compute_torch_dtype(request["parameters"].get("compute_dtype"))
    compact = resolution != 1024
    max_sequence_length = 4096
    try:
        require_available_ram("moss text stage")
        device_index, properties = select_device(
            8 if compact else 10, (7, 5) if compact else (8, 6)
        )
        torch.cuda.set_device(device_index)
        torch.cuda.empty_cache()
        log(
            f"moss device={properties.name} steps={steps} resolution={resolution} "
            f"prepared={prepared.name}"
        )
        module = load_moss_pipeline()

        # stage 1: NF4 text encoder, positive + empty negative prompt
        progress("loading")
        cuda.reset_phase()
        stage_started = time.perf_counter()
        tokenizer = load_tokenizer(model_dir)
        checkpoint(cancel)
        text_encoder = load_text_encoder(
            model_dir, prepared / "text_encoder", compute_dtype
        )
        text_load_seconds = time.perf_counter() - stage_started
        if text_load_seconds > LOAD_LIMIT_SECONDS:
            raise BudgetExceeded(
                f"moss text encoder load {text_load_seconds:.1f} s > "
                f"{LOAD_LIMIT_SECONDS:.0f} s"
            )
        cuda.record_phase()
        progress("encoding")
        encode_started = time.perf_counter()
        positive, positive_tokens = encode_prompt(
            tokenizer, text_encoder, request["instruction"], max_sequence_length
        )
        negative, negative_tokens = encode_prompt(
            tokenizer,
            text_encoder,
            str(request["parameters"]["negative_prompt"]),
            max_sequence_length,
        )
        positive, positive_mask, negative, negative_mask = _pad_prompt_rows(
            positive, negative
        )
        prompt_tokens = int(positive.shape[1])
        log(
            f"moss prompt_tokens={prompt_tokens} positive={positive_tokens} "
            f"negative={negative_tokens} token_budget={token_budget} "
            f"encode_seconds={time.perf_counter() - encode_started:.2f}"
        )
        if prompt_tokens > token_budget:
            raise ValueError(
                f"prompt uses {prompt_tokens} tokens, over the {token_budget}-token budget"
            )
        checkpoint(cancel)
        del text_encoder
        del tokenizer
        _release()
        _cleanup_check("text")

        # stage 2: NF4 DiT with the documented dtype shim
        progress("loading")
        require_available_ram("moss sampling stage")
        cuda.reset_phase()
        stage_started = time.perf_counter()
        transformer = load_transformer(module, prepared / "transformer", compute_dtype)
        transformer_load_seconds = time.perf_counter() - stage_started
        if transformer_load_seconds > LOAD_LIMIT_SECONDS:
            raise BudgetExceeded(
                f"moss DiT load {transformer_load_seconds:.1f} s > "
                f"{LOAD_LIMIT_SECONDS:.0f} s"
            )
        cuda.record_phase()
        moss_bnb_shim.install(module)
        checkpoint(cancel)
        latents, scheduler = _sample(
            module,
            transformer,
            positive.to("cuda:0"),
            positive_mask.to("cuda:0"),
            negative.to("cuda:0"),
            negative_mask.to("cuda:0"),
            request,
            progress,
            cancel,
            resolution,
        )
        cuda.record_phase()
        checkpoint(cancel)
        del transformer
        del scheduler
        _release()
        _cleanup_check("dit")

        # stage 3: fp32 FLUX.2 VAE decode
        progress("decoding")
        require_available_ram("moss decode stage")
        cuda.reset_phase()
        stage_started = time.perf_counter()
        vae = AutoencoderKLFlux2.from_pretrained(
            str(model_dir), subfolder="vae", torch_dtype=torch.float32
        ).to("cuda:0")
        vae.eval()
        vae_load_seconds = time.perf_counter() - stage_started
        if vae_load_seconds > LOAD_LIMIT_SECONDS:
            raise BudgetExceeded(
                f"moss VAE load {vae_load_seconds:.1f} s > {LOAD_LIMIT_SECONDS:.0f} s"
            )
        cuda.record_phase()
        image = _decode_vae(vae, latents)
        del vae
        _release()
        _cleanup_check("vae")
        progress("writing")
        candidate = save_primary_image(
            Path(request["output_dir"]), image, width=resolution, height=resolution
        )
        elapsed_seconds = time.monotonic() - started
        resources = resource_report(sampler, cuda, elapsed_seconds)
        log("moss resources=" + json.dumps(resources, ensure_ascii=False))
        enforce_budget(
            sampler,
            cuda,
            elapsed_seconds,
            COMPACT_CUDA_LIMIT_GIB if compact else CUDA_RESERVED_LIMIT_GIB,
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
