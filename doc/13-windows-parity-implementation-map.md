# Windows model-parity implementation map

Status: research notes for the confirmed OpenSpec change
`openspec/changes/achieve-windows-model-parity` (2026-11-03). This is an
implementation map, not a spec; the change documents stay authoritative.
Legend: **[V]** verified by reading the cited file.

## 1. Identity map

`PROTOCOL_VERSION = 1` / `PROTOCOL_VERSION_V2 = 2`
(`crates/worker-protocol/src/generation.rs:13-23`). The host chooses v1 only for
`Recipe::ZImageTurbo` (`apps/desktop/src/adapters/generation.rs:372`), so every
new Windows model must use **generation v2**.

| Model | macOS worker / build | Windows runtime | Windows worker |
|---|---|---|---|
| Z-Image Turbo | `studio_mflux_worker` 0.1.0, v1 | `windows-python31210-torch2100-cu126-bnb0482-v1` (shipped) | `studio_zimage_cuda_worker` |
| FLUX.2 Klein 4B distilled | `studio_mflux_worker_v2` 0.2.0, v2 | `windows-python31210-torch2100-cu126-diff041-transformers53-v2` | new diffusers worker |
| BiRefNet epoch_244 | `studio_onnx_worker` 0.1.0, v2 | v2 runtime + onnxruntime CPU | new ONNX worker |
| Real-ESRGAN x4plus | `realesrgan-native-worker` 0.1.0, v2 | `realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1` | Rust ncnn worker |
| Qwen-Image 2.1 | `studio_mflux_worker_v3` 0.3.0, v2 | v2 + staged NF4 | new diffusers worker |
| Qwen-Image 2.1-Turbo | same | v2 + staged NF4 | new diffusers worker |
| MOSS-Image-SFT | same | v2 + staged NF4/INT8 | new diffusers worker |

macOS capability ids: `crates/studio-core/src/recipes.rs:18-30`. Z-Image Windows
already uses its own model id and capability id
(`z-image-turbo-diffusers-cuda-nf4-v1`); the six new Windows models must follow
that precedent instead of reusing the macOS ids, otherwise
`Recipe::from_request` (`recipes.rs:145-166`) resolves the macOS recipe and freezes
q4/q8 parameter maps a Windows NF4 worker rejects. Historical macOS ids stay
untouched (spec: 不修改历史 ID).

## 2. Code change map

### Protocol / recipes (tasks 2.1, 3.1) - do first, unblocks all workers

- `crates/worker-protocol/src/generation.rs:253-425` `ExecuteRequestV2::validate`
  is a closed allowlist of `(model_id, capability_id, recipe_id)` plus exact
  parameter maps. Add the six Windows arms + `windows_*_parameters()` helpers.
- `crates/studio-core/src/recipes.rs`: new `Recipe` variants, Windows consts,
  `from_request`, `id()`, `accepts_output`, `max_output_bytes`,
  `validate_request`, `validate_plan`, `freeze_*`, `Timeouts` per model.
- `resources/windows-foundation/model-catalog.json` currently reuses macOS ids
  with `installId: null`; switch to Windows ids once recipes exist.

### Workers

- **Diffusers worker** (Klein, Qwen base/Turbo, MOSS): new
  `tools/diffusers-worker-windows/studio_diffusers_worker/` forked from
  `tools/zimage-cuda-worker/` (v2 JSONL framing `__main__.py:238,279,326`).
  Needs `--probe` returning `python`, `identity.build_id`,
  `identity.runtime_lock_id`, `identity.model_snapshot_id`, `device`,
  `device_name` (`apps/desktop/src/adapters/installation.rs:1322-1329`).
  Staged executor: `staged.py` = text encode -> release -> DiT sample ->
  release -> tiled VAE decode, hard budget failure on overrun.
- **ONNX worker** (BiRefNet): copy `tools/onnx-worker/` (backend is pure
  numpy/Pillow/onnxruntime, no MLX); new Windows identity and runtime lock id.
- **Real-ESRGAN ncnn worker**: windows build of
  `tools/realesrgan-worker/src/main.rs`; runtime/snapshot/ncnn SHA constants at
  `:20-28` are macOS-specific; add the binary to `externalBin` in
  `tauri.windows.conf.json:26-29`; native runtime platform gate
  `installation.rs:695-702` currently only accepts `macos-universal2`.
- Worker-module allowlist `generation.rs:307-315` must accept the new modules;
  the supervisor already supports v2 (`tools/inference-supervisor/src/windows.rs:159-170`).

### Installer / catalog (tasks 3.2-3.4)

- Manifests are compile-time embedded: `apps/desktop/src/adapters/installation.rs:37-90`,
  `catalog.rs:8-10`, `feature_catalog.rs:7-9`; adding a model means a new
  `InstallProfile` and `Installations` field.
- Manifest validation: model `component_bytes` must equal the installed file
  bytes without `README.md` (`installation.rs:683-694`); runtime Python manifests
  must have no archives (`:706-708`), native runtimes need `kind:"native"` +
  `platform` + `executable`; URL allowlist at `:2194-2217`.
- `Installations::snapshots()` currently returns only the Z-Image profile
  (`:399-414`); `for_id()` (`:474-492`) and `start()` (`:830-833`) also need the
  new profiles.
- Shared v2 runtime hazard: `prepare_python_runtime` does `remove_dir_all` then
  re-extract (`:1235-1241`), so a cancelled rebuild strands every other v2
  model; extract to a staging dir + rename, or key runtimes per model.
- `ReadyRecord.matches_profile` compares the runtime manifest digest
  (`:505-520`); all v2 models must reference the identical manifest constant.
- Space budget formula `model*2 + runtime*7 + 4 GiB` (`:715`, recomputed `*6` at
  `:1104-1106`) must include the prepared quantized cache once staging is used.
- Regeneration scripts must be updated in the same change or they silently
  revert parity: `tools/windows-gate0/build_windows_catalog.py:47-57` forces
  entries 2-7 to `planned`, `build_product_manifests.py:59-95` rewrites the
  runtime manifest.
- Catalog structs use `deny_unknown_fields` (`catalog.rs:12-50`,
  `feature_catalog.rs:11-41`): add `min_vram_gb`, `min_ram_gb`,
  `quantization`, `license_notice`, `windows_status` to the structs and JSON
  together.
- No VRAM/RAM probe exists today: `deviceVerified` means the install probe
  passed, not that the device meets the model threshold. Add a `DeviceFacts`
  source (nvidia-smi total + GlobalMemoryStatusEx) and a `hardwareEligible`
  projection.

### UI (tasks 4.1-4.3)

- Rust carriers: `ModelCatalog::query` / `project_entry`
  (`catalog.rs:151-264`), `ModelView` (`apps/desktop/src/studio.rs:333`),
  `SettingsSnapshot.environment.runtimes` (`studio.rs:423-531`),
  `FeatureCatalogEntry.state` (`feature_catalog.rs:119-193`).
- Frontend: `apps/frontend/src/model-catalog.ts` (ModelEntry),
  `ModelLibrary.tsx`, `Workspace.tsx:428` (send gating),
  `model-recipes.ts:30-85`, `SettingsPage.tsx:311-320`.
- "Seven models / eight capabilities" = 7 model ids x capabilities: Z-Image
  t2i, Klein t2i, Klein edit, BiRefNet, Real-ESRGAN, Qwen, Turbo, MOSS.

## 3. Suggested implementation order

1. Protocol allowlist + Windows recipe identities (with unit tests).
2. Frozen v2 runtime manifest + installer wiring (shared runtime; fix the
   atomic-runtime hazard at the same time).
3. BiRefNet ONNX worker first (portable backend) - proves v2 runtime, probe and
   dual-output commit on Windows.
4. Klein diffusers worker (t2i + single-image edit) - proves v2 inputs/assets.
5. Real-ESRGAN ncnn: native-runtime platform gate, externalBin, SHA constants.
6. Staged quantized executor for Qwen/Qwen-Turbo/MOSS (measured Gate 0 design:
   see `verification/windows-parity-gate0/staged-loader-*.md`).
7. Catalog/UI projection with hardware gate and license notice.
8. Evidence, docs, archive.

## 4. Open decisions carried into design

- V2 runtime identity must pin bitsandbytes 0.50.2 (Gate 0 measured with it) and
  the full transitive wheel set frozen by `runtime-freeze` (task 1.2).
- Qwen base/Turbo and MOSS text encoders are only ever loaded quantized on CPU
  and then moved to the card; the bf16 quantization transient belongs to an
  install "prepare" step with a RAM preflight, not to generation.
- MOSS custom pipeline derives compute dtype from `weight.dtype`; bitsandbytes
  reports storage dtype there, so the product needs the documented dtype
  compatibility shim (`tools/windows-parity-staged/moss_bnb_shim.py`) or a
  bf16 layer-offload path. Record the decision with evidence under task 1.4.
