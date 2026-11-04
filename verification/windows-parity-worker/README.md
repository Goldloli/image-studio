# Windows diffusers worker (Klein / Qwen-Image 2.1 / MOSS)

Status: worker package and wheel built, installer-facing probe/prepare paths and
five real diffusers generation-v2 protocol exchanges executed on the Windows 10
target machine (2026-11-04, RTX 3080 Ti 12 GB / i7-13700K / 31.7 GiB RAM), plus
two real native Real-ESRGAN ncnn worker exchanges and three cancel checks. The
Qwen-Image 2.1 base and MOSS-Image-SFT real exchanges are covered here too. Two
genuine Windows-only Real-ESRGAN worker defects were found and fixed during this
task (path normalization and opaque-RGBA output); the historical defect record is
kept below and the official fixed-binary evidence passes.

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
- The Qwen base and MOSS real runs reused the Lead's Gate 0 prepared caches by
  copying them into the agreed `models/<snapshot>.prepared` locations and
  re-hashing every file: Qwen base `prepared-qwen21-base` (5 files,
  10,049,397,583 bytes) and MOSS `prepared-moss` (4 files, 8,471,497,242 bytes)
  are byte-identical to the source caches, so the real runs exercised the same
  deterministic prepared bytes already measured at Gate 0.

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
| Qwen-Image 2.1 base, 40 steps | [real-protocol-qwen21-base.json](real-protocol-qwen21-base.json) | 78.73 s (generation 69.64 s) | 5.961 GiB | 13.968 GiB | 14.737 GiB | 7,804 MiB | 1024x1024, 1,915,726 B, `1a38ca6f...d4a543`, [png](real-protocol-qwen21-base.png) |
| MOSS-Image-SFT, 50 steps | [real-protocol-moss.json](real-protocol-moss.json) | 139.27 s (generation 127.08 s) | 5.539 GiB | 13.064 GiB | 2.696 GiB | 7,750 MiB | 1024x1024, 1,842,307 B, `4cd42c8f...01b3a`, [png](real-protocol-moss.png) |

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
- Qwen base: the same staged NF4 executor as Turbo with the model's 40-step
  linear schedule and 2,048-token budget; progress `sampling 1..40 of 40` then
  `decoding`/`writing`, fixed seed 3148150817, terminal `output_candidate_v2`.
- MOSS: staged NF4 text encoder + NF4 DiT with the vendored fixed pipeline module
  and the documented dtype shim, 50 steps, CFG 3.0, empty negative prompt,
  negative-branch skip block 22; progress `sampling 1..50 of 50`. The staged-NF4
  output is a real MOSS generation and is not bit-identical to the Gate 0
  bf16 model-offload sample (different execution strategy); that is expected and
  not a determinism claim across strategies.
- All five runs stayed inside the design budgets (CUDA reserved <= 10.5 GiB,
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

[cancel-check-realesrgan.json](cancel-check-realesrgan.json): a `cancel` frame
sent immediately after `execute_v2` to the native Real-ESRGAN worker. The worker
terminated the ncnn child before a candidate was committed: terminal
`{"type":"cancelled"}`, worker exit 0, 0.125 s elapsed, only `loading` progress,
and the Attempt directory contains only the input copy (no `primary.png`).

## Real-ESRGAN Windows ncnn worker (native, passed after fix)

The Windows native worker release binary was rebuilt after the two Windows-only
worker defects below were fixed (commit `7c64c36`):
`target\release\realesrgan-worker.exe`, 1,115,136 bytes, SHA-256
`14789e96...13030f` (same binary staged at
`apps/desktop/binaries/realesrgan-worker-x86_64-pc-windows-msvc.exe`). The frozen
ncnn bundle was verified against the worker constants:
`realesrgan-ncnn-vulkan.exe` 6,161,408 bytes / SHA-256 `07e49f7c...7b1a0c`,
`realesrgan-x4plus.bin` 33,424,520 bytes / SHA-256 `713ee713...cdbcddf`,
`realesrgan-x4plus.param` 116,029 bytes / SHA-256 `35330ece...9f10a86`.

| Run | Evidence | Input | Output | Elapsed | Check |
|---|---|---|---|---|---|
| 220x220 official sample | [real-protocol-realesrgan-220x220.json](real-protocol-realesrgan-220x220.json) | 220x220, 117,553 B | 880x880, 984,304 B, `73eaef40...20c6817`, [png](real-protocol-realesrgan-220x220.png) | 2.562 s | exact 4x; RGB pixels identical to Gate 0 reference |
| 97x61 controlled image | [real-protocol-realesrgan-97x61.json](real-protocol-realesrgan-97x61.json) | 97x61, 14,643 B | 388x244, 98,673 B, `cbe33a46...0366f95`, [png](real-protocol-realesrgan-97x61.png) | 2.172 s | exact 4x; RGB pixels identical to Gate 0 reference |

Both runs used the normal installer-style absolute paths (not verbatim), so they
exercise the fixed product path, with the frozen recipe (`-s 4 -t 128 -g 0
-j 1:1:1`, no TTA) and a verified `--ncnn-binary`. Terminal frame was
`output_candidate_v2` with exit code 0. The ncnn stderr forwarded by the worker
enumerates the Vulkan devices, with `[0 NVIDIA GeForce RTX 3080 Ti]` selected and
`[1 Intel(R) UHD Graphics 770]` present but unused; `nvidia-smi` reports GPU
`GPU-78b715aa-ec51-31af-8813-eac9653458a7`, driver 596.49, 12,288 MiB.

Byte-vs-pixel comparison with the Gate 0 direct-ncnn evidence: the Windows ncnn
build writes opaque **RGBA** PNGs, while the frozen v2 contract requires one
8-bit **RGB** candidate, so the worker normalizes opaque RGBA to 8-bit RGB
(refusing non-opaque alpha). The output SHA-256 therefore differs from the Gate 0
direct-run RGBA file (Gate 0 220x220 `33a47bc5...`, 1,191,677 B; 97x61
`36894682...`, 124,974 B), but decoding both to RGB gives **zero differing
pixels / zero max channel difference** for both images
(`gate0_pixel_cross_check` in the evidence). This is the fixed behavior, not a
quality regression.

Historical defect evidence (kept intentionally): [realesrgan-worker-path-bug.json](realesrgan-worker-path-bug.json)
records the originally committed Windows binary rejecting every normal absolute
path (`Path::canonicalize()` returns `\\?\C:\...` on Windows) and the verbatim
path workaround crashing the frozen ncnn executable (`_wfopen`, 0xc0000409). The
fix commit `7c64c36` adds the platform-aware `trusted_user_path()` normalization
plus opaque-RGBA→RGB output normalization; its fix validation is recorded in
[realesrgan-worker-windows-path-fix.json](realesrgan-worker-windows-path-fix.json)
and the plain/verbatim runs
([realesrgan-path-fix-plain.json](realesrgan-path-fix-plain.json),
[realesrgan-path-fix-verbatim.json](realesrgan-path-fix-verbatim.json)). The
defect is resolved; the bug record stays as the found-before-fix evidence.

## Not run (time-boxed, clearly marked)

| Case | Status | Why / coverage |
|---|---|---|
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
  the fixed-seed SHA-256 equality only holds for the same strategy and settings
  (e.g. the staged-NF4 MOSS output differs from the Gate 0 bf16 model-offload
  sample, as expected).
- The MOSS run's minimum available RAM was 2.696 GiB, close to the frozen 2 GiB
  floor but inside budget; the worker would have failed closed below 2 GiB. No
  other heavy agent jobs were running during that run.
- Real-ESRGAN worker evidence: the two Windows-only defects found here
  (verbatim-path canonicalize comparison and opaque-RGBA ncnn output) are fixed
  in commit `7c64c36` and the official rerun passes; the RGBA→RGB normalization
  means the PNG byte SHA-256 differs from the Gate 0 direct-ncnn RGBA file by
  container color type, while decoded RGB pixels are identical (recorded in
  `gate0_pixel_cross_check`), so the records state both SHA-256 values instead of
  conflating them.

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

# native Real-ESRGAN worker (fixed binary)
target\parity-gate0\py312\python.exe tools\diffusers-worker-windows\tests\run_realesrgan_protocol.py `
  --worker-exe target\release\realesrgan-worker.exe `
  --ncnn-binary target\parity-runtime\realesrgan\app\realesrgan-ncnn-vulkan.exe `
  --model-dir target\parity-worker\realesrgan\model --input <png> `
  --output target\parity-worker\attempts\<case> --report <json>
```
