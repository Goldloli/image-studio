"""Experiment-only staged quantized Gate 0 for Qwen-Image 2.1 on Windows.

The official diffusers pipeline keeps every component resident (or moves bf16
components through CPU RAM with model offload). On a 12 GiB card that oversells
both VRAM and system RAM (see verification/windows-parity-gate0). This runner
executes the same math in explicit stages so only one large component owns the
GPU at a time:

1. text  - Qwen3-VL text encoder (NF4) encodes the prompt, then is released.
2. dit   - transformer (NF4) denoises the packed latents, then is released.
3. vae   - VAE (bf16) decodes the latents with tiling, then is released.

Intermediate tensors are tiny and travel through CPU tensors / files, so each
stage can run in its own process. The image is NOT expected to be bit-identical
to the official pipeline (component placement changes rounding), but it is a
real sample of the same model, schedule and quantization.

Experiment-only: not product code. The product staged executor is written later
against the same measurements.
"""

from __future__ import annotations

import argparse
import ctypes
import gc
import hashlib
import json
import os
import subprocess
import threading
import time
import traceback
from pathlib import Path

import torch

SYS_PROMPT = "Comprehend and analyze the provided prompt."
PROMPT_TEMPLATE_T2I = (
    f"<|im_start|>system\n{SYS_PROMPT}<|im_end|>\n"
    f"<|im_start|>user\n{{}}<|im_end|>\n"
    f"<|im_start|>assistant\n"
)
IMAGE_PAD_TOKEN = "<|image_pad|>"
BNB = {
    "load_in_4bit": True,
    "bnb_4bit_quant_type": "nf4",
    "bnb_4bit_compute_dtype": torch.bfloat16,
    "bnb_4bit_use_double_quant": True,
}


class MemoryStatusEx(ctypes.Structure):
    _fields_ = [
        ("dwLength", ctypes.c_ulong),
        ("dwMemoryLoad", ctypes.c_ulong),
        ("ullTotalPhys", ctypes.c_ulonglong),
        ("ullAvailPhys", ctypes.c_ulonglong),
        ("ullTotalPageFile", ctypes.c_ulonglong),
        ("ullAvailPageFile", ctypes.c_ulonglong),
        ("ullTotalVirtual", ctypes.c_ulonglong),
        ("ullAvailVirtual", ctypes.c_ulonglong),
        ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
    ]


def system_memory() -> dict[str, int]:
    status = MemoryStatusEx()
    status.dwLength = ctypes.sizeof(MemoryStatusEx)
    if not ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)):
        raise ctypes.WinError()
    return {
        "physical_total_bytes": status.ullTotalPhys,
        "physical_available_bytes": status.ullAvailPhys,
        "commit_limit_bytes": status.ullTotalPageFile,
        "commit_available_bytes": status.ullAvailPageFile,
    }


def process_tree_private_commit() -> int:
    import psutil

    root = psutil.Process(os.getpid())
    total = 0
    for process in [root, *root.children(recursive=True)]:
        try:
            total += int(process.memory_info().private)
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass
    return total


def nvidia_used_mib() -> int | None:
    try:
        output = subprocess.run(
            ["nvidia-smi", "--query-gpu=memory.used", "--format=csv,noheader,nounits"],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout.strip()
        return int(output.splitlines()[0])
    except (OSError, subprocess.SubprocessError, ValueError):
        return None


class Sampler:
    def __init__(self, interval_seconds: float = 0.1) -> None:
        self.interval = interval_seconds
        self.stop_event = threading.Event()
        self.thread: threading.Thread | None = None
        self.reset()

    def reset(self) -> None:
        self.private_peak = 0
        self.available_ram_min = 1 << 62
        self.system_total = 0
        self.cuda_reserved_peak = 0
        self.cuda_allocated_peak = 0
        self.nvidia_used_peak_mib = 0

    def _loop(self) -> None:
        while not self.stop_event.is_set():
            try:
                self.private_peak = max(self.private_peak, process_tree_private_commit())
                memory = system_memory()
                self.system_total = memory["physical_total_bytes"]
                self.available_ram_min = min(
                    self.available_ram_min, memory["physical_available_bytes"]
                )
                if torch.cuda.is_available():
                    self.cuda_reserved_peak = max(
                        self.cuda_reserved_peak, torch.cuda.memory_reserved()
                    )
                    self.cuda_allocated_peak = max(
                        self.cuda_allocated_peak, torch.cuda.memory_allocated()
                    )
                used = nvidia_used_mib()
                if used is not None:
                    self.nvidia_used_peak_mib = max(self.nvidia_used_peak_mib, used)
            except Exception:  # noqa: BLE001 - experiment sampler
                pass
            self.stop_event.wait(self.interval)

    def start(self) -> None:
        self.stop_event.clear()
        self.thread = threading.Thread(target=self._loop, daemon=True)
        self.thread.start()

    def stop(self) -> None:
        self.stop_event.set()
        if self.thread is not None:
            self.thread.join(timeout=10)

    def snapshot(self) -> dict:
        return {
            "process_tree_private_commit_peak_gib": round(self.private_peak / 1024**3, 3),
            "system_available_ram_min_gib": round(self.available_ram_min / 1024**3, 3),
            "system_physical_total_gib": round(self.system_total / 1024**3, 3),
            "cuda_reserved_peak_gib": round(self.cuda_reserved_peak / 1024**3, 3),
            "cuda_allocated_peak_gib": round(self.cuda_allocated_peak / 1024**3, 3),
            "nvidia_used_peak_mib": self.nvidia_used_peak_mib,
        }


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(payload, indent=2, ensure_ascii=False), encoding="utf-8")


def quant_config():
    from transformers import BitsAndBytesConfig

    return BitsAndBytesConfig(**BNB)


# --------------------------------------------------------------------------
# stage 1: text encoder
# --------------------------------------------------------------------------


def load_processor(model_dir: Path):
    from transformers import Qwen3VLProcessor

    processor = Qwen3VLProcessor.from_pretrained(str(model_dir), subfolder="processor")
    return processor


def load_text_encoder(model_dir: Path, prepared_dir: Path | None = None):
    """Load the text encoder NF4 on CPU, then move the quantized modules to the GPU.

    Quantizing directly onto the card loads the bf16 shard on the GPU before
    replacing it with 4-bit weights; that transient peak (measured 15.4 GiB
    reserved for this 16.3 GB encoder) oversells a 12 GiB card. CPU-first keeps
    the GPU peak at the quantized size (5.96 GiB measured).

    A prepared NF4 directory (``model.save_pretrained`` output) is loaded
    directly: 6.38 GB on disk, 0.7 s reload, 11.2 s host-to-device. Preparing
    once keeps the generation-time process commit far below the 24 GiB budget
    instead of the ~25.3 GiB bf16 quantization transient.
    """
    from transformers import Qwen3VLForConditionalGeneration

    source = prepared_dir if prepared_dir is not None and (prepared_dir / "config.json").is_file() else model_dir
    kwargs = dict(
        torch_dtype=torch.bfloat16,
        device_map="cpu",
        low_cpu_mem_usage=True,
    )
    if source is model_dir:
        kwargs["subfolder"] = "text_encoder"
        kwargs["quantization_config"] = quant_config()
    encoder = Qwen3VLForConditionalGeneration.from_pretrained(str(source), **kwargs)
    encoder = encoder.to("cuda:0")
    encoder.eval()
    return encoder


def encode_prompt(processor, text_encoder, prompt: str):
    """Mirror of QwenImage21Pipeline._get_qwen_prompt_embeds for text-to-image."""
    sys_message = [{"role": "system", "content": [{"type": "text", "text": SYS_PROMPT}]}]
    sys_tokens = processor.apply_chat_template(sys_message, tokenize=True, return_dict=False)
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
    handle = text_model.norm.register_forward_hook(lambda module, args, output: args[0])
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
        e[drop_idx:] for e in torch.split(selected, valid_lengths.tolist(), dim=0)
    ]
    image_pad_mask = [
        (sample_ids[sample_mask.bool()] == img_token_id)[drop_idx:]
        for sample_ids, sample_mask in zip(
            model_inputs.input_ids, model_inputs.attention_mask
        )
    ]

    max_seq_len = max(e.size(0) for e in split_hidden_states)
    prompt_embeds = torch.stack(
        [
            torch.cat([u, u.new_zeros(max_seq_len - u.size(0), u.size(1))])
            for u in split_hidden_states
        ]
    )
    encoder_attention_mask = torch.stack(
        [
            torch.cat(
                [torch.ones(e.size(0), dtype=torch.long, device=e.device), e.new_zeros(max_seq_len - e.size(0))]
            )
            for e in split_hidden_states
        ]
    )
    image_pad_mask = torch.stack(
        [torch.cat([u, u.new_zeros(max_seq_len - u.size(0))]) for u in image_pad_mask]
    )
    return prompt_embeds.cpu(), encoder_attention_mask.cpu(), image_pad_mask.cpu()


def stage_text(args, sampler: Sampler, work: Path) -> dict:
    sampler.reset()
    started = time.perf_counter()
    processor = load_processor(args.model_dir)
    text_encoder = load_text_encoder(
        args.model_dir, args.prepared_dir / "text_encoder" if args.prepared_dir else None
    )
    load_seconds = time.perf_counter() - started
    encode_started = time.perf_counter()
    embeds, embeds_mask, image_pad_mask = encode_prompt(processor, text_encoder, args.prompt)
    encode_seconds = time.perf_counter() - encode_started
    record = {
        "load_seconds": round(load_seconds, 2),
        "encode_seconds": round(encode_seconds, 2),
        "prompt_tokens": int(embeds.shape[1]),
        "prompt_embeds_shape": list(embeds.shape),
        "prompt_embeds_dtype": str(embeds.dtype),
        "image_pad_tokens": int(image_pad_mask.sum().item()),
        "resources": sampler.snapshot(),
    }
    torch.save(
        {
            "prompt_embeds": embeds.to(torch.bfloat16),
            "prompt_embeds_mask": embeds_mask,
            "image_pad_mask": image_pad_mask,
            "prompt": args.prompt,
        },
        work / "prompt.pt",
    )
    del text_encoder, processor, embeds, embeds_mask, image_pad_mask
    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.synchronize()
    record["reserved_after_release_gib"] = round(torch.cuda.memory_reserved() / 1024**3, 3)
    record["allocated_after_release_gib"] = round(torch.cuda.memory_allocated() / 1024**3, 3)
    return record


# --------------------------------------------------------------------------
# stage 2: transformer denoise
# --------------------------------------------------------------------------


def load_transformer(model_dir: Path, prepared_dir: Path | None = None):
    """CPU-first NF4 quantization, then move to the card (see load_text_encoder)."""
    from diffusers import QwenImage21Transformer2DModel

    source = prepared_dir if prepared_dir is not None and (prepared_dir / "config.json").is_file() else model_dir
    kwargs = dict(torch_dtype=torch.bfloat16, device_map="cpu", low_cpu_mem_usage=True)
    if source is model_dir:
        kwargs["subfolder"] = "transformer"
        kwargs["quantization_config"] = quant_config()
    transformer = QwenImage21Transformer2DModel.from_pretrained(str(source), **kwargs)
    transformer = transformer.to("cuda:0")
    transformer.eval()
    return transformer


def stage_transformer(args, sampler: Sampler, work: Path) -> dict:
    import numpy as np
    from diffusers import FlowMatchEulerDiscreteScheduler
    from diffusers.pipelines.qwenimage21.pipeline_qwenimage21 import (
        QwenImage21KVCache,
        calculate_shift,
        randn_tensor,
        retrieve_timesteps,
    )

    sampler.reset()
    prompt = torch.load(work / "prompt.pt", map_location="cpu", weights_only=False)
    prompt_embeds = prompt["prompt_embeds"].to("cuda:0")
    prompt_embeds_mask = prompt["prompt_embeds_mask"]
    if prompt_embeds_mask is not None and prompt_embeds_mask.all():
        prompt_embeds_mask = None
    if prompt_embeds_mask is not None:
        prompt_embeds_mask = prompt_embeds_mask.to("cuda:0")
    image_pad_mask = prompt["image_pad_mask"].to("cuda:0")

    started = time.perf_counter()
    scheduler = FlowMatchEulerDiscreteScheduler.from_pretrained(
        str(args.model_dir), subfolder="scheduler"
    )
    transformer = load_transformer(
        args.model_dir, args.prepared_dir / "transformer" if args.prepared_dir else None
    )
    load_seconds = time.perf_counter() - started

    height = args.height // 16
    width = args.width // 16
    num_channels = transformer.config.in_channels
    generator = torch.Generator(device="cpu").manual_seed(args.seed)
    latents = randn_tensor(
        (1, 1, num_channels, height, width),
        generator=generator,
        device="cuda:0",
        dtype=torch.bfloat16,
    )
    latents = latents.view(1, num_channels, height * width).transpose(1, 2)

    sample_sigmas = None
    model_index = json.loads((args.model_dir / "model_index.json").read_text(encoding="utf-8"))
    sample_sigmas = model_index.get("sample_sigmas")
    mu = calculate_shift(
        latents.shape[1],
        scheduler.config.get("base_image_seq_len", 256),
        scheduler.config.get("max_image_seq_len", 4096),
        scheduler.config.get("base_shift", 0.5),
        scheduler.config.get("max_shift", 1.15),
    )
    timesteps, num_inference_steps = retrieve_timesteps(
        scheduler, args.steps, "cuda:0", sigmas=sample_sigmas, mu=mu
    )
    num_warmup_steps = max(len(timesteps) - num_inference_steps * scheduler.order, 0)
    img_shapes = [[(1, height, width)]]
    image_pad_mask = torch.cat(
        [image_pad_mask, image_pad_mask.new_ones(image_pad_mask.shape[0], latents.shape[1] // 4)],
        dim=1,
    )
    num_blocks = len(transformer.transformer_blocks)
    cond_cache = QwenImage21KVCache(num_blocks)

    sample_started = time.perf_counter()
    scheduler.set_begin_index(0)
    with torch.no_grad():
        for index, timestep in enumerate(timesteps):
            kv_mode = "extract" if index == 0 else "cached"
            timestep_tensor = timestep.expand(latents.shape[0]).to(latents.dtype)
            with transformer.cache_context("cond"):
                noise_pred = transformer(
                    hidden_states=latents,
                    timestep=timestep_tensor / 1000,
                    encoder_hidden_states=prompt_embeds,
                    encoder_hidden_states_mask=prompt_embeds_mask,
                    img_shapes=img_shapes,
                    img_mask=image_pad_mask,
                    attention_kwargs={},
                    kv_cache=cond_cache,
                    kv_cache_mode=kv_mode,
                    return_dict=False,
                )[0]
            noise_pred = noise_pred[:, -latents.size(1):]
            latents = scheduler.step(noise_pred, timestep, latents, return_dict=False)[0]
    torch.cuda.synchronize()
    sample_seconds = time.perf_counter() - sample_started

    torch.save(
        {
            "latents": latents.cpu(),
            "height": args.height,
            "width": args.width,
            "seed": args.seed,
            "steps": len(timesteps),
        },
        work / "latents.pt",
    )
    record = {
        "load_seconds": round(load_seconds, 2),
        "sample_seconds": round(sample_seconds, 2),
        "steps": len(timesteps),
        "sigmas": sample_sigmas,
        "latent_tokens": int(latents.shape[1]),
        "resources": sampler.snapshot(),
    }
    del transformer, scheduler, cond_cache
    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.synchronize()
    record["reserved_after_release_gib"] = round(torch.cuda.memory_reserved() / 1024**3, 3)
    record["allocated_after_release_gib"] = round(torch.cuda.memory_allocated() / 1024**3, 3)
    return record


# --------------------------------------------------------------------------
# stage 3: VAE decode
# --------------------------------------------------------------------------


def stage_vae(args, sampler: Sampler, work: Path) -> dict:
    from diffusers import AutoencoderKLQwenImage21
    from diffusers.image_processor import VaeImageProcessor

    sampler.reset()
    payload = torch.load(work / "latents.pt", map_location="cpu", weights_only=False)
    latents = payload["latents"]
    height, width = payload["height"], payload["width"]

    started = time.perf_counter()
    vae = AutoencoderKLQwenImage21.from_pretrained(
        str(args.model_dir), subfolder="vae", torch_dtype=torch.bfloat16
    ).to("cuda:0")
    vae.eval()
    load_seconds = time.perf_counter() - started
    if args.vae_tiling and hasattr(vae, "enable_tiling"):
        vae.enable_tiling()

    decode_started = time.perf_counter()
    with torch.no_grad():
        latent_height = 2 * (height // 32)
        latent_width = 2 * (width // 32)
        unpacked = latents.transpose(1, 2).reshape(1, vae.config.z_dim, 1, latent_height, latent_width)
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
        image = vae.decode(unpacked, return_dict=False)[0][:, :, 0]
    torch.cuda.synchronize()
    decode_seconds = time.perf_counter() - decode_started

    image_processor = VaeImageProcessor(vae_scale_factor=16, vae_latent_channels=vae.config.z_dim)
    pil_image = image_processor.postprocess(image, output_type="pil")[0]
    args.output.parent.mkdir(parents=True, exist_ok=True)
    pil_image.save(args.output)
    digest = hashlib.sha256(args.output.read_bytes()).hexdigest()
    record = {
        "load_seconds": round(load_seconds, 2),
        "decode_seconds": round(decode_seconds, 2),
        "resources": sampler.snapshot(),
        "image": {
            "path": str(args.output),
            "bytes": args.output.stat().st_size,
            "sha256": digest,
            "width": pil_image.width,
            "height": pil_image.height,
        },
    }
    del vae, image, unpacked
    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.synchronize()
    record["reserved_after_release_gib"] = round(torch.cuda.memory_reserved() / 1024**3, 3)
    record["allocated_after_release_gib"] = round(torch.cuda.memory_allocated() / 1024**3, 3)
    return record


def prepare_components(args, sampler: Sampler) -> dict:
    """Quantize the big components once and persist the NF4 checkpoints.

    The quantization transient is inherent to the bitsandbytes loader (it needs
    the bf16 shard in memory before replacing it), so it happens in the install
    prepare step instead of in every generation. The prepared directories are a
    local derived cache of the fixed upstream snapshot, not a new model source.
    """
    from diffusers import QwenImage21Transformer2DModel
    from transformers import Qwen3VLForConditionalGeneration

    args.prepared_dir.mkdir(parents=True, exist_ok=True)
    record: dict = {}

    def dir_bytes(path: Path) -> int:
        return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())

    sampler.reset()
    started = time.perf_counter()
    text_encoder = Qwen3VLForConditionalGeneration.from_pretrained(
        str(args.model_dir),
        subfolder="text_encoder",
        torch_dtype=torch.bfloat16,
        quantization_config=quant_config(),
        device_map="cpu",
        low_cpu_mem_usage=True,
    )
    load_seconds = time.perf_counter() - started
    save_started = time.perf_counter()
    text_encoder.save_pretrained(str(args.prepared_dir / "text_encoder"))
    save_seconds = time.perf_counter() - save_started
    record["text_encoder"] = {
        "load_seconds": round(load_seconds, 2),
        "save_seconds": round(save_seconds, 2),
        "prepared_bytes": dir_bytes(args.prepared_dir / "text_encoder"),
        "resources": sampler.snapshot(),
    }
    del text_encoder
    gc.collect()

    sampler.reset()
    started = time.perf_counter()
    transformer = QwenImage21Transformer2DModel.from_pretrained(
        str(args.model_dir),
        subfolder="transformer",
        torch_dtype=torch.bfloat16,
        quantization_config=quant_config(),
        device_map="cpu",
        low_cpu_mem_usage=True,
    )
    load_seconds = time.perf_counter() - started
    save_started = time.perf_counter()
    transformer.save_pretrained(str(args.prepared_dir / "transformer"))
    save_seconds = time.perf_counter() - save_started
    record["transformer"] = {
        "load_seconds": round(load_seconds, 2),
        "save_seconds": round(save_seconds, 2),
        "prepared_bytes": dir_bytes(args.prepared_dir / "transformer"),
        "resources": sampler.snapshot(),
    }
    del transformer
    gc.collect()
    record["total_prepared_bytes"] = dir_bytes(args.prepared_dir)
    return record


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-dir", required=True, type=Path)
    parser.add_argument("--work-dir", default="target/parity-gate0/staged", type=Path)
    parser.add_argument("--output", default="target/parity-gate0/results/qwen21-staged.png", type=Path)
    parser.add_argument("--report", default="verification/windows-parity-gate0/staged-qwen21.json", type=Path)
    parser.add_argument("--mode", choices=["run", "prepare"], default="run")
    parser.add_argument(
        "--prepared-dir",
        default="target/parity-gate0/staged/prepared-qwen21-turbo",
        type=Path,
    )
    parser.add_argument("--prompt", default="雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光")
    parser.add_argument("--stage", choices=["text", "transformer", "vae", "all"], default="all")
    parser.add_argument("--steps", type=int, default=8)
    parser.add_argument("--width", type=int, default=1024)
    parser.add_argument("--height", type=int, default=1024)
    parser.add_argument("--seed", type=int, default=3148150817)
    parser.add_argument("--vae-tiling", action="store_true", default=True)
    parser.add_argument("--cuda-reserved-limit-gib", type=float, default=10.5)
    parser.add_argument("--private-commit-limit-gib", type=float, default=24)
    parser.add_argument("--available-ram-minimum-gib", type=float, default=2)
    parser.add_argument("--load-limit-seconds", type=float, default=600)
    parser.add_argument("--generation-limit-seconds", type=float, default=900)
    args = parser.parse_args()

    args.model_dir = args.model_dir.resolve()
    args.output = args.output.resolve()
    args.report = args.report.resolve()
    args.work_dir = args.work_dir.resolve()
    args.prepared_dir = args.prepared_dir.resolve()
    args.work_dir.mkdir(parents=True, exist_ok=True)

    stages = ["text", "transformer", "vae"] if args.stage == "all" else [args.stage]
    started_utc = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    record: dict = {
        "kind": "qwen-image-2.1-staged-windows-gate0",
        "started_at_utc": started_utc,
        "machine": {
            "gpu": torch.cuda.get_device_name(0) if torch.cuda.is_available() else None,
            "gpu_total_mib": round(torch.cuda.get_device_properties(0).total_memory / 1024**2)
            if torch.cuda.is_available()
            else None,
            "physical_total_gib": round(system_memory()["physical_total_bytes"] / 1024**3, 2),
        },
        "model_dir": str(args.model_dir),
        "parameters": {
            "prompt": args.prompt,
            "steps": args.steps,
            "width": args.width,
            "height": args.height,
            "seed": args.seed,
            "vae_tiling": args.vae_tiling,
        },
        "quantization": {"backend": "bitsandbytes_4bit", "quant_type": "nf4", **{
            "compute_dtype": "bfloat16", "double_quant": True,
        }},
        "budget": {
            "cuda_reserved_limit_gib": args.cuda_reserved_limit_gib,
            "private_commit_limit_gib": args.private_commit_limit_gib,
            "available_ram_minimum_gib": args.available_ram_minimum_gib,
            "load_limit_seconds": args.load_limit_seconds,
            "generation_limit_seconds": args.generation_limit_seconds,
        },
        "stages": {},
    }

    record["mode"] = args.mode
    record["prepared_dir"] = str(args.prepared_dir)

    import diffusers
    import transformers

    record["versions"] = {
        "diffusers": diffusers.__version__,
        "transformers": transformers.__version__,
        "torch": torch.__version__,
        "cuda": torch.version.cuda,
        "python": os.sys.version.split()[0],
    }

    if args.mode == "prepare":
        sampler = Sampler()
        sampler.start()
        status = "prepared"
        try:
            record["stages"]["prepare"] = prepare_components(args, sampler)
            print("[staged] prepare done", flush=True)
        except BaseException as error:  # noqa: BLE001 - experiment harness
            status = "error"
            record["error"] = f"{type(error).__name__}: {error}"
            record["traceback"] = traceback.format_exc()
        finally:
            sampler.stop()
        record["status"] = status
        record["finished_at_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        write_json(args.report, record)
        print(json.dumps(record, indent=2, ensure_ascii=False), flush=True)
        return 0 if status == "prepared" else 1

    sampler = Sampler()
    sampler.start()
    status = "passed"
    try:
        for stage in stages:
            if stage == "text":
                record["stages"]["text"] = stage_text(args, sampler, args.work_dir)
            elif stage == "transformer":
                record["stages"]["transformer"] = stage_transformer(args, sampler, args.work_dir)
            else:
                record["stages"]["vae"] = stage_vae(args, sampler, args.work_dir)
            print(f"[staged] {stage} done", flush=True)
    except BaseException as error:  # noqa: BLE001 - experiment harness
        status = "error"
        record["error"] = f"{type(error).__name__}: {error}"
        record["traceback"] = traceback.format_exc()
    finally:
        sampler.stop()

    # total peak is the max over stage peaks (the sampler resets per stage)
    if args.prepared_dir.is_dir():
        record["prepared_cache_bytes"] = sum(
            p.stat().st_size for p in args.prepared_dir.rglob("*") if p.is_file()
        )
        record["prepared_components"] = sorted(
            p.name for p in args.prepared_dir.iterdir() if p.is_dir()
        )
    total = {
        "cuda_reserved_peak_gib": max(
            (s["resources"]["cuda_reserved_peak_gib"] for s in record["stages"].values()), default=0
        ),
        "process_tree_private_commit_peak_gib": max(
            (
                s["resources"]["process_tree_private_commit_peak_gib"]
                for s in record["stages"].values()
            ),
            default=0,
        ),
        "system_available_ram_min_gib": min(
            (
                s["resources"]["system_available_ram_min_gib"]
                for s in record["stages"].values()
            ),
            default=0,
        ),
        "nvidia_used_peak_mib": max(
            (s["resources"]["nvidia_used_peak_mib"] for s in record["stages"].values()), default=0
        ),
    }
    record["total"] = total
    failures = []
    if status == "error":
        failures.append("stage error")
    if total["cuda_reserved_peak_gib"] > args.cuda_reserved_limit_gib:
        failures.append("cuda reserved exceeds limit")
    if total["process_tree_private_commit_peak_gib"] > args.private_commit_limit_gib:
        failures.append("private commit exceeds limit")
    if total["system_available_ram_min_gib"] < args.available_ram_minimum_gib:
        failures.append("available RAM below minimum")
    for name, stage in record["stages"].items():
        if stage.get("load_seconds", 0) > args.load_limit_seconds:
            failures.append(f"{name} load exceeds limit")
    record["failures"] = failures
    record["status"] = "passed" if not failures else ("error" if status == "error" else "budget_failed")
    record["finished_at_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    write_json(args.report, record)
    print(json.dumps(record, indent=2, ensure_ascii=False), flush=True)
    return 0 if not failures else 2


if __name__ == "__main__":
    raise SystemExit(main())
