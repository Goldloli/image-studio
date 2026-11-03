# Staged quantized Gate 0: MOSS-Image-SFT (Windows 12 GB)

Status: **Gate 0 G2-M1 passed on 2026-11-03.** Real 1024x1024 output inside every
resource budget on the target machine, at the product recipe (50 steps, CFG 3.0,
empty negative prompt, negative branch skipping block 22).

This record extends the pre-change research in [README.md](README.md), which
measured the naive `enable_model_cpu_offload` bf16 path at 15.73 GiB CUDA
reserved / 50.19 GiB process commit / 0.06 GiB available RAM (budget failed). It
also extends the earlier 8-step speed probe, which is not the product recipe.

## Staged executor under test

`tools/windows-parity-staged/run_moss_staged.py` (experiment-only) plus
`moss_bnb_shim.py`:

1. **text** - Qwen3.5 text encoder (NF4, prepared on CPU once) encodes the
   positive prompt and the empty negative prompt, then is released.
2. **dit** - the custom `MOSSImageTransformer` (NF4, prepared on CPU once) runs
   50 CFG steps with `skip_layers=(22,)` on the negative branch, then is
   released. Seeds/latents stay fp32 like the upstream pipeline; predictions are
   combined in fp32.
3. **vae** - the FLUX.2 VAE (fp32) decodes with the pipeline's batch-norm
   rescale and pixel shuffle, then is released.

Prepared cache: text encoder 5.85 GB, DiT 2.62 GB (8.47 GB total), rebuilt
deterministically from the fixed upstream snapshot at install "prepare" time.

## The INT8 detour (recorded, not hidden)

The first attempt quantized the DiT with bitsandbytes **INT8** (5.09 GB prepared
cache). Loading worked, but the forward failed with
`NotImplementedError: "silu_cuda" not implemented for 'Char'`: the upstream
custom pipeline derives its compute dtype from `<module>.weight.dtype`, and
bitsandbytes reports the *storage* dtype there (`int8` for INT8, `uint8` for
NF4), so activations were cast to integers.

`moss_bnb_shim.py` replaces the three forwards that inspect `weight.dtype`
(`MOSSImageTransformer.forward`, `_Block.forward`, `_TextProjection.forward`)
with the same math using an explicit bf16 compute dtype. All other upstream
math is untouched. This shim is a recorded compatibility requirement for the
product worker (either this shim or a bf16 layer-offload path; the decision is
carried in the change design supplement).

## Results (NF4 DiT + shim, 2026-11-03)

Machine: Windows 10 22H2, RTX 3080 Ti 12288 MiB, driver 596.49, i7-13700K,
31.73 GiB RAM. Versions: Python 3.12.10, torch 2.10.0+cu126, diffusers 0.41.0,
transformers 5.3.0, bitsandbytes 0.50.2. Parameters: prompt
`雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光`, negative prompt empty,
seed 3148150817, 1024x1024, 50 steps, guidance 3.0, skip block 22.

| Stage | Load | Execute | CUDA reserved peak | Private commit peak | Available RAM min |
|---|---|---|---|---|---|
| text (NF4) | 12.07 s | 1.18 s | 5.574 GiB | 8.680 GiB | 21.640 GiB |
| dit (NF4 + shim) | 3.96 s | 102.58 s (50 steps, 2 forwards/step) | 3.162 GiB | 6.376 GiB | 21.473 GiB |
| vae (fp32) | 0.37 s | 0.52 s | 5.838 GiB | 9.195 GiB | 21.631 GiB |
| **total** | | **~120 s** | **5.838 GiB** | **9.195 GiB** | **21.473 GiB** |

Budget: CUDA reserved <= 10.5 GiB, private commit <= 24 GiB, available RAM
>= 2 GiB, load <= 600 s, generation <= 900 s. **All pass.**

Output: [staged-moss-image-sft.png](staged-moss-image-sft.png), 1024x1024,
1,842,307 bytes, SHA-256
`4cd42c8f351e4ff49e7b41ef3996cc5cdbb7c530f1d796ef9d533535b7d01b3a`. A realistic
rainy-street scene matching the prompt (pedestrian with umbrella, wet pavement,
reflections). Quality note: signboard/lettering is garbled, as is typical for
this model at 50 steps; the quality review belongs to task 4.4, not this Gate 0.

Machine-readable records:
[staged-moss-image-sft-run.json](staged-moss-image-sft-run.json) and
[staged-moss-image-sft-prepare.json](staged-moss-image-sft-prepare.json)
(one-time quantization: text encoder 22.1 s / 23.0 GiB commit, DiT NF4 14.1 s /
16.7 GiB commit, 0.83 GiB available-RAM minimum - an install-time constraint).

## Reproduction

```powershell
$env:HF_HUB_OFFLINE="1"; $env:TRANSFORMERS_OFFLINE="1"
target\parity-gate0\py312\python.exe tools\windows-parity-staged\run_moss_staged.py `
  --mode prepare --dit-quant nf4 --model-dir target\parity-gate0\models\moss-image-sft `
  --prepared-dir target\parity-gate0\staged\prepared-moss `
  --report target\parity-gate0\staged\moss-prepare-nf4.json
target\parity-gate0\py312\python.exe tools\windows-parity-staged\run_moss_staged.py `
  --mode run --model-dir target\parity-gate0\models\moss-image-sft `
  --prepared-dir target\parity-gate0\staged\prepared-moss `
  --output target\parity-gate0\staged\moss-staged.png `
  --report target\parity-gate0\staged\moss-staged.json
```

## Limits

- Experiment code, not the product worker; the product still needs the
  generation v2 protocol, native UI, cancel/kill recovery and export evidence.
- NF4 DiT + NF4 text encoder is a quantization trade-off for the 12 GB target.
  If the user wants higher fidelity, the documented alternatives are a bf16
  layer-offload path (slower, CPU-heavy) or a hardware threshold.
- The prepare transient (23.0 GiB commit, 0.83 GiB available RAM) is real and
  needs a RAM preflight in the installer.
- The dtype shim is a compatibility requirement against the fixed upstream
  pipeline revision; it must be reviewed and vendored as part of the worker.
