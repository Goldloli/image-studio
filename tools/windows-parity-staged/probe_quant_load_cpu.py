"""Probe CPU-first NF4 loading for the Qwen3-VL text encoder.

Goal: avoid the transient GPU spike of on-the-fly bitsandbytes quantization by
quantizing on CPU first (if supported) and then moving the quantized modules to
CUDA.
"""

from __future__ import annotations

import json
import os
import time
from pathlib import Path

import torch

MODEL = Path("target/parity-gate0/models/qwen-image-2.1-turbo")
OUT = Path("target/parity-gate0/staged/quant-probe-cpu.json")


def snapshot(label: str) -> dict:
    return {
        "label": label,
        "cuda_reserved_gib": round(torch.cuda.memory_reserved() / 1024**3, 3),
        "cuda_allocated_gib": round(torch.cuda.memory_allocated() / 1024**3, 3),
        "cuda_max_reserved_gib": round(torch.cuda.max_memory_reserved() / 1024**3, 3),
    }


def main() -> None:
    from transformers import BitsAndBytesConfig, Qwen3VLForConditionalGeneration

    records = [snapshot("before")]
    torch.cuda.init()
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
        device_map="cpu",
        low_cpu_mem_usage=True,
    )
    cpu_load_seconds = time.perf_counter() - started
    records.append(snapshot("after_cpu_load"))
    linear_types = set()
    for name, module in model.named_modules():
        if "q_proj" in name:
            linear_types.add(type(module).__name__)
    quantizer = getattr(model, "hf_quantizer", None)
    param_device = next(model.parameters()).device

    started = time.perf_counter()
    model = model.to("cuda:0")
    move_seconds = time.perf_counter() - started
    torch.cuda.synchronize()
    records.append(snapshot("after_move"))

    payload = {
        "cpu_load_seconds": round(cpu_load_seconds, 2),
        "move_seconds": round(move_seconds, 2),
        "snapshots": records,
        "linear_types": sorted(linear_types),
        "hf_quantizer": type(quantizer).__name__ if quantizer else None,
        "param_device_before_move": str(param_device),
        "param_device_after_move": str(next(model.parameters()).device),
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload, indent=2))


if __name__ == "__main__":
    os.environ.setdefault("HF_HUB_OFFLINE", "1")
    raise SystemExit(main())
