"""Probe: quantize the Qwen-Image 2.1 transformer to NF4 on CPU, save it, and
reload the pre-quantized checkpoint to measure the generation-time footprint."""

from __future__ import annotations

import json
import os
import time
from pathlib import Path

import torch

MODEL = Path("target/parity-gate0/models/qwen-image-2.1-turbo")
QUANT_DIR = Path("target/parity-gate0/staged/qwen21-transformer-nf4")
OUT = Path("target/parity-gate0/staged/transformer-save-load.json")


def snapshot(label: str) -> dict:
    return {
        "label": label,
        "cuda_reserved_gib": round(torch.cuda.memory_reserved() / 1024**3, 3),
        "cuda_allocated_gib": round(torch.cuda.memory_allocated() / 1024**3, 3),
        "cuda_max_reserved_gib": round(torch.cuda.max_memory_reserved() / 1024**3, 3),
    }


def quant_config():
    from transformers import BitsAndBytesConfig

    return BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )


def main() -> None:
    from diffusers import QwenImage21Transformer2DModel

    records = [snapshot("before")]
    prepare = {}
    if not (QUANT_DIR / "config.json").is_file():
        started = time.perf_counter()
        model = QwenImage21Transformer2DModel.from_pretrained(
            str(MODEL),
            subfolder="transformer",
            torch_dtype=torch.bfloat16,
            quantization_config=quant_config(),
            device_map="cpu",
            low_cpu_mem_usage=True,
        )
        prepare["load_seconds"] = round(time.perf_counter() - started, 2)
        records.append(snapshot("after_quantize"))
        started = time.perf_counter()
        QUANT_DIR.mkdir(parents=True, exist_ok=True)
        model.save_pretrained(str(QUANT_DIR))
        prepare["save_seconds"] = round(time.perf_counter() - started, 2)
        del model

    total_bytes = sum(p.stat().st_size for p in QUANT_DIR.iterdir()) if QUANT_DIR.is_dir() else 0
    started = time.perf_counter()
    reloaded = QwenImage21Transformer2DModel.from_pretrained(
        str(QUANT_DIR), torch_dtype=torch.bfloat16, device_map="cpu", low_cpu_mem_usage=True
    )
    reload_seconds = time.perf_counter() - started
    records.append(snapshot("after_reload_cpu"))
    started = time.perf_counter()
    reloaded = reloaded.to("cuda:0")
    move_seconds = time.perf_counter() - started
    torch.cuda.synchronize()
    records.append(snapshot("after_move"))
    quantizer = getattr(reloaded, "hf_quantizer", None)
    linear_types = sorted(
        {type(m).__name__ for n, m in reloaded.named_modules() if "to_q" in n or "q_proj" in n}
    )
    payload = {
        "prepare": prepare,
        "reload_seconds": round(reload_seconds, 2),
        "move_seconds": round(move_seconds, 2),
        "quantized_dir_bytes": total_bytes,
        "quantized_files": sorted(p.name for p in QUANT_DIR.iterdir()) if QUANT_DIR.is_dir() else [],
        "snapshots": records,
        "hf_quantizer": type(quantizer).__name__ if quantizer else None,
        "linear_types": linear_types,
        "config_quantization": bool(getattr(reloaded.config, "quantization_config", None)),
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload, indent=2))


if __name__ == "__main__":
    os.environ.setdefault("HF_HUB_OFFLINE", "1")
    raise SystemExit(main())
