# Windows UI projection evidence (parity tasks 3.3 / 4.1)

Scope: host + webview projection for the seven Windows models and eight capabilities — catalog
metadata, feature page, model page hardware/license state, composer gating and settings runtime
rows. This is **unit/type-level evidence**; the installed-app screenshots and the real seven-model
send/process acceptance (parity tasks 4.2) are recorded as `not_run` below because no installed
package exists in this round yet.

Change: `openspec/changes/achieve-windows-model-parity` (tasks 3.3, 4.1). Task: task-7.
Machine: Windows 10 22H2 target machine (RTX 3080 Ti 12 GiB · driver 596.49 · 32 GB RAM).

## What was verified

| Claim | Evidence |
|---|---|
| Catalog registers 7 implemented Windows models, 0 planned | `ui-projection.json` `catalog`, `resources/windows-foundation/model-catalog.json` |
| Exactly 8 (model, capability) pairs; Klein has text + edit | `capability_projection.pairs` |
| All seven Windows feature entries are implemented with `open-composer` | `feature_catalog` (4 feature entries; 7 model variants × capabilities = 8 pairs) |
| Model page fields include `minVramGb`, `minRamGb`, `quantization`, `licenseNotice`, `windowsStatus`, `hardwareEligible`, `hardwareReason` | `model_page_field_shape` |
| Hardware gate blocks with an actionable Chinese reason on an 8 GB / 16 GB device; CPU route (BiRefNet) and small ncnn route (Real-ESRGAN) stay eligible | `hardware_gate.low_8gb_16gb`; Rust test `unsatisfied_hardware_disables_entry_points_with_a_chinese_reason` |
| Unknown device facts never silently pass a required threshold | `hardware_gate.unknown_device`; Rust tests `unknown_device_facts_are_not_silently_accepted`, `unknown_device_facts_disable_required_thresholds_instead_of_passing` |
| Certified target device satisfies every registered threshold | `device-probe.log` (vram 12 / ram 32); Rust test `windows_catalog_registers_every_parity_identity_with_metadata` |
| Qwen base/Turbo show the Qwen Research License (non-commercial); others show their license text | `license_projection`; frontend test `license projection` |
| Settings reports v1 / shared-v2 / ncnn runtime lock rows and per-model hardware status | `runtime_rows`; Rust tests `windows_runtime_lock_matrix_is_pinned_for_settings_rows`, `settings_snapshot_reports_the_current_platform_runtime` |
| Composer disables sending when `hardwareEligible=false` and shows the reason | frontend tests in `apps/frontend/src/model-catalog.test.ts` (`hardware gate projection`) and `Workspace.tsx` gating (`selectedModelExecutable = ready && hardwareEligible`, model menu disables blocked entries) |

## Hardware threshold table (design.md §10.2)

| Model | minVramGb | minRamGb | quantization |
|---|---|---|---|
| Z-Image Turbo | 12 | 32 | nf4 |
| FLUX.2 Klein 4B | 12 | 32 | nf4 |
| Qwen-Image 2.1 | 12 | 32 | nf4 |
| Qwen-Image 2.1 Turbo | 12 | 32 | nf4 |
| MOSS-Image-SFT | 12 | 32 | nf4 |
| BiRefNet epoch_244 | 0 | 16 | float32 (CPU) |
| Real-ESRGAN x4plus | 4 | 8 | fp32 (ncnn) |

On an 8 GB / 16 GB device the expected blocked set is the five staged/v1 GPU models; BiRefNet and
Real-ESRGAN remain eligible. The real target machine probes as 12 GB / 32 GB and passes everything.

## Model page JSON projection shape

`catalog.rs::project_entry` adds these fields next to the existing ones (the macOS catalog keeps
parsing because the fields are optional there):

```
minVramGb, minRamGb, quantization, licenseNotice, windowsStatus,
hardwareEligible, hardwareReason,
canInstall / canContinue / canUse  (now require hardwareEligible)
```

`ModelCatalog::query(installations, installation_error, device)` additionally returns
`counts.hardwareEligible`. Settings `environment` adds `deviceFacts`, `deviceVramGb`,
`deviceRamGb`; each `environment.runtimes[]` row adds `label`, `runtimeLockId`,
`hardwareEligible`, `hardwareReason`, `minVramGb`, `minRamGb`.

## Commands and results

```
npm --prefix apps/frontend test          -> 7 files / 52 tests passed
npm --prefix apps/frontend run check     -> tsc --noEmit exit 0
cargo test -p image-studio-desktop       -> 161 passed / 0 failed / 7 ignored (after
                                            desktop-install committed the seven Windows install
                                            profiles and the v2 execution wiring; this task's
                                            projection tests are part of that run)
cargo test -p image-studio-desktop --lib studio::                 -> 18 passed / 0 failed
cargo test -p image-studio-desktop --lib adapters::catalog         -> 7 passed / 0 failed
cargo test -p image-studio-desktop --lib adapters::feature_catalog -> 4 passed / 0 failed
cargo test -p image-studio-desktop --lib adapters::device          -> 9 passed / 0 failed
cargo clippy -p image-studio-desktop --all-targets -- -D warnings  -> exit 0, no warnings
cargo fmt --all -- --check                                        -> exit 0
```

Frontend new/changed tests: `model-recipes.test.ts` (Windows identity matrix, 7 models / 8 pairs,
frozen capability ids, historical macOS ids untouched), `model-catalog.test.ts` (device-eligible
filter, hardware gate disabling install/send including a stale-payload case, CPU route, license
projection), plus the existing conversation/workspace suites kept green.

Logs: `npm-test.log`, `npm-check.log`, `cargo-test.log`, `cargo-clippy.log`, `targeted-tests.log`,
`device-probe.log`, `nvidia-smi.log`.

## Not run (requires the installed app)

- Native UI screenshots of the model page / feature page / settings page: `not_run` — the app
  cannot be launched from this task's scope and the installer profiles are landing in parallel.
- Real seven-model send/process/view/export in the installed app (parity task 4.2): `not_run` —
  depends on the installed package with worker wheels and the six Windows install profiles.
- Real screenshot of a hardware-blocked model on a sub-threshold machine: `not_run` — the gate is
  covered by unit tests with injected device facts (`8 GB / 16 GB`, `None / 0`).

Integrity note: `not_run` items are not claimed as passes. All other rows cite a real test or a
committed resource file that was re-read when this JSON was generated.
