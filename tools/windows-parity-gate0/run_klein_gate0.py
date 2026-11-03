"""Gate 0 G2-K1: FLUX.2 Klein 4B distilled on a 12 GB Windows GPU.

Runs the frozen FLUX.2 Klein 4B distilled snapshot with diffusers
``Flux2KleinPipeline`` on the Windows RTX 3080 Ti target and records real
loading/generation timing and resource evidence for two tasks:

1. text-to-image (1024x1024, 4 steps, guidance 1.0 per the model card), and
2. single-image instruction edit (one reference image + instruction prompt, no
   img2img disguise).

Strategies are measured in order and the first one that stays inside every
budget is chosen:

* ``bf16-model-offload`` — official bf16 components plus
  ``enable_model_cpu_offload``; no quantization, best quality.
* ``nf4-model-offload`` — bitsandbytes NF4 on the transformer and text encoder
  plus model CPU offload; recorded when bf16 does not fit.

The GPU is only used after the Lead grants an exclusive window; the runner
itself never starts other GPU jobs and records ``nvidia-smi`` and
``torch.cuda`` peaks for the budget decision. Experiment-only tooling for
OpenSpec change ``achieve-windows-model-parity`` (tasks.md 1.1 / G2-K1, bd
issue image-studio-xwh.1). Not part of the product.
"""

from __future__ import annotations

import argparse
import ctypes
import gc
import hashlib
import json
import os
import platform
import subprocess
import time
import traceback
from pathlib import Path

import torch

DEFAULT_MODEL_DIR = Path("target/parity-gate0/models/klein")
DEFAULT_MANIFEST = Path("resources/planned-models/klein/model-manifest.json")
DEFAULT_OUTPUT_DIR = Path("verification/windows-parity-gate0")
DEFAULT_EDIT_SOURCE = Path(
    "verification/planned-models-20261031/fixtures/original/portrait-hair.png"
)
DEFAULT_PROMPT = "雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光"
DEFAULT_EDIT_PROMPT = "把背景换成雨后的上海夜景街道，保留人物的面部和发型，写实摄影"
SEED_TXT2IMG = 3148150817
SEED_EDIT = 3148150818
STEPS = 4
GUIDANCE_SCALE = 1.0


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


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def nvidia_smi_identity() -> dict:
    output = subprocess.run(
        [
            "nvidia-smi",
            "--query-gpu=name,driver_version,memory.total",
            "--format=csv,noheader,nounits",
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=15,
    ).stdout.strip()
    name, driver, total = [item.strip() for item in output.splitlines()[0].split(",")]
    return {"name": name, "driver": driver, "memory_total_mib": int(total)}


def nvidia_smi_used_mib() -> int | None:
    try:
        output = subprocess.run(
            [
                "nvidia-smi",
                "--query-gpu=memory.used",
                "--format=csv,noheader,nounits",
            ],
            check=True,
            capture_output=True,
            text=True,
            timeout=10,
        ).stdout.strip()
        return int(output.splitlines()[0])
    except (OSError, subprocess.SubprocessError, ValueError):
        return None


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


class Sampler:
    """Sample process commit, system RAM, torch CUDA pools and nvidia-smi."""

    def __init__(self, interval_seconds: float = 0.2) -> None:
        import threading

        self.threading = threading
        self.interval = interval_seconds
        self.stop_event = threading.Event()
        self.thread: threading.Thread | None = None
        self.private_peak = 0
        self.available_ram_min = 1 << 62
        self.system_total = 0
        self.cuda_reserved_peak = 0
        self.cuda_allocated_peak = 0
        self.nvidia_used_peak_mib = 0
        self.samples = 0

    def _sample(self) -> None:
        self.private_peak = max(self.private_peak, process_tree_private_commit())
        memory = system_memory()
        self.system_total = memory["physical_total_bytes"]
        self.available_ram_min = min(
            self.available_ram_min, memory["physical_available_bytes"]
        )
        if torch.cuda.is_available():
            self.cuda_reserved_peak = max(
                self.cuda_reserved_peak, int(torch.cuda.memory_reserved())
            )
            self.cuda_allocated_peak = max(
                self.cuda_allocated_peak, int(torch.cuda.memory_allocated())
            )
        if self.samples % 5 == 0:
            used = nvidia_smi_used_mib()
            if used is not None:
                self.nvidia_used_peak_mib = max(self.nvidia_used_peak_mib, used)
        self.samples += 1

    def _loop(self) -> None:
        while not self.stop_event.wait(self.interval):
            try:
                self._sample()
            except Exception:  # noqa: BLE001 - experiment sampler
                pass

    def start(self) -> None:
        self._sample()
        self.thread = self.threading.Thread(target=self._loop, daemon=True)
        self.thread.start()

    def stop(self) -> None:
        self.stop_event.set()
        if self.thread is not None:
            self.thread.join(timeout=10)

    def snapshot(self) -> dict:
        return {
            "process_tree_private_commit_peak_gib": round(
                self.private_peak / 1024**3, 3
            ),
            "system_available_ram_min_gib": round(self.available_ram_min / 1024**3, 3),
            "system_physical_total_gib": round(self.system_total / 1024**3, 3),
            "cuda_reserved_peak_gib": round(self.cuda_reserved_peak / 1024**3, 3),
            "cuda_allocated_peak_gib": round(self.cuda_allocated_peak / 1024**3, 3),
            "nvidia_used_peak_mib": self.nvidia_used_peak_mib,
            "sample_interval_seconds": self.interval,
            "sample_count": self.samples,
        }


def write_json(path: Path, payload: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".pending")
    temporary.write_text(
        json.dumps(payload, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    os.replace(temporary, path)


def preflight(model_dir: Path, manifest_path: Path) -> dict:
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    files = manifest["files"]
    missing = []
    bad_size = []
    total_bytes = 0
    for artifact in files:
        relative = artifact["path"]
        path = model_dir.joinpath(*relative.split("/"))
        expected = int(artifact["bytes"])
        if not path.is_file():
            missing.append(relative)
            continue
        actual = path.stat().st_size
        if actual != expected:
            bad_size.append(
                {"path": relative, "expected_bytes": expected, "actual_bytes": actual}
            )
            continue
        total_bytes += actual
    model_index = {}
    index_path = model_dir / "model_index.json"
    if index_path.is_file():
        model_index = json.loads(index_path.read_text(encoding="utf-8"))
    return {
        "manifest": str(manifest_path),
        "manifest_sha256": sha256_file(manifest_path),
        "repository": manifest.get("repository"),
        "revision": manifest.get("revision"),
        "declared_component_bytes": int(manifest["component_bytes"]),
        "verified_file_bytes": total_bytes,
        "file_count": len(files),
        "missing": missing,
        "bad_size": bad_size,
        "model_index_class": model_index.get("_class_name"),
        "model_index_is_distilled": model_index.get("is_distilled"),
        "component_classes": {
            key: value
            for key, value in model_index.items()
            if key not in ("_class_name", "_diffusers_version", "is_distilled")
        },
    }


def quantized_pipeline_config():
    from diffusers import PipelineQuantizationConfig

    return PipelineQuantizationConfig(
        quant_backend="bitsandbytes_4bit",
        quant_kwargs={
            "load_in_4bit": True,
            "bnb_4bit_quant_type": "nf4",
            "bnb_4bit_compute_dtype": torch.bfloat16,
            "bnb_4bit_use_double_quant": True,
        },
        components_to_quantize=["transformer", "text_encoder"],
    )


def build_pipeline(model_dir: Path, strategy: str):
    from diffusers import Flux2KleinPipeline

    kwargs: dict = {
        "torch_dtype": torch.bfloat16,
        "low_cpu_mem_usage": True,
    }
    if strategy == "nf4-model-offload":
        kwargs["quantization_config"] = quantized_pipeline_config()
    pipeline = Flux2KleinPipeline.from_pretrained(str(model_dir), **kwargs)
    pipeline.enable_model_cpu_offload(device=torch.device("cuda", 0))
    pipeline.set_progress_bar_config(disable=True)
    return pipeline


def strategy_definition(strategy: str) -> dict:
    if strategy == "bf16-model-offload":
        return {
            "name": strategy,
            "dtype": "bfloat16",
            "quantization": None,
            "offload": "enable_model_cpu_offload",
            "tasks": ["txt2img", "edit"],
        }
    if strategy == "nf4-model-offload":
        return {
            "name": strategy,
            "dtype": "bfloat16 compute",
            "quantization": {
                "backend": "bitsandbytes_4bit",
                "quant_type": "nf4",
                "compute_dtype": "bfloat16",
                "double_quant": True,
                "components": ["transformer", "text_encoder"],
            },
            "offload": "enable_model_cpu_offload",
            "tasks": ["txt2img", "edit"],
        }
    raise RuntimeError(f"unknown strategy: {strategy}")


def image_record(path: Path) -> dict:
    from PIL import Image

    with Image.open(path) as opened:
        width, height = opened.size
    return {
        "path": str(path),
        "bytes": path.stat().st_size,
        "sha256": sha256_file(path),
        "width": width,
        "height": height,
    }


def write_checkpoint(report_path: Path, record: dict) -> None:
    write_json(report_path, record)


def run_strategy(
    strategy: str,
    model_dir: Path,
    output_dir: Path,
    edit_source_path: Path,
    prompt: str,
    edit_prompt: str,
    steps: int,
    width: int,
    height: int,
    budget: dict,
    output_tag: str | None = None,
) -> dict:
    from PIL import Image

    definition = strategy_definition(strategy)
    edit_source = None
    edit_source_record = None
    if "edit" in definition["tasks"]:
        with Image.open(edit_source_path) as opened:
            opened.load()
            edit_source = opened.convert("RGB")
        edit_source_record = {
            "path": str(edit_source_path),
            "bytes": edit_source_path.stat().st_size,
            "sha256": sha256_file(edit_source_path),
            "width": edit_source.width,
            "height": edit_source.height,
            "mode": "RGB",
        }

    torch.cuda.set_device(0)
    torch.cuda.empty_cache()
    torch.cuda.reset_peak_memory_stats()
    sampler = Sampler()
    sampler.start()
    pipeline = None
    result: dict = {
        "strategy": strategy,
        "definition": definition,
        "edit_source": edit_source_record,
        "phases": {},
        "tasks": {},
        "status": "error",
        "failures": [],
    }
    try:
        load_started = time.perf_counter()
        pipeline = build_pipeline(model_dir, strategy)
        torch.cuda.synchronize()
        load_seconds = time.perf_counter() - load_started
        result["phases"]["load"] = {
            "seconds": round(load_seconds, 2),
            "cuda_reserved_peak_gib": round(
                torch.cuda.max_memory_reserved() / 1024**3, 3
            ),
            "cuda_allocated_peak_gib": round(
                torch.cuda.max_memory_allocated() / 1024**3, 3
            ),
        }
        if load_seconds > budget["load_limit_seconds"]:
            result["failures"].append("load exceeds limit")

        task_index = 0
        for task in definition["tasks"]:
            task_index += 1
            if task == "txt2img":
                seed = SEED_TXT2IMG
                kwargs = {
                    "prompt": prompt,
                    "height": height,
                    "width": width,
                    "num_inference_steps": steps,
                    "guidance_scale": GUIDANCE_SCALE,
                    "generator": torch.Generator(device="cpu").manual_seed(seed),
                }
                parameters = {
                    "prompt": prompt,
                    "height": height,
                    "width": width,
                    "num_inference_steps": steps,
                    "guidance_scale": GUIDANCE_SCALE,
                    "seed": seed,
                    "reference_image": None,
                }
            else:
                seed = SEED_EDIT
                kwargs = {
                    "image": edit_source,
                    "prompt": edit_prompt,
                    "num_inference_steps": steps,
                    "guidance_scale": GUIDANCE_SCALE,
                    "generator": torch.Generator(device="cpu").manual_seed(seed),
                }
                parameters = {
                    "prompt": edit_prompt,
                    "height": None,
                    "width": None,
                    "num_inference_steps": steps,
                    "guidance_scale": GUIDANCE_SCALE,
                    "seed": seed,
                    "reference_image": edit_source_record,
                }
            torch.cuda.reset_peak_memory_stats()
            torch.cuda.synchronize()
            started = time.perf_counter()
            output = pipeline(**kwargs)
            torch.cuda.synchronize()
            seconds = time.perf_counter() - started
            image = output.images[0]
            suffix = output_tag if output_tag else strategy
            image_path = output_dir / f"klein-gate0-{task}-{suffix}.png"
            image.save(image_path)
            task_record = {
                "task": task,
                "parameters": parameters,
                "seconds": round(seconds, 2),
                "cuda_reserved_peak_gib": round(
                    torch.cuda.max_memory_reserved() / 1024**3, 3
                ),
                "cuda_allocated_peak_gib": round(
                    torch.cuda.max_memory_allocated() / 1024**3, 3
                ),
                "output": image_record(image_path),
            }
            result["tasks"][task] = task_record
            result["phases"][task] = {
                "seconds": round(seconds, 2),
                "cuda_reserved_peak_gib": task_record["cuda_reserved_peak_gib"],
                "cuda_allocated_peak_gib": task_record["cuda_allocated_peak_gib"],
            }
            if seconds > budget["generation_limit_seconds"]:
                result["failures"].append(f"{task} exceeds generation limit")

        sampler.stop()
        resources = sampler.snapshot()
        result["resources"] = resources
        if resources["cuda_reserved_peak_gib"] > budget["cuda_reserved_limit_gib"]:
            result["failures"].append("cuda reserved exceeds limit")
        if resources["process_tree_private_commit_peak_gib"] > budget[
            "private_commit_limit_gib"
        ]:
            result["failures"].append("private commit exceeds limit")
        if resources["system_available_ram_min_gib"] < budget[
            "available_ram_minimum_gib"
        ]:
            result["failures"].append("available RAM below minimum")
        result["status"] = "passed" if not result["failures"] else "budget_failed"
        return result
    except BaseException as error:  # noqa: BLE001 - experiment harness
        sampler.stop()
        result["status"] = "error"
        result["error"] = f"{type(error).__name__}: {error}"
        result["traceback"] = traceback.format_exc()
        try:
            result["resources"] = sampler.snapshot()
        except Exception:  # noqa: BLE001 - best effort
            pass
        return result
    finally:
        if pipeline is not None:
            try:
                pipeline.maybe_free_model_hooks()
            except Exception:  # noqa: BLE001 - best effort
                pass
            del pipeline
        gc.collect()
        torch.cuda.empty_cache()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model-dir", type=Path, default=DEFAULT_MODEL_DIR)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--edit-source", type=Path, default=DEFAULT_EDIT_SOURCE)
    parser.add_argument("--prompt", default=DEFAULT_PROMPT)
    parser.add_argument("--edit-prompt", default=DEFAULT_EDIT_PROMPT)
    parser.add_argument("--steps", type=int, default=STEPS)
    parser.add_argument("--width", type=int, default=1024)
    parser.add_argument("--height", type=int, default=1024)
    parser.add_argument(
        "--strategies",
        default="auto",
        help="auto (bf16 first, nf4 if bf16 fails), all, or comma separated names",
    )
    parser.add_argument("--preflight-only", action="store_true")
    parser.add_argument(
        "--output-tag",
        help="optional file tag for the generated PNG names (default: strategy)",
    )
    parser.add_argument("--load-limit-seconds", type=float, default=600)
    parser.add_argument("--generation-limit-seconds", type=float, default=900)
    parser.add_argument("--cuda-reserved-limit-gib", type=float, default=10.5)
    parser.add_argument("--private-commit-limit-gib", type=float, default=24)
    parser.add_argument("--available-ram-minimum-gib", type=float, default=2)
    args = parser.parse_args()

    model_dir = args.model_dir.resolve()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    report_path = (
        args.report.resolve() if args.report else output_dir / "klein-gate0.json"
    )
    edit_source_path = args.edit_source.resolve()

    import diffusers
    import transformers

    identity = preflight(model_dir, args.manifest.resolve())
    budget = {
        "load_limit_seconds": args.load_limit_seconds,
        "generation_limit_seconds": args.generation_limit_seconds,
        "cuda_reserved_limit_gib": args.cuda_reserved_limit_gib,
        "private_commit_limit_gib": args.private_commit_limit_gib,
        "available_ram_minimum_gib": args.available_ram_minimum_gib,
        "wddm_shared_memory_fallback_is_failure": True,
    }
    gpu = nvidia_smi_identity()
    record: dict = {
        "schema_version": 1,
        "kind": "flux2-klein-4b-distilled-windows-gate0",
        "case_id": "G2-K1",
        "started_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "machine": {
            "os": platform.system(),
            "os_version": platform.version(),
            "platform": platform.platform(),
            "cpu": platform.processor(),
            "logical_cores": os.cpu_count(),
            "physical_total_gib": round(
                system_memory()["physical_total_bytes"] / 1024**3, 3
            ),
            "gpu": gpu,
        },
        "versions": {
            "python": platform.python_version(),
            "torch": torch.__version__,
            "cuda": torch.version.cuda,
            "diffusers": diffusers.__version__,
            "transformers": transformers.__version__,
        },
        "model": identity,
        "budget": budget,
        "parameters": {
            "prompt": args.prompt,
            "edit_prompt": args.edit_prompt,
            "steps": args.steps,
            "width": args.width,
            "height": args.height,
            "seed_txt2img": SEED_TXT2IMG,
            "seed_edit": SEED_EDIT,
            "guidance_scale": GUIDANCE_SCALE,
            "resolution_note": "4 steps and guidance 1.0 follow the FLUX.2 klein "
            "4B model card for the distilled checkpoint.",
        },
        "preflight": {
            "model_ready": not identity["missing"] and not identity["bad_size"],
            "cuda_available": torch.cuda.is_available(),
        },
        "strategies": [],
        "status": "in_progress",
    }
    write_checkpoint(report_path, record)

    if identity["missing"] or identity["bad_size"]:
        record["status"] = "preflight_failed"
        record["error"] = (
            "Klein snapshot is incomplete; transfers must finish before Gate 0"
        )
        record["finished_at_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        write_checkpoint(report_path, record)
        print(json.dumps(record, ensure_ascii=False, indent=2))
        return 2
    if not torch.cuda.is_available():
        record["status"] = "preflight_failed"
        record["error"] = "CUDA is not available in this interpreter"
        record["finished_at_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        write_checkpoint(report_path, record)
        print(json.dumps(record, ensure_ascii=False, indent=2))
        return 2
    if args.preflight_only:
        record["status"] = "preflight_passed"
        record["finished_at_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        write_checkpoint(report_path, record)
        print(json.dumps(record, ensure_ascii=False, indent=2))
        return 0

    if args.strategies == "auto":
        order = ["bf16-model-offload", "nf4-model-offload"]
    elif args.strategies == "all":
        order = ["bf16-model-offload", "nf4-model-offload"]
    else:
        order = [item.strip() for item in args.strategies.split(",") if item.strip()]

    chosen = None
    for strategy in order:
        if args.strategies == "auto" and chosen is not None:
            break
        print(f"[gate0] strategy start {strategy}", flush=True)
        try:
            result = run_strategy(
                strategy,
                model_dir,
                output_dir,
                edit_source_path,
                args.prompt,
                args.edit_prompt,
                args.steps,
                args.width,
                args.height,
                budget,
                args.output_tag,
            )
        except BaseException as error:  # noqa: BLE001 - keep other strategies
            result = {
                "strategy": strategy,
                "status": "error",
                "error": f"{type(error).__name__}: {error}",
                "traceback": traceback.format_exc(),
            }
        record["strategies"].append(result)
        print(
            f"[gate0] strategy {strategy}: {result['status']}",
            flush=True,
        )
        write_checkpoint(report_path, record)
        if result["status"] == "passed" and chosen is None:
            chosen = strategy
            if args.strategies == "auto":
                break

    record["chosen_strategy"] = chosen
    if chosen is not None:
        record["status"] = "passed"
        record["decision"] = (
            f"{chosen} stayed inside the Gate 0 budgets; "
            + (
                "official bf16 weights with model CPU offload fit the 12 GB card, "
                "so no quantization is required."
                if chosen == "bf16-model-offload"
                else "bf16 did not fit, so bitsandbytes NF4 on transformer and "
                "text encoder with model CPU offload was selected."
            )
        )
    else:
        record["status"] = "budget_failed"
        record["decision"] = (
            "No measured strategy stayed inside the Gate 0 budgets; the model "
            "needs a hardware threshold or a different loader."
        )
    record["failures"] = [
        f"{item['strategy']}: {failure}"
        for item in record["strategies"]
        for failure in item.get("failures", [])
    ]
    record["finished_at_utc"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    write_checkpoint(report_path, record)
    print(json.dumps(record, ensure_ascii=False, indent=2))
    return 0 if chosen is not None else 2


if __name__ == "__main__":
    raise SystemExit(main())
