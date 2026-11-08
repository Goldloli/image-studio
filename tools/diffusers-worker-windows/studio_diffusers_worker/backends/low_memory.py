"""Low-memory install-time quantization for the staged Qwen/MOSS models.

The legacy path (``prepare.py``) constructs a whole component on the CPU with
``from_pretrained(quantization_config=..., device_map="cpu")``. Measured commit
peaks were 16.4-23.1 GiB, which blocks installation on 16 GiB machines.

This module splits the quantization work into short-lived child processes: each
child loads only its assigned tensors from the fixed snapshot, quantizes them to
NF4 with the same bitsandbytes call the legacy path uses, writes a safetensors
fragment in the exact bnb save layout, and exits. The coordinator then streams
the fragments (and the non-quantized tensors, copied byte-for-byte from the
snapshot) into the final prepared-cache safetensors without ever holding a full
component in memory.

Child-process invocation (also usable manually)::

    python -m studio_diffusers_worker.backends.low_memory --job <job.json>

The coordinator calls itself with the current interpreter, so the installed
runtime is reused unchanged.
"""

from __future__ import annotations

import gc
import hashlib
import json
import os
import struct
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

QUANT_SUFFIXES = (
    ".absmax",
    ".nested_absmax",
    ".nested_quant_map",
    ".quant_map",
    ".quant_state.bitsandbytes__nf4",
)

_BNB_QUANT_CONFIG = {
    "_load_in_4bit": True,
    "_load_in_8bit": False,
    "bnb_4bit_compute_dtype": "bfloat16",
    "bnb_4bit_quant_storage": "uint8",
    "bnb_4bit_quant_type": "nf4",
    "bnb_4bit_use_double_quant": True,
    "llm_int8_enable_fp32_cpu_offload": False,
    "llm_int8_has_fp16_weight": False,
    "llm_int8_skip_modules": None,
    "llm_int8_threshold": 6.0,
    "load_in_4bit": True,
    "load_in_8bit": False,
    "quant_method": "bitsandbytes",
}

_DEFAULT_GROUP_BYTES = 1 << 30  # 1 GiB of source bf16 bytes per child process
_COPY_CHUNK = 8 << 20


def log(message: str) -> None:
    print(message, file=sys.stderr, flush=True)


# ---------------------------------------------------------------------------
# safetensors helpers (header parsing + streamed assembly)
# ---------------------------------------------------------------------------


def _read_header(path: Path) -> tuple[dict, int]:
    with open(path, "rb") as handle:
        raw = handle.read(8)
        if len(raw) != 8:
            raise ValueError(f"{path} is too small to be a safetensors file")
        (length,) = struct.unpack("<Q", raw)
        header = json.loads(handle.read(length))
    return header, 8 + length


def _header_bytes(header: dict) -> bytes:
    data = json.dumps(header, separators=(",", ":")).encode("utf-8")
    padding = (-len(data)) % 8
    return data + b" " * padding


_DTYPE_ALIGNMENT = {
    "F64": 8,
    "I64": 8,
    "U64": 8,
    "F32": 4,
    "I32": 4,
    "U32": 4,
    "F16": 2,
    "BF16": 2,
    "I16": 2,
    "U16": 2,
    "I8": 1,
    "U8": 1,
    "BOOL": 1,
    "F8_E4M3": 1,
    "F8_E5M2": 1,
}


def _ordered_header_keys(shapes: dict[str, tuple[str, list[int]]]) -> list[str]:
    """Reproduce the safetensors writer order: dtype groups by descending
    alignment, each group sorted lexicographically, metadata first."""
    groups: dict[str, list[str]] = {}
    for key, (dtype, _shape) in shapes.items():
        groups.setdefault(dtype, []).append(key)
    ordered: list[str] = []
    for dtype in sorted(groups, key=lambda item: (-_DTYPE_ALIGNMENT.get(item, 1), item)):
        ordered.extend(sorted(groups[dtype]))
    return ordered


def _write_streamed(
    path: Path,
    shapes: dict[str, tuple[str, list[int]]],
    sources: dict[str, tuple[Path, int, int]],
) -> dict:
    """Write a safetensors file from ranges without materialising the tensors.

    ``sources`` maps key -> (file, offset, length). Header entries are generated
    here so offsets are always consistent with the written layout.
    """
    ordered = _ordered_header_keys(shapes)
    if sorted(ordered) != sorted(sources):
        raise ValueError("header keys and sources disagree")
    offset = 0
    data_entries = {}
    for key in ordered:
        _, _, length = sources[key]
        dtype, shape = shapes[key]
        data_entries[key] = {
            "dtype": dtype,
            "shape": shape,
            "data_offsets": [offset, offset + length],
        }
        offset += length
    header_out = {"__metadata__": {"format": "pt"}}
    header_out.update(data_entries)
    blob = _header_bytes(header_out)
    digest = hashlib.sha256()
    path.parent.mkdir(parents=True, exist_ok=True)
    with open(path, "wb") as out:
        out.write(struct.pack("<Q", len(blob)))
        out.write(blob)
        digest.update(struct.pack("<Q", len(blob)))
        digest.update(blob)
        buffer = bytearray(_COPY_CHUNK)
        view = memoryview(buffer)
        for key in ordered:
            source, start, length = sources[key]
            with open(source, "rb") as handle:
                handle.seek(start)
                remaining = length
                while remaining:
                    take = min(remaining, _COPY_CHUNK)
                    read = handle.readinto(view[:take])
                    if read != take:
                        raise OSError(f"short read from {source} for {key}")
                    out.write(view[:take])
                    digest.update(view[:take])
                    remaining -= take
    return {"bytes": 8 + len(blob) + offset, "sha256": digest.hexdigest()}


# ---------------------------------------------------------------------------
# meta skeleton: derive tensor order + which parameters are quantized
# ---------------------------------------------------------------------------


def _replace_linears(module, skip_names=()) -> None:
    import bitsandbytes as bnb
    import torch

    for name, child in list(module.named_children()):
        if isinstance(child, bnb.nn.Linear4bit):
            continue
        if isinstance(child, torch.nn.Linear) and name not in skip_names:
            setattr(
                module,
                name,
                bnb.nn.Linear4bit(
                    child.in_features,
                    child.out_features,
                    bias=child.bias is not None,
                    compute_dtype=torch.bfloat16,
                    compress_statistics=True,
                    quant_type="nf4",
                    quant_storage=torch.uint8,
                    device="meta",
                ),
            )
        else:
            _replace_linears(child, skip_names)


def _walk(module, prefix=""):
    import bitsandbytes as bnb

    for name, param in module._parameters.items():
        if param is not None:
            yield prefix + name
    for name, buffer in module._buffers.items():
        if buffer is None or name in getattr(module, "_non_persistent_buffers_set", set()):
            continue
        yield prefix + name
    if isinstance(module, bnb.nn.Linear4bit):
        for suffix in (".absmax", ".nested_absmax", ".nested_quant_map", ".quant_map"):
            yield prefix + "weight" + suffix
        yield prefix + "weight.quant_state.bitsandbytes__nf4"
    for name, child in module._modules.items():
        yield from _walk(child, prefix + name + ".")


def _quantized_keys(module) -> set[str]:
    import bitsandbytes as bnb

    found = set()
    for name, child in module.named_modules():
        if isinstance(child, bnb.nn.Linear4bit):
            found.add((name + ".weight") if name else "weight")
    return found


def _skeleton(component: str, source_dir: Path):
    """Return (ordered base parameter names, quantized parameter name set)."""
    import json as _json

    import torch
    from accelerate import init_empty_weights

    from .. import SNAPSHOTS

    if component == "qwen_transformer":
        from diffusers import QwenImage21Transformer2DModel

        config = _json.loads((source_dir / "config.json").read_text(encoding="utf-8"))
        with init_empty_weights():
            model = QwenImage21Transformer2DModel.from_config(config)
        _replace_linears(model)
        skip = set()
    elif component == "qwen_text_encoder":
        from transformers import AutoConfig, Qwen3VLForConditionalGeneration

        config = AutoConfig.from_pretrained(str(source_dir))
        with init_empty_weights():
            model = Qwen3VLForConditionalGeneration._from_config(config)
        _replace_linears(model, skip_names={"lm_head"})
        skip = set()
    elif component == "moss_text_encoder":
        from transformers import AutoConfig, AutoModel

        config = AutoConfig.from_pretrained(str(source_dir))
        with init_empty_weights():
            model = AutoModel.from_config(config)
        _replace_linears(model, skip_names={"lm_head"})
        skip = set()
    elif component == "moss_transformer":
        from ..vendored import load_moss_pipeline

        module = load_moss_pipeline()
        config = _json.loads((source_dir / "config.json").read_text(encoding="utf-8"))
        with init_empty_weights():
            model = module.MOSSImageTransformer.from_config(config)
        _replace_linears(model)
        skip = set()
    else:  # pragma: no cover - closed set
        raise ValueError(f"unsupported low-memory prepare component: {component}")

    quantized = _quantized_keys(model)
    base = []
    for key in _walk(model):
        if key.endswith(QUANT_SUFFIXES):
            continue
        if key in skip:
            continue
        base.append(key)
    del model
    gc.collect()
    return base, quantized


# ---------------------------------------------------------------------------
# source snapshot metadata
# ---------------------------------------------------------------------------


@dataclass
class BaseTensor:
    name: str
    quantized: bool
    file: Path
    dtype: str
    shape: list[int]
    offset: int
    length: int


def _source_files(source_dir: Path) -> list[Path]:
    files = sorted(
        item
        for item in source_dir.glob("*.safetensors")
        if item.is_file()
    )
    if files:
        return files
    raise FileNotFoundError(f"no safetensors files under {source_dir}")


def _source_tensors(source_dir: Path) -> dict[str, BaseTensor]:
    tensors: dict[str, BaseTensor] = {}
    for file in _source_files(source_dir):
        header, data_start = _read_header(file)
        for key, entry in header.items():
            if key == "__metadata__":
                continue
            start, end = entry["data_offsets"]
            tensors[key] = BaseTensor(
                name=key,
                quantized=False,
                file=file,
                dtype=entry["dtype"],
                shape=list(entry["shape"]),
                offset=data_start + start,
                length=end - start,
            )
    return tensors


# ---------------------------------------------------------------------------
# child process: quantize one group of tensors
# ---------------------------------------------------------------------------


_DTYPE_TORCH = {
    "BF16": "bfloat16",
    "F16": "float16",
    "F32": "float32",
    "F64": "float64",
    "I8": "int8",
    "U8": "uint8",
    "I16": "int16",
    "U16": "uint16",
    "I32": "int32",
    "U32": "uint32",
    "I64": "int64",
    "U64": "uint64",
    "BOOL": "bool",
}


def _load_tensor(file: str, dtype: str, shape: list[int], offset: int, length: int):
    """Read exactly one tensor range from the snapshot instead of mapping the
    whole file (``safe_open`` commits the full file size on Windows)."""
    import torch

    torch_dtype = getattr(torch, _DTYPE_TORCH[dtype])
    with open(file, "rb") as handle:
        handle.seek(offset)
        raw = handle.read(length)
    if len(raw) != length:
        raise OSError(f"short read for {file} @ {offset}+{length}")
    return torch.frombuffer(bytearray(raw), dtype=torch_dtype).reshape(shape)


def _quantize_with_bnb(tensor):
    import bitsandbytes as bnb
    import torch

    return bnb.functional.quantize_4bit(
        tensor,
        blocksize=64,
        compress_statistics=True,
        quant_type="nf4",
        quant_storage=torch.uint8,
    )


def _peak_commit_mib() -> float | None:
    try:
        import psutil

        return round(psutil.Process().memory_info().peak_pagefile / (1 << 20), 1)
    except Exception:  # noqa: BLE001 - optional diagnostics only
        return None


def _child_run(job: dict) -> dict:
    import torch
    from safetensors.torch import save_file

    fragment = Path(job["fragment"])
    entries = job["entries"]
    out: dict[str, torch.Tensor] = {}
    started = time.perf_counter()
    try:
        for entry in entries:
            tensor = _load_tensor(
                entry["file"],
                entry["dtype"],
                entry["shape"],
                entry["offset"],
                entry["length"],
            )
            data, quant_state = _quantize_with_bnb(tensor)
            name = entry["name"]
            out[name] = data
            for key, value in quant_state.as_dict(packed=True).items():
                out[name + "." + key] = value
            del tensor, data, quant_state
            gc.collect()
        fragment.parent.mkdir(parents=True, exist_ok=True)
        save_file(out, str(fragment))
        return {
            "status": "ok",
            "fragment": str(fragment),
            "tensor_count": len(entries),
            "elapsed_seconds": round(time.perf_counter() - started, 2),
            "peak_commit_mib": _peak_commit_mib(),
        }
    finally:
        out.clear()
        gc.collect()


def _spawn_child(job: dict) -> dict:
    payload = json.dumps(job)
    started = time.perf_counter()
    env = dict(os.environ)
    env.setdefault("PYTHONIOENCODING", "utf-8")
    completed = subprocess.run(
        [sys.executable, "-m", "studio_diffusers_worker.backends.low_memory", "--job", "-"],
        input=payload.encode("utf-8"),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        timeout=job.get("timeout_seconds", 1800),
        env=env,
        check=False,
    )
    elapsed = round(time.perf_counter() - started, 2)
    if completed.returncode != 0:
        stderr = completed.stderr.decode("utf-8", errors="replace")[-4000:]
        raise RuntimeError(f"quantize child failed ({completed.returncode}) in {elapsed}s: {stderr}")
    text = completed.stdout.decode("utf-8", errors="replace").strip().splitlines()
    if not text:
        raise RuntimeError(f"quantize child produced no result (stderr: {completed.stderr.decode('utf-8', errors='replace')[-2000:]})")
    try:
        result = json.loads(text[-1])
    except json.JSONDecodeError as error:
        raise RuntimeError(f"quantize child result is not JSON: {text[-1][:500]}") from error
    result["process_seconds"] = elapsed
    return result


def _main_child() -> int:
    if "--job" not in sys.argv:
        log("usage: python -m studio_diffusers_worker.backends.low_memory --job <json|->")
        return 2
    index = sys.argv.index("--job")
    source = sys.argv[index + 1]
    if source == "-":
        job = json.load(sys.stdin)
    else:
        job = json.loads(Path(source).read_text(encoding="utf-8"))
    try:
        result = _child_run(job)
    except BaseException as error:  # noqa: BLE001 - reported to the coordinator
        import traceback

        print(json.dumps({"status": "error", "error": f"{type(error).__name__}: {error}", "traceback": traceback.format_exc()}))
        return 1
    print(json.dumps(result))
    return 0


# ---------------------------------------------------------------------------
# coordinator
# ---------------------------------------------------------------------------


def _inventory(path: Path) -> dict:
    files = sorted(item for item in path.rglob("*") if item.is_file())
    inventory = []
    total = 0
    for item in files:
        digest = hashlib.sha256()
        size = 0
        with open(item, "rb") as handle:
            for chunk in iter(lambda: handle.read(_COPY_CHUNK), b""):
                digest.update(chunk)
                size += len(chunk)
        inventory.append(
            {
                "relative_path": item.relative_to(path).as_posix(),
                "bytes": size,
                "sha256": digest.hexdigest(),
            }
        )
        total += size
    return {"path": str(path), "file_count": len(inventory), "total_bytes": total, "files": inventory}


def _component_ready(path: Path) -> bool:
    return (path / "config.json").is_file() and any(item.suffix == ".safetensors" for item in path.glob("*.safetensors"))


def _write_config(component: str, source_dir: Path, target_dir: Path) -> None:
    source_config = json.loads((source_dir / "config.json").read_text(encoding="utf-8"))
    source_config.pop("quantization_config", None)
    target_dir.mkdir(parents=True, exist_ok=True)
    if component.endswith("transformer"):
        import diffusers

        class_name = "MOSSImageTransformer" if component == "moss_transformer" else "QwenImage21Transformer2DModel"
        config = {
            "_class_name": class_name,
            "_diffusers_version": diffusers.__version__,
            "_name_or_path": str(source_dir),
            **source_config,
            "quantization_config": dict(_BNB_QUANT_CONFIG),
        }
        (target_dir / "config.json").write_text(
            json.dumps(config, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    else:
        config = {**source_config, "quantization_config": dict(_BNB_QUANT_CONFIG)}
        (target_dir / "config.json").write_text(
            json.dumps(config, indent=2, sort_keys=True) + "\n", encoding="utf-8"
        )
    generation = source_dir / "generation_config.json"
    if generation.is_file():
        (target_dir / "generation_config.json").write_text(
            generation.read_text(encoding="utf-8"), encoding="utf-8"
        )


def _group_quantized(tensors: dict[str, BaseTensor], order: list[str], group_bytes: int) -> list[list[BaseTensor]]:
    groups: list[list[BaseTensor]] = []
    current: list[BaseTensor] = []
    size = 0
    for name in order:
        tensor = tensors.get(name)
        if tensor is None or not tensor.quantized:
            continue
        if current and size + tensor.length > group_bytes:
            groups.append(current)
            current = []
            size = 0
        current.append(tensor)
        size += tensor.length
    if current:
        groups.append(current)
    return groups


def prepare_component_low_memory(
    kind: str,
    component: str,
    source_dir: Path,
    target_dir: Path,
    group_bytes: int | None = None,
) -> dict:
    source_dir = Path(source_dir)
    target_dir = Path(target_dir)
    group_bytes = group_bytes or int(os.environ.get("STUDIO_PREPARE_GROUP_MIB", "1024")) << 20
    started = time.perf_counter()

    base_keys, quantized = _skeleton(component, source_dir)
    tensors = _source_tensors(source_dir)
    missing = [key for key in base_keys if key not in tensors]
    if missing:
        raise ValueError(f"{component}: skeleton keys missing from snapshot: {missing[:5]}")
    for key in base_keys:
        tensors[key].quantized = key in quantized

    groups = _group_quantized(tensors, base_keys, group_bytes)
    fragments_dir = target_dir / "_fragments"
    if fragments_dir.exists():
        for stale in fragments_dir.iterdir():
            stale.unlink()
    fragments_dir.mkdir(parents=True, exist_ok=True)

    total_groups = len(groups)
    total_quantized = sum(len(group) for group in groups)
    log(
        f"[prepare-lowmem] {component} start tensors={len(base_keys)} quantized={total_quantized} "
        f"groups={total_groups} group_bytes={group_bytes}"
    )

    fragment_results = []
    for index, group in enumerate(groups):
        job = {
            "component": component,
            "fragment": str(fragments_dir / f"fragment-{index:05d}.safetensors"),
            "entries": [
                {
                    "name": item.name,
                    "file": str(item.file),
                    "dtype": item.dtype,
                    "shape": item.shape,
                    "offset": item.offset,
                    "length": item.length,
                }
                for item in group
            ],
            "timeout_seconds": 1800,
        }
        result = _spawn_child(job)
        result["group_index"] = index
        result["source_bytes"] = sum(item.length for item in group)
        fragment_results.append(result)
        log(
            f"[prepare-lowmem] {component} group {index + 1}/{total_groups} "
            f"tensors={len(group)} source_mib={result['source_bytes'] // (1 << 20)} "
            f"elapsed={result['process_seconds']}s"
        )

    final_name = "diffusion_pytorch_model.safetensors" if component.endswith("transformer") else "model.safetensors"
    final_path = target_dir / final_name

    shapes: dict[str, tuple[str, list[int]]] = {}
    sources: dict[str, tuple[Path, int, int]] = {}
    for result in fragment_results:
        fragment, data_start = _read_header(Path(result["fragment"]))
        for key, entry in fragment.items():
            if key == "__metadata__":
                continue
            start, end = entry["data_offsets"]
            shapes[key] = (entry["dtype"], list(entry["shape"]))
            sources[key] = (Path(result["fragment"]), data_start + start, end - start)

    expected_quant = total_quantized * (1 + len(QUANT_SUFFIXES))
    if len(sources) != expected_quant:
        raise ValueError(
            f"{component}: fragment key count {len(sources)} != expected {expected_quant}"
        )

    for key in base_keys:
        tensor = tensors[key]
        if tensor.quantized:
            continue
        if key not in tensors:
            raise ValueError(f"{component}: missing source tensor {key}")
        shapes[key] = (tensor.dtype, tensor.shape)
        sources[key] = (tensor.file, tensor.offset, tensor.length)

    written = _write_streamed(final_path, shapes, sources)
    for stale in fragments_dir.iterdir():
        stale.unlink()
    fragments_dir.rmdir()

    _write_config(component, source_dir, target_dir)
    inventory = _inventory(target_dir)
    elapsed = round(time.perf_counter() - started, 2)
    record = {
        "status": "prepared",
        "mode": "low_memory_children",
        "quantization": "nf4",
        "component": component,
        "groups": total_groups,
        "group_bytes": group_bytes,
        "quantized_tensors": total_quantized,
        "fragments": fragment_results,
        "safetensors": {"file": final_path.name, **written},
        "elapsed_seconds": elapsed,
        "peak_commit_mib": _peak_commit_mib(),
        "inventory": inventory,
    }
    log(
        f"[prepare-lowmem] {component} done elapsed={elapsed}s bytes={written['bytes']} sha256={written['sha256'][:16]}"
    )
    return record


if __name__ == "__main__":
    sys.exit(_main_child())

