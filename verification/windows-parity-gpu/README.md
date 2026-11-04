# GPU residency / utilization audit (GPU-AUD-1)

Independent measurement that answers the user's question "why did the GPU look
idle / is the GPU actually doing the work?" for the Windows parity staged
execution. The audit runs one real Qwen-Image 2.1-Turbo staged generation from
the existing prepared NF4 cache while sampling GPU utilization, clocks, memory,
CUDA allocator state and CPU load, and asserts device residency for every phase.

Status: **pass**. No phase ran on CPU unexpectedly, no CPU fallback was found,
and the DiT sampling loop shows sustained near-saturated GPU utilization.

## Machine and versions

- Windows 10 Pro 22H2 (10.0.19045), i7-13700K (24 logical CPUs), 32 GB RAM
- NVIDIA GeForce RTX 3080 Ti 12 GB, driver 596.49
- Frozen Windows v2 runtime (`windows-python31210-torch2100-cu126-diff041-transformers53-v2`):
  Python 3.12.10, torch 2.10.0+cu126 (CUDA 12.6), diffusers 0.41.0,
  transformers 5.3.0
- Model: Qwen-Image 2.1-Turbo snapshot (32,451,624,991 bytes on disk), prepared
  NF4 cache `target/parity-gate0/staged/prepared-qwen21-turbo`
  (10,049,397,619 bytes: text encoder + transformer, `bnb_4bit_quant_type=nf4`,
  `bnb_4bit_quant_storage=uint8`, compute dtype bfloat16)
- Generation: 1024x1024, 8 steps, seed 3148150817, VAE tiling; same math as
  `tools/windows-parity-staged/run_qwen21_staged.py`

## Command

```powershell
$env:PYTHONDONTWRITEBYTECODE='1'; $env:PYTHONUTF8='1'; $env:PYTHONNOUSERSITE='1'
$env:HF_HUB_OFFLINE='1'; $env:TRANSFORMERS_OFFLINE='1'
target/parity-gate0/py312/python.exe tools/windows-parity-gpu-audit/sample_utilization.py `
  --model-dir target/parity-gate0/models/qwen-image-2.1-turbo `
  --prepared-dir target/parity-gate0/staged/prepared-qwen21-turbo `
  --work-dir target/parity-gate0/staged/gpu-audit `
  --output verification/windows-parity-gpu/qwen21-turbo-gpu-audit.png `
  --report verification/windows-parity-gpu/gpu-residency-audit.json
python tools/windows-parity-gpu-audit/summarize_evidence.py
```

Sample interval target 200 ms; 126 samples were taken with an effective median
interval of 289 ms (each sample includes an `nvidia-smi` call plus CUDA and
psutil reads). The GPU window was granted by the Lead; `nvidia-smi` was checked
before/after and the GPU was released immediately after the run.

## Phase split: why the earlier timeline looked CPU-bound

| Phase | Device | Wall time | Status | Evidence |
|---|---|---|---|---|
| Model transfer (Qwen base reference, 33.13 GB) | network+disk | 753.1 s | measured reference | `verification/windows-parity-gate0/transfer-qwen21-base.json` |
| NF4 prepare: quantize + save text encoder/transformer | CPU | 62.0 s | measured | `target/parity-gate0/staged/qwen21-turbo-prepare.json` |
| Windows v2 runtime install (2.71 GB wheelhouse → 4.78 GB) | CPU+disk | 124.0 s | measured | `verification/windows-parity-runtime/install-evidence.json` |
| Generation, prior Gate 0 (warm cache) | GPU | 34.0 s | measured | `target/parity-gate0/staged/qwen21-turbo-staged-prepared.json` |
| Generation, this audit (warm cache) | GPU | 40.8 s | measured | this directory |

Reference one-time setup for this model class is therefore ~939 s
(753 + 62 + 124), while the DiT sampling that owns the GPU is 10.24 s — about
**1.05 %** of that reference pipeline. A warm-cache generation is ~34–41 s of
which ~13 s (text encode 0.82 s + DiT sampling 10.24 s + VAE decode 1.98 s) is
GPU-active. Downloads, NF4 quantization/prepare, runtime install and Rust builds
are what dominate the long CPU-looking periods; the GPU only gets short, bursty
single-batch 4-bit work.

## GPU utilization by phase

| Phase | Samples | GPU util mean/median/p95/max % | SM clock mean/max MHz | Mem used max MiB | Process CPU mean % | System CPU mean % |
|---|---|---|---|---|---|---|
| idle_baseline | 3 | 20.33 / 27 / 27 / 27 | 1815 / 1965 | 4569 | 5.87 | 39.67 |
| text_load | 45 | 31.84 / 15 / 100 / 100 | 1814 / 1965 | 7795 | 70.04 | 43.04 |
| text_encode | 2 | 8.50 / 8 / 9 / 9 | 1665 / 1665 | 8061 | 99.80 | 64.00 |
| dit_load | 18 | 12.00 / 12 / 15 / 15 | 1665 / 1665 | 5642 | 109.18 | 45.92 |
| **dit_sample** | **36** | **96.94 / 100 / 100 / 100** | **1895 / 1920** | **8756** | **106.09** | **59.35** |
| vae_load | 6 | 13.17 / 12 / 22 / 22 | 1827.5 / 1950 | 4427 | 54.72 | 72.78 |
| vae_decode | 7 | 68.00 / 82 / 98 / 98 | 1630.7 / 1905 | 4949 | 87.29 | 76.07 |
| idle_after | 2 | 12.00 / 12 / 12 / 12 | 1950 / 1950 | 3857 | 13.15 | 77.90 |

DiT sampling also shows GPU memory-controller utilization
mean 28.22 % / median 29 / p95 30 / max 30 (HBM traffic), while memory used
reached 8,756 MiB (desktop baseline included). Low utilization during
`text_load`/`text_encode`/`dit_load` is normal: those phases move ~6.4 GB and
~3.7 GB of weights over PCIe and do CPU-side preparation, so they are
transfer/CPU-bound, not evidence of a CPU fallback.

## Per-step DiT time series (synchronized)

All 8 steps, timed with `time.perf_counter()` around a forced
`torch.cuda.synchronize()`: 1.354, 1.259, 1.251, 1.248, 1.301, 1.260, 1.290,
1.275 s — mean 1.28 s, median 1.27 s, p95 1.35 s, max 1.35 s, min 1.25 s. Slow
step 0 is the KV-cache extract step; the rest are stable, which is what a
GPU-resident sampling loop looks like.

## Device residency assertions (no CPU fallback)

- text encoder: 750/750 parameters on `cuda` (`device_counts={"cuda": 750}`)
- DiT transformer: 297/297 parameters on `cuda`
- VAE: 238/238 parameters on `cuda`
- all 8 DiT forward passes received their `hidden_states` on cuda (forward
  pre-hook assertion, 8/8)
- tensors: `prompt_embeds`, `prompt_embeds_mask`, `image_pad_mask`, `latents`,
  VAE `unpacked` latents and decoded image all recorded as `cuda`; text-encode
  inputs/`hidden_states` also cuda
- main CUDA thread allocator during every DiT step:
  3,699,684,352 bytes allocated / 4,911,529,984 bytes reserved
- after each stage release the runtime returns to ~0.02 GiB reserved, proving
  the staged design really unloads components between phases

## Output identity

Output image `qwen21-turbo-gpu-audit.png`: 1024x1024, 2,007,139 bytes,
SHA-256 `cf071e9ce8d1bc7769fa55a5b90faab2bcca69aaf1921163e2aa77abfeb9cd73`.
This is byte-identical to the previous Gate 0 staged output
(`qwen21-turbo-staged-prepared.png`), so instrumentation did not change the
computation: same prepared cache, same seed/steps → same image.

## Citable conclusion

> During staged generation the GPU is genuinely doing the work: every model
> parameter and intermediate tensor (text encoder 750/750, DiT 297/297, VAE
> 238/238 parameters, latents and prompt embeds) is CUDA-resident, all 8 DiT
> forward passes ran on cuda, GPU utilization during DiT sampling was
> mean 96.94% / median 100% / p95 100% / max 100% with SM clocks at ~1.9 GHz,
> and each 1024x1024 step took ~1.28 s; the long CPU-looking timeline is
> one-time download/prepare/build work, not CPU fallback or idle inference.

## Limitations

- The background sampler thread's `torch.cuda.memory_allocated/reserved` values
  read 0 in this run (thread-level allocator visibility limitation of the
  sampler, not a computation problem). The real allocator values were recorded
  from the main CUDA thread at every step (3.70 GiB allocated / 4.91 GiB
  reserved) and are the numbers quoted above. The harness now also samples
  `torch.cuda.mem_get_info()` for future reruns.
- The checked-in JSON is the summarized phase statistics (the raw 200 ms sample
  rows and the enriched sampler belong to a later harness revision; the raw
  audit copy is kept at `target/parity-gate0/staged/gpu-audit/gpu-audit-raw.json`).
- One generation only (one seed, 8 steps); this is not a throughput benchmark.
- Idle baseline and low-utilization phases include desktop-compositor GPU usage
  (~1.4–4.5 GiB), so "idle" here is not zero.
- The audit script mirrors the experiment staged runner; it is not product code.

## Files

| File | Content |
|---|---|
| `gpu-residency-audit.json` | per-phase utilization/clock/memory/CPU statistics, device residency assertions, step time series, phase split + wall-clock context, limitations |
| `qwen21-turbo-gpu-audit.png` | generated 1024x1024 output (sha256 above; identical to prior Gate 0 output) |
| `../../tools/windows-parity-gpu-audit/sample_utilization.py` | measurement harness |
| `../../tools/windows-parity-gpu-audit/summarize_evidence.py` | phase-split / explanation summarizer |
