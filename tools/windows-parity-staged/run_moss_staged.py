"""Experiment-only staged Gate 0 for MOSS-Image-SFT on Windows.

Same shape as run_qwen21_staged.py, but MOSS ships its own pipeline module
(``pipeline_moss_image.py``) with a custom DiT class and scheduler, plus a
Qwen3.5-9B text encoder. Product recipe: 50 steps, CFG 3.0, empty negative
prompt, negative branch skipping block 22.

Stages:
1. text - Qwen3.5 text encoder (NF4) encodes positive and negative prompts.
2. dit  - MOSSImageTransformer (INT8 or NF4) runs 50 CFG steps, then released.
3. vae  - FLUX.2 VAE (fp32) decodes, then released.

``--mode prepare`` quantizes the text encoder and the DiT once and persists the
pre-quantized checkpoints; ``--mode run`` uses them so generation-time memory
only holds the quantized footprint (the bf16 quantization transient is moved to
the install prepare step).
"""

from __future__ import annotations

import argparse
import gc
import hashlib
import importlib.util
import json
import os
import sys
import time
import traceback
from pathlib import Path

import torch

# The product Python is the embeddable distribution with a `._pth` file, so the
# script directory is not on sys.path automatically.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from run_qwen21_staged import Sampler, system_memory, write_json


def load_pipeline_module(model_dir: Path):
    path = model_dir / "pipeline_moss_image.py"
    spec = importlib.util.spec_from_file_location("pipeline_moss_image", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules["pipeline_moss_image"] = module
    spec.loader.exec_module(module)
    return module


def bnb_config(quant: str):
    from transformers import BitsAndBytesConfig

    if quant == "nf4":
        return BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=torch.bfloat16,
            bnb_4bit_use_double_quant=True,
        )
    if quant == "int8":
        return BitsAndBytesConfig(load_in_8bit=True)
    raise ValueError(f"unsupported quantization {quant}")


def load_text_encoder(model_dir: Path, prepared_dir: Path | None):
    from transformers import AutoModel

    source = prepared_dir if prepared_dir is not None and (prepared_dir / "config.json").is_file() else model_dir
    kwargs = dict(dtype=torch.bfloat16, device_map="cpu", low_cpu_mem_usage=True)
    if source is model_dir:
        kwargs["subfolder"] = "text_encoder"
        kwargs["quantization_config"] = bnb_config("nf4")
    encoder = AutoModel.from_pretrained(str(source), **kwargs)
    encoder = encoder.to("cuda:0")
    encoder.eval()
    return encoder


def load_tokenizer(model_dir: Path):
    from transformers import AutoTokenizer

    return AutoTokenizer.from_pretrained(str(model_dir), subfolder="tokenizer")


def encode_prompt(tokenizer, text_encoder, prompt: str, max_sequence_length: int = 4096):
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


def stage_text(args, sampler: Sampler, work: Path) -> dict:
    sampler.reset()
    started = time.perf_counter()
    tokenizer = load_tokenizer(args.model_dir)
    text_encoder = load_text_encoder(
        args.model_dir, args.prepared_dir / "text_encoder" if args.prepared_dir else None
    )
    load_seconds = time.perf_counter() - started
    sampler.reset()
    encode_started = time.perf_counter()
    positive, positive_tokens = encode_prompt(tokenizer, text_encoder, args.prompt, args.max_sequence_length)
    negative, negative_tokens = encode_prompt(tokenizer, text_encoder, args.negative_prompt, args.max_sequence_length)
    encode_seconds = time.perf_counter() - encode_started
    max_length = max(positive.shape[0], negative.shape[0])

    def pad(row):
        padded = row.new_zeros(max_length, row.shape[-1])
        padded[: row.shape[0]] = row
        mask = torch.zeros(max_length, dtype=torch.bool)
        mask[: row.shape[0]] = True
        return padded, mask

    positive, positive_mask = pad(positive)
    negative, negative_mask = pad(negative)
    torch.save(
        {
            "positive": positive,
            "positive_mask": positive_mask,
            "negative": negative,
            "negative_mask": negative_mask,
            "prompt": args.prompt,
            "negative_prompt": args.negative_prompt,
        },
        work / "prompt.pt",
    )
    record = {
        "load_seconds": round(load_seconds, 2),
        "encode_seconds": round(encode_seconds, 2),
        "positive_tokens": positive_tokens,
        "negative_tokens": negative_tokens,
        "padded_tokens": max_length,
        "resources": sampler.snapshot(),
    }
    del text_encoder, tokenizer
    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.synchronize()
    record["reserved_after_release_gib"] = round(torch.cuda.memory_reserved() / 1024**3, 3)
    record["allocated_after_release_gib"] = round(torch.cuda.memory_allocated() / 1024**3, 3)
    return record


def load_transformer(module, model_dir: Path, prepared_dir: Path | None):
    source = prepared_dir if prepared_dir is not None and (prepared_dir / "config.json").is_file() else model_dir
    kwargs = dict(torch_dtype=torch.bfloat16, device_map="cpu", low_cpu_mem_usage=True)
    if source is model_dir:
        kwargs["subfolder"] = "transformer"
        kwargs["quantization_config"] = bnb_config("int8")
    transformer = module.MOSSImageTransformer.from_pretrained(str(source), **kwargs)
    transformer = transformer.to("cuda:0")
    transformer.eval()
    return transformer


def stage_dit(args, sampler: Sampler, work: Path) -> dict:
    from diffusers.utils.torch_utils import randn_tensor

    module = load_pipeline_module(args.model_dir)
    prompt = torch.load(work / "prompt.pt", map_location="cpu", weights_only=False)
    positive = prompt["positive"].to("cuda:0")
    positive_mask = prompt["positive_mask"].to("cuda:0")
    negative = prompt["negative"].to("cuda:0")
    negative_mask = prompt["negative_mask"].to("cuda:0")

    sampler.reset()
    started = time.perf_counter()
    transformer = load_transformer(
        module, args.model_dir, args.prepared_dir / "transformer" if args.prepared_dir else None
    )
    scheduler = module.MOSSImageScheduler()
    scheduler.set_timesteps(args.steps, device="cuda:0")
    generator = torch.Generator(device="cpu").manual_seed(args.seed)
    latents = randn_tensor(
        (1, transformer.config.latent_channels, args.height // 16, args.width // 16),
        generator=generator,
        device="cuda:0",
        dtype=torch.float32,
    )
    load_seconds = time.perf_counter() - started

    sampler.reset()
    sample_started = time.perf_counter()
    steps = len(scheduler.timesteps)
    with torch.no_grad():
        for index, timestep in enumerate(scheduler.timesteps):
            prediction = transformer(latents, timestep, positive, positive_mask).sample.float()
            uncond = transformer(
                latents, timestep, negative, negative_mask, skip_layers=(22,)
            ).sample.float()
            prediction = uncond + args.guidance_scale * (prediction - uncond)
            latents = scheduler.step(prediction, timestep, latents, return_dict=False)[0]
    torch.cuda.synchronize()
    sample_seconds = time.perf_counter() - sample_started

    torch.save(
        {
            "latents": latents.cpu(),
            "height": args.height,
            "width": args.width,
            "steps": steps,
            "seed": args.seed,
        },
        work / "latents.pt",
    )
    record = {
        "load_seconds": round(load_seconds, 2),
        "sample_seconds": round(sample_seconds, 2),
        "steps": steps,
        "resources": sampler.snapshot(),
    }
    del transformer, scheduler
    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.synchronize()
    record["reserved_after_release_gib"] = round(torch.cuda.memory_reserved() / 1024**3, 3)
    record["allocated_after_release_gib"] = round(torch.cuda.memory_allocated() / 1024**3, 3)
    return record


def stage_vae(args, sampler: Sampler, work: Path) -> dict:
    from diffusers import AutoencoderKLFlux2
    from diffusers.image_processor import VaeImageProcessor

    payload = torch.load(work / "latents.pt", map_location="cpu", weights_only=False)
    latents = payload["latents"]

    sampler.reset()
    started = time.perf_counter()
    vae = AutoencoderKLFlux2.from_pretrained(
        str(args.model_dir), subfolder="vae", torch_dtype=torch.float32
    ).to("cuda:0")
    vae.eval()
    load_seconds = time.perf_counter() - started

    sampler.reset()
    decode_started = time.perf_counter()
    with torch.no_grad():
        z = latents.to("cuda:0", dtype=vae.dtype)
        bn = vae.bn
        eps = vae.config.batch_norm_eps
        z = z * (bn.running_var + eps).sqrt().to(z).view(1, -1, 1, 1) + bn.running_mean.to(z).view(
            1, -1, 1, 1
        )
        batch, channels, height, width = z.shape
        z = (
            z.reshape(batch, channels // 4, 2, 2, height, width)
            .permute(0, 1, 4, 2, 5, 3)
            .reshape(batch, channels // 4, 2 * height, 2 * width)
        )
        images = vae.decode(z, return_dict=False)[0].clamp(-1, 1)
    torch.cuda.synchronize()
    decode_seconds = time.perf_counter() - decode_started
    processor = VaeImageProcessor(vae_scale_factor=16)
    pil_image = processor.postprocess(images, output_type="pil")[0]
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
    del vae, images, z
    gc.collect()
    torch.cuda.empty_cache()
    torch.cuda.synchronize()
    record["reserved_after_release_gib"] = round(torch.cuda.memory_reserved() / 1024**3, 3)
    record["allocated_after_release_gib"] = round(torch.cuda.memory_allocated() / 1024**3, 3)
    return record


def prepare_components(args, sampler: Sampler) -> dict:
    """Quantize the text encoder (NF4) and the DiT (INT8, NF4 fallback) once."""
    from transformers import AutoModel

    module = load_pipeline_module(args.model_dir)
    prepared = args.prepared_dir
    prepared.mkdir(parents=True, exist_ok=True)

    def dir_bytes(path: Path) -> int:
        return sum(p.stat().st_size for p in path.rglob("*") if p.is_file())

    record: dict = {}
    sampler.reset()
    started = time.perf_counter()
    text_encoder = AutoModel.from_pretrained(
        str(args.model_dir),
        subfolder="text_encoder",
        dtype=torch.bfloat16,
        quantization_config=bnb_config("nf4"),
        device_map="cpu",
        low_cpu_mem_usage=True,
    )
    load_seconds = time.perf_counter() - started
    save_started = time.perf_counter()
    text_encoder.save_pretrained(str(prepared / "text_encoder"))
    save_seconds = time.perf_counter() - save_started
    record["text_encoder"] = {
        "quantization": "nf4",
        "load_seconds": round(load_seconds, 2),
        "save_seconds": round(save_seconds, 2),
        "prepared_bytes": dir_bytes(prepared / "text_encoder"),
        "resources": sampler.snapshot(),
    }
    del text_encoder
    gc.collect()

    dit_records = {}
    for quant in ("int8", "nf4"):
        target = prepared / "transformer"
        if (target / "config.json").is_file():
            dit_records[quant] = {"status": "already_prepared", "prepared_bytes": dir_bytes(target)}
            continue
        sampler.reset()
        started = time.perf_counter()
        try:
            transformer = module.MOSSImageTransformer.from_pretrained(
                str(args.model_dir),
                subfolder="transformer",
                torch_dtype=torch.bfloat16,
                quantization_config=bnb_config(quant),
                device_map="cpu",
                low_cpu_mem_usage=True,
            )
            load_seconds = time.perf_counter() - started
            save_started = time.perf_counter()
            target.mkdir(parents=True, exist_ok=True)
            transformer.save_pretrained(str(target))
            save_seconds = time.perf_counter() - save_started
            dit_records[quant] = {
                "status": "prepared",
                "load_seconds": round(load_seconds, 2),
                "save_seconds": round(save_seconds, 2),
                "prepared_bytes": dir_bytes(target),
                "resources": sampler.snapshot(),
            }
            del transformer
            gc.collect()
            break
        except BaseException as error:  # noqa: BLE001 - experiment fallback
            dit_records[quant] = {
                "status": "failed",
                "error": f"{type(error).__name__}: {error}",
                "traceback": traceback.format_exc(),
            }
    record["transformer"] = dit_records
    record["total_prepared_bytes"] = dir_bytes(prepared)
    return record


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--model-dir", required=True, type=Path)
    parser.add_argument("--prepared-dir", default="target/parity-gate0/staged/prepared-moss", type=Path)
    parser.add_argument("--work-dir", default="target/parity-gate0/staged/moss-work", type=Path)
    parser.add_argument("--output", default="target/parity-gate0/results/moss-staged.png", type=Path)
    parser.add_argument("--report", default="verification/windows-parity-gate0/staged-moss.json", type=Path)
    parser.add_argument("--mode", choices=["run", "prepare"], default="run")
    parser.add_argument("--stage", choices=["text", "dit", "vae", "all"], default="all")
    parser.add_argument("--prompt", default="雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光")
    parser.add_argument("--negative-prompt", default="")
    parser.add_argument("--steps", type=int, default=50)
    parser.add_argument("--guidance-scale", type=float, default=3.0)
    parser.add_argument("--width", type=int, default=1024)
    parser.add_argument("--height", type=int, default=1024)
    parser.add_argument("--seed", type=int, default=3148150817)
    parser.add_argument("--max-sequence-length", type=int, default=4096)
    parser.add_argument("--cuda-reserved-limit-gib", type=float, default=10.5)
    parser.add_argument("--private-commit-limit-gib", type=float, default=24)
    parser.add_argument("--available-ram-minimum-gib", type=float, default=2)
    parser.add_argument("--load-limit-seconds", type=float, default=600)
    parser.add_argument("--generation-limit-seconds", type=float, default=900)
    args = parser.parse_args()

    args.model_dir = args.model_dir.resolve()
    args.prepared_dir = args.prepared_dir.resolve()
    args.work_dir = args.work_dir.resolve()
    args.output = args.output.resolve()
    args.report = args.report.resolve()
    args.work_dir.mkdir(parents=True, exist_ok=True)

    stages = ["text", "dit", "vae"] if args.stage == "all" else [args.stage]
    record: dict = {
        "kind": "moss-image-sft-staged-windows-gate0",
        "started_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "machine": {
            "gpu": torch.cuda.get_device_name(0) if torch.cuda.is_available() else None,
            "gpu_total_mib": round(torch.cuda.get_device_properties(0).total_memory / 1024**2)
            if torch.cuda.is_available()
            else None,
            "physical_total_gib": round(system_memory()["physical_total_bytes"] / 1024**3, 2),
        },
        "model_dir": str(args.model_dir),
        "prepared_dir": str(args.prepared_dir),
        "mode": args.mode,
        "parameters": {
            "prompt": args.prompt,
            "negative_prompt": args.negative_prompt,
            "steps": args.steps,
            "guidance_scale": args.guidance_scale,
            "uncond_skip_layers": [22],
            "width": args.width,
            "height": args.height,
            "seed": args.seed,
            "max_sequence_length": args.max_sequence_length,
        },
        "budget": {
            "cuda_reserved_limit_gib": args.cuda_reserved_limit_gib,
            "private_commit_limit_gib": args.private_commit_limit_gib,
            "available_ram_minimum_gib": args.available_ram_minimum_gib,
            "load_limit_seconds": args.load_limit_seconds,
            "generation_limit_seconds": args.generation_limit_seconds,
        },
        "stages": {},
    }

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
            print("[moss] prepare done", flush=True)
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
            elif stage == "dit":
                record["stages"]["dit"] = stage_dit(args, sampler, args.work_dir)
            else:
                record["stages"]["vae"] = stage_vae(args, sampler, args.work_dir)
            print(f"[moss] {stage} done", flush=True)
    except BaseException as error:  # noqa: BLE001 - experiment harness
        status = "error"
        record["error"] = f"{type(error).__name__}: {error}"
        record["traceback"] = traceback.format_exc()
    finally:
        sampler.stop()

    if args.prepared_dir.is_dir():
        record["prepared_cache_bytes"] = sum(
            p.stat().st_size for p in args.prepared_dir.rglob("*") if p.is_file()
        )
    total = {
        "cuda_reserved_peak_gib": max(
            (s["resources"]["cuda_reserved_peak_gib"] for s in record["stages"].values()), default=0
        ),
        "process_tree_private_commit_peak_gib": max(
            (s["resources"]["process_tree_private_commit_peak_gib"] for s in record["stages"].values()),
            default=0,
        ),
        "system_available_ram_min_gib": min(
            (s["resources"]["system_available_ram_min_gib"] for s in record["stages"].values()),
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
    dit = record["stages"].get("dit", {})
    if dit.get("sample_seconds", 0) > args.generation_limit_seconds:
        failures.append("sampling exceeds generation limit")
    record["failures"] = failures
    record["status"] = "passed" if not failures else ("error" if status == "error" else "budget_failed")
    record["finished_at_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    write_json(args.report, record)
    print(json.dumps(record, indent=2, ensure_ascii=False), flush=True)
    return 0 if not failures else 2


if __name__ == "__main__":
    raise SystemExit(main())
