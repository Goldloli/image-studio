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

## Change Gate 0 execution (`achieve-windows-model-parity`, tasks.md 1.1)

Status: the owner confirmed the change on 2026-11-03 and implementation started.
Everything in this section belongs to the change itself and is separate from the
pre-change research above; each model keeps its own evidence and its own
readiness state. Tooling lives in `tools/windows-parity-gate0/`, model snapshots
in `target/parity-gate0/models/`, and the isolated BiRefNet environment in
`target/parity-gate0/py-aux/`.

### Fixed-source transfers

`tools/windows-parity-gate0/download.py` transfers the fixed snapshots declared
by `resources/planned-models/*/model-manifest.json`, resumes `.part` files with
HTTP Range, and verifies every file's byte count and SHA-256 against the
manifest. The manifests stay authoritative; a transfer provider only carries
those fixed bytes.

| Model | Declared source (manifest) | Actual transfer provider | Files | Bytes | Report |
|---|---|---|---|---|---|
| BiRefNet general epoch_244 | GitHub release `ZhengPeng7/BiRefNet` v1 asset 186740004 | fixed manifest URL (GitHub release) | 1 | 972,666,916 | [transfer-birefnet.json](transfer-birefnet.json) |
| Qwen-Image 2.1 base | ModelScope `Qwen/Qwen-Image-2.1` with fixed per-file revisions | fixed ModelScope per-file URLs | 25 | 33,131,609,424 | [transfer-qwen21-base.json](transfer-qwen21-base.json) |
| FLUX.2 Klein 4B distilled | Hugging Face `black-forest-labs/FLUX.2-klein-4B` @ `e7b7dc27f91deacad38e78976d1f2b499d76a294` | ModelScope mirror `black-forest-labs/FLUX.2-klein-4B` (listing revision `master`, per-file revision `a6e056113f18cb00eb3d9639b5c67fe60c32dd80`) | 18 | 15,980,131,745 | [transfer-klein.json](transfer-klein.json) |

Hugging Face endpoints are not reachable from this Windows network: the
recorded probe in [transfer-klein.json](transfer-klein.json) returned `curl`
exit 28 / HTTP 000 after a 10.0 s connect timeout for
`https://huggingface.co/black-forest-labs/FLUX.2-klein-4B/resolve/<revision>/model_index.json`.
`hf-mirror.com` was probed reachable (HTTP 200) and the ModelScope listing API
was reachable, but neither is trusted blindly. For Klein the ModelScope mirror of
the same repository was used only after proving every one of the 18 mirror files
has the same byte count and SHA-256 as the fixed Hugging Face manifest
(`transfer.mirror_verification.checks`; 18/18 match). Each transferred file keeps
the declared Hugging Face URL in `declared_url`, the actual mirror URL and the
per-file mirror revision next to it, so the install manifest can keep the
Hugging Face content identity while the transfer source is auditable. This is the
same "fixed manifest + ModelScope transfer + per-file hashes" strategy used for
Qwen/MOSS on macOS.

Every snapshot was also re-hashed independently with PowerShell
`Get-FileHash -Algorithm SHA256` (Windows CNG, a different implementation from
the downloader's Python `hashlib`):
[reverification-birefnet.json](reverification-birefnet.json),
[reverification-qwen21-base.json](reverification-qwen21-base.json),
[reverification-klein.json](reverification-klein.json). All files match.

### G2-B1 — BiRefNet ONNX Runtime CPU (passed)

Frozen `BiRefNet-general-epoch_244.onnx` (972,666,916 bytes,
`58f621f0...1ebd3c`) executed with `CPUExecutionProvider` only on the controlled
real photo `verification/planned-models-20261031/fixtures/original/portrait-hair.png`
(1122x1402 RGB, `63d07fbf...de8d6`). The frozen product worker wheel
`studio_onnx_worker-0.1.0` (sha256 `59d709c7...c66c3`) provides the exact
preprocessing/postprocessing code; because the wheel pins
`Requires-Python ==3.13.*`, its pure-Python module is extracted into the
isolated py-aux environment instead of pip-installed.

| Phase | Seconds |
|---|---|
| ONNX session load | 8.146 |
| Inference (`session.run`) | 13.329 |
| Postprocess (one sigmoid, align-corners resize, mask compose) | 0.072 |
| Total | 21.698 |

Resources: process-tree private commit peak 10.139 GiB (limit 24 GiB), system
available RAM minimum 15.522 GiB (limit 2 GiB), CUDA not used. Graph contract
confirmed at runtime: one float32 input `1x3x1024x1024` (`input_image`) and one
float32 output `1x1x1024x1024` (`output_image`); raw logits span
[-72.415, 20.498], i.e. outside `[0, 1]`, so exactly one sigmoid is required on
the final Conv logits. Outputs are
[birefnet-gate0-rgba.png](birefnet-gate0-rgba.png) (2,797,794 bytes,
`9d1800cf...79309`, 1122x1402 RGBA) and
[birefnet-gate0-mask.png](birefnet-gate0-mask.png) (35,198 bytes,
`4433c05c...22684e`, 1122x1402 8-bit). All parity checks pass: RGBA alpha equals
the 8-bit mask pixel-for-pixel, RGB equals the source RGB (straight alpha, no
premultiplication), and the mask equals `round(probability * source_alpha * 255)`.
A cold first run took 28.9 s total (inference 20.48 s); the committed record is a
warm rerun and both runs produced the identical RGBA/mask SHA-256.

Quality review: the subject and hair silhouette are preserved, the background is
fully transparent, and hair strands keep soft partial alpha (30,329 partial
pixels = 1.93 % of the image; 630,334 fully opaque and 912,381 fully transparent
pixels; 41.09 % coverage at alpha >= 128, 256 distinct alpha values, alpha
mean 104.8 / std 125.0). No blocky artifacts were observed at this review size.
Limits: one controlled photo, CPU-only feasibility, and this is the worker's
inference recipe rather than the full product worker/host path.

Machine-readable record: [birefnet-gate0.json](birefnet-gate0.json).

### G2-K1 — FLUX.2 Klein 4B distilled (passed with NF4 + component CPU offload)

Frozen snapshot `black-forest-labs/FLUX.2-klein-4B` @
`e7b7dc27...` (18 files, 15,980,131,745 bytes) executed with diffusers 0.41.0
`Flux2KleinPipeline` (components: `Qwen3ForCausalLM` text encoder,
`Flux2Transformer2DModel`, `AutoencoderKLFlux2`; `is_distilled: true`). Settings
follow the model card: 1024x1024, 4 steps, `guidance_scale=1.0`, fixed seeds
3148150817 (text-to-image) and 3148150818 (single-image instruction edit, source
`portrait-hair.png`, output 912x1136).

Chosen strategy — **`nf4-model-offload` in a fresh process** (bitsandbytes NF4 on
transformer and text encoder, compute dtype bf16, double quantization, plus
`enable_model_cpu_offload`):

| Measurement | Value | Budget |
|---|---|---|
| Load | 12.88 s | <= 600 s |
| Text-to-image | 7.16 s | <= 900 s |
| Single-image edit | 10.36 s | <= 900 s |
| CUDA reserved peak | 9.207 GiB (allocated 7.292) | <= 10.5 GiB |
| Process-tree private commit peak | 19.132 GiB | <= 24 GiB |
| System available RAM minimum | 14.931 GiB | >= 2 GiB |
| `nvidia-smi` used peak | 10,481 MiB | no WDDM fallback observed |

Outputs (real generations, 4 steps):
[klein-gate0-txt2img-fresh-process-nf4.png](klein-gate0-txt2img-fresh-process-nf4.png)
(2,074,705 bytes, `2802c200...0ff404`, 1024x1024) and
[klein-gate0-edit-fresh-process-nf4.png](klein-gate0-edit-fresh-process-nf4.png)
(1,809,872 bytes, `c65f9616...6acb61`, 912x1136). The edit keeps the person's
face and hairstyle and replaces the background with a rainy Shanghai night
street; both images are good enough for the human quality review sample set.
Three independent fresh processes produced byte-identical output SHA-256, so the
fixed seed path is deterministic.

Machine-readable record:
[klein-gate0-fresh-process-nf4.json](klein-gate0-fresh-process-nf4.json).

Failure evidence kept for tasks.md 1.4 (do not read these as the chosen
strategy):

- **bf16 + `enable_model_cpu_offload` is not acceptable.** Loading both official
  bf16 components (7.2 + 7.5 GiB) plus load transients peaked at 31.745 GiB
  process-tree private commit (limit 24 GiB); CUDA reserved itself stayed inside
  budget (9.719 GiB). The run still produced real bf16 images before failing the
  budget, kept as
  [klein-gate0-txt2img-bf16-model-offload.png](klein-gate0-txt2img-bf16-model-offload.png)
  and
  [klein-gate0-edit-bf16-model-offload.png](klein-gate0-edit-bf16-model-offload.png).
- **Never stack two strategies in one process.** [klein-gate0.json](klein-gate0.json)
  is explicitly labelled `combined-process-failure`: it ran bf16 and then NF4 in
  one Python process, so the second strategy's resources are inflated (commit
  39.107 GiB, available RAM 0.190 GiB) by the already-resident pipeline and
  allocator. The worker must load exactly one strategy per fresh process.
- No Klein staged/prepared-cache experiment was needed: with the fresh-process
  NF4 strategy every Gate 0 budget passes, so the owner's design note records
  Klein as `quantization="nf4"` + `load_strategy="nf4-component-offload-v1"`,
  while Qwen/MOSS keep the staged + prepared-cache executor.

Decision: **Klein Gate 0 passes** with NF4 component offload in a fresh process;
bf16 needs more system memory than this 32 GiB machine can safely provide, and no
hardware threshold is needed for Klein.

### Experiment environment and limits

- BiRefNet ran in an isolated py-aux tree built with
  `python -m pip install --target` from the frozen Python 3.12.10 interpreter
  (that embeddable distribution has no `venv`/`ensurepip` and its `python312._pth`
  ignores `PYTHONPATH`). A before/after inventory of
  `target/parity-gate0/py312/Lib/site-packages` (35,286 files, 4,714,170,451
  bytes) is identical, proving the Lead's experiment environment was not
  modified. Versions: onnxruntime 1.31.1 (CPU), numpy 2.5.4, pillow 13.0.0,
  psutil 7.2.2.
- Klein ran in the frozen py312 experiment interpreter (torch 2.10.0+cu126,
  diffusers 0.41.0, transformers 5.3.0, bitsandbytes 0.50.2) with
  `PYTHONDONTWRITEBYTECODE=1`; the same py312 inventory stayed identical.
- Klein numbers are Gate 0 feasibility on an exclusive GPU window, not the
  product worker path; the product worker, cancellation/leases and the staged
  Qwen/MOSS executor are separate implementation tasks.
- The `nvidia-smi` used value includes desktop WDDM graphics clients (~0.8 GiB
  baseline here), so it is reported alongside `torch.cuda` reserved instead of
  used as the single VRAM decision value.
- All gates were measured with fixed seeds; images are real model outputs, no
  shell pass or synthesized evidence.

