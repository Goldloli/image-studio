"""Probe: save a CPU-quantized NF4 text encoder and reload it without the
bf16 transient.

If the pre-quantized checkpoint reloads directly, generation-time CPU commit
drops from ~25 GiB to the quantized footprint, and the fixed upstream snapshot
stays the installation source (the quantized copy is a local prepared cache).
"""

from __future__ import annotations

import json
import os
import time
from pathlib import Path

import torch

MODEL = Path("target/parity-gate0/models/qwen-image-2.1-turbo")
QUANT_DIR = Path("target/parity-gate0/staged/qwen21-textencoder-nf4")
OUT = Path("target/parity-gate0/staged/quant-save-load.json")


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
    from transformers import Qwen3VLForConditionalGeneration

    records = [snapshot("before")]
    if not (QUANT_DIR / "config.json").is_file():
        started = time.perf_counter()
        model = Qwen3VLForConditionalGeneration.from_pretrained(
            str(MODEL),
            subfolder="text_encoder",
            torch_dtype=torch.bfloat16,
            quantization_config=quant_config(),
            device_map="cpu",
            low_cpu_mem_usage=True,
        )
        load_seconds = time.perf_counter() - started
        started = time.perf_counter()
        QUANT_DIR.mkdir(parents=True, exist_ok=True)
        model.save_pretrained(str(QUANT_DIR))
        save_seconds = time.perf_counter() - started
        records.append(snapshot("after_quantize"))
        del model
    else:
        load_seconds = save_seconds = None

    files = sorted(p.name for p in QUANT_DIR.iterdir()) if QUANT_DIR.is_dir() else []
    total_bytes = sum(p.stat().st_size for p in QUANT_DIR.iterdir()) if QUANT_DIR.is_dir() else 0

    started = time.perf_counter()
    reloaded = Qwen3VLForConditionalGeneration.from_pretrained(
        str(QUANT_DIR),
        torch_dtype=torch.bfloat16,
        device_map="cpu",
        low_cpu_mem_usage=True,
    )
    reload_seconds = time.perf_counter() - started
    records.append(snapshot("after_reload_cpu"))
    quantizer = getattr(reloaded, "hf_quantizer", None)
    linear_types = sorted(
        {type(m).__name__ for n, m in reloaded.named_modules() if "q_proj" in n}
    )
    started = time.perf_counter()
    reloaded = reloaded.to("cuda:0")
    move_seconds = time.perf_counter() - started
    torch.cuda.synchronize()
    records.append(snapshot("after_move"))

    payload = {
        "quantize_load_seconds": round(load_seconds, 2) if load_seconds else None,
        "save_seconds": round(save_seconds, 2) if save_seconds else None,
        "reload_seconds": round(reload_seconds, 2),
        "move_seconds": round(move_seconds, 2),
        "quantized_dir_bytes": total_bytes,
        "quantized_files": files,
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
