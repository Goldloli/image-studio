# Windows parity staged-loader experiments

Experiment-only scripts used to decide the Windows execution strategy for the
OpenSpec change `achieve-windows-model-parity` (tasks 1.1 and 1.4). They are not
product code; the product staged executor is written separately against the same
measurements.

| Script | Purpose | Evidence |
|---|---|---|
| `run_qwen21_staged.py` | staged NF4 executor for Qwen-Image 2.1 / Turbo: `prepare` persists pre-quantized components, `run` executes text -> dit -> vae | [staged-loader-qwen21-turbo.md](../../verification/windows-parity-gate0/staged-loader-qwen21-turbo.md) |
| `run_moss_staged.py` | staged executor for MOSS-Image-SFT: NF4 text encoder, INT8/NF4 DiT, FLUX.2 VAE; 50 steps / CFG 3.0 / skip block 22 | `verification/windows-parity-gate0/staged-loader-moss-image-sft.md` |
| `probe_quant_load.py`, `probe_quant_load_cpu.py`, `probe_save_quantized.py`, `probe_transformer_quantized.py` | measure bitsandbytes load transients and the pre-quantized reload path | `target/parity-gate0/staged/quant-*.json` (local scratch, not committed) |

Usage (target machine, experiment environment under `target/parity-gate0/py312`):

```powershell
$env:HF_HUB_OFFLINE="1"; $env:TRANSFORMERS_OFFLINE="1"
# one-time CPU quantization of a fixed snapshot
target\parity-gate0\py312\python.exe tools\windows-parity-staged\run_qwen21_staged.py `
  --mode prepare --model-dir target\parity-gate0\models\qwen-image-2.1-turbo `
  --prepared-dir target\parity-gate0\staged\prepared-qwen21-turbo `
  --report target\parity-gate0\staged\qwen21-turbo-prepare.json
# staged generation against the prepared cache
target\parity-gate0\py312\python.exe tools\windows-parity-staged\run_qwen21_staged.py `
  --mode run --model-dir target\parity-gate0\models\qwen-image-2.1-turbo `
  --prepared-dir target\parity-gate0\staged\prepared-qwen21-turbo
```

Notes:

- The product Python is the Windows embeddable distribution with a `._pth`
  file, so scripts add their own directory to `sys.path` before importing each
  other.
- Prepared caches are derived locally from the fixed upstream snapshot. They are
  never committed and never replace the installation manifest; a rebuild from
  the same files is expected to reproduce the same bytes.
- Every run samples process-tree private commit, system available RAM,
  `torch.cuda.memory_reserved/allocated` and `nvidia-smi memory.used`; a stage
  that exceeds the design budget is recorded as `budget_failed`, not hidden.
