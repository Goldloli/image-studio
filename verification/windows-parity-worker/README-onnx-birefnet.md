# Windows parity — BiRefNet ONNX Runtime CPU worker evidence

Task 2.3/2.5 of `openspec/changes/achieve-windows-model-parity`: one Windows
generation-v2 worker process for BiRefNet `epoch_244` running **only** on the
ONNX Runtime CPU execution provider. The worker package is
[`tools/onnx-worker-windows/`](../../tools/onnx-worker-windows/README.md); this
file records the probe and one real end-to-end v2 protocol exchange.

This directory is shared with the Windows diffusers worker (task-5); files are
deliberately prefixed `onnx-birefnet*` / `probe-onnx-*` so the two evidence sets
do not collide.

## Status

| evidence | result |
|---|---|
| unit tests (`unittest`, 9 tests) | pass |
| wheel build (reproducible) | pass |
| wheel install + import (`pip --no-index --no-deps --target`) | pass |
| `--probe` on the real ONNX graph, CPUExecutionProvider | pass |
| real generation-v2 protocol exchange producing RGBA + mask | **pass** |

## Wheel and identity

```text
file      resources/windows-parity/workers/studio_onnx_worker-0.1.0-py3-none-any.whl
bytes     10190
sha256    453757530db217424f1aa6799d1df3d163901e1d711ac49c3ecd447fd5b1e1d6
name      studio-onnx-worker 0.1.0 (Requires-Python ==3.12.*, py3-none-any)
members   studio_onnx_worker/{__init__,__main__,backend}.py + dist-info
```

Building the wheel twice from the same source produced the same sha256 (fixed
zip timestamps, sorted members, stdlib-only builder).

Worker identity announced in the handshake and probe:

```json
{"build_id":"studio-onnx-worker-windows-0.1.0",
 "runtime_lock_id":"windows-python31210-torch2100-cu126-diff041-transformers53-v2",
 "model_snapshot_id":"birefnet-general-epoch-244-onnx-windows-v1"}
```

## Commands (Windows v2 runtime `target/parity-runtime/runtime/python.exe`, Python 3.12.10)

The frozen runtime is an embeddable distribution without pytest, so the repo
standard for this worker is the stdlib `unittest` runner (documented in the
package README); the harness/unit-test subprocesses bootstrap `sys.path`
because `python312._pth` ignores `PYTHONPATH`.

```powershell
$py = 'target\parity-runtime\runtime\python.exe'

# 1) unit tests -> "Ran 9 tests ... OK" (0.47 s)
& $py -m unittest discover -s tools\onnx-worker-windows\tests -p 'test_*.py'

# 2) reproducible wheel -> 10190 B / sha256 4537575...
& $py tools\onnx-worker-windows\build_wheel.py

# 3) install the committed wheel into an isolated site dir, then import
& $py -I -m pip install --no-index --no-deps --upgrade `
    --target target\parity-worker\onnx-birefnet\wheel-site `
    resources\windows-parity\workers\studio_onnx_worker-0.1.0-py3-none-any.whl

# 4) real probe on the local frozen ONNX graph (CPU only)
& $py -I -c "import sys; sys.path.insert(0, r'target/parity-worker/onnx-birefnet/wheel-site'); from studio_onnx_worker.__main__ import main; raise SystemExit(main())" `
    --model-snapshot-id birefnet-general-epoch-244-onnx-windows-v1 `
    --probe (Resolve-Path target\parity-worker\onnx-birefnet\model).Path

# 5) real generation-v2 exchange (hello -> hello_accepted -> execute_v2 ->
#    loading/inference/decoding/writing progress -> 2 output_candidate_v2)
& $py tools\onnx-worker-windows\tests\run_real_protocol.py `
    --python $py `
    --package-root target\parity-worker\onnx-birefnet\wheel-site `
    --wheel resources/windows-parity\workers\studio_onnx_worker-0.1.0-py3-none-any.whl `
    --model target\parity-worker\onnx-birefnet\model `
    --output target\parity-worker\onnx-birefnet\attempt-real-wheel `
    --source-image verification\planned-models-20261031\fixtures\original\portrait-hair.png `
    --report verification\windows-parity-worker\onnx-birefnet.json `
    --evidence-dir verification\windows-parity-worker
```

The model directory used by the run is a hard link to
`target/parity-gate0/models/birefnet/BiRefNet-general-epoch_244.onnx`
(972,666,916 B, sha256 `58f621f0…`); the harness re-hashed the file and the
report records `matches_frozen_asset: true`. The source image is the same
controlled real photograph used by Gate 0 G2-B1.

## Real protocol run (wheel-installed worker)

Request: model `birefnet-general-epoch-244-windows-v1`, capability
`image.remove-background.birefnet-general-epoch-244.windows-v1`, recipe
`birefnet-general-cpu-windows-v1`, empty instruction, no seed/steps, exact
frozen parameter map, one `source` input (1122×1402 RGB), `primary` RGBA +
`mask` outputs at source dimensions, 32 MiB bound each.

| field | value |
|---|---|
| protocol | `image-studio.generation` v2, session accepted, 2 candidate frames |
| worker stderr | empty (no warnings/errors) |
| total wall time | 29.484 s |
| handshake | 0.125 s |
| progress stages | loading @1.125 s, inference @10.812 s, decoding @27.922 s, writing @28.156 s |
| worker peak working set | 8318.8 MiB (0.1 s samples, 264 samples, psapi) |
| primary | `primary.png` RGBA 1122×1402, 2,797,794 B, sha256 `9d1800cfaab8cc346c9fdd29385fa0fd405985c6ec1aa8fe02a02d052a479309` |
| mask | `mask.png` L 1122×1402, 35,198 B, sha256 `4433c05c9af567ee5455d8f9176f01cc81d8e9ab764bbb37de0b82f95522684e` |

Candidate sha256 values are **identical to the Gate 0 macOS-worker reference**
(`verification/windows-parity-gate0/birefnet-gate0-rgba.png` /
`-mask.png`), which confirms the Windows v2 shell preserves the frozen
preprocess → single sigmoid → straight-alpha contract byte for byte.

Checks recorded in `onnx-birefnet.json` (all true):

* primary mode RGBA, mask mode 8-bit L, both dimensions equal the source;
* `primary.alpha == mask` for every pixel (dual-output atomicity contract);
* `primary.rgb == source.rgb` (straight alpha, no background compositing);
* mask has partial alpha (30,329 px), spatial variation, 256 distinct values,
  912,381 fully transparent / 630,334 fully opaque pixels, mean 104.779.

## Probe (`probe-onnx-birefnet-windows.json`)

```json
{"identity":{"build_id":"studio-onnx-worker-windows-0.1.0",
             "runtime_lock_id":"windows-python31210-torch2100-cu126-diff041-transformers53-v2",
             "model_snapshot_id":"birefnet-general-epoch-244-onnx-windows-v1"},
 "python":"3.12.10",
 "device":"windows-cpu",
 "device_name":"Intel64 Family 6 Model 183 Stepping 1, GenuineIntel",
 "packages":{"onnxruntime":"1.31.1","numpy":"2.5.4","pillow":"13.0.0"},
 "graph":{"model_file":"BiRefNet-general-epoch_244.onnx",
          "input":{"name":"input_image","type":"tensor(float)","shape":[1,3,1024,1024]},
          "outputs":[{"name":"output_image","type":"tensor(float)","shape":[1,1,1024,1024]}],
          "providers":["CPUExecutionProvider"]}}
```

## Evidence files

| path | bytes | sha256 |
|---|---|---|
| `onnx-birefnet.json` | 5260 | `e2a56a015176c232611e984b10c286fb0bd8f6314102ce9b0d62ff62fef3f3f3` |
| `onnx-birefnet-primary.png` | 2797794 | `9d1800cfaab8cc346c9fdd29385fa0fd405985c6ec1aa8fe02a02d052a479309` |
| `onnx-birefnet-mask.png` | 35198 | `4433c05c9af567ee5455d8f9176f01cc81d8e9ab764bbb37de0b82f95522684e` |
| `probe-onnx-birefnet-windows.json` | 691 | `9c122e3729752f77eeda8b9a3232e25617688d3702ff1a7bf6af835e11eae2b2` |

## Not run / limitations (recorded, not hidden)

* **Cancel / SIGKILL / lease recovery** (task 2.5) is not exercised by this
  evidence: it is owned by the desktop supervisor + generation controller
  integration (`apps/desktop`, task-6) and the worker exposes the same
  cooperative cancel frame as the macOS shell (unit-tested structure only).
* Only one controlled real photograph was run. CPU-only inference: no CUDA
  reserved-memory or WDDM shared-memory checks apply to this worker.
* Peak working set is sampled every 0.1 s via `psapi`; it is a diagnostic
  number, not a hard budget for this CPU recipe (Gate 0 recorded 10.14 GiB
  private commit for the same graph under a different measurement method).
* Installer acceptance (real install → probe → generation through the app) is
  task-6/lead acceptance, not part of this worker evidence.

## Installer interface notes

* Probe device string is `windows-cpu`; `identity.model_snapshot_id` must equal
  the ReadyRecord/ReadyPaths model snapshot the profile expects.
* The worker accepts `--model-snapshot-id` with either the model manifest id
  (`birefnet-general-epoch-244-windows-v1`) or the design section 11 snapshot id
  (`birefnet-general-epoch-244-onnx-windows-v1`) and echoes the passed value, so
  the current installer (which passes the model manifest id) and the frozen
  section 11 identity can both be hosted without rewriting the worker.
  Everything else is rejected before compute.
* Wheel to append to the product runtime manifest:
  `resources/windows-parity/workers/studio_onnx_worker-0.1.0-py3-none-any.whl`,
  10190 B, sha256 `453757530db217424f1aa6799d1df3d163901e1d711ac49c3ecd447fd5b1e1d6`.
