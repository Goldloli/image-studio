"""Probe how the Qwen3-VL text encoder loads under bitsandbytes NF4."""

from __future__ import annotations

import json
import os
import time
from pathlib import Path

import torch

MODEL = Path("target/parity-gate0/models/qwen-image-2.1-turbo")
OUT = Path("target/parity-gate0/staged/quant-probe.json")


def snapshot(label: str) -> dict:
    return {
        "label": label,
        "cuda_reserved_gib": round(torch.cuda.memory_reserved() / 1024**3, 3),
        "cuda_allocated_gib": round(torch.cuda.memory_allocated() / 1024**3, 3),
        "cuda_max_reserved_gib": round(torch.cuda.max_memory_reserved() / 1024**3, 3),
    }


def main() -> None:
    from transformers import BitsAndBytesConfig, Qwen3VLForConditionalGeneration

    records = []
    torch.cuda.init()
    records.append(snapshot("before"))

    started = time.perf_counter()
    model = Qwen3VLForConditionalGeneration.from_pretrained(
        str(MODEL),
        subfolder="text_encoder",
        torch_dtype=torch.bfloat16,
        quantization_config=BitsAndBytesConfig(
            load_in_4bit=True,
            bnb_4bit_quant_type="nf4",
            bnb_4bit_compute_dtype=torch.bfloat16,
            bnb_4bit_use_double_quant=True,
        ),
        device_map="cuda:0",
    )
    load_seconds = time.perf_counter() - started
    records.append(snapshot("after_load"))

    linear_types = set()
    for name, module in model.named_modules():
        if "q_proj" in name or "gate_proj" in name:
            linear_types.add(type(module).__name__)
            if len(linear_types) >= 3:
                break
    quantizer = getattr(model, "hf_quantizer", None)
    config_quant = getattr(model.config, "quantization_config", None)
    device = next(model.parameters()).device
    payload = {
        "load_seconds": round(load_seconds, 2),
        "snapshots": records,
        "linear_types": sorted(linear_types),
        "hf_quantizer": type(quantizer).__name__ if quantizer else None,
        "config_quantization": type(config_quant).__name__ if config_quant else None,
        "device": str(device),
        "model_class": type(model).__name__,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload, indent=2))

    del model
    torch.cuda.empty_cache()
    print("after release", snapshot("released"))


if __name__ == "__main__":
    os.environ.setdefault("HF_HUB_OFFLINE", "1")
    raise SystemExit(main())
