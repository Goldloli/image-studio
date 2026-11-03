# Staged quantized Gate 0: Qwen-Image 2.1-Turbo (Windows 12 GB)

Status: **Gate 0 G2-Q2 passed on 2026-11-03.** Real 1024x1024 output inside every
resource budget on the target machine, using an experiment-only staged loader.
Qwen-Image 2.1 base (G2-Q1) and MOSS (G2-M1) are tracked separately.

This record extends the pre-change research in [README.md](README.md), which
measured the naive `enable_model_cpu_offload` path at 15.46 GiB CUDA reserved /
40.63 GiB process commit / 0.15 GiB available RAM (budget failed). The staged
executor below removes the oversubscription.

## Why the naive path failed

The official diffusers pipeline keeps the whole model group resident or moves
bf16 components through CPU RAM. On a 12 GiB card with 32 GiB RAM that means:

- `enable_model_cpu_offload` keeps bf16 weights in RAM (text encoder 16.33 GB,
  transformer 13.25 GB) and CUDA reserved grows past the card.
- bitsandbytes on-the-fly quantization needs the bf16 shard on the target device
  before replacing it with 4-bit weights. Quantizing directly onto the card
  measured a **15.4 GiB reserved transient** for the text encoder alone; that is
  WDDM shared-memory territory.
- component-level offload does not free CPU memory between phases; process
  commit reached 40.6 GiB and available RAM 0.15 GiB.

## Staged executor under test

`tools/windows-parity-staged/run_qwen21_staged.py` (experiment-only, not
product code) executes the same model/schedule in explicit stages:

1. **text** - Qwen3-VL text encoder (NF4) encodes the prompt, then is released
   (`del` + `gc.collect()` + `torch.cuda.empty_cache()`).
2. **dit** - `QwenImage21Transformer2DModel` (NF4) runs the model's own
   `sample_sigmas` schedule (8 steps for Turbo) with the causal KV cache, then is
   released.
3. **vae** - `AutoencoderKLQwenImage21` (bf16) decodes the packed latents with
   tiling, then is released.

Two loading techniques were measured:

- **CPU-first NF4**: quantize on CPU (`device_map="cpu"`), then move the
  quantized modules to the card. GPU peak drops from 15.4 GiB to 5.96 GiB, but
  the quantization transient stays in the *generation* process (25.3 GiB
  private-commit peak, over the 24 GiB budget).
- **prepared NF4 cache** (`--mode prepare`): the one-time CPU quantization is
  persisted with `save_pretrained` (text encoder 6.38 GB, transformer 3.67 GB),
  and generation reloads the pre-quantized checkpoints (0.7 s reload, 11.2 s
  host-to-device). This moves the transient to the install prepare step, where
  generation budgets do not apply, and passes every generation-time budget.

The prepared cache is a local derived copy of the fixed upstream snapshot, not a
new model source: the installation manifest still pins the original files and
their SHA-256, and the cache is rebuilt deterministically from them.

## Results (prepared-cache run, 2026-11-03)

Machine: Windows 10 22H2, RTX 3080 Ti 12288 MiB, driver 596.49, i7-13700K,
31.73 GiB RAM. Versions: Python 3.12.10, torch 2.10.0+cu126, diffusers 0.41.0,
transformers 5.3.0, bitsandbytes 0.50.2. Parameters: prompt
`雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光`, seed 3148150817,
1024x1024, 8 steps, guidance 1.0, NF4 text encoder + NF4 transformer.

| Stage | Load | Execute | CUDA reserved peak | Private commit peak | Available RAM min |
|---|---|---|---|---|---|
| text (NF4) | 12.11 s | 0.71 s | 6.002 GiB | 13.953 GiB | 17.323 GiB |
| dit (NF4) | 4.45 s | 10.66 s (8 steps) | 4.574 GiB | 10.125 GiB | 19.307 GiB |
| vae (bf16, tiled) | 0.92 s | 1.54 s | 1.180 GiB | 4.628 GiB | 21.997 GiB |
| **total** | | **~30 s** | **6.002 GiB** | **13.953 GiB** | **17.323 GiB** |

Budget: CUDA reserved <= 10.5 GiB, private commit <= 24 GiB, available RAM
>= 2 GiB, load <= 600 s, generation <= 900 s. **All pass.**

Output: [staged-qwen21-turbo.png](staged-qwen21-turbo.png), 1024x1024,
2,007,139 bytes, SHA-256
`cf071e9ce8d1bc7769fa55a5b90faab2bcca69aaf1921163e2aa77abfeb9cd73`. The image is
a realistic photograph matching the prompt (Shanghai street corner after rain,
pedestrian with an umbrella) and is deterministic across runs.

Machine-readable records:
[staged-qwen21-turbo-run.json](staged-qwen21-turbo-run.json) (generation) and
[staged-qwen21-turbo-prepare.json](staged-qwen21-turbo-prepare.json) (one-time
quantization; 10.05 GB prepared cache, 23.38 GiB commit / 0.58 GiB available RAM
transient - it runs at install time, outside the generation budgets).

## Reproduction

```powershell
$env:HF_HUB_OFFLINE="1"; $env:TRANSFORMERS_OFFLINE="1"
target\parity-gate0\py312\python.exe tools\windows-parity-staged\run_qwen21_staged.py `
  --mode prepare --model-dir target\parity-gate0\models\qwen-image-2.1-turbo `
  --prepared-dir target\parity-gate0\staged\prepared-qwen21-turbo `
  --report target\parity-gate0\staged\qwen21-turbo-prepare.json
target\parity-gate0\py312\python.exe tools\windows-parity-staged\run_qwen21_staged.py `
  --mode run --model-dir target\parity-gate0\models\qwen-image-2.1-turbo `
  --prepared-dir target\parity-gate0\staged\prepared-qwen21-turbo `
  --output target\parity-gate0\staged\qwen21-turbo-staged-prepared.png `
  --report target\parity-gate0\staged\qwen21-turbo-staged-prepared.json
```

## Limits

- Experiment code, not the product worker. The product staged executor must be
  re-verified through the generation v2 protocol and the native UI before this
  model can be called delivered.
- The staged path is not bit-identical to the official pipeline image, because
  component placement changes rounding (the official run's SHA was
  `46d73913...718740`, this one `cf071e9c...bcd73`). Quality is the acceptance
  criterion, not the old hash.
- The prepare transient (23.4 GiB commit, 0.58 GiB available RAM) is a real
  install-time constraint on the 32 GiB machine and must get a RAM preflight and
  actionable failure in the product.
- This record covers Qwen-Image 2.1-Turbo only. Base 2.1 (40 steps) and
  MOSS-Image-SFT (50 steps, CFG 3.0, skip block 22) need their own runs.
