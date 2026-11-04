# Windows diffusers worker (Klein / Qwen-Image 2.1 / MOSS)

Status: worker package and wheel built, installer-facing probe/prepare paths and
three real generation-v2 protocol exchanges executed on the Windows 10 target
machine (2026-11-04, RTX 3080 Ti 12 GB / i7-13700K / 31.7 GiB RAM). Qwen-Image
2.1 base and MOSS-Image-SFT real generations are marked `not_run` (see below);
their request validation, prepared-cache logic and vendored pipeline modules are
covered by unit tests and the frozen Gate 0 staged evidence from the Lead.

## Deliverable

- Package: `tools/diffusers-worker-windows/studio_diffusers_worker/`
- Wheel: [studio_diffusers_worker-0.1.0-py3-none-any.whl](../../resources/windows-parity/workers/studio_diffusers_worker-0.1.0-py3-none-any.whl)
  - bytes: `41,530`
  - sha256: `79799fd9f0fae9f8f54e94cb57472779d8c49c1ba26c34c066a2018d75909e1b`
  - build: `python tools\diffusers-worker-windows\build_wheel.py` (stdlib only,
    fixed zip timestamps, deterministic rebuild produced the identical SHA-256)
  - installability: `pip install --target <dir> --no-deps <wheel>` then
    `import studio_diffusers_worker` succeeds with the frozen Python 3.12.10
    experiment interpreter; `BUILD_ID = studio-diffusers-worker-windows-0.1.0`,
    runtime lock `windows-python31210-torch2100-cu126-diff041-transformers53-v2`.
- Unit tests: `python -m pytest tools\diffusers-worker-windows\tests` ->
  `25 passed` (identity/spec matrix for 5 capabilities, exact effective
  parameters, request mutation rejections, Klein edit source descriptor
  strictness, protocol envelope strictness, installer CLI argument shapes,
  prepare dispatch, vendored MOSS pipeline hash, Windows process-memory API).
- Design record: [tools/windows-parity-gate0/README.md](../../tools/windows-parity-gate0/README.md)
  and [tools/diffusers-worker-windows/README.md](../../tools/diffusers-worker-windows/README.md).

## Installer-facing probe (`--probe`)

Four snapshots were probed on the real card with `--model-snapshot-id <snapshot>
--probe <model_dir>`; every report carries `python=3.12.10`, `device=gpu`,
`device_name=NVIDIA GeForce RTX 3080 Ti`, the frozen `identity` and the pinned
package versions asserted by the installer.

| Evidence | Snapshot | Result |
|---|---|---|
| [probe-flux2-klein-4b.json](probe-flux2-klein-4b.json) | `flux2-klein-4b-windows-e7b7...` | passed |
| [probe-qwen-image-2.1.json](probe-qwen-image-2.1.json) | `qwen-image-2.1-windows-staged-d26b...` | passed |
| [probe-qwen-image-2.1-turbo.json](probe-qwen-image-2.1-turbo.json) | `qwen-image-2.1-turbo-windows-staged-fc13...` | passed |
| [probe-moss-image-sft.json](probe-moss-image-sft.json) | `moss-image-sft-windows-staged-f972...` | passed |

(BiRefNet is served by the ONNX worker and its probe evidence lives in
`probe-onnx-birefnet-windows.json` / `README-onnx-birefnet.md`.)

## `--prepare` (install-time prepared cache)

The v2 request schema has no `prepared_dir` field, so the worker derives the
staged Qwen/MOSS cache from the installed snapshot path:
`models/<snapshot-directory>` -> `models/<snapshot-directory>.prepared`. The
installer calls `--prepare --model-dir <source snapshot> --prepared-dir
<...>.prepared`; both sides were agreed with the Lead.

Qwen-Image 2.1-Turbo rebuild on the target machine
([prepare-qwen-turbo.json](prepare-qwen-turbo.json), `--rebuild-prepared`):

| Component | Load | Save | Prepared bytes | Process commit peak | Available RAM min |
|---|---|---|---|---|---|
| NF4 text encoder | 25.02 s | 6.07 s | 6,378,436,043 | 23.384 GiB | 0.845 GiB |
| NF4 transformer | 45.45 s | 3.00 s | 3,670,961,576 | 25.313 GiB | 0.003 GiB |
| total cache | 126.16 s wall | | 10,049,397,619 (5 files) | | |

- The prepared cache is byte-identical to the cache built independently by the
  Lead's Gate 0 staged experiment: text encoder
  `13c2f7cdb087b336125a993489543a30df574aa94d4cfad029b8a01b0596e6f7` and
  transformer
  `010c2c3bfd1101797c63c874560243dcf770db7b68473e5693acfacefc26615d` match, so
  the cache is a deterministic rebuild of the fixed snapshot, not a new source.
- This confirms the design warning that prepare is install-time work with a
  large bf16-quantization transient (23-25 GiB commit, available RAM under
  1 GiB): the installer must run a RAM/commit preflight before `--prepare` and
  must not run it concurrently with generation. Prepare quality/time is outside
  the generation budget by design (design.md 10.2 item 3).
- Klein returns `not_required`
  ([prepare-klein-not-required.json](prepare-klein-not-required.json)); its
  Gate 0 strategy loads NF4 + component CPU offload directly from the fixed
  snapshot in a fresh generation process.

## Real generation-v2 protocol exchanges

Each run: worker spawned from the installed wheel with the frozen runtime
`python.exe`, v2 hello/execute/cancel JSONL framing, host-side and worker-side
resource sampling, offline environment, real PNG candidate committed through the
v2 `output_candidate_v2` frame. Images were copied next to the evidence JSON;
the SHA-256 below is what the worker put into the candidate frame.

| Case | Evidence | Elapsed (host) | CUDA reserved peak | Private commit peak | RAM min | nvidia-smi peak | Candidate |
|---|---|---|---|---|---|---|---|
| Qwen Turbo, 8 steps | [real-protocol-qwen-turbo.json](real-protocol-qwen-turbo.json) | 33.45 s (generation 25.12 s) | 5.961 GiB | 13.985 GiB | 13.005 GiB | 7,183 MiB | 1024x1024, 1,759,574 B, `b4be0981...60d68e`, [png](real-protocol-qwen-turbo.png) |
| Klein text-to-image, 4 steps | [real-protocol-klein-text.json](real-protocol-klein-text.json) | 42.27 s (generation 30.75 s) | 9.221 GiB | 19.485 GiB | 9.994 GiB | 11,135 MiB | 1024x1024, 2,074,705 B, `2802c200...0ff404`, [png](real-protocol-klein-text.png) |
| Klein single-image edit, 4 steps | [real-protocol-klein-edit.json](real-protocol-klein-edit.json) | 38.03 s (generation 28.62 s) | 9.182 GiB | 18.925 GiB | 11.457 GiB | 11,033 MiB | 1024x1024, 1,859,640 B, `3c337047...b914a9`, [png](real-protocol-klein-edit.png) |

Protocol details:

- Qwen Turbo: staged NF4 executor over the prepared cache; progress frames
  `loading/encoding/sampling 1..8 of 8/decoding/writing`; 13 progress frames,
  terminal `output_candidate_v2`.
- Klein text: `Flux2KleinPipeline` NF4 components + `enable_model_cpu_offload`;
  progress `loading`, then `sampling 0..4 of 4`, `writing`; 4 sampling steps as
  frozen; the output SHA-256 equals the Gate 0 fresh-process output for the same
  seed and size, an independent determinism cross-check.
- Klein edit: exactly one `source` input descriptor (Attempt-local regular file,
  SHA-256 verified by the worker), prompt instruction, 4 steps, output
  1024x1024 RGB per the frozen output specification. Gate 0 measured the
  condition-derived 912x1136 size without explicit dimensions; the product
  contract pins 1024 square, so the worker passes explicit output dimensions.
- All three runs stayed inside the design budgets (CUDA reserved <= 10.5 GiB,
  private commit <= 24 GiB, available RAM >= 2 GiB, load/generation limits); the
  worker enforces them as hard failures.
- One earlier Klein edit attempt during a period of concurrent builds by other
  agents was rejected by the hard budget guard (`available RAM 1.478 GiB <
  2.0 GiB`); the clean rerun above passed at 11.457 GiB. The worker fails closed
  rather than oversubscribing, by design.

## Cancel check

[cancel-check-qwen-turbo.json](cancel-check-qwen-turbo.json): a `cancel` control
frame sent immediately after `execute_v2`; the worker acknowledged it before
generation, emitted only `loading` progress and the terminal
`{"type":"cancelled"}` frame in 12.3 s. No candidate was produced, no image
asset was committed, and the Attempt output directory stayed empty (worker
elapsed 12.3 s, worker process commit 2.084 GiB, nvidia-smi peak 1,908 MiB -
i.e. no generation happened).

## Not run (time-boxed, clearly marked)

| Case | Status | Why / coverage |
|---|---|---|
| Qwen-Image 2.1 base real generation | not_run | Same staged executor code path as Turbo (40 steps, 2048-token budget); measured basis is the Lead's Gate 0 staged evidence. Prepare for the base snapshot was not rerun to avoid another 10 GB/25 GiB-transient build. |
| MOSS-Image-SFT real generation | not_run | Staged executor + vendored pipeline module are unit-tested (frozen 25,060 B / SHA-256 `1dac1aca...`), and the Gate 0 staged run from the Lead is the measured basis; not rerun within this task. |
| Supervisor-wrapped exchange | not_run | The harness currently spawns the wheel directly with the same worker arguments the supervisor will use; supervisor lifecycle evidence is covered by the supervisor/worker tasks. |

## Safety and limitations

- No network access: the worker forces `HF_HUB_OFFLINE`, `TRANSFORMERS_OFFLINE`,
  telemetry off, and local snapshot dirs; no runtime download path exists.
- The reviewed MOSS pipeline module is vendored inside the wheel (25,060 bytes,
  SHA-256 verified before import); Python is never executed from an installed
  model directory. The bnb dtype shim is documented in `vendored/moss_bnb_shim.py`.
- Worker-side resource sampling uses the Windows process private commit
  (`K32GetProcessMemoryInfo`) and system memory; host-side sampling uses psutil +
  `nvidia-smi`. The two agree within sampling resolution (see tables above).
- `nvidia-smi` totals include ~0.8-1.5 GiB of desktop WDDM clients, so CUDA
  reserved is the primary VRAM decision value.
- The pipeline variants are not bit-comparable across staged/offload strategies;
  the fixed-seed SHA-256 equality only holds for the same strategy and settings.

## Commands used

```powershell
# unit tests
python -m pytest tools\diffusers-worker-windows\tests

# wheel + install check
python tools\diffusers-worker-windows\build_wheel.py
target\parity-gate0\py312\python.exe -m pip install --no-deps --target <dir> `
  resources\windows-parity\workers\studio_diffusers_worker-0.1.0-py3-none-any.whl

# probe / prepare / real protocol (frozen experiment interpreter)
target\parity-gate0\py312\python.exe tools\diffusers-worker-windows\tests\run_real_protocol.py `
  --mode probe   --python <py312> --worker-site <dir> --snapshot <snapshot> --model <model-dir> --report <json>
target\parity-gate0\py312\python.exe tools\diffusers-worker-windows\tests\run_real_protocol.py `
  --mode prepare --python <py312> --worker-site <dir> --snapshot <snapshot> `
  --model <source-snapshot> --prepared-dir <...>.prepared --report <json>
target\parity-gate0\py312\python.exe tools\diffusers-worker-windows\tests\run_real_protocol.py `
  --mode run     --python <py312> --worker-site <dir> --snapshot <snapshot> --model <model-dir> `
  --report-output target\parity-worker\attempts\<case> --report <json>
```
