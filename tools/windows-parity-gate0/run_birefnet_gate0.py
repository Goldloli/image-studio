"""Gate 0 G2-B1: BiRefNet ONNX Runtime CPU feasibility on the Windows target.

Runs the frozen BiRefNet-general-epoch_244 ONNX graph with
CPUExecutionProvider on a controlled real photo and records real timing,
resources, the graph contract, RGBA/mask parity and quality data.

The preprocessing/postprocessing functions are imported unchanged from the
frozen product worker wheel ``studio_onnx_worker-0.1.0-py3-none-any.whl``
(sha256 59d709c7705569602d01eeffebeca6b2cbc4d02fda181ecb157f937ea91c66c3),
which pins ``Requires-Python ==3.13.*``; Gate 0 runs on the frozen Python
3.12.10 experiment interpreter, so the pure-Python worker module is extracted
into the isolated py-aux environment instead of pip-installed. Its code is used
verbatim, so the product's "one sigmoid on final Conv logits", align-corners
resize and straight-alpha mask composition are exercised exactly.

Experiment-only tooling for OpenSpec change ``achieve-windows-model-parity``
(tasks.md 1.1 / G2-B1, bd issue image-studio-xwh.1). Not part of the product.
"""

from __future__ import annotations

import argparse
import ctypes
import hashlib
import json
import os
import platform
import subprocess
import time
import traceback
from pathlib import Path

import numpy as np
from PIL import Image

from studio_onnx_worker.backend import (  # frozen product worker contract
    NETWORK_SIZE,
    compose_mask,
    preprocess,
    resize_align_corners,
    sigmoid_logits,
)

DEFAULT_MODEL = Path(
    "target/parity-gate0/models/birefnet/BiRefNet-general-epoch_244.onnx"
)
DEFAULT_INPUT = Path(
    "verification/planned-models-20261031/fixtures/original/portrait-hair.png"
)
DEFAULT_OUTPUT_DIR = Path("verification/windows-parity-gate0")
EXPECTED_MODEL_BYTES = 972666916
EXPECTED_MODEL_SHA256 = (
    "58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c"
)
EXPECTED_WORKER_WHEEL_SHA256 = (
    "59d709c7705569602d01eeffebeca6b2cbc4d02fda181ecb157f937ea91c66c3"
)
WORKER_WHEEL = Path(
    "resources/planned-models/birefnet/studio_onnx_worker-0.1.0-py3-none-any.whl"
)


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


def gpu_identity() -> dict | None:
    try:
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
    except (OSError, subprocess.SubprocessError):
        return None
    if not output:
        return None
    name, driver, total = [item.strip() for item in output.splitlines()[0].split(",")]
    return {"name": name, "driver": driver, "memory_total_mib": int(total)}


class Sampler:
    """Sample process-tree private commit and system available RAM."""

    def __init__(self, interval_seconds: float = 0.1) -> None:
        import psutil

        self.psutil = psutil
        self.interval = interval_seconds
        self.stop_event = None
        self.thread = None
        self.private_peak = 0
        self.available_ram_min = 1 << 62
        self.system_total = 0
        self.samples = 0

    def _sample(self) -> None:
        psutil = self.psutil
        root = psutil.Process(os.getpid())
        total = 0
        for process in [root, *root.children(recursive=True)]:
            try:
                total += int(process.memory_info().private)
            except (psutil.NoSuchProcess, psutil.AccessDenied):
                pass
        self.private_peak = max(self.private_peak, total)
        memory = system_memory()
        self.system_total = memory["physical_total_bytes"]
        self.available_ram_min = min(
            self.available_ram_min, memory["physical_available_bytes"]
        )
        self.samples += 1

    def _loop(self) -> None:
        import threading

        while not self.stop_event.wait(self.interval):
            try:
                self._sample()
            except Exception:  # noqa: BLE001 - experiment sampler
                pass

    def start(self) -> None:
        import threading

        self.stop_event = threading.Event()
        self._sample()
        self.thread = threading.Thread(target=self._loop, daemon=True)
        self.thread.start()

    def stop(self) -> None:
        if self.stop_event is not None:
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


def input_identity(path: Path) -> dict:
    with Image.open(path) as opened:
        opened.load()
        return {
            "path": str(path),
            "bytes": path.stat().st_size,
            "sha256": sha256_file(path),
            "width": opened.width,
            "height": opened.height,
            "mode": opened.mode,
        }


def mask_quality(alpha: np.ndarray, source_alpha: np.ndarray) -> dict:
    values = alpha.astype(np.uint16).ravel()
    source = np.asarray(source_alpha, dtype=np.uint8)
    total = int(values.size)
    transparent = int(np.count_nonzero(values == 0))
    opaque = int(np.count_nonzero(values == 255))
    partial = total - transparent - opaque
    return {
        "min": int(values.min()),
        "max": int(values.max()),
        "mean": round(float(values.mean()), 3),
        "std": round(float(values.std()), 3),
        "distinct_values": int(np.unique(values).size),
        "fully_transparent_pixels": transparent,
        "fully_opaque_pixels": opaque,
        "partial_alpha_pixels": partial,
        "partial_alpha_fraction": round(partial / total, 6),
        "nonempty_mask": bool(np.any(values > 0)),
        "spatial_variation": bool(float(values.std()) > 1.0),
        "source_alpha_uniform_255": bool(np.all(source == 255)),
        "coverage_ge_128": round(
            float(np.count_nonzero(values >= 128)) / total, 6
        ),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, default=DEFAULT_MODEL)
    parser.add_argument("--input", type=Path, default=DEFAULT_INPUT)
    parser.add_argument("--output-dir", type=Path, default=DEFAULT_OUTPUT_DIR)
    parser.add_argument("--report", type=Path)
    parser.add_argument("--intra-op-threads", type=int, default=4)
    parser.add_argument("--sample-interval", type=float, default=0.1)
    parser.add_argument("--load-limit-seconds", type=float, default=600)
    parser.add_argument("--inference-limit-seconds", type=float, default=900)
    parser.add_argument("--private-commit-limit-gib", type=float, default=24)
    parser.add_argument("--available-ram-minimum-gib", type=float, default=2)
    args = parser.parse_args()

    model_path = args.model.resolve()
    input_path = args.input.resolve()
    output_dir = args.output_dir.resolve()
    output_dir.mkdir(parents=True, exist_ok=True)
    report_path = (
        args.report.resolve()
        if args.report
        else output_dir / "birefnet-gate0.json"
    )

    record: dict = {
        "schema_version": 1,
        "kind": "birefnet-general-epoch-244-windows-gate0",
        "case_id": "G2-B1",
        "started_at_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "machine": {
            "os": platform.system(),
            "os_version": platform.version(),
            "platform": platform.platform(),
            "cpu": platform.processor(),
            "logical_cores": os.cpu_count(),
            "gpu": gpu_identity(),
        },
        "budget": {
            "load_limit_seconds": args.load_limit_seconds,
            "inference_limit_seconds": args.inference_limit_seconds,
            "private_commit_limit_gib": args.private_commit_limit_gib,
            "available_ram_minimum_gib": args.available_ram_minimum_gib,
            "cuda_reserved_limit_gib": 10.5,
            "cuda_in_use": False,
        },
        "model_manifest": {
            "manifest": "resources/planned-models/birefnet/model-manifest.json",
            "id": "birefnet-general-epoch-244-onnx",
            "repository": "ZhengPeng7/BiRefNet",
            "revision": "v1 / release asset 186740004",
            "expected_bytes": EXPECTED_MODEL_BYTES,
            "expected_sha256": EXPECTED_MODEL_SHA256,
        },
    }

    sampler = Sampler(args.sample_interval)
    sampler.start()
    try:
        import onnxruntime as ort
        import numpy
        import PIL

        record["versions"] = {
            "python": platform.python_version(),
            "onnxruntime": ort.__version__,
            "numpy": numpy.__version__,
            "pillow": PIL.__version__,
            "worker_wheel": "studio-onnx-worker-0.1.0",
        }
        worker_wheel = WORKER_WHEEL.resolve()
        worker_wheel_sha = sha256_file(worker_wheel) if worker_wheel.exists() else None
        record["worker"] = {
            "wheel": "resources/planned-models/birefnet/"
            "studio_onnx_worker-0.1.0-py3-none-any.whl",
            "wheel_sha256": worker_wheel_sha,
            "expected_wheel_sha256": EXPECTED_WORKER_WHEEL_SHA256,
            "module": "studio_onnx_worker.backend",
            "imported_functions": [
                "preprocess",
                "sigmoid_logits",
                "resize_align_corners",
                "compose_mask",
            ],
            "install_mode": "extracted into isolated py-aux site-packages "
            "(wheel pins Requires-Python ==3.13.*, code used verbatim)",
        }
        if worker_wheel_sha != EXPECTED_WORKER_WHEEL_SHA256:
            raise RuntimeError(
                f"worker wheel SHA-256 mismatch: {worker_wheel_sha} != "
                f"{EXPECTED_WORKER_WHEEL_SHA256}"
            )

        if not model_path.is_file():
            raise RuntimeError(f"model not found: {model_path}")
        model_bytes = model_path.stat().st_size
        model_sha = sha256_file(model_path)
        record["model"] = {
            "path": str(model_path),
            "bytes": model_bytes,
            "sha256": model_sha,
            "size_matches_manifest": model_bytes == EXPECTED_MODEL_BYTES,
            "sha256_matches_manifest": model_sha == EXPECTED_MODEL_SHA256,
        }
        if model_bytes != EXPECTED_MODEL_BYTES or model_sha != EXPECTED_MODEL_SHA256:
            raise RuntimeError("model bytes/SHA-256 do not match the frozen manifest")

        source = input_identity(input_path)
        record["input"] = source
        if (
            source["width"] <= 0
            or source["height"] <= 0
            or source["width"] > 4096
            or source["height"] > 4096
            or source["width"] * source["height"] > 4 * 1024 * 1024
        ):
            raise RuntimeError("controlled photo exceeds the product source limits")
        with Image.open(input_path) as opened:
            opened.load()
            image = opened.convert("RGBA")
        source_rgb = np.asarray(image.convert("RGB"), dtype=np.uint8).copy()

        options = ort.SessionOptions()
        options.intra_op_num_threads = args.intra_op_threads
        options.inter_op_num_threads = 1
        options.execution_mode = ort.ExecutionMode.ORT_SEQUENTIAL
        load_started = time.perf_counter()
        session = ort.InferenceSession(
            str(model_path),
            sess_options=options,
            providers=["CPUExecutionProvider"],
        )
        load_seconds = time.perf_counter() - load_started
        if session.get_providers() != ["CPUExecutionProvider"]:
            raise RuntimeError(
                f"expected only CPUExecutionProvider, got {session.get_providers()}"
            )
        inputs = session.get_inputs()
        outputs = session.get_outputs()
        if len(inputs) != 1 or len(outputs) < 1:
            raise RuntimeError("unexpected ONNX graph input/output count")
        input_spec = inputs[0]
        record["graph"] = {
            "inputs": [
                {"name": spec.name, "type": spec.type, "shape": list(spec.shape)}
                for spec in inputs
            ],
            "outputs": [
                {"name": spec.name, "type": spec.type, "shape": list(spec.shape)}
                for spec in outputs
            ],
            "expected_input": {
                "name": "input_image",
                "type": "tensor(float)",
                "shape": [1, 3, 1024, 1024],
            },
            "expected_output": {
                "name": "output_image",
                "type": "tensor(float)",
                "shape": [1, 1, 1024, 1024],
            },
        }
        record["session"] = {
            "providers": session.get_providers(),
            "intra_op_num_threads": args.intra_op_threads,
            "inter_op_num_threads": 1,
            "execution_mode": "ORT_SEQUENTIAL",
        }
        record["phases"] = {
            "load": {"seconds": round(load_seconds, 3)},
        }
        if input_spec.type != "tensor(float)" or tuple(input_spec.shape) != (
            1,
            3,
            NETWORK_SIZE,
            NETWORK_SIZE,
        ):
            raise RuntimeError(
                f"unexpected graph input contract: {input_spec.type} "
                f"{input_spec.shape}"
            )

        preprocess_started = time.perf_counter()
        tensor = preprocess(image)
        preprocess_seconds = time.perf_counter() - preprocess_started
        record["preprocessing"] = {
            "contract": "birefnet-1024-rgb-v1",
            "seconds": round(preprocess_seconds, 3),
            "tensor_shape": list(tensor.shape),
            "tensor_dtype": str(tensor.dtype),
            "tensor_min": round(float(tensor.min()), 6),
            "tensor_max": round(float(tensor.max()), 6),
        }

        inference_started = time.perf_counter()
        raw = session.run(None, {input_spec.name: tensor})[0]
        inference_seconds = time.perf_counter() - inference_started
        record["phases"]["inference"] = {"seconds": round(inference_seconds, 3)}
        record["raw_logits"] = {
            "shape": list(raw.shape),
            "dtype": str(raw.dtype),
            "min": float(np.min(raw)),
            "max": float(np.max(raw)),
            "contains_values_outside_probability_range": bool(
                float(np.min(raw)) < 0.0 or float(np.max(raw)) > 1.0
            ),
        }
        if tuple(raw.shape) != (1, 1, NETWORK_SIZE, NETWORK_SIZE):
            raise RuntimeError(f"unexpected output shape: {raw.shape}")

        postprocess_started = time.perf_counter()
        probability = sigmoid_logits(raw)
        resized = resize_align_corners(
            probability[0, 0], image.width, image.height
        )
        alpha = compose_mask(resized, image.getchannel("A"))
        rgba = np.asarray(image, dtype=np.uint8).copy()
        rgba[..., 3] = alpha
        postprocess_seconds = time.perf_counter() - postprocess_started
        record["phases"]["postprocess"] = {"seconds": round(postprocess_seconds, 3)}

        primary_path = output_dir / "birefnet-gate0-rgba.png"
        mask_path = output_dir / "birefnet-gate0-mask.png"
        Image.fromarray(rgba, "RGBA").save(primary_path, format="PNG")
        Image.fromarray(alpha, "L").save(mask_path, format="PNG")

        loaded_primary = np.asarray(Image.open(primary_path).convert("RGBA"))
        loaded_mask = np.asarray(Image.open(mask_path).convert("L"))
        checks = {
            "primary_alpha_equals_mask": bool(
                np.array_equal(loaded_primary[..., 3], loaded_mask)
            ),
            "primary_rgb_equals_source_rgb_straight_alpha": bool(
                np.array_equal(loaded_primary[..., :3], source_rgb)
            ),
            "output_dimensions_equal_source": bool(
                loaded_primary.shape[1] == image.width
                and loaded_primary.shape[0] == image.height
                and loaded_mask.shape == (image.height, image.width)
            ),
            "single_sigmoid_probability_range": bool(
                float(probability.min()) >= 0.0
                and float(probability.max()) <= 1.0
            ),
            "mask_matches_rounded_probability": bool(
                np.array_equal(
                    alpha,
                    np.floor(resized * 255.0 + 0.5).clip(0, 255).astype(np.uint8),
                )
            ),
        }
        record["outputs"] = {
            "rgba": {
                "path": str(primary_path),
                "bytes": primary_path.stat().st_size,
                "sha256": sha256_file(primary_path),
                "width": int(loaded_primary.shape[1]),
                "height": int(loaded_primary.shape[0]),
                "mode": "RGBA",
            },
            "mask": {
                "path": str(mask_path),
                "bytes": mask_path.stat().st_size,
                "sha256": sha256_file(mask_path),
                "width": int(loaded_mask.shape[1]),
                "height": int(loaded_mask.shape[0]),
                "mode": "L (8-bit)",
            },
        }
        record["checks"] = checks
        record["quality"] = mask_quality(alpha, image.getchannel("A"))

        record["phases"]["total"] = {
            "seconds": round(
                load_seconds
                + preprocess_seconds
                + inference_seconds
                + postprocess_seconds,
                3,
            )
        }
        sampler.stop()
        resources = sampler.snapshot()
        record["resources"] = resources
        failures = []
        if load_seconds > args.load_limit_seconds:
            failures.append("load exceeds limit")
        if inference_seconds > args.inference_limit_seconds:
            failures.append("inference exceeds limit")
        if resources["process_tree_private_commit_peak_gib"] > args.private_commit_limit_gib:
            failures.append("private commit exceeds limit")
        if resources["system_available_ram_min_gib"] < args.available_ram_minimum_gib:
            failures.append("available RAM below minimum")
        if not all(checks.values()):
            failures.append("output contract check failed")
        if not record["raw_logits"]["contains_values_outside_probability_range"]:
            failures.append("raw logits look already normalized; sigmoid contract unclear")
        record["failures"] = failures
        record["status"] = "passed" if not failures else "failed"
        record["finished_at_utc"] = time.strftime(
            "%Y-%m-%dT%H:%M:%SZ", time.gmtime()
        )
        write_json(report_path, record)
        print(json.dumps(record, ensure_ascii=False, indent=2))
        return 0 if not failures else 2
    except BaseException as error:  # noqa: BLE001 - experiment harness
        sampler.stop()
        record["status"] = "error"
        record["error"] = f"{type(error).__name__}: {error}"
        record["traceback"] = traceback.format_exc()
        try:
            record["resources"] = sampler.snapshot()
        except Exception:  # noqa: BLE001 - best effort
            pass
        record["finished_at_utc"] = time.strftime(
            "%Y-%m-%dT%H:%M:%SZ", time.gmtime()
        )
        write_json(report_path, record)
        print(json.dumps(record, ensure_ascii=False, indent=2))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
