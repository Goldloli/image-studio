# Windows model-parity installed-app acceptance

Status: **passed with documented not-run/blocked rows** (2026-11-04), on the
NSIS-installed current-user application (`Image Studio_0.1.0_x64-setup.exe`,
244,990,805 B, SHA-256 `0c274ad0…` at build commit `888dcca` + the fixes listed
below) driven through its real WebView2 page on the target Windows 10 machine
(RTX 3080 Ti 12 GB / 32 GB RAM). The application used an isolated
`--data-dir C:\ist-a` root and a locally verified cache seed; every image below
came from the installed app, not from a helper process.

Machine-readable summary: [results.json](results.json) (10 jobs with frozen
plans, seeds, effective parameters, devices, runtimes, output content hashes and
export hashes), raw IPC snapshots in [raw/](raw/), lifecycle cases in
[lifecycle.json](lifecycle.json), blocked/not-run rows in
[not-run.json](not-run.json).

## Installation

The shared v2 runtime and the six parity models were installed from the
installed app's own installer UI/IPC after seeding `cache/downloads/<sha256>`
with locally verified artifacts (`scripts/windows-parity/seed-parity-cache.ps1`;
132 artifacts / 123 unique SHAs / 112,508,722,670 B / 0 missing / 0 hash
mismatch). Confirmed by the installed app:

| Model | Install id | Phase | Runtime | Notes |
|---|---|---|---|---|
| FLUX.2 Klein 4B distilled | `flux2-klein-4b-distilled-windows-v1` | ready | shared v2 | components 16 GB; NF4 component offload |
| BiRefNet epoch_244 | `birefnet-general-epoch-244-windows-v1` | ready | shared v2 | ONNX CPU; licenses archived |
| Real-ESRGAN x4plus | `realesrgan-x4plus-windows-v1` | ready | ncnn Vulkan | native runtime; licenses archived; ready across restart after fix |
| Qwen-Image 2.1 | `qwen-image-2.1-windows-v1` | ready | shared v2 | prepared NF4 cache + RAM preflight |
| Qwen-Image 2.1-Turbo | `qwen-image-2.1-turbo-windows-v1` | ready | shared v2 | prepared NF4 cache + RAM preflight |
| MOSS-Image-SFT | `moss-image-sft-windows-v1` | ready | shared v2 | prepared NF4 cache + RAM preflight; vendored pipeline |

Z-Image Windows was not reinstalled in this root (first-round evidence stands);
the app projected it as `not_installed`. Catalog counts after install: total 7,
installed 6, executable 6, hardwareEligible 7, planned 0.

Install evidence: [raw/installations/](raw/installations/) (ready records with
runtime inventories, probes, licenses, prepared-cache digests), cache seed
records `raw/seed-cache-ist-a.json` / `raw/seed-cache-root-a.json`.

## Real generation / processing through the native UI

Every capability was sent from the installed app's composer (model menu or
feature page), with the real worker supervisor underneath. Jobs are in
[results.json](results.json); exports are byte-identical to the committed
formal assets (export SHA-256 equals the asset content hash).

| Capability | Job | Result | Seed / size | Output | Export |
|---|---|---|---|---|---|
| Klein text-to-image | `job-a8d977…` | Succeeded | 1980923821 / 1024² | `b5965b3d…` | [export](artifacts/export-klein-t2i.png) |
| Klein single-image edit | `job-dcb27d…` | Succeeded | 1905335761 / 1024² | `245f8b1e…` | not exported separately (same PNG path) |
| BiRefNet remove background | `job-8e8369…` | Succeeded | source Klein 1024² | RGBA `3a5089bd…` + mask `d0348d86…` | [RGBA](artifacts/export-birefnet-primary.png), [mask](artifacts/export-birefnet-mask.png) |
| Real-ESRGAN 4× upscale | `job-59e568…` | Succeeded | 1024² → 4096² | `984b9045…` | [export](artifacts/export-realesrgan-4x.png) |
| Qwen-Image 2.1 | `job-689499…` | Succeeded | 1208863210 / 1024² | `7349ba3f…` | not exported separately (same PNG path) |
| Qwen-Image 2.1-Turbo | `job-a0122e…` | Succeeded | 3571548759 / 1024² | `18cac6f3…` | [export](artifacts/export-qwen-turbo.png) |
| MOSS-Image-SFT | `job-c5dca4…` | Succeeded | 2908961624 / 1024² | `b730b183…` | not exported separately (same PNG path) |

BiRefNet dual-output checks: exported primary is 1024×1024 RGBA (colour type 6),
mask is 1024×1024 8-bit grayscale (colour type 0); sampled alpha of the primary
equals the mask byte at 21,609 sampled pixels with **0 mismatches**. The works
library lists the primary as one formal work and does not count the mask.

Screenshots: [model page with 7 models](screenshots/model-page-seven.png),
[conversation with the seven results](screenshots/conversation-seven-results.png),
[works page](screenshots/works-page.png), [settings runtime rows](screenshots/settings-runtime.png).

Quality review sheet (4.4): [quality-samples.png](artifacts/quality-samples.png)
shows every primary output plus the BiRefNet RGBA/mask pair with labels. The
individual formal assets are copied next to it as `asset-*.png`; the exported
copies are `export-*.png`.

## Lifecycle

| Case | Result | Evidence |
|---|---|---|
| Cancel during generation | terminal `Cancelled`, no asset added, no candidate/attempt left behind, UI shows 本次生成已取消 | lifecycle.json |
| Hard-kill the supervisor during sampling | job `job-a137cb…` ends `Cancelled`, no asset, resources reclaimed (`生成已取消，资源已回收`), next jobs accepted | lifecycle.json, task-list/job-details |
| Cross-conversation mutex | while one conversation generates, a second conversation's send is disabled (`其他会话正在生成`), prompt preserved, no turn/job created | lifecycle.json |
| Restart recovery | close + relaunch with the same root: 6/6 models stay `ready` (including the native Real-ESRGAN runtime), 10 turns / 8 assets persist, Real-ESRGAN still ready | lifecycle.json |
| Corrupted download-cache artifact | corrupting the BiRefNet ONNX cache entry is detected, the bad file is removed, a fresh download starts and the install is not activated; existing data untouched; cancel cleans up | lifecycle.json |

## Bugs found and fixed during acceptance

1. **Native restart readiness** — the startup health check required a POSIX
   executable bit, so the Real-ESRGAN runtime became `unavailable` after the
   first restart. Fixed in `installation.rs` (Windows native executables do not
   need a POSIX mode bit); verified by the restart case above.
2. **ncnn install probe paths** — canonical Windows paths carry `\\?\`; the
   official ncnn build cannot open them (`_wfopen … .param failed`). Installer
   child-process arguments now use plain Win32 paths.
3. **Asset pixel budgets** — the asset gate knew only the macOS Real-ESRGAN /
   BiRefNet recipes, so a real 4096×4096 Windows upscale was rejected with
   “PNG 头部尺寸或像素格式不符合冻结计划”. The Windows recipes now share the
   16 MP upscale / 4 MP processing budgets.
4. **Generation-send allowlist** — `conversation_send` rejected all six parity
   models until the Windows model/capability pairs were added to the library
   allowlists (found by the unit suite and fixed before acceptance).
5. **Audit must-fixes** — install-time RAM preflight, prepared-cache digest
   binding and worker-wheel activation identity (M1–M3) plus the host-side
   hardware gate (M4) were implemented from the independent pre-acceptance
   audit before this run.

The earlier failed Real-ESRGAN job (`job-5792ea…`, error “PNG 头部尺寸或像素
格式不符合冻结计划”) is kept as the failure evidence for bug 3.

## Blocked / not run (not counted as passes)

- **Empty-cache install over the real network (I2-1)**: the shared-runtime wheel
  downloads stalled at the current PyPI throughput (~0.2–0.3 MB/s while
  PyTorch/GitHub hosts were 30–45 MB/s), so the empty-cache attempt was stopped
  before activation; recorded in
  [install-birefnet-empty-cache-network-blocked.json](install-birefnet-empty-cache-network-blocked.json).
  The seeded-cache installs above are **cache-hit** evidence, not a substitute.
- **Deep data-root path**: an artificially deep acceptance root made the runtime
  staging path approach the legacy MAX_PATH limit and the onnxruntime probe
  failed with “文件名或扩展名太长”; recorded in
  [install-failure-long-data-root.json](install-failure-long-data-root.json).
- **Space-shortage and missing-file regressions (I2-2/I2-3)** for the new models:
  the first-round mechanism is unchanged; this run exercised the corrupt-cache
  rejection only. No small spare volume was available to repeat the 27.74 GiB
  space-shortage case.
- **Network-denied install (I2-4)**: waived by the user on 2026-11-03
  (`verification/windows-network-denial-decision.json`); not run, not a pass.
- **macOS regression R02–R04**: user acceptance on the Mac side; not run here.
- **Human quality review (4.4)**: the seven sample images are exported/visible,
  but the visual quality sign-off is a user step.

## Quality gates

The final installer build ran the repository gates (frontend test/build, fmt,
clippy `-D warnings`, workspace tests) and passed; see
[verification/windows-parity-quality-gate.json](../windows-parity-quality-gate.json)
and `target/parity-gates/build-installer-final4.log`.
