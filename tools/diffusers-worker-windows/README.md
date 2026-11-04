# Windows diffusers worker (Klein / Qwen-Image 2.1 / MOSS)

Product worker for the OpenSpec change `achieve-windows-model-parity`
(tasks.md 2.1/2.2/2.5, bd issue `image-studio-xwh.2`). It speaks the generation
v2 JSONL protocol of `crates/worker-protocol` / `tools/zimage-cuda-worker` for
four Windows model snapshots and runs their measured execution strategies.

## Identity

Build identity `studio-diffusers-worker-windows-0.1.0`, runtime lock
`windows-python31210-torch2100-cu126-diff041-transformers53-v2`. One worker
process serves exactly one snapshot; the host passes `--model-snapshot-id`
(installer profile `worker_model_argument = true`, supervisor worker args).

| Snapshot | Model variant | Capabilities | Recipe | Steps |
|---|---|---|---|---|
| `flux2-klein-4b-windows-e7b7dc27f91deacad38e78976d1f2b499d76a294` | `flux2-klein-4b-distilled-windows-v1` | `image.text-to-image.flux2-klein-4b.windows-v1`, `image.edit.flux2-klein-4b.windows-v1` | `klein4b-text-windows-v1`, `klein4b-edit-windows-v1` | 4 |
| `qwen-image-2.1-windows-staged-d26bb61231c349cf6b7896fa83353113880e1ba3` | `qwen-image-2.1-windows-v1` | `image.text-to-image.qwen-image-2.1.windows-v1` | `qwen21-windows-txt2img-nf4-40-v1` | 40 |
| `qwen-image-2.1-turbo-windows-staged-fc13ba1986fcb905854dc8de2f52eee395f49b5b` | `qwen-image-2.1-turbo-windows-v1` | `image.text-to-image.qwen-image-2.1-turbo.windows-v1` | `qwen21-turbo-windows-txt2img-nf4-8-v1` | 8 |
| `moss-image-sft-windows-staged-f972bd4b4b9e3852133cc5da954d87bc32cd8fb9` | `moss-image-sft-windows-v1` | `image.text-to-image.moss-image-sft.windows-v1` | `moss-image-sft-windows-txt2img-nf4-50-v1` | 50 |

`studio_diffusers_worker.worker_identity(snapshot_id)` and
`model_spec(snapshot_id)` expose the frozen values; the effective parameter map
and output specification are validated exactly like the Z-Image worker, so a
macOS identity, an unknown snapshot or a changed recipe parameter is rejected
before any torch import.

## CLI contract

```text
python -I -m studio_diffusers_worker --model-snapshot-id <snapshot>              # v2 JSONL run
python -I -m studio_diffusers_worker --model-snapshot-id <snapshot> --probe [MODEL_DIR]
python -I -m studio_diffusers_worker --model-snapshot-id <snapshot> --prepare \
    --model-dir SNAPSHOT_DIR --prepared-dir PREPARED_DIR [--no-reuse-prepared]
```

`--probe` prints one JSON object: `python`, `identity`, `device` (`gpu`),
`device_name`, `compute_capability`, package versions. The installer asserts
`python`, `identity.build_id`, `identity.runtime_lock_id`,
`identity.model_snapshot_id` and `device` (see
`apps/desktop/src/adapters/installation.rs` probe expectations).
`--prepare` prints one JSON object with the prepared component inventory
(path/bytes/SHA-256) for installer reuse.

The v2 request schema is frozen (no extra fields), so the staged Qwen/MOSS
prepared cache is derived from the installed snapshot path:
`models/<snapshot-directory>` -> `models/<snapshot-directory>.prepared`.
The installer `--prepare` step must use that same directory. Klein needs no
prepared cache.

## Execution strategies (from Gate 0)

- **Klein** (`backends/klein.py`): `Flux2KleinPipeline` with bitsandbytes NF4 on
  transformer and text encoder plus `enable_model_cpu_offload`, in this dedicated
  worker process. Text-to-image and single-image instruction edit (4 steps,
  guidance 1.0, 1024x1024, fixed seed); progress steps total 4. Measured:
  load 12.88 s, t2i 7.16 s, edit 10.36 s, CUDA reserved 9.207 GiB, private commit
  19.132 GiB, available RAM 14.931 GiB. Never stack a second strategy in the same
  process; the combined-process failure record is kept in
  `verification/windows-parity-gate0/klein-gate0.json`.
- **Qwen base/Turbo** (`backends/qwen.py`): staged executor over the prepared NF4
  cache - text encode -> release -> DiT sample (40/8 steps, model `sample_sigmas`
  schedule, KV cache) -> release -> tiled VAE decode -> release. No
  `enable_model_cpu_offload`. Each stage checks load time, cleanup (CUDA reserved
  after release <= 1 GiB) and available RAM; token budget 2048 is enforced on the
  rendered prompt.
- **MOSS** (`backends/moss.py`): staged NF4 text encoder -> release -> NF4 DiT
  with 50 steps, CFG 3.0, empty negative prompt and negative-branch skipping of
  block 22 -> release -> fp32 FLUX.2 VAE decode. The reviewed upstream
  `pipeline_moss_image.py` is vendored at its frozen 25,060 bytes / SHA-256
  `1dac1aca...` and hash-checked before import; Python is never executed from the
  installed model directory. `vendored/moss_bnb_shim.py` supplies the documented
  bf16 compute dtype compatibility patch for bitsandbytes weights.

All backends enforce the design budget hard: load <= 600 s, generation <= 900 s,
CUDA reserved <= 10.5 GiB, process private commit <= 24 GiB, available RAM
>= 2 GiB. A breach raises an `inference_error` instead of oversubscribing the
machine. Offline environment variables are forced; no network fallback exists.

## Build, tests, evidence

```powershell
python tools\diffusers-worker-windows\build_wheel.py
# -> resources/windows-parity/workers/studio_diffusers_worker-0.1.0-py3-none-any.whl

# unit tests (any Python >= 3.12 with pytest; the worker itself needs 3.12)
python -m pytest tools\diffusers-worker-windows\tests

# real protocol evidence (frozen experiment interpreter)
target\parity-gate0\py312\python.exe tools\diffusers-worker-windows\tests\run_real_protocol.py `
  --mode run --python target\parity-gate0\py312\python.exe `
  --worker-site target\parity-worker\worker-site `
  --snapshot <snapshot> --model <model-dir> --report <evidence.json> ...
```

Real exchange evidence and the wheel digest are recorded in
`verification/windows-parity-worker/README.md`.
