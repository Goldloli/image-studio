"""FLUX.2 Klein 4B distilled backend: NF4 components + model CPU offload.

Gate 0 (verification/windows-parity-gate0/klein-gate0-fresh-process-nf4.json)
measured this strategy inside the frozen Windows budget: load 12.88 s,
text-to-image 7.16 s, edit 10.36 s, CUDA reserved 9.207 GiB, process private
commit 19.132 GiB, available RAM minimum 14.931 GiB. The worker must run exactly
one of these strategies per fresh process; stacking a second strategy in the
same process oversells system memory (combined-process failure record kept in
verification/windows-parity-gate0/klein-gate0.json).
"""

from __future__ import annotations

import gc
import json
import time
from pathlib import Path

from .. import expected_parameters
from ..resources import (
    COMPACT_CUDA_LIMIT_GIB,
    CUDA_RESERVED_LIMIT_GIB,
    BudgetExceeded,
    CudaPeaks,
    ResourceSampler,
    compute_torch_dtype,
    enforce_budget,
    require_available_ram,
    resource_report,
)
from .common import (
    load_source_rgb,
    log,
    save_primary_image,
    select_device,
)

BUDGET_LOAD_SECONDS = 600.0
RESERVED_LIMIT_GIB = 10.5


def quantized_pipeline_config(compute_dtype=None):
    import torch
    from diffusers import PipelineQuantizationConfig

    return PipelineQuantizationConfig(
        quant_backend="bitsandbytes_4bit",
        quant_kwargs={
            "load_in_4bit": True,
            "bnb_4bit_quant_type": "nf4",
            "bnb_4bit_compute_dtype": compute_dtype or torch.bfloat16,
            "bnb_4bit_use_double_quant": True,
        },
        components_to_quantize=["transformer", "text_encoder"],
    )


def load_pipeline(model_path: Path, device_index: int, compute_dtype=None):
    import torch
    from diffusers import Flux2KleinPipeline

    pipeline = Flux2KleinPipeline.from_pretrained(
        str(model_path),
        torch_dtype=compute_dtype or torch.bfloat16,
        low_cpu_mem_usage=True,
        quantization_config=quantized_pipeline_config(compute_dtype),
    )
    pipeline.enable_model_cpu_offload(device=torch.device("cuda", device_index))
    pipeline.set_progress_bar_config(disable=True)
    log("klein pipeline loaded (nf4 components, model cpu offload)")
    return pipeline


def generate(request, progress, cancel):
    import torch

    from .common import checkpoint

    started = time.monotonic()
    sampler = ResourceSampler()
    sampler.start()
    cuda = CudaPeaks()
    pipeline = None
    try:
        require_available_ram("klein load")
        parameters = request["parameters"]
        resolution = int(parameters.get("resolution", 1024))
        compute_dtype = compute_torch_dtype(parameters.get("compute_dtype"))
        compact = resolution != 1024
        device_index, properties = select_device(
            8 if compact else 10, (7, 5) if compact else (8, 6)
        )
        torch.cuda.set_device(device_index)
        torch.cuda.empty_cache()
        log(
            f"klein device={properties.name} cc={properties.major}.{properties.minor} "
            f"resolution={resolution}"
        )
        progress("loading")
        cuda.reset_phase()
        load_started = time.perf_counter()
        pipeline = load_pipeline(Path(request["model_path"]), device_index, compute_dtype)
        load_seconds = time.perf_counter() - load_started
        load_phase = cuda.record_phase()
        log(
            f"klein load_seconds={load_seconds:.2f} "
            f"cuda_reserved_peak_gib={load_phase['cuda_reserved_peak_gib']}"
        )
        if load_seconds > BUDGET_LOAD_SECONDS:
            raise BudgetExceeded(
                f"klein load {load_seconds:.1f} s > {BUDGET_LOAD_SECONDS:.0f} s"
            )
        checkpoint(cancel)

        capability = request["capability_id"]
        kind = "edit" if "edit" in capability else "text"
        source = None
        output_dir = Path(request["output_dir"])
        if kind == "edit":
            source = load_source_rgb(request, output_dir)
        steps = int(request["parameters"]["steps"])
        tokenizer = getattr(pipeline, "tokenizer", None)
        if tokenizer is not None:
            tokens = len(tokenizer.encode(request["instruction"]))
            log(f"klein instruction_tokens={tokens} token_budget=512")
            if tokens > 512:
                raise ValueError(
                    f"instruction uses {tokens} tokens, over the 512-token budget"
                )

        def callback(_pipe, step, _timestep, callback_kwargs):
            checkpoint(cancel)
            progress("sampling", step + 1, steps)
            return callback_kwargs

        progress("sampling", 0, steps)
        cuda.reset_phase()
        generation_started = time.perf_counter()
        generator = torch.Generator(device="cpu").manual_seed(request["seed"])
        result = pipeline(
            image=source,
            prompt=request["instruction"],
            height=resolution,
            width=resolution,
            num_inference_steps=steps,
            guidance_scale=float(request["parameters"]["guidance"]),
            generator=generator,
            callback_on_step_end=callback,
            callback_on_step_end_tensor_inputs=["latents"],
        )
        torch.cuda.synchronize()
        generation_seconds = time.perf_counter() - generation_started
        generation_phase = cuda.record_phase()
        log(
            f"klein generation_seconds={generation_seconds:.2f} kind={kind} "
            f"cuda_reserved_peak_gib={generation_phase['cuda_reserved_peak_gib']}"
        )
        checkpoint(cancel)
        progress("writing")
        if len(result.images) != 1:
            raise ValueError("pipeline did not return exactly one image")
        candidate = save_primary_image(
            output_dir, result.images[0], width=resolution, height=resolution
        )
        elapsed_seconds = time.monotonic() - started
        resources = resource_report(sampler, cuda, elapsed_seconds)
        log("klein resources=" + json.dumps(resources, ensure_ascii=False))
        enforce_budget(
            sampler,
            cuda,
            elapsed_seconds,
            COMPACT_CUDA_LIMIT_GIB if compact else CUDA_RESERVED_LIMIT_GIB,
        )
        return candidate
    finally:
        sampler.stop()
        if pipeline is not None:
            try:
                pipeline.maybe_free_model_hooks()
            except Exception:  # noqa: BLE001 - best effort cleanup
                pass
            del pipeline
        gc.collect()
        try:
            import torch

            torch.cuda.empty_cache()
        except Exception:  # noqa: BLE001 - best effort cleanup
            pass
