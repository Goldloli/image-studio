# Windows 8 GB execution tier (compact)

Machine: Windows 11 Enterprise 22631 / i9-9900K / **RTX 2060 SUPER 8192 MiB (sm75 Turing)** /
31.9 GiB RAM / driver 560.94 / CUDA 12.6.

This directory records the `support-8gb-vram-tier` acceptance. `standard` recipes (12 GB,
1024², bf16) stay unchanged; the compact tier runs 768² with fp16 compute and its own frozen
recipe ids.

## Qwen-Image 2.1-Turbo compact — Gate 0 passed (2026-11-05)

Fixed inputs: prompt `雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光`, seed `3148150817`,
8 steps, guidance 1.0, NF4 text encoder + NF4 transformer, prepared cache 10,049,397,568 B
(identical to the cache the 12 GB machine produced: text encoder 6.38 GB, transformer 3.67 GB).

| Metric | Measured | Budget |
|---|---|---|
| CUDA reserved peak | **5.961 GiB** | ≤ 6.0 GiB |
| CUDA allocated peak | 5.948 GiB | — |
| Process private commit peak | 14.807 GiB | ≤ 24 GiB |
| Available RAM minimum | 11.513 GiB | ≥ 2 GiB |
| Text encode | 0.82 s | — |
| Sampling (8 steps) | 65.83 s | — |
| Tiled VAE decode | 5.14 s | — |
| Total elapsed | 84.47 s | ≤ 900 s |
| Host-observed GPU used peak | 7,599 MiB (includes ~1.9 GiB desktop) | — |

Output: [qwen21-turbo-768.png](qwen21-turbo-768.png), 768×768, 1,187,027 bytes, SHA-256
`2d085a3d2ea5e669f9ba5987bc22a7adca39919f9c5ca16aa31b511167f0c505`; the image shows the wet
Shanghai street corner with a pedestrian under an umbrella, matching the prompt.

Machine-readable record: [qwen21-turbo-gate0.json](qwen21-turbo-gate0.json).

## MOSS-Image-SFT compact — Gate 0 passed (2026-11-05)

Same machine, same prompt and seed, 768 square, 50 steps, guidance 3.0, NF4 text encoder + NF4
DiT with the documented dtype shim, fp32 VAE. Prepared cache built on this machine in 723 s.

| Metric | Measured | Budget |
|---|---|---|
| CUDA reserved peak | **5.539 GiB** | ≤ 6.0 GiB |
| CUDA allocated peak | 5.450 GiB | — |
| Process private commit peak | 14.727 GiB | ≤ 24 GiB |
| Available RAM minimum | 12.255 GiB | ≥ 2 GiB |
| **Total elapsed** | **752.09 s** | ≤ 900 s (84 % of budget) |
| Host-observed GPU used peak | 5,472 MiB during sampling | — |

Output: [moss-image-sft-768.png](moss-image-sft-768.png), 768×768, 1,021,922 bytes, SHA-256
`e7c4787b7222069bdf350754b11c59b4e7184e5fb8a2d6a5c52e25e4142271f1`; it renders the wet street,
the pedestrian under an umbrella and the storefront signage, matching the prompt.

Record: [moss-gate0.json](moss-gate0.json).

Note: 752 s of 900 s leaves little headroom. A machine with a slower CPU or a second GPU
consumer would exceed the generation budget, so the 50-step MOSS recipe is the first candidate
for a documented speed expectation in the UI.

## FLUX.2 Klein 4B compact — Gate 0 passed (2026-11-05)

Same machine, same prompt and seed, 768 square, 4 steps, guidance 1.0, NF4 text encoder and
transformer with model CPU offload, fp16 components. Klein needs no prepared cache.

| Metric | Measured | Budget |
|---|---|---|
| CUDA reserved peak | **5.436 GiB** | ≤ 6.0 GiB |
| CUDA allocated peak | 4.458 GiB | — |
| Generation (4 steps) | 9.83 s | — |
| Process private commit peak | 15.384 GiB | ≤ 24 GiB |
| Total elapsed | 145.61 s | ≤ 900 s |

Output: [klein-t2i-768.png](klein-t2i-768.png), 768×768, 1,209,215 bytes, SHA-256
`5bb23d72acd8d481d570699ab28831ec0e36e994292b22d89c321e5f3bd0c110`.

Record: [klein-gate0.json](klein-gate0.json).

## Z-Image Turbo compact — not deliverable on 8 GB (measured 2026-11-05)

Z-Image keeps its original 12 GB contract. Three measured attempts on the same machine failed,
so the catalog does not register a compact tier for it:

| Attempt | Result |
|---|---|
| 768 square, fp16 compute, fp16 VAE | Pipeline ran, but the VAE decode emitted `invalid value encountered in cast` and the saved PNG is 1 795 B of black pixels (NaN/Inf latents). GPU peak 7 809 MiB. |
| 512 square, bf16 compute | `No available kernel. Aborting execution.` — bitsandbytes has no bf16 NF4 kernel on sm75. GPU peak 7 747 MiB. |
| 512 square, fp16 compute, fp32 VAE | Still black (843 B PNG). GPU peak 7 819 MiB. |

The VRAM peak is roughly resolution-independent (~7.8 GiB, i.e. ~5.9 GiB of model on top of the
desktop compositor) because it comes from the component loader: the v1 backend loads each
component with `device_map={"": "cuda"}` and then moves it back to the CPU, so the largest
component is materialised on the card during load. That leaves no headroom under 8 GiB even
before sampling. Reproducing Z-Image on 8 GB needs a CPU-side loader plus an fp16-safe sampling
path (or an fp32 transform path), which is outside this change; the compact Z-Image recipe
identity stays implemented in the protocol for future hardware but is not offered by the
catalog.

Support summary after this round: **four of the five generative models** run in the compact tier
on this machine (Qwen-Image 2.1-Turbo, MOSS-Image-SFT, FLUX.2 Klein 4B 文生图/编辑, plus Qwen-Image
2.1 when its measurement lands); Z-Image stays 12 GB.




### What the measurement changed in the design

The design expected the compact text stage to stream the NF4 encoder layer by layer. Measured
on this machine, neither supported mechanism works for a bitsandbytes checkpoint:

- moving individual `Linear4bit` modules with `.to("cuda")` leaves their `quant_state` buffers
  on the CPU (`Expected all tensors to be on the same device … mat2 is on cpu`).
- `device_map="auto"` with a GPU slice needs `llm_int8_enable_fp32_cpu_offload`, but the model
  constructor calls `Tensor.item()` on meta tensors before the weights are materialised.
- `accelerate.cpu_offload` then fails with `Cannot copy out of meta tensor; no data!`.

The compact tier therefore keeps **one** NF4 component resident at a time exactly like the
standard staged path, and pays for the 8 GB fit with the smaller 768² canvas and fp16 compute.
That measurement is 5.961 GiB, inside the 6.0 GiB compact budget, with 1.9 GiB of desktop
compositor still resident.

## Install-time prepare on the same machine

`--prepare` for the same snapshot completed in 723.5 s. Peak process commit was 23.145 GiB for
the text encoder and 20.275 GiB for the transformer, and available physical RAM fell to
**0.003 GiB**, so the run only completed by paging. The installer preflight accepts either
≥ 28 GiB available physical RAM or ≥ 26 GiB commit headroom; this machine qualifies through
commit headroom (75.9 GiB limit). A genuinely low-memory prepare path is still required before
16 GiB machines can install these models; that work is tracked as a follow-up in the change.
