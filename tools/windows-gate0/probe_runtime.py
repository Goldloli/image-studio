"""Validate the private Windows Python/CUDA candidate without model weights."""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import subprocess
import sys
import time
from pathlib import Path


def nvidia_devices() -> list[dict[str, str]]:
    command = [
        "nvidia-smi",
        "--query-gpu=index,name,uuid,pci.bus_id,compute_cap,memory.total,driver_version",
        "--format=csv,noheader,nounits",
    ]
    try:
        output = subprocess.run(
            command,
            check=True,
            capture_output=True,
            text=True,
            timeout=20,
        ).stdout
    except (OSError, subprocess.SubprocessError) as error:
        return [{"error": str(error)}]
    devices = []
    for line in output.splitlines():
        fields = [field.strip() for field in line.split(",")]
        if len(fields) == 7:
            devices.append(
                dict(
                    zip(
                        (
                            "index",
                            "name",
                            "uuid",
                            "pci_bus_id",
                            "compute_capability",
                            "memory_total_mib",
                            "driver_version",
                        ),
                        fields,
                    )
                )
            )
    return devices


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()

    started = time.monotonic()
    project_root = Path(__file__).resolve().parents[2]

    def display_path(path: Path) -> str:
        try:
            return path.resolve().relative_to(project_root).as_posix()
        except ValueError:
            return path.name

    import accelerate
    import bitsandbytes
    import diffusers
    import numpy
    import psutil
    import torch
    import transformers
    from bitsandbytes.nn import Linear4bit
    from diffusers import FlowMatchEulerDiscreteScheduler, ZImagePipeline
    from transformers import Qwen2Tokenizer, Qwen3Model

    external_devices = nvidia_devices()
    candidates = []
    selected_index = None
    if torch.cuda.is_available():
        for index in range(torch.cuda.device_count()):
            properties = torch.cuda.get_device_properties(index)
            candidate = {
                "index": index,
                "name": properties.name,
                "major": properties.major,
                "minor": properties.minor,
                "total_memory_bytes": properties.total_memory,
                "uuid": str(getattr(properties, "uuid", "")),
            }
            candidates.append(candidate)
            if (
                selected_index is None
                and "NVIDIA" in properties.name.upper()
                and (properties.major, properties.minor) == (8, 6)
                and properties.total_memory >= 10 * 1024**3
            ):
                selected_index = index
        if selected_index is None:
            raise RuntimeError("no NVIDIA sm86 CUDA device with at least 10 GiB memory")
        torch.cuda.set_device(selected_index)
        properties = torch.cuda.get_device_properties(selected_index)
        external_match = next(
            (
                device
                for device in external_devices
                if device.get("uuid", "").lower().removeprefix("gpu-")
                == str(getattr(properties, "uuid", "")).lower()
            ),
            None,
        )
        torch.cuda.reset_peak_memory_stats(selected_index)

        torch.manual_seed(20261101)
        left = torch.randn((64, 64), device="cuda:0", dtype=torch.bfloat16)
        right = torch.randn((64, 64), device="cuda:0", dtype=torch.bfloat16)
        product = left @ right
        if product.dtype != torch.bfloat16 or not torch.isfinite(product).all():
            raise RuntimeError("BF16 matrix multiplication failed")
        torch.cuda.synchronize(selected_index)

        layer = Linear4bit(64, 32, quant_type="nf4", compute_dtype=torch.bfloat16)
        layer = layer.cuda(selected_index)
        sample = torch.randn((2, 64), device="cuda:0", dtype=torch.bfloat16)
        output = layer(sample)
        output.float().square().mean().backward()
        torch.cuda.synchronize(selected_index)
        if output.shape != (2, 32) or output.dtype != torch.bfloat16:
            raise RuntimeError("NF4 Linear4bit output contract failed")
        if not torch.isfinite(output).all():
            raise RuntimeError("NF4 Linear4bit produced non-finite output")

        cuda = {
            "selected_device_index": selected_index,
            "selected_device": candidates[selected_index],
            "external_nvidia_devices": external_devices,
            "external_uuid_match": external_match,
            "bf16_matmul": True,
            "nf4_linear_forward_backward": True,
            "peak_reserved_bytes": torch.cuda.max_memory_reserved(selected_index),
            "allocated_after_probe_bytes": torch.cuda.memory_allocated(selected_index),
        }
    else:
        raise RuntimeError("torch.cuda.is_available() is false")

    model_manifest = (
        project_root
        / "resources"
        / "windows-foundation"
        / "z-image-diffusers"
        / "model-manifest.json"
    )
    pipeline_imports = {
        "ZImagePipeline": ZImagePipeline.__name__,
        "FlowMatchEulerDiscreteScheduler": FlowMatchEulerDiscreteScheduler.__name__,
        "Qwen3Model": Qwen3Model.__name__,
        "Qwen2Tokenizer": Qwen2Tokenizer.__name__,
    }
    report = {
        "schema_version": 1,
        "kind": "windows-gate0-runtime-probe",
        "status": "passed",
        "is_full_model_inference": False,
        "is_installer_activation": False,
        "elapsed_seconds": time.monotonic() - started,
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "version": platform.version(),
            "machine": platform.machine(),
            "python_executable": display_path(Path(sys.executable)),
            "python_version": platform.python_version(),
        },
        "versions": {
            "torch": torch.__version__,
            "torch_cuda": torch.version.cuda,
            "diffusers": diffusers.__version__,
            "transformers": transformers.__version__,
            "accelerate": accelerate.__version__,
            "bitsandbytes": bitsandbytes.__version__,
            "numpy": numpy.__version__,
        },
        "imports": pipeline_imports,
        "cuda": cuda,
        "host": {
            "cpu_count_logical": os.cpu_count(),
            "ram_total_bytes": psutil.virtual_memory().total,
            "ram_available_bytes": psutil.virtual_memory().available,
            "process_rss_bytes": psutil.Process().memory_info().rss,
        },
        "model_manifest": display_path(model_manifest),
        "limitations": [
            "This probe imports packages and executes BF16/NF4 small operators only.",
            "It does not prove model loading, pipeline compatibility, memory budgets, quality, or inference latency.",
        ],
    }
    if not math.isfinite(report["elapsed_seconds"]):
        raise RuntimeError("invalid elapsed time")
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_suffix(output.suffix + ".pending")
    temporary.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, output)
    print(json.dumps(report, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
