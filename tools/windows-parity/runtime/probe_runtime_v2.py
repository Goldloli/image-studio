"""Probe the frozen Windows v2 runtime offline.

Verifies that every distribution pinned by requirements.lock is installed at
the exact version, imports the runtime libraries without network access, and
reports CUDA availability. By default CUDA is queried at driver/device level
only (no CUDA context is created) so the probe is safe while another agent runs
a GPU experiment. Pass --cuda-context-probe to execute a small real CUDA
operation (used inside an explicitly granted GPU window).
"""

from __future__ import annotations

import argparse
import importlib
import importlib.metadata
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path


OFFLINE_ENV = {
    "HF_HUB_OFFLINE": "1",
    "TRANSFORMERS_OFFLINE": "1",
    "HF_DATASETS_OFFLINE": "1",
    "PIP_NO_INDEX": "1",
    "HF_HUB_DISABLE_TELEMETRY": "1",
}


def normalize(name: str) -> str:
    return re.sub(r"[-_.]+", "-", name).lower()


def parse_lock(path: Path) -> dict[str, str]:
    pins: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        match = re.fullmatch(r"([A-Za-z0-9_.-]+)==(.+)", line)
        if not match:
            raise ValueError(f"unexpected lock line: {line}")
        pins[normalize(match.group(1))] = match.group(2)
    return pins


def nvidia_smi() -> dict[str, object]:
    command = [
        "nvidia-smi",
        "--query-gpu=name,uuid,driver_version,memory.total,memory.used,utilization.gpu",
        "--format=csv,noheader",
    ]
    try:
        result = subprocess.run(command, capture_output=True, text=True, timeout=30)
    except OSError as error:  # pragma: no cover
        return {"available": False, "error": str(error)}
    if result.returncode != 0:
        return {"available": False, "error": result.stderr.strip()}
    return {
        "available": True,
        "rows": [line.strip() for line in result.stdout.splitlines() if line.strip()],
    }


def directory_bytes(path: Path) -> int:
    total = 0
    for item in path.rglob("*"):
        if item.is_file():
            total += item.stat().st_size
    return total


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--lock",
        type=Path,
        default=Path("resources/windows-parity/runtime-v2/requirements.lock"),
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runtime-dir", type=Path, default=Path("target/parity-runtime/runtime"))
    parser.add_argument("--wheelhouse", type=Path, default=Path("target/parity-runtime/wheelhouse"))
    parser.add_argument("--cuda-context-probe", action="store_true")
    args = parser.parse_args()

    for key, value in OFFLINE_ENV.items():
        os.environ.setdefault(key, value)

    started = time.perf_counter()
    gpu_before = nvidia_smi()

    checks: list[dict[str, object]] = []

    def check(name: str, function) -> None:
        item: dict[str, object] = {"check": name}
        try:
            item["result"] = function()
            item["status"] = "pass"
        except Exception as error:  # noqa: BLE001 - evidence must record failures
            item["status"] = "fail"
            item["error"] = f"{type(error).__name__}: {error}"
        checks.append(item)

    pins = parse_lock(args.lock)
    check("lock_parse", lambda: {"pins": len(pins)})

    installed: dict[str, str] = {}
    missing: list[str] = []
    mismatched: dict[str, dict[str, str]] = {}
    for name, expected in sorted(pins.items()):
        try:
            actual = importlib.metadata.version(name)
        except importlib.metadata.PackageNotFoundError:
            missing.append(name)
            continue
        installed[name] = actual
        if actual != expected:
            mismatched[name] = {"expected": expected, "actual": actual}
    checks.append(
        {
            "check": "pinned_distributions_installed",
            "status": "pass" if not missing and not mismatched else "fail",
            "installed_count": len(installed),
            "expected_count": len(pins),
            "missing": missing,
            "mismatched": mismatched,
        }
    )

    modules = [
        "torch",
        "torchvision",
        "diffusers",
        "transformers",
        "accelerate",
        "bitsandbytes",
        "safetensors",
        "huggingface_hub",
        "onnxruntime",
        "numpy",
        "PIL",
        "psutil",
    ]
    imports: dict[str, str] = {}
    import_errors: dict[str, str] = {}
    for module_name in modules:
        try:
            imported = importlib.import_module(module_name)
            imports[module_name] = str(getattr(imported, "__version__", "unknown"))
        except Exception as error:  # noqa: BLE001
            import_errors[module_name] = f"{type(error).__name__}: {error}"
    checks.append(
        {
            "check": "library_imports",
            "status": "pass" if not import_errors else "fail",
            "imports": imports,
            "errors": import_errors,
        }
    )

    def torch_versions() -> dict[str, object]:
        import torch

        return {
            "torch": torch.__version__,
            "torch_cuda": torch.version.cuda,
            "torch_cudnn": torch.backends.cudnn.version() if torch.backends.cudnn.is_available() else None,
            "cuda_available": torch.cuda.is_available(),
            "device_count": torch.cuda.device_count(),
            "device_name": torch.cuda.get_device_name(0) if torch.cuda.is_available() else None,
            "device_capability": list(torch.cuda.get_device_capability(0)) if torch.cuda.is_available() else None,
            "bf16_supported": torch.cuda.is_bf16_supported() if torch.cuda.is_available() else None,
        }

    check("torch_cuda_driver_level", torch_versions)

    def cuda_context_probe() -> dict[str, object]:
        import torch

        if not torch.cuda.is_available():
            raise RuntimeError("CUDA is not available")
        device = torch.device("cuda", 0)
        torch.cuda.init()
        torch.cuda.reset_peak_memory_stats(device)
        left = torch.randn(256, 256, device=device)
        right = torch.randn(256, 256, device=device)
        product = left @ right
        torch.cuda.synchronize(device)
        checksum = float(product.sum().item())
        return {
            "operation": "256x256 randn matmul in float32 on cuda:0",
            "checksum": checksum,
            "allocated_bytes": torch.cuda.memory_allocated(device),
            "reserved_bytes": torch.cuda.memory_reserved(device),
            "peak_reserved_bytes": torch.cuda.max_memory_reserved(device),
        }

    if args.cuda_context_probe:
        check("cuda_context_and_operator", cuda_context_probe)

    gpu_after = nvidia_smi()
    elapsed = time.perf_counter() - started

    windows_version = None
    if hasattr(sys, "getwindowsversion"):
        version = sys.getwindowsversion()
        windows_version = {
            "major": version.major,
            "minor": version.minor,
            "build": version.build,
            "platform_version": getattr(version, "platform_version", None),
            "service_pack": getattr(version, "service_pack", None),
        }

    failures = [item for item in checks if item.get("status") == "fail"]
    evidence = {
        "schema_version": 1,
        "kind": "windows-parity-runtime-probe",
        "status": "pass" if not failures else "fail",
        "elapsed_seconds": round(elapsed, 3),
        "platform": {
            "system": sys.platform,
            "windows_version": windows_version,
            "machine": os.environ.get("PROCESSOR_IDENTIFIER", ""),
            "python_executable": sys.executable,
            "python_version": sys.version,
            "isolated": bool(sys.flags.isolated),
        },
        "runtime_dir": {
            "path": str(args.runtime_dir),
            "bytes": directory_bytes(args.runtime_dir) if args.runtime_dir.exists() else None,
        },
        "wheelhouse": {
            "path": str(args.wheelhouse),
            "bytes": directory_bytes(args.wheelhouse) if args.wheelhouse.exists() else None,
        },
        "offline_environment": {key: os.environ.get(key) for key in OFFLINE_ENV},
        "pins": pins,
        "checks": checks,
        "gpu": {"before": gpu_before, "after": gpu_after},
        "limitations": [
            "This probe proves the frozen interpreter installs and imports the pinned closure offline.",
            "It does not prove model loading, inference quality, or resource budgets.",
            "Without --cuda-context-probe no CUDA context is created, to avoid disturbing concurrent GPU work.",
        ],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, ensure_ascii=False, indent=2) + "\n", encoding="utf-8", newline="\n")
    print(json.dumps({"status": evidence["status"], "elapsed_seconds": evidence["elapsed_seconds"], "output": str(args.output)}))
    return 0 if evidence["status"] == "pass" else 1


if __name__ == "__main__":
    raise SystemExit(main())
