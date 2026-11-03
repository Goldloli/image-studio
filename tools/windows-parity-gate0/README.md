# Windows model-parity Gate 0 tooling (experiment only)

Experiment-only tooling for OpenSpec change `achieve-windows-model-parity`
(`openspec/changes/achieve-windows-model-parity`, tasks.md 1.1) and bd issue
`image-studio-xwh.1`. Nothing in this directory ships in the product.

## Files

- `download.py` — resumable, hash-verified transfer of the fixed model
  snapshots declared in `resources/planned-models/*/model-manifest.json`.
  It writes `.part` files, resumes with HTTP Range through `curl --continue-at`,
  verifies every file's byte count and SHA-256 against the manifest, and emits a
  machine-readable transfer report (source URL, source revision, timing, speed,
  disk headroom, manifest SHA-256).
- `run_birefnet_gate0.py` — Gate 0 G2-B1: runs the frozen BiRefNet
  `general-epoch_244` ONNX graph with `CPUExecutionProvider` on a controlled
  real photo, using the product worker's frozen preprocessing/postprocessing
  code, and records the graph contract, timings, process/system resources,
  RGBA+mask parity and quality statistics.
- `run_klein_gate0.py` — Gate 0 G2-K1: runs the frozen FLUX.2 Klein 4B
  distilled snapshot with `Flux2KleinPipeline` (text-to-image and single-image
  instruction edit), records load/generation timings, CUDA reserved/allocated,
  process-tree private commit, available RAM and `nvidia-smi`, and applies the
  change budget thresholds. GPU runs are only started after the Lead grants an
  exclusive GPU window.

## Fixed sources

The manifests under `resources/planned-models/` stay authoritative; no floating
resolution or runtime downloads are accepted.

| Model | Manifest | Declared source |
|---|---|---|
| FLUX.2 Klein 4B distilled | `resources/planned-models/klein/model-manifest.json` | Hugging Face `black-forest-labs/FLUX.2-klein-4B` @ `e7b7dc27f91deacad38e78976d1f2b499d76a294` |
| BiRefNet general epoch_244 | `resources/planned-models/birefnet/model-manifest.json` | GitHub release `ZhengPeng7/BiRefNet` `v1` asset `186740004` |
| Qwen-Image 2.1 base | `resources/planned-models/qwen21/model-manifest.json` | ModelScope `Qwen/Qwen-Image-2.1` with fixed per-file revisions |

ModelScope is preferred for transfer. The Hugging Face endpoints are not
reachable from this Windows network, so the Klein snapshot is transferred from
the ModelScope mirror repository `black-forest-labs/FLUX.2-klein-4B` only after
`download.py` proves every mirror file's byte count and SHA-256 equal the fixed
manifest values. The report keeps the declared Hugging Face URLs side by side
with the actual transfer URLs and per-file mirror revisions.

## Isolated BiRefNet environment (`target/parity-gate0/py-aux`)

`target/parity-gate0/py312` is the Lead's experiment interpreter and must not be
modified. It is an embeddable CPython 3.12.10 distribution: it has no
`venv`/`ensurepip` module and its `python312._pth` makes CPython ignore
`PYTHONPATH`, so a normal `python -m venv` is impossible.

Isolation is therefore implemented as a dedicated pip target tree:

```powershell
target\parity-gate0\py312\python.exe -m pip install --target `
  target\parity-gate0\py-aux\Lib\site-packages `
  --report target\parity-gate0\results\py-aux-install-report.json `
  onnxruntime==1.31.1 numpy==2.5.4 pillow==13.0.0 psutil
```

The frozen product worker wheel
`resources/planned-models/birefnet/studio_onnx_worker-0.1.0-py3-none-any.whl`
pins `Requires-Python ==3.13.*`, so it is extracted (not pip-installed) into the
same target tree; its pure-Python preprocessing/postprocessing code is imported
verbatim. Scripts run through `target/parity-gate0/py-aux/run-isolated.py`,
which prepends the target site-packages and disables bytecode writing. A
before/after file inventory of `py312/Lib/site-packages` proves the base
interpreter stayed byte-identical.

```powershell
target\parity-gate0\py312\python.exe target\parity-gate0\py-aux\run-isolated.py `
  tools\windows-parity-gate0\run_birefnet_gate0.py
```

## GPU coordination

Only one GPU job may run at a time. Klein Gate 0 runs (`run_klein_gate0.py`)
start only after the Lead explicitly grants a window; the runner is invoked with
the frozen `target/parity-gate0/py312` interpreter (which already carries
torch 2.10.0+cu126, diffusers 0.41.0, transformers 5.3.0, bitsandbytes 0.50.2)
and the GPU must be confirmed idle with `nvidia-smi` first. The Lead is notified
immediately when the run releases the GPU.
