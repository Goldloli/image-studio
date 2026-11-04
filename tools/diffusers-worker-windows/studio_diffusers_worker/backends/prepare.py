"""Install-time prepared-cache builder for the staged Qwen/MOSS models.

``--prepare`` quantizes the fixed upstream snapshot once on the CPU and persists
the NF4 components next to the installed snapshot
(``models/<snapshot>.prepared``). Generation then loads only the prepared
quantized footprint, which keeps the generation-time process commit around the
Gate 0 measurements instead of the quantization transient. The cache is a
derived artifact: it never replaces the fixed snapshot files, and its inventory
(path/bytes/SHA-256) is returned to the installer for reuse decisions.

The rebuild is deterministic: both files of the freshly built Qwen Turbo cache
are byte-identical (same SHA-256) to the cache built independently by the Lead's
Gate 0 staged experiment.

Klein needs no prepared cache: its fresh-process NF4 + model CPU offload strategy
was measured inside the generation budget directly from the fixed snapshot.
"""

from __future__ import annotations

import gc
import hashlib
import json
import sys
import time
from pathlib import Path

from .. import SNAPSHOTS
from ..resources import ResourceSampler


def _inventory(path: Path) -> dict:
    files = sorted(item for item in path.rglob("*") if item.is_file())
    inventory = []
    total = 0
    for item in files:
        data = item.read_bytes()
        digest = hashlib.sha256(data).hexdigest()
        total += len(data)
        inventory.append(
            {
                "relative_path": item.relative_to(path).as_posix(),
                "bytes": len(data),
                "sha256": digest,
            }
        )
    return {
        "path": str(path),
        "file_count": len(inventory),
        "total_bytes": total,
        "files": inventory,
    }


def _component_ready(path: Path) -> bool:
    return (path / "config.json").is_file() and any(
        item.suffix == ".safetensors" for item in path.glob("*.safetensors")
    )


def _build_component(
    label: str, target: Path, loader, sampler: ResourceSampler, reuse: bool
) -> dict:
    if reuse and _component_ready(target):
        print(f"[prepare] {label} reuse", file=sys.stderr, flush=True)
        return {"status": "reused", **_inventory(target)}
    target.mkdir(parents=True, exist_ok=True)
    sampler.reset()
    sampler.start()
    record = None
    try:
        load_started = time.perf_counter()
        model = loader()
        load_seconds = time.perf_counter() - load_started
        print(
            f"[prepare] {label} load_seconds={load_seconds:.2f}",
            file=sys.stderr,
            flush=True,
        )
        save_started = time.perf_counter()
        model.save_pretrained(str(target))
        save_seconds = time.perf_counter() - save_started
        del model
        gc.collect()
        record = {
            "status": "prepared",
            "quantization": "nf4",
            "load_seconds": round(load_seconds, 2),
            "save_seconds": round(save_seconds, 2),
            **_inventory(target),
        }
        return record
    finally:
        sampler.stop()
        resources = sampler.snapshot()
        print(
            f"[prepare] {label} resources="
            + json.dumps(resources, ensure_ascii=False),
            file=sys.stderr,
            flush=True,
        )
        if record is not None:
            record["resources"] = resources


def _bnb_config():
    import torch
    from transformers import BitsAndBytesConfig

    return BitsAndBytesConfig(
        load_in_4bit=True,
        bnb_4bit_quant_type="nf4",
        bnb_4bit_compute_dtype=torch.bfloat16,
        bnb_4bit_use_double_quant=True,
    )


def prepare_qwen(model_dir: Path, prepared_dir: Path, reuse: bool) -> dict:
    from diffusers import QwenImage21Transformer2DModel
    from transformers import Qwen3VLForConditionalGeneration

    import torch

    sampler = ResourceSampler()

    def text_encoder_loader():
        return Qwen3VLForConditionalGeneration.from_pretrained(
            str(model_dir),
            subfolder="text_encoder",
            torch_dtype=torch.bfloat16,
            quantization_config=_bnb_config(),
            device_map="cpu",
            low_cpu_mem_usage=True,
        )

    def transformer_loader():
        return QwenImage21Transformer2DModel.from_pretrained(
            str(model_dir),
            subfolder="transformer",
            torch_dtype=torch.bfloat16,
            quantization_config=_bnb_config(),
            device_map="cpu",
            low_cpu_mem_usage=True,
        )

    record = {
        "kind": "qwen",
        "text_encoder": _build_component(
            "qwen text_encoder",
            prepared_dir / "text_encoder",
            text_encoder_loader,
            sampler,
            reuse,
        ),
        "transformer": _build_component(
            "qwen transformer",
            prepared_dir / "transformer",
            transformer_loader,
            sampler,
            reuse,
        ),
    }
    record["prepared_cache"] = _inventory(prepared_dir)
    return record


def prepare_moss(model_dir: Path, prepared_dir: Path, reuse: bool) -> dict:
    from transformers import AutoModel

    import torch

    from ..vendored import load_moss_pipeline

    module = load_moss_pipeline()
    sampler = ResourceSampler()

    def text_encoder_loader():
        return AutoModel.from_pretrained(
            str(model_dir),
            subfolder="text_encoder",
            dtype=torch.bfloat16,
            quantization_config=_bnb_config(),
            device_map="cpu",
            low_cpu_mem_usage=True,
        )

    def transformer_loader():
        return module.MOSSImageTransformer.from_pretrained(
            str(model_dir),
            subfolder="transformer",
            torch_dtype=torch.bfloat16,
            quantization_config=_bnb_config(),
            device_map="cpu",
            low_cpu_mem_usage=True,
        )

    record = {
        "kind": "moss",
        "text_encoder": _build_component(
            "moss text_encoder",
            prepared_dir / "text_encoder",
            text_encoder_loader,
            sampler,
            reuse,
        ),
        "transformer": _build_component(
            "moss transformer",
            prepared_dir / "transformer",
            transformer_loader,
            sampler,
            reuse,
        ),
    }
    record["prepared_cache"] = _inventory(prepared_dir)
    return record


def prepare_cache(
    snapshot_id: str, model_dir: Path, prepared_dir: Path, reuse: bool
) -> dict:
    started = time.monotonic()
    model_dir = Path(model_dir)
    prepared_dir = Path(prepared_dir)
    kind = SNAPSHOTS[snapshot_id]["kind"]
    result: dict = {
        "schema_version": 1,
        "kind": "windows-diffusers-prepared-cache",
        "model_snapshot_id": snapshot_id,
        "worker_kind": kind,
        "model_dir": str(model_dir),
        "prepared_dir": str(prepared_dir),
        "mode": "reuse_then_prepare" if reuse else "rebuild",
    }
    try:
        if not model_dir.is_dir() or not (model_dir / "model_index.json").is_file():
            raise ValueError("model snapshot directory is incomplete")
        if kind == "klein":
            result["status"] = "not_required"
            result["reason"] = (
                "Klein uses fresh-process NF4 + model_cpu_offload directly from "
                "the fixed snapshot; no prepared cache is needed."
            )
            return result
        prepared_dir.mkdir(parents=True, exist_ok=True)
        if kind == "qwen":
            components = prepare_qwen(model_dir, prepared_dir, reuse)
        elif kind == "moss":
            components = prepare_moss(model_dir, prepared_dir, reuse)
        else:  # pragma: no cover - snapshots are closed
            raise ValueError(f"unsupported worker kind: {kind}")
        result.update(components)
        cached = components.get("text_encoder", {}).get("status") in (
            "prepared",
            "reused",
        ) and components.get("transformer", {}).get("status") in (
            "prepared",
            "reused",
        )
        result["status"] = "prepared" if cached else "error"
        result["elapsed_seconds"] = round(time.monotonic() - started, 2)
        return result
    except BaseException as error:  # noqa: BLE001 - prepare is installer-facing
        import traceback

        result["status"] = "error"
        result["error"] = f"{type(error).__name__}: {error}"
        result["traceback"] = traceback.format_exc()
        result["elapsed_seconds"] = round(time.monotonic() - started, 2)
        return result
