# Windows Gate 0

Status: passed on the target Windows 10 machine, 2026-11-01. Product
installation and native application generation are not activated by this
evidence.

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
