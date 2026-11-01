# Windows Gate 0 tools

These tools run the approved Windows feasibility experiment. They do not
activate a product installation.

The target runtime is prepared under `target/windows-gate0`; downloaded model
and runtime artifacts are not committed.

## Model transfer

ModelScope is the preferred transfer source:

```powershell
python tools/windows-gate0/download_model.py `
  --manifest resources/windows-foundation/z-image-diffusers/model-manifest.json `
  --output target/windows-gate0/model `
  --report verification/windows-gate0/model-download.json `
  --modelscope
```

The downloader resumes `.part` files with curl and verifies every final file
against the fixed upstream bytes and SHA-256. `verify_modelscope_source.py`
checks all 20 files against ModelScope API hashes or downloaded content.

## Generation

Candidate A loads transformer and text encoder as NF4, uses BF16 compute and
memory-efficient SDPA, keeps the VAE in BF16 with tiling, and enables component
CPU offload:

```powershell
$env:HF_HUB_OFFLINE = "1"
$env:TRANSFORMERS_OFFLINE = "1"
target/windows-gate0/runtime/python.exe -I `
  tools/windows-gate0/run_generation.py `
  --candidate a `
  --model-dir target/windows-gate0/model `
  --output-dir target/windows-gate0/runs/candidate-a `
  --report verification/windows-gate0/candidate-a.json `
  --prompt "一只橘猫坐在洒满午后阳光的木窗前，写实摄影" `
  --seed 20261101
```

`summarize_gate0.py` validates three successful independent reports and the
fixed runtime/model evidence before writing `gate0-results.json`.
