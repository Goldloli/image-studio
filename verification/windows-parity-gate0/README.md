# Windows model-parity Gate 0 feasibility (pre-change research)

Status: measured on 2026-11-03 on the Windows 10 target machine (RTX 3080 Ti
12 GB, i7-13700K, 31.7 GiB RAM). Both planned large models generated real
1024x1024 images, but both exceeded the first-round Gate 0 memory budgets. This
record is **pre-change research for the planned Windows model-parity change**,
not product acceptance.

## Method

- Experiment-only Python 3.12.10 environment copied from the product runtime,
  with `diffusers 0.41.0`, `transformers 5.3.0`, `torch 2.10.0+cu126`,
  `torchvision 0.25.0+cu126`, `bitsandbytes 0.50.2`. The product runtime
  (`diffusers 0.36.0` / `transformers 4.57.3`) cannot load either model:
  Qwen-Image-2.1 needs diffusers >= 0.41 and MOSS needs transformers >= 5.3 for
  its Qwen3.5 text encoder.
- Weights from ModelScope: `Qwen/Qwen-Image-2.1-Turbo` (30.22 GiB) and
  `openmoss/MOSS-Image-SFT` (25.35 GiB), revision `master`, downloaded on
  2026-11-03.
- Runners (experiment-only, not committed): `target/parity-gate0/run_qwen21_gate0.py`
  and `run_moss_gate0.py`. Resource sampling every 100-250 ms: process-tree
  private commit, system available RAM, `torch.cuda.memory_reserved/allocated`
  and `nvidia-smi memory.used`.
- Settings: prompt `雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光`,
  seed `3148150817`, 1024x1024, 8 denoising steps.
  Qwen uses NF4 on the transformer and text encoder with
  `enable_model_cpu_offload`. MOSS uses bf16 with
  `enable_model_cpu_offload`.

## Results

| Model | Load | Generation | Image | CUDA reserved peak | Process commit peak | Available RAM min | nvidia-smi peak | Result |
|---|---|---|---|---|---|---|---|---|
| Qwen-Image-2.1-Turbo (NF4) | 56.36 s | 19.15 s | [2,059,380 B](qwen-image-2.1-turbo-nf4-model-offload.png) | 15.459 GiB | 40.633 GiB | 0.152 GiB | 11,963 MiB | budget failed |
| MOSS-Image-SFT (bf16) | 2.32 s | 68.45 s | [1,789,049 B](moss-image-sft-bf16-model-offload.png) | 15.729 GiB | 50.193 GiB | 0.059 GiB | 11,790 MiB | budget failed |

Machine-readable records: [Qwen](qwen-image-2.1-turbo-nf4-model-offload.json),
[MOSS](moss-image-sft-bf16-model-offload.json). Image SHA-256:
`46d7391302295a475fcaf6de1bcb8c1ebad83fd5cf0190b1a71dab0c1a718740` (Qwen),
`a5dcc293c0f91258b1c0270d69703238a4b3d4ef7027e0274dd4c85e1123c11a` (MOSS).

Both outputs are real 1024x1024 photographs matching the prompt. CUDA reserved
peaks above the 12 GiB physical card and near-zero available RAM show WDDM
shared-memory fallback plus pagefile-backed oversubscription while the
model-offload hooks move components between CPU and GPU.

## Interpretation

- **Functional feasibility: yes.** Both models run on this machine and produce
  acceptable images; Qwen Turbo is fast (19 s for its 8-step schedule).
- **Budget feasibility: no, under the naive path.** The official bf16 models
  plus component-level CPU offload need 16+ GiB VRAM or ~64 GiB system RAM to
  avoid shared-memory/pagefile oversubscription. A Qwen sequential-offload
  attempt failed before generation on an accelerate/bitsandbytes `Params4bit`
  incompatibility in this environment; it is recorded as not-run, not as a
  model failure.
- **Product consequence.** The parity change must either implement a staged,
  quantized loader (the documented ComfyUI INT8/W4A8 flow: encode prompt,
  release the text encoder, then load the DiT, then a tiled VAE decode), or
  explicitly document a higher hardware requirement on the model page, per the
  owner's decision. These measurements are the baseline for that design choice.

## Researched alternatives for the change

- Comfy-Org quantized sets mirrored on ModelScope: Qwen
  `int8_convrot` 7.257 GB + `qwen3vl_8b_w4a8` 6.312 GB + VAE 0.676 GB = 14.24 GB;
  MOSS `moss_image_sft_int8_convrot` 5.414 GB + `qwen3.5_9b_w4a8_gptq` 7.344 GB
  + VAE 0.250 GB = 13.01 GB. Both are ComfyUI single-file formats and need a
  custom loader, not stock diffusers.
- GGUF Q4 alternatives: Qwen Turbo 11.19 GB, MOSS 10.79 GB including the same
  w4a8 text encoders.
- Community 12 GB evidence exists through ComfyUI staged loading; no measured
  RTX 3080 Ti run is published for either model.
- Licenses: Qwen-Image-2.1 is Qwen Research License (non-commercial
  research/evaluation only); MOSS-Image-SFT is Apache 2.0. The owner decided to
  include Qwen in this project despite the non-commercial licence.

## Limits

- 8 steps is Qwen Turbo's official schedule but not MOSS's 50-step/CFG-3
  product recipe; this run measures memory and speed, not final recipe quality.
- No WDDM dedicated/shared split was captured at 100 ms resolution; CUDA
  reserved and `nvidia-smi` used are reported separately.
- The experiment environment is not the product runtime; the change must define
  and freeze a second Windows runtime stack.
