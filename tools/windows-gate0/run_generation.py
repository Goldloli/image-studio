"""Run one bounded Windows Gate 0 candidate and record reproducible evidence."""

from __future__ import annotations

import argparse
import ctypes
import gc
import hashlib
import json
import math
import os
import platform
import subprocess
import threading
import time
import traceback
from pathlib import Path
from typing import Any

import numpy as np
import psutil
import torch
from PIL import Image

from diffusers import (
    AutoencoderKL,
    BitsAndBytesConfig as DiffusersBitsAndBytesConfig,
    FlowMatchEulerDiscreteScheduler,
    ZImagePipeline,
    ZImageTransformer2DModel,
)
from transformers import (
    BitsAndBytesConfig as TransformersBitsAndBytesConfig,
    Qwen2Tokenizer,
    Qwen3Model,
)


LOAD_SECONDS = 600
GENERATION_SECONDS = 900
CUDA_RESERVED_LIMIT = int(10.5 * 1024**3)
PRIVATE_COMMIT_LIMIT = 24 * 1024**3
AVAILABLE_RAM_MINIMUM = 2 * 1024**3
TEXT_UTF8_LIMIT = 16 * 1024
TOKEN_LIMIT = 512


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


def commit_status() -> dict[str, int]:
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


def process_tree(root_pid: int) -> list[psutil.Process]:
    processes = []
    root = psutil.Process(root_pid)
    processes.append(root)
    try:
        processes.extend(root.children(recursive=True))
    except (psutil.NoSuchProcess, psutil.AccessDenied):
        pass
    return processes


def private_commit(processes: list[psutil.Process]) -> int:
    total = 0
    for process in processes:
        try:
            total += int(process.memory_info().private)
        except (psutil.NoSuchProcess, psutil.AccessDenied):
            pass
    return total


def nvidia_gpu_status() -> list[dict[str, str]]:
    command = [
        "nvidia-smi",
        "--query-gpu=index,name,uuid,pci.bus_id,compute_cap,memory.total,memory.used,utilization.gpu,driver_version",
        "--format=csv,noheader,nounits",
    ]
    try:
        output = subprocess.run(
            command, check=True, capture_output=True, text=True, timeout=15
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    fields = (
        "index",
        "name",
        "uuid",
        "pci_bus_id",
        "compute_capability",
        "memory_total_mib",
        "memory_used_mib",
        "utilization_gpu_percent",
        "driver_version",
    )
    result = []
    for line in output.splitlines():
        values = [value.strip() for value in line.split(",")]
        if len(values) == len(fields):
            result.append(dict(zip(fields, values)))
    return result


def nvidia_compute_apps() -> list[dict[str, int]]:
    command = [
        "nvidia-smi",
        "--query-compute-apps=pid,used_gpu_memory",
        "--format=csv,noheader,nounits",
    ]
    try:
        output = subprocess.run(
            command, check=True, capture_output=True, text=True, timeout=15
        ).stdout
    except (OSError, subprocess.SubprocessError):
        return []
    result = []
    for line in output.splitlines():
        fields = [field.strip() for field in line.split(",")]
        if len(fields) == 2 and fields[0].isdigit() and fields[1].isdigit():
            result.append({"pid": int(fields[0]), "used_memory_mib": int(fields[1])})
    return result


class ResourceSampler:
    def __init__(self, interval_seconds: float = 0.05) -> None:
        self.interval_seconds = interval_seconds
        self.stage = "startup"
        self.started = time.monotonic()
        self._stop = threading.Event()
        self._lock = threading.Lock()
        self._thread = threading.Thread(target=self._run, daemon=True)
        self.peaks: dict[str, dict[str, float]] = {}
        self.samples = 0
        self.max_private_commit = 0
        self.minimum_available_ram = math.inf
        self.max_gpu_used_mib = 0
        self.max_compute_used_mib = 0
        self.gpu_samples = 0
        self._next_gpu_sample = 0.0
        self.start_commit = commit_status()

    def set_stage(self, stage: str) -> None:
        with self._lock:
            self.stage = stage

    def _run(self) -> None:
        while not self._stop.is_set():
            processes = process_tree(os.getpid())
            memory = commit_status()
            private = private_commit(processes)
            available = memory["physical_available_bytes"]
            with self._lock:
                current_stage = self.stage
            stage = self.peaks.setdefault(
                current_stage,
                {
                    "private_commit_peak_bytes": 0,
                    "available_ram_min_bytes": math.inf,
                    "sampled_seconds": 0,
                },
            )
            stage["private_commit_peak_bytes"] = max(
                stage["private_commit_peak_bytes"], private
            )
            stage["available_ram_min_bytes"] = min(
                stage["available_ram_min_bytes"], available
            )
            stage["sampled_seconds"] += self.interval_seconds
            self.max_private_commit = max(self.max_private_commit, private)
            self.minimum_available_ram = min(self.minimum_available_ram, available)
            self.samples += 1
            now = time.monotonic()
            if now >= self._next_gpu_sample:
                self._next_gpu_sample = now + 1
                gpu = nvidia_gpu_status()
                for device in gpu:
                    try:
                        self.max_gpu_used_mib = max(
                            self.max_gpu_used_mib,
                            int(device["memory_used_mib"]),
                        )
                    except (KeyError, ValueError):
                        pass
                tree_pids = {process.pid for process in processes}
                for application in nvidia_compute_apps():
                    if application["pid"] in tree_pids:
                        self.max_compute_used_mib = max(
                            self.max_compute_used_mib,
                            application["used_memory_mib"],
                        )
                self.gpu_samples += 1
            self._stop.wait(self.interval_seconds)

    def start(self) -> None:
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        self._thread.join(timeout=5)

    def report(self) -> dict[str, Any]:
        end_commit = commit_status()
        stages = {}
        for stage, values in self.peaks.items():
            stages[stage] = {
                **values,
                "available_ram_min_bytes": (
                    None
                    if math.isinf(values["available_ram_min_bytes"])
                    else int(values["available_ram_min_bytes"])
                ),
                "private_commit_peak_bytes": int(values["private_commit_peak_bytes"]),
            }
        return {
            "sample_interval_ms": int(self.interval_seconds * 1000),
            "sample_count": self.samples,
            "private_commit_peak_bytes": int(self.max_private_commit),
            "available_ram_min_bytes": (
                None
                if math.isinf(self.minimum_available_ram)
                else int(self.minimum_available_ram)
            ),
            "start": self.start_commit,
            "end": end_commit,
            "gpu": {
                "sample_interval_ms": 1000,
                "sample_count": self.gpu_samples,
                "global_memory_used_peak_mib": self.max_gpu_used_mib,
                "process_tree_compute_memory_peak_mib": self.max_compute_used_mib,
                "measurement_source": "nvidia-smi NVML counters",
                "wddm_shared_memory_peak": "unavailable from nvidia-smi",
            },
            "stages": stages,
            "limitations": [
                "Private commit and available RAM are polling observations, not exact peaks.",
                "The process tree has one Python process in this runner; sampler frequency is 20 Hz.",
            ],
        }


def normalize_uuid(value: str) -> str:
    return value.lower().removeprefix("gpu-")


def select_cuda_device() -> tuple[int, dict[str, Any], list[dict[str, str]]]:
    smi = nvidia_gpu_status()
    candidates = []
    for index in range(torch.cuda.device_count()):
        properties = torch.cuda.get_device_properties(index)
        candidate = {
            "index": index,
            "name": properties.name,
            "uuid": str(getattr(properties, "uuid", "")),
            "major": properties.major,
            "minor": properties.minor,
            "total_memory_bytes": properties.total_memory,
        }
        candidates.append(candidate)
        if (
            "NVIDIA" in properties.name.upper()
            and (properties.major, properties.minor) == (8, 6)
            and properties.total_memory >= 10 * 1024**3
            and any(
                normalize_uuid(device.get("uuid", ""))
                == normalize_uuid(candidate["uuid"])
                for device in smi
            )
        ):
            match = next(
                device
                for device in smi
                if normalize_uuid(device.get("uuid", ""))
                == normalize_uuid(candidate["uuid"])
            )
            return index, {**candidate, "external": match}, candidates
    raise RuntimeError(
        f"required NVIDIA RTX sm86 device was not found: {candidates} / {smi}"
    )


def quant_config(module: str, compute_dtype: torch.dtype):
    kwargs = {
        "load_in_4bit": True,
        "bnb_4bit_quant_type": "nf4",
        "bnb_4bit_compute_dtype": compute_dtype,
        "bnb_4bit_use_double_quant": True,
        "llm_int8_skip_modules": ["proj_out"],
    }
    if module == "text_encoder":
        return TransformersBitsAndBytesConfig(**kwargs)
    return DiffusersBitsAndBytesConfig(**kwargs)


def token_count(tokenizer: Qwen2Tokenizer, prompt: str) -> int:
    templated = tokenizer.apply_chat_template(
        [{"role": "user", "content": prompt}],
        tokenize=False,
        add_generation_prompt=True,
        enable_thinking=True,
    )
    encoded = tokenizer(templated, truncation=False, return_tensors="pt")
    return int(encoded.input_ids.shape[1])


def image_checks(image: Image.Image) -> dict[str, Any]:
    pixels = np.asarray(image.convert("RGB"), dtype=np.float32)
    channel_std = [float(value) for value in pixels.std(axis=(0, 1))]
    mean = float(pixels.mean())
    return {
        "mode": image.mode,
        "size": list(image.size),
        "pixel_mean": mean,
        "pixel_std": float(pixels.std()),
        "channel_std": channel_std,
        "non_black": mean > 1 and max(channel_std) > 1,
        "decodable_png": True,
    }


def write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".pending")
    temporary.write_text(
        json.dumps(value, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, path)


def resource_budget(
    cuda_peak: dict[str, int], resources: dict[str, Any]
) -> dict[str, Any]:
    budget = {
        "cuda_peak_reserved_bytes": max(cuda_peak.values(), default=0),
        "cuda_peak_reserved_limit_bytes": CUDA_RESERVED_LIMIT,
        "private_commit_peak_bytes": resources["private_commit_peak_bytes"],
        "private_commit_limit_bytes": PRIVATE_COMMIT_LIMIT,
        "available_ram_min_bytes": resources["available_ram_min_bytes"],
        "available_ram_minimum_bytes": AVAILABLE_RAM_MINIMUM,
    }
    budget["passed"] = (
        budget["cuda_peak_reserved_bytes"] <= budget["cuda_peak_reserved_limit_bytes"]
        and budget["private_commit_peak_bytes"] <= budget["private_commit_limit_bytes"]
        and budget["available_ram_min_bytes"] is not None
        and budget["available_ram_min_bytes"] >= budget["available_ram_minimum_bytes"]
    )
    return budget


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--candidate", choices=("a", "b"), required=True)
    parser.add_argument("--model-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--prompt", required=True)
    parser.add_argument("--seed", type=int, default=20261101)
    parser.add_argument("--load-only", action="store_true")
    args = parser.parse_args()

    prompt_bytes = len(args.prompt.encode("utf-8"))
    if prompt_bytes == 0 or prompt_bytes > TEXT_UTF8_LIMIT:
        raise RuntimeError("prompt must be 1..16384 UTF-8 bytes")
    if not args.model_dir.is_dir():
        raise RuntimeError(f"model directory is missing: {args.model_dir}")

    started = time.monotonic()
    sampler = ResourceSampler()
    sampler.start()
    report: dict[str, Any] = {
        "schema_version": 1,
        "kind": "windows-gate0-generation",
        "status": "running",
        "candidate": args.candidate,
        "is_full_model_inference": not args.load_only,
        "started_at_unix": time.time(),
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "version": platform.version(),
            "python": platform.python_version(),
        },
        "versions": {
            "torch": torch.__version__,
            "torch_cuda": torch.version.cuda,
            "diffusers": __import__("diffusers").__version__,
            "transformers": __import__("transformers").__version__,
            "accelerate": __import__("accelerate").__version__,
            "bitsandbytes": __import__("bitsandbytes").__version__,
            "numpy": np.__version__,
        },
        "request": {
            "prompt": args.prompt,
            "prompt_utf8_bytes": prompt_bytes,
            "width": 1024,
            "height": 1024,
            "steps": 9,
            "guidance": 0.0,
            "seed": args.seed,
            "max_sequence_length": TOKEN_LIMIT,
        },
        "configuration": {
            "text_encoder": (
                "bitsandbytes-nf4;bf16;double-quant"
                if args.candidate == "a"
                else "bfloat16"
            ),
            "transformer": "bitsandbytes-nf4;bf16;double-quant",
            "vae": "bfloat16;tiling",
            "offload": "diffusers-model-cpu-offload",
            "attention": "PyTorch native memory-efficient SDPA",
        },
        "stage_seconds": {},
    }

    timings: dict[str, float] = {}
    cuda_peak: dict[str, int] = {}

    def stage(name: str) -> float:
        sampler.set_stage(name)
        torch.cuda.reset_peak_memory_stats()
        return time.monotonic()

    def finish_stage(name: str, frozen_start: float) -> None:
        torch.cuda.synchronize()
        timings[name] = time.monotonic() - frozen_start
        cuda_peak[name] = torch.cuda.max_memory_reserved()

    try:
        device_index, device, cuda_candidates = select_cuda_device()
        torch.cuda.set_device(device_index)
        device_string = f"cuda:{device_index}"
        report["device"] = {
            "selected": device,
            "cuda_candidates": cuda_candidates,
            "external_nvidia_devices": nvidia_gpu_status(),
        }

        frozen_start = stage("tokenizer")
        tokenizer = Qwen2Tokenizer.from_pretrained(
            args.model_dir, subfolder="tokenizer", local_files_only=True
        )
        actual_tokens = token_count(tokenizer, args.prompt)
        finish_stage("tokenizer", frozen_start)
        if actual_tokens > TOKEN_LIMIT:
            raise RuntimeError(
                f"templated prompt has {actual_tokens} tokens, limit is {TOKEN_LIMIT}"
            )
        report["request"]["templated_tokens"] = actual_tokens

        frozen_start = stage("transformer_load")
        transformer = ZImageTransformer2DModel.from_pretrained(
            args.model_dir,
            subfolder="transformer",
            local_files_only=True,
            torch_dtype=torch.bfloat16,
            low_cpu_mem_usage=True,
            quantization_config=quant_config("transformer", torch.bfloat16),
            device_map={"": device_string},
        )
        transformer = transformer.to("cpu")
        transformer.set_attention_backend("_native_efficient")
        transformer.eval()
        finish_stage("transformer_load", frozen_start)

        frozen_start = stage("text_encoder_load")
        if args.candidate == "a":
            text_encoder = Qwen3Model.from_pretrained(
                args.model_dir,
                subfolder="text_encoder",
                local_files_only=True,
                torch_dtype=torch.bfloat16,
                low_cpu_mem_usage=True,
                quantization_config=quant_config("text_encoder", torch.bfloat16),
                device_map={"": device_string},
            )
            text_encoder = text_encoder.to("cpu")
        else:
            text_encoder = Qwen3Model.from_pretrained(
                args.model_dir,
                subfolder="text_encoder",
                local_files_only=True,
                torch_dtype=torch.bfloat16,
                low_cpu_mem_usage=True,
            )
        text_encoder.eval()
        finish_stage("text_encoder_load", frozen_start)

        frozen_start = stage("vae_load")
        vae = AutoencoderKL.from_pretrained(
            args.model_dir,
            subfolder="vae",
            local_files_only=True,
            torch_dtype=torch.bfloat16,
            low_cpu_mem_usage=True,
        )
        vae.eval()
        vae.enable_tiling()
        scheduler = FlowMatchEulerDiscreteScheduler.from_pretrained(
            args.model_dir, subfolder="scheduler", local_files_only=True
        )
        finish_stage("vae_load", frozen_start)

        frozen_start = stage("pipeline_assemble")
        pipeline = ZImagePipeline(
            scheduler=scheduler,
            vae=vae,
            text_encoder=text_encoder,
            tokenizer=tokenizer,
            transformer=transformer,
        )
        pipeline.enable_model_cpu_offload(gpu_id=device_index)
        finish_stage("pipeline_assemble", frozen_start)
        report["load_seconds"] = sum(
            value
            for key, value in timings.items()
            if key.endswith("_load") or key in {"tokenizer", "pipeline_assemble"}
        )
        if report["load_seconds"] > LOAD_SECONDS:
            raise RuntimeError(f"load exceeded {LOAD_SECONDS} seconds")
        if args.load_only:
            report["status"] = "load_only_passed"
            return 0

        frozen_start = stage("encoding")
        prompt_embeds, _ = pipeline.encode_prompt(
            prompt=args.prompt,
            device=device_string,
            do_classifier_free_guidance=False,
            max_sequence_length=TOKEN_LIMIT,
        )
        finish_stage("encoding", frozen_start)

        original_decode = vae.decode
        decode_started: list[float] = []
        sampling_peak = 0

        def measured_decode(*decode_args, **decode_kwargs):
            nonlocal sampling_peak
            sampling_peak = max(sampling_peak, torch.cuda.max_memory_reserved())
            decode_started.append(stage("decoding"))
            return original_decode(*decode_args, **decode_kwargs)

        vae.decode = measured_decode
        sampler.set_stage("sampling")
        torch.cuda.reset_peak_memory_stats()
        generation_started = time.monotonic()
        callback_steps = []

        def callback(_pipe, step, _timestep, callback_kwargs):
            nonlocal sampling_peak
            callback_steps.append(step)
            sampler.set_stage("sampling")
            sampling_peak = max(sampling_peak, torch.cuda.max_memory_reserved())
            return callback_kwargs

        generator = torch.Generator(device=device_string).manual_seed(args.seed)
        result = pipeline(
            prompt=None,
            prompt_embeds=prompt_embeds,
            height=1024,
            width=1024,
            num_inference_steps=9,
            guidance_scale=0.0,
            generator=generator,
            output_type="pil",
            return_dict=True,
            callback_on_step_end=callback,
            callback_on_step_end_tensor_inputs=["latents"],
            max_sequence_length=TOKEN_LIMIT,
        )
        generation_seconds = time.monotonic() - generation_started
        if generation_seconds > GENERATION_SECONDS:
            raise RuntimeError(f"generation exceeded {GENERATION_SECONDS} seconds")
        if len(result.images) != 1:
            raise RuntimeError("pipeline did not return exactly one image")
        image = result.images[0]
        if image.size != (1024, 1024) or image.mode != "RGB":
            raise RuntimeError("pipeline did not return one 1024x1024 RGB image")
        if decode_started:
            finish_stage("decoding", decode_started[-1])
        torch.cuda.synchronize()
        sampling_peak = max(sampling_peak, torch.cuda.max_memory_reserved())
        timings["sampling"] = generation_seconds - timings.get("decoding", 0)
        cuda_peak["sampling"] = sampling_peak
        checks = image_checks(image)
        if not checks["non_black"]:
            raise RuntimeError("generated image is black or visually degenerate")

        args.output_dir.mkdir(parents=True, exist_ok=True)
        image_path = args.output_dir / f"candidate-{args.candidate}.png"
        image.save(image_path, format="PNG")
        image_bytes = image_path.read_bytes()
        report["output"] = {
            "path": str(image_path),
            "bytes": len(image_bytes),
            "sha256": hashlib.sha256(image_bytes).hexdigest(),
            "checks": checks,
        }
        report["stage_seconds"] = timings
        report["generation_seconds"] = generation_seconds
        report["callback_steps"] = callback_steps
        report["cuda_peak_reserved_bytes"] = cuda_peak
        report["scheduler"] = {
            "class": pipeline.scheduler.__class__.__name__,
            "config": dict(pipeline.scheduler.config),
            "callback_total": len(callback_steps),
        }
        sampler.stop()
        budget = resource_budget(cuda_peak, sampler.report())
        report["budget"] = budget
        if not budget["passed"]:
            raise RuntimeError("Gate 0 resource budget failed")
        report["status"] = "passed"
        return 0
    except Exception as error:
        report["status"] = "failed"
        error_traceback = traceback.format_exc().replace(
            str(Path(__file__).resolve().parents[2]), "<repo>"
        )
        report["error"] = {
            "type": type(error).__name__,
            "message": str(error),
            "traceback": error_traceback,
        }
        return 1
    finally:
        sampler.stop()
        report["elapsed_seconds"] = time.monotonic() - started
        report["resources"] = sampler.report()
        if "stage_seconds" not in report or not report["stage_seconds"]:
            report["stage_seconds"] = timings
        gc.collect()
        torch.cuda.empty_cache()
        write_json(args.report, report)
        print(
            json.dumps(
                {
                    "status": report["status"],
                    "candidate": args.candidate,
                    "load_seconds": report.get("load_seconds"),
                    "generation_seconds": report.get("generation_seconds"),
                    "budget": report.get("budget"),
                    "report": str(args.report),
                },
                ensure_ascii=False,
            )
        )


if __name__ == "__main__":
    raise SystemExit(main())
