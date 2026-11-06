# Windows 8 GB execution tier

Status: implemented and measured on the reference 8 GB machine
(Windows 11 Enterprise 22631 / i9-9900K / **RTX 2060 SUPER 8192 MiB, sm75 Turing** / 31.9 GiB RAM,
driver 560.94). Acceptance evidence lives in
[`verification/windows-8gb-tier/`](../verification/windows-8gb-tier/README.md).

## What the tier is

Every Windows model registers hardware thresholds per tier. A device that satisfies only the
compact thresholds runs a frozen **compact recipe**: 768×768, `float16`, its own recipe id, and
the same model snapshot, runtime and prepared cache as the standard tier.

| Tier | VRAM | Canvas | Compute | Load strategy |
|---|---|---|---|---|
| `standard` | ≥ 12 GB | 1024² | bfloat16 | staged / component offload (unchanged) |
| `compact` | ≥ 8 GB | 768² | float16 | staged, one NF4 component resident at a time |

Both tiers share one 25–33 GB model download and one prepared NF4 cache; only the recipe identity
and its exact parameter map differ. A device below both tiers keeps the model disabled with the
lowest unmet requirement as its reason.

## Support matrix after the change

| Model | 8 GB eligibility | Measured compact peak | Notes |
|---|---|---|---|
| Qwen-Image 2.1-Turbo | 紧凑档 | **5.961 GiB** reserved | 84 s, 8 steps, 768² |
| Qwen-Image 2.1 | 紧凑档 | **5.961 GiB** reserved | 391 s, 40 steps, 768² |
| MOSS-Image-SFT | 紧凑档 | **5.539 GiB** reserved | 752 s of a 900 s budget, 50 steps |
| FLUX.2 Klein 4B (t2i / edit) | 紧凑档 | **5.436 GiB** reserved | 146 s, 4 steps |
| BiRefNet epoch_244 | unchanged (CPU) | 0 GiB | ONNX Runtime CPU route already fit |
| Real-ESRGAN x4plus | unchanged | < 1 GiB | ncnn Vulkan, fixed 4× |
| Z-Image Turbo | **not supported** | ~7.8 GiB | stays gated at 12 GB |

Budget for the compact tier: CUDA reserved ≤ 6.0 GiB with ≥ 1.5 GiB left for the desktop
compositor, loading ≤ 600 s, one generation ≤ 900 s, and no WDDM shared-memory fallback.

## Z-Image: why it is not on this list

Three measured attempts failed on the reference machine:

1. 768², fp16 compute, fp16 VAE → the VAE decode reported `invalid value encountered in cast` and
   the PNG is black (NaN latents); GPU peak 7,809 MiB.
2. 512², bf16 compute → `No available kernel` — bitsandbytes ships no bf16 NF4 kernel for sm75.
3. 512², fp16 compute, fp32 VAE → still black; GPU peak 7,819 MiB.

The peak is resolution-independent because the v1 backend loads each component with
`device_map={"": "cuda"}` and then moves it back to the CPU. The quantization investigation
([doc/16](16-zimage-quantization-investigation.md)) went further: SDNQ `uint4` lowers the peak to
**4.385 GiB**, but the fp16 NaN is a model-level overflow. With the conditioning path forced to
fp32, a per-module scan places the first non-finite output in **`feed_forward`** (SwiGLU) of the
first transformer layer; attention, norms and embeddings stay finite. Rescuing it would need an
unquantized fp32 FFN (~9 GB of weights) or an upstream numeric fix, so the compact recipe identity
stays in the protocol but the catalog deliberately does not offer it.

## Install-time preparation on 8 GB machines

The 8 GB tier does **not** lower the install-time quantization cost. Preparing a Qwen/MOSS cache
takes a whole component in memory: measured commit peaks 16.4–23.1 GiB and available physical RAM
can fall to ~0 GiB, so the installer requires **available physical RAM ≥ 28 GiB or commit headroom
≥ 26 GiB**. On the reference machine (75.9 GiB commit limit) the commit branch passes and the
prepare completes in 723 s.

A shard-streaming prepare that would fit 16 GiB machines is an explicit follow-up; see
`openspec/specs/model-installation/spec.md` (安装期量化准备使用低内存分片路径).

## Verifying on another 8 GB machine

```powershell
# 1. Catalog view: the five generative models must be eligible and show 紧凑档 768×768
#    (Z-Image must stay blocked with the 12 GB reason).
# 2. A compact generation must report CUDA reserved ≤ 6.0 GiB in its worker resources.
# 3. The exported PNG must match the stored asset byte-for-byte.
```

The exact commands and captured JSON for the reference run are in
[`verification/windows-8gb-tier/README.md`](../verification/windows-8gb-tier/README.md).

## Measured through the product UI (2026-11-06)

| Capability | Wall clock | Notes |
|---|---|---|
| Qwen-Image 2.1 Turbo 文生图 | 101–116 s | 20 consecutive sends, log shows preflight 50 ms warm |
| Qwen-Image 2.1 文生图 | ~14 min | 40 steps, measured while two other apps held CPU cores |
| MOSS-Image-SFT 文生图 | ~18.5 min | 50 steps, same contention; isolated Gate 0 was 752 s |
| FLUX.2 Klein 4B 文生图 | 177 s | 155 s of it is component load |
| FLUX.2 Klein 4B 编辑 | 36 s | warm component load |
| BiRefNet 去背景 | 12 s | RGBA + mask |
| Real-ESRGAN 4× 放大 | 4 s | 220×220 → 880×880 |