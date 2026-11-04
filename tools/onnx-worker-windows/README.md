# Windows BiRefNet ONNX Runtime CPU worker

Managed, offline ONNX Runtime **CPU** worker for BiRefNet `general-epoch_244` on
the Windows model-parity track (OpenSpec change
`achieve-windows-model-parity`, tasks 2.3/2.5). This is a Windows fork of the
portable macOS worker in [`../onnx-worker`](../onnx-worker): the protocol shell
is generation **v2** JSONL, and the inference backend is the same verbatim
numpy/Pillow/onnxruntime implementation (no MLX/CoreML, no network).

## Frozen identity (design.md section 11)

| field | value |
|---|---|
| build id | `studio-onnx-worker-windows-0.1.0` |
| runtime lock id | `windows-python31210-torch2100-cu126-diff041-transformers53-v2` |
| model id (model variant) | `birefnet-general-epoch-244-windows-v1` |
| model snapshot id | `birefnet-general-epoch-244-onnx-windows-v1` |
| capability | `image.remove-background.birefnet-general-epoch-244.windows-v1` |
| recipe | `birefnet-general-cpu-windows-v1` |
| probe device | `windows-cpu` (CPUExecutionProvider) |

Frozen effective parameters (exact map, `studio_core`/`worker-protocol` mirror
it): `recipe`, `recipe_version=1`, `provider="CPUExecutionProvider"`,
`precision="float32"`, `preprocessing="birefnet-1024-rgb-v1"`,
`postprocessing="birefnet-logits-sigmoid-v1"`, `intra_op_threads=4`,
`inter_op_threads=1`, `execution_mode="sequential"`.

Outputs are the macOS dual-output transaction: one `primary` RGBA PNG
(straight alpha, source-normalized dimensions) and one 8-bit `mask` PNG, each
bounded by 32 MiB; candidates are rejected on pre-existing paths. The mask is
exactly one sigmoid of the final Conv logits scaled by the source alpha, and the
primary alpha equals the mask byte for byte.

## Layout

```
studio_onnx_worker/__init__.py   identity, model spec, exact parameter map (stdlib only)
studio_onnx_worker/__main__.py   generation v2 JSONL shell, --probe, --model-snapshot-id
studio_onnx_worker/backend.py    portable BiRefNet CPU inference + dual-output transaction
build_wheel.py                   reproducible stdlib wheel builder
tests/test_onnx_worker.py        unittest suite (identity/protocol/backend/probe)
tests/run_real_protocol.py       real v2 exchange against the local ONNX model
```

## Installer / host interface assumptions

* Wheel: `resources/windows-parity/workers/studio_onnx_worker-0.1.0-py3-none-any.whl`
  (`Name: studio-onnx-worker`, `Version: 0.1.0`, `Requires-Python: ==3.12.*`,
  pins `onnxruntime==1.31.1`, `numpy==2.5.4`, `pillow==13.0.0`).
* Launch: `<runtime>/python.exe -I -m studio_onnx_worker [--model-snapshot-id <id>]`.
* Probe: append `--probe <normalized model directory>`; the worker opens the
  single `*.onnx` file with `CPUExecutionProvider`, returns
  `{identity, python, device:"windows-cpu", device_name:<CPU name>, packages,
  graph}`, and refuses when the provider or float32 graph contract is missing.
* `--model-snapshot-id` is optional; both the manifest/model-variant id
  (`birefnet-general-epoch-244-windows-v1`, what the installer currently passes)
  and the design section 11 snapshot id
  (`birefnet-general-epoch-244-onnx-windows-v1`) are accepted launch identities,
  and the worker echoes exactly the id it was launched with so probe,
  `ReadyRecord`, `ReadyPaths` and `ExecuteRequestV2.model_snapshot_id` agree.
  Any other value fails before compute.
* `HF_HUB_OFFLINE`/`TRANSFORMERS_OFFLINE` are forced; the worker never downloads.

### Known deviation from the macOS fork

`backend._candidate` hashes and fsyncs the written PNG through a read/write
handle. The macOS original opened the output read-only, and Windows rejects
`FlushFileBuffers`/`os.fsync` on read-only handles, so the fork reopens it
`rb+`. Candidate bytes and hashes are unchanged.

## Build and test

The frozen Windows v2 runtime (`target/parity-runtime/runtime/python.exe`,
Python 3.12.10) ships numpy/Pillow/onnxruntime but not pytest, so tests use the
standard library `unittest` runner:

```powershell
$py = 'target\parity-runtime\runtime\python.exe'
# 9 tests, unittest runner (repo py312 lacks pytest)
& $py -m unittest discover -s tools\onnx-worker-windows\tests -p 'test_*.py'
# reproducible wheel (fixed zip timestamps, sorted members)
& $py tools\onnx-worker-windows\build_wheel.py
```

Real CPU protocol exchange (CPU-only, ~30 s, no GPU coordination needed):

```powershell
& $py -I -m pip install --no-index --no-deps --target target\parity-worker\onnx-birefnet\wheel-site `
    resources\windows-parity\workers\studio_onnx_worker-0.1.0-py3-none-any.whl
& $py tools\onnx-worker-windows\tests\run_real_protocol.py `
    --python $py `
    --package-root target\parity-worker\onnx-birefnet\wheel-site `
    --wheel resources\windows-parity\workers\studio_onnx_worker-0.1.0-py3-none-any.whl `
    --model target\parity-worker\onnx-birefnet\model `
    --output target\parity-worker\onnx-birefnet\attempt-real-wheel `
    --source-image verification\planned-models-20261031\fixtures\original\portrait-hair.png `
    --report verification\windows-parity-worker\onnx-birefnet.json `
    --evidence-dir verification\windows-parity-worker
```

The Windows v2 runtime is an embeddable distribution whose `python312._pth`
ignores `PYTHONPATH`/cwd, so the harness and unit-test subprocesses launch the
package through a small `-c` bootstrap that inserts the wheel site directory
into `sys.path` and then calls the real `main()`. Production launches the same
package from the runtime site-packages with `-I -m studio_onnx_worker`.

Evidence: [`verification/windows-parity-worker/README-onnx-birefnet.md`](../../verification/windows-parity-worker/README-onnx-birefnet.md).

License: worker code Apache-2.0; BiRefNet model MIT (see the installed model
manifest license records).
