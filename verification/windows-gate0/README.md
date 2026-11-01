# Windows Gate 0

Status: Gate 0 passed on the target Windows 10 machine, 2026-11-01.
Product installation and controller generation were subsequently validated as
separate evidence linked below; those results do not change the original Gate
0 measurements.

## Fixed runtime

- Python 3.12.10 Windows embeddable x64
- PyTorch 2.10.0+cu126
- Diffusers 0.36.0
- Transformers 4.57.3
- Accelerate 1.12.0
- bitsandbytes 0.48.2
- RTX 3080 Ti, UUID `GPU-78b715aa-ec51-31af-8813-eac9653458a7`,
  PCI bus `00000000:01:00.0`, compute capability `8.6`, 12288 MiB

## Selected candidate

Candidate A:

- transformer and text encoder: bitsandbytes NF4, BF16 compute, double
  quantization
- VAE: BF16 with tiling
- Diffusers model CPU offload
- PyTorch native memory-efficient SDPA
- FlowMatchEulerDiscreteScheduler
- 1024x1024, one image, 9 steps, guidance 0.0, 512 token limit

Candidate B was loaded successfully but was not needed after candidate A
passed.

## Three independent processes

| Run | Load | Execute | CUDA reserved peak (GiB) | Private commit peak (GiB) | Minimum available RAM (GiB) | Image |
|---|---:|---:|---:|---:|---:|---|
| 1 | 24.36 s | 14.33 s | 8.87 | 18.95 | 8.77 | [cat](candidate-a-run1-cat.png) |
| 2 | 24.38 s | 14.45 s | 8.87 | 23.69 | 9.16 | [teapot](candidate-a-run2-teapot.png) |
| 3 | 23.89 s | 14.45 s | 8.87 | 18.93 | 10.63 | [desk](candidate-a-run3-desk.png) |

All runs are below 10.5 GiB CUDA reserved, 24 GiB private commit, 600 seconds
load, and 900 seconds execution. Every run produced a decodable 1024x1024 RGB
PNG with 9 step callbacks.

## Model content

The 20 downloaded files, including 19 inference components, total
`32,848,305,533` bytes and match the fixed revision
`f332072aa78be7aecdf3ee76d5c247082da564a6`.

ModelScope `Tongyi-MAI/Z-Image-Turbo@master` was independently checked against
all 20 entries. The seven large safetensors files match ModelScope's API
SHA-256 values; every remaining file was downloaded from ModelScope and hashed
locally. See `modelscope-equivalence.json`.

## Retained failure

The initial default-SDPA/float32-VAE run generated a valid image but used
`12.55 GB` (`11.69 GiB`) CUDA reserved, above the 10.5 GiB budget. It remains recorded in
`candidate-a-default-attention-failed.json`. The fixed configuration uses
memory-efficient SDPA and BF16 VAE.

## Measurement limitations

Process private commit and available RAM were sampled at 50 ms. CUDA reserved
was sampled at stage and callback boundaries. `nvidia-smi` did not expose a
per-process compute-apps value under this WDDM setup; global NVML memory and
Torch allocator values are retained. WDDM shared-memory peak is unavailable
from the selected tooling.

## Product closure evidence

The product installer acceptance used the validated Gate 0 cache as immutable
input and then exercised the formal installer instead of the Gate 0 helper:

- all 20 ModelScope-verified model files and the private Python/wheel closure
  were verified again;
- the installed worker reported the fixed runtime, model snapshot, worker
  build, RTX 3080 Ti, sm86 and required package versions;
- all 19 model components were frozen into the Windows generation plan;
- the direct supervisor protocol and `GenerationController` both produced a
  valid 1024x1024 RGB PNG and matching SHA-256 candidate/asset identity.
- loading-time and sampling-time cancellation both ended with zero outputs;
- force-killing the real supervisor ended the job as failed with zero outputs,
  and a new job in the same installed runtime then generated a committed Asset.

Reports:

- [Product install](product-install.json)
- [Installed worker protocol](product-install-worker-protocol.json)
- [Controller generation](product-controller-generation.json)
- [Controller asset](product-controller-primary.png)
- [Cancellation and supervisor-kill recovery](product-cancel-recovery.json)
- [Post-recovery asset](product-recovery.png)

This is a cache-seeded installation acceptance, not a measure of empty-cache
download time. Network denial, clean-user installation, complete IME/DPI
interaction and Mac regression remain separate pending gates.
