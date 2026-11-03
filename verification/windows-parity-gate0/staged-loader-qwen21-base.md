# Staged quantized Gate 0: Qwen-Image 2.1 base (Windows 12 GB)

Status: **Gate 0 G2-Q1 passed on 2026-11-03.** Real 1024x1024 output at the base
model's 40-step schedule inside every resource budget on the target machine.

Same executor and prepared-cache approach as
[staged-loader-qwen21-turbo.md](staged-loader-qwen21-turbo.md). The base snapshot
has no `sample_sigmas` in `model_index.json`, so the run uses the pipeline's
40-step linear sigma grid (the product recipe for Qwen-Image 2.1).

## Results (2026-11-03)

Machine: Windows 10 22H2, RTX 3080 Ti 12288 MiB, driver 596.49, i7-13700K,
31.73 GiB RAM. Versions: Python 3.12.10, torch 2.10.0+cu126, diffusers 0.41.0,
transformers 5.3.0, bitsandbytes 0.50.2. Parameters: prompt
`雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光`, seed 3148150817,
1024x1024, 40 steps, guidance 1.0, NF4 text encoder + NF4 transformer.

| Stage | Load | Execute | CUDA reserved peak | Private commit peak | Available RAM min |
|---|---|---|---|---|---|
| text (NF4) | 11.58 s | 0.87 s | 6.002 GiB | 13.955 GiB | 16.082 GiB |
| dit (NF4) | 6.59 s | 53.08 s (40 steps) | 4.574 GiB | 10.204 GiB | 17.928 GiB |
| vae (bf16, tiled) | 0.85 s | 1.55 s | 1.180 GiB | 5.377 GiB | 20.143 GiB |
| **total** | | **~78 s** | **6.002 GiB** | **13.955 GiB** | **16.082 GiB** |

Budget: CUDA reserved <= 10.5 GiB, private commit <= 24 GiB, available RAM
>= 2 GiB, load <= 600 s, generation <= 900 s. **All pass.**

Output: [staged-qwen21-base.png](staged-qwen21-base.png), 1024x1024,
2,169,666 bytes, SHA-256
`831478553551632b7652fbc49b02d116860285f4c38449a6fc4aa780f11719d4`. The image is
a realistic rainy street scene matching the prompt; small signage lettering is
garbled, which the quality review (task 4.4) will judge against the base model's
expected behavior.

Machine-readable records:
[staged-qwen21-base-run.json](staged-qwen21-base-run.json) and
[staged-qwen21-base-prepare.json](staged-qwen21-base-prepare.json) (one-time
quantization: 10.05 GB prepared cache, 23.38 GiB commit / 0.62 GiB available RAM
transient - install-time only).

## Reproduction

```powershell
$env:HF_HUB_OFFLINE="1"; $env:TRANSFORMERS_OFFLINE="1"
target\parity-gate0\py312\python.exe tools\windows-parity-staged\run_qwen21_staged.py `
  --mode prepare --model-dir target\parity-gate0\models\qwen-image-2.1 `
  --prepared-dir target\parity-gate0\staged\prepared-qwen21-base `
  --report target\parity-gate0\staged\qwen21-base-prepare.json
target\parity-gate0\py312\python.exe tools\windows-parity-staged\run_qwen21_staged.py `
  --mode run --model-dir target\parity-gate0\models\qwen-image-2.1 --steps 40 `
  --prepared-dir target\parity-gate0\staged\prepared-qwen21-base `
  --output target\parity-gate0\staged\qwen21-base-staged.png `
  --report target\parity-gate0\staged\qwen21-base-staged.json
```

## Limits

- Experiment code; the product worker must re-verify through the generation v2
  protocol, native UI, cancel/kill recovery and export.
- The base and Turbo snapshots share component layout but are separate install
  identities; the prepared caches are per snapshot (a component may be reused
  only after digest comparison, per the installation spec).
- Qwen-Image 2.1 is Qwen Research License (non-commercial); the model page and
  records must show that restriction (task 3.3/4.1).
