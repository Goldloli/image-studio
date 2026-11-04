# Windows parity app acceptance plan

Status: **plan only — no acceptance case has been executed from this directory
yet.** Every case below starts as `not_run`; results are only recorded when the
real Windows installed app exercises them. This plan is the runbook for the
parity change (`openspec/changes/archive/2026-11-04-achieve-windows-model-parity`, tasks 3.2-3.4 /
4.x / test-plan I2, P2, R2) and it must not be read as evidence of a pass.

## 1. Scope and preconditions

Acceptance target: the Windows installed application (tauri build from the
current branch) with the frozen resources of this change:

- shared runtime `windows-python31210-torch2100-cu126-diff041-transformers53-v2`
  (`resources/windows-parity/runtime-v2/product-runtime-manifest.json`)
- six model variants in `resources/windows-parity/models/*/model-manifest.json`
- Real-ESRGAN native package
  (`resources/windows-parity/realesrgan-ncnn-vulkan/{model,runtime}-manifest.json`)

Preconditions:

1. The installer implementation wires the resources above (desktop-install
   stream) and builds successfully; record the exact build commit/version.
2. Local content for cache seeding is present and hash-verified by
   `verification/windows-parity-manifests/` (all 8 manifests pass there).
3. Data roots are explicit and empty per empty-cache case; seeded cases use a
   data root prepared by `scripts/windows-parity/seed-parity-cache.ps1`.
4. GPU cases run only inside a Lead-granted exclusive GPU window; record
   `nvidia-smi` before/after and never run two GPU jobs at once.
5. Disk budget: full model set is ~110 GB of model artifacts plus ~4.8 GB
   runtime; leave ≥ 2× the selected workspace free (measured free space on the
   target machine at plan time: ~1.4 TB). Per-model figures are in §7.
6. Network: empty-cache installs use the real network. A strict network-denial
   run is currently **waived/not_run** by explicit user decision
   (`verification/windows-network-denial-decision.json`, cases I07/P06); the
   optional mechanism is `scripts/windows-admin/enable-network-denial.ps1` run
   from an elevated shell. A seeded-cache install is not a substitute for a
   network-denial claim.

## 2. Cache seeding (offline-install preparation)

The installer's download cache is `<DataRoot>/cache/downloads/<sha256>`
(see `apps/desktop/src/adapters/installation.rs`). Seed it from the verified
local copies without any network access:

```powershell
# dry run first: reports missing / hash-mismatch without writing links
./scripts/windows-parity/seed-parity-cache.ps1 -DataRoot <data-root> -DryRun `
  -OutputPath verification/windows-parity-app/seed-cache-<case>.json

# real seed: hard-links (copy fallback) every verified artifact
./scripts/windows-parity/seed-parity-cache.ps1 -DataRoot <data-root> `
  -OutputPath verification/windows-parity-app/seed-cache-<case>.json
```

The script reads the six model manifests + product runtime manifest + native
runtime manifest, resolves each declared download artifact (files, wheels,
interpreter, archives) against `target/parity-gate0/models` and
`target/parity-runtime`, re-verifies byte count and SHA-256, then links it into
the cache. It performs no downloads. Missing items are reported as `missing`
and mismatches as `hash_mismatch`; neither is ever fabricated. The Real-ESRGAN
model archive and native-runtime archive share one SHA-256 (linked once,
`deduplicated`). Licenses and the installer-embedded worker wheels are not
download artifacts and are not seeded.

Expected outcome for a complete local set: `status=complete`, `missing=0`,
`hash_mismatch=0`, `unique_shas` ≈ 123 with 9 deduplicated artifacts (shared
tokenizer files across Qwen/Klein/MOSS plus the Real-ESRGAN zip referenced by
both manifests), and `unique_bytes` ≈ 112,508,722,670. Any missing item blocks
the corresponding seeded-case row and must be listed in the evidence JSON as
blocked by source, not skipped silently.

Local tool validation on the development machine (2026-11-04, dry-run + seed
into a scratch data root, recorded in
`verification/windows-parity-app/seed-cache-local-validation.json`): 132
artifacts, 123 unique SHA-256 values, 9 deduplicated, 0 missing, 0 hash
mismatch, 112,508,722,670 unique bytes. This validates that the manifests
resolve against the local verified content; it is **not** an acceptance install
result.

## 3. Install acceptance matrix

The shared runtime installs once per data root; the per-model install rows still
verify the runtime probe, per-file hash checks, activation and ready record.

| Case | Model / variant | Operation | Expected result | Evidence |
|---|---|---|---|---|
| I2-1a | Klein `flux2-klein-4b-distilled-windows-v1` | empty data root, install from real network | all 19 files + runtime verified, probe ready, no `bundled:worker-wheel` download | `verification/windows-parity-app/install-flux2-klein.json` |
| I2-1b | BiRefNet `birefnet-general-epoch-244-windows-v1` | empty data root, install | ONNX 972,666,916 B verified, CPU probe ready | `install-birefnet.json` |
| I2-1c | Qwen base `qwen-image-2.1-windows-v1` | empty data root, install **with prepare** | 25 files verified, prepare completes, RAM preflight passes, probe ready | `install-qwen21-base.json` |
| I2-1d | Qwen Turbo `qwen-image-2.1-turbo-windows-v1` | empty data root, install **with prepare** | 16 files verified, prepare completes, probe ready | `install-qwen21-turbo.json` |
| I2-1e | MOSS `moss-image-sft-windows-v1` | empty data root, install **with prepare** | 19 files verified, prepare completes, probe ready | `install-moss-image-sft.json` |
| I2-1f | Real-ESRGAN `realesrgan-x4plus-windows-v1` | empty data root, install native package | zip 45,474,481 B verified, exe/dll/README extracted, weights verified, native probe ready | `install-realesrgan.json` |
| I2-2a..f | same six variants | seeded cache from §2, install | zero download bytes (all artifacts served from cache), identical verification/activation; installer may still re-verify hashes | `install-seeded-<variant>.json` |
| I2-3 | selected variant(s) | corrupt one cached file / remove one cache file / force disk shortage | install fails before activation with actionable error, no partial activation, existing models untouched | `install-negative-<case>.json` |
| I2-4 | any | whole-tree network denial per elevated script | **waived/not_run** by user decision (see `verification/windows-network-denial-decision.json`); do not record as pass | `network-denial-decision.md` |
| I2-5 | v1 Z-Image runtime/model | install v1 after/before v2 to confirm shared-runtime fix | v1 still installs, other model not stranded | `install-v1-regression.json` |

Every install case records: build id, data root, start/end UTC, network used,
received bytes, per-file hash verification count, activation/ready record path,
probe output, `nvidia-smi` (GPU cases), and `pass`/`fail`/`not_run` + reason.

## 4. Functional acceptance matrix (six models / eight capabilities)

Each row requires a real generation/processing through the native UI, not a
worker helper. Prompts and fixed parameters follow design §11; seed fixed per
case and recorded.

| Case | Model / capability | Operation | Expected result | Evidence |
|---|---|---|---|---|
| P2-1a | Klein text-to-image | UI send, 1024², 4 steps, guidance 1.0, seed fixed | 1024x1024 RGB PNG; budget: load ≤600 s, generation ≤900 s, CUDA reserved ≤10.5 GiB with no shared fallback, commit ≤24 GiB, available RAM ≥2 GiB | `generate-klein-t2i.json` + PNG + sha256 |
| P2-1b / P2-2 | Klein single-image edit | UI edit with source image role `source`, instruction prompt | edited 1024² RGB PNG; source image preserved; not an img2img disguise; cancellation leaves source intact | `generate-klein-edit.json` |
| P2-3 | BiRefNet remove background | UI process with source image | RGBA straight-alpha PNG + 8-bit mask, atomic dual-output commit, one primary asset counted | `generate-birefnet.json` |
| P2-4 | Real-ESRGAN 4x | UI process, exact 4x contract | output exactly 4× input, PNG, output sha256 logged, no transparency input | `generate-realesrgan.json` |
| P2-1c | Qwen-Image 2.1 base | UI send, 40 steps, guidance 1.0, 2048 token budget, NF4 staged | 1024² PNG; staged prepare reused; budget as above | `generate-qwen21-base.json` |
| P2-1d | Qwen-Image 2.1 Turbo | UI send, 8 steps + turbo sampling schedule | 1024² PNG; faster than base; budget as above | `generate-qwen21-turbo.json` |
| P2-1e | MOSS-Image-SFT | UI send, 50 steps, CFG 3, negative-branch skip 22 | 1024² PNG; bnb dtype shim in effect; budget as above | `generate-moss-image-sft.json` |

Resource evidence for every generation case: `nvidia-smi` before/after,
CUDA allocated/reserved peaks (main thread), process-tree commit, available RAM,
wall time and stage breakdown if exposed. The measurement method is the one
validated by `verification/windows-parity-gpu/` (Qwen Turbo audit).

## 5. Resilience, UI and product-path matrix

| Case | Operation | Expected result | Evidence |
|---|---|---|---|
| P2-5a | Cancel during loading, during sampling and during decode | task becomes interrupted/cancelled; worker tree reaped; no asset committed; new attempt allowed | `cancel-<model>.json` |
| P2-5b | Hard-kill worker process mid-generation | supervisor detects exit, labels attempt interrupted, no fabricated asset, next attempt succeeds | `kill-<model>.json` |
| P2-6 | Native UI send / progress / view / export PNG / export RGBA+mask / task center / works (gallery) | all product surfaces driven natively; exported files byte-identical to the library asset | screenshots + `ui-<case>.json` |
| P2-7 | Hardware gate and license display | below-threshold entries disabled with reason; Qwen shows non-commercial license notice; Real-ESRGAN/BiRefNet show their floors | `libraries-<case>.json` + screenshots |
| 4.3 / R2-2 | Cross-session mutual exclusion, old records readonly, restart recovery, Z-Image regression | no concurrent duplicate execution; old records readable; restart recovers cleanly; v1 behavior unchanged | `recovery-<case>.json` |

## 6. Prepare-phase expectations (install-time, CPU)

Prepare is part of install for Qwen base/Turbo and MOSS: the fixed upstream
snapshot is NF4-quantized on CPU and persisted. Measured Gate 0 values on this
machine (see `target/parity-gate0/staged/*-prepare*.json`):

| Model | Prepare wall time | Component load/save | Prepared cache | Notes |
|---|---|---|---|---|
| Qwen base | 62 s (16:19:26→16:20:28) | text encoder 23.88 s load + 6.61 s save; transformer 20.71 s + 1.93 s | 10,049,397,583 B | transient commit peak ~23.4 GiB; min available RAM during prepare 0.58 GiB |
| Qwen Turbo | 62 s (16:08:18→16:09:20) | text encoder 22.23 s + 6.94 s; transformer 23.22 s + 1.64 s | 10,049,397,619 B | same transient profile |
| MOSS | 52 s (16:14:14→16:15:06, NF4 variant) | text encoder 22.21 s + 5.65 s | 8,471,497,242 B | NF4 DiT variant is ~8.47 GB |

Install-time RAM preflight (design §10.2 item 3): require available physical RAM
≥ 28 GiB or commit headroom ≥ 26 GiB before starting prepare; if not met the
installer must fail with an actionable message before quantization starts
(never a silent crash or a swap storm). Expected one-time install costs at the
measured transfer speed (~44 MB/s from the Gate 0 transfers):

| Model | Download (artifact bytes) | Prepare | Runtime install | Total one-time estimate |
|---|---|---|---|---|
| Qwen base | ~33.1 GB ≈ 12.5 min | ~1 min | ~2 min (shared) | ~15.5 min |
| Qwen Turbo | ~32.5 GB ≈ 12.3 min | ~1 min | shared | ~15.3 min |
| MOSS | ~27.2 GB ≈ 10.3 min | ~1 min | shared | ~13.3 min |
| Klein | ~16.0 GB ≈ 6.1 min | none | shared | ~8.1 min |
| BiRefNet | ~0.93 GB ≈ 21 s | none | shared | ~2.4 min |
| Real-ESRGAN | ~45 MB ≈ 1 s | none | native extract | <1 min |

A seeded-cache install should take only verification + runtime extraction time
(single-digit minutes, dominated by hashing ~110 GB and wheel extraction).

## 7. Space budget reference (per model, manifest bytes)

| Model | component_bytes | Prepared cache | Download cache (seed) |
|---|---|---|---|
| Klein | 15,980,141,329 | none (NF4 at load) | same artifacts, one copy (~15.98 GB) |
| BiRefNet | 972,666,916 | none | ~0.97 GB |
| Qwen base | 33,131,609,424 | ~10.05 GB | ~33.13 GB |
| Qwen Turbo | 32,451,607,098 | ~10.05 GB | ~32.45 GB |
| MOSS | 27,219,783,878 | ~8.47 GB | ~27.22 GB |
| Real-ESRGAN | 33,540,549 (weights) | none | 45,474,481 B zip |
| Shared runtime | 4,778,861,981 installed (2,714,172,955 wheelhouse) | — | 2.71 GB wheelhouse |

Full-model installed set ≈ 110 GB model files + ~28.6 GB prepared caches +
4.78 GB runtime ≈ 143 GB; during an empty-cache install the download cache
roughly doubles the model bytes until it can be released, so budget ≥ 260 GB
free for exercising all cases serially (or run per-model with cleanup between).

## 8. Runbook order

1. Preflight: confirm build commit, frozen manifests verification pass
   (`verification/windows-parity-manifests/`), record machine/GPU/driver/disk.
2. Cache seed dry-run, then real seed for the seeded-cache cases
   (record `seed-cache-*.json`).
3. Per-model empty-cache installs (I2-1a..f) one at a time; after each, capture
   install evidence and move to a fresh data root or clean the cache. Qwen and
   MOSS rows need the RAM preflight check recorded before install.
4. Seeded-cache installs (I2-2a..f) from the seeded data root; confirm zero
   download bytes (cache hit per SHA-256).
5. Functional generations (P2 rows) during Lead-granted GPU windows, one model
   per window if budgets are tight; capture resource samples and outputs.
6. Cancel/kill/recovery rows (P2-5, 4.3) while the GPU is free; no concurrent
   generation.
7. UI/export/task/works rows (P2-6, P2-7); capture screenshots and exported
   file hashes.
8. Summarize per-case results in `verification/windows-parity-app/README.md`;
   mark any unexecuted case `not_run` with the blocking reason and keep
   pass/fail/not_run separate from "worker helper output".

## 9. Evidence conventions and honesty rules

- Per-case JSON: `{kind, case_id, build, started_at_utc, finished_at_utc,
  machine, data_root, commands_or_ui_steps, expected, actual, status
  (pass|fail|not_run), evidence_files, resources, error}`.
- Screenshots for UI cases; PNG/RGBA/mask exports with SHA-256 for image rows.
- Never record a pass from a helper script, a previous platform, or a different
  model. If a case is not executed it is `not_run` (or `blocked` with the
  concrete blocker); missing hashes are reported, never invented.
- Network-denial claims stay `waived_not_run` until the user changes the scope
  decision; seeded-cache installs may be recorded as "cache-hit install" but
  not as network-denial proof.
- After the rows are executed, update test-plan tasks and only then consider the
  change archive step (Lead/user decision).

## 10. Known deferred / not_run at plan time

| Item | Reason | Consequence |
|---|---|---|
| I2-4 / I07 / P06 network-denial install | user decision 2026-11-03 (`verification/windows-network-denial-decision.json`) | not_run, never counted as pass |
| Worker real protocol evidence | owned by desktop-install / gate0-transfer streams | rows can only run after their commits |
| All rows in this plan | plan created before the Windows app build with the new manifests exists | every case starts as `not_run` |
