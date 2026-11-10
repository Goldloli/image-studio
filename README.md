# Image Studio

**English** | [简体中文](README.zh-CN.md)

[![Quality](https://github.com/Goldloli/image-studio/actions/workflows/quality.yml/badge.svg)](https://github.com/Goldloli/image-studio/actions/workflows/quality.yml)
[![Latest release](https://img.shields.io/github/v/release/Goldloli/image-studio)](https://github.com/Goldloli/image-studio/releases/latest)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Image Studio is a local-first desktop image creation studio for macOS arm64 and
Windows x64, built with Rust, Tauri 2, and React. It provides text-to-image,
instruction-based image editing, background removal with RGBA/mask output, and
exact 4× upscaling while keeping model execution and project data on the local
machine.

![Image Studio generation workspace](verification/windows-installed-app/screenshots/generation-conversation.png)

## Latest Release: v0.1.1

The current release adds an 8 GB VRAM execution tier, a low-memory install path,
and a round of Windows desktop reliability fixes on top of the first public
build:

- **8 GB compact tier** — Qwen-Image 2.1 / Turbo, MOSS-Image-SFT, and FLUX.2
  Klein 4B (text-to-image and edit) run at 768×768 with `float16` on an RTX 2060
  SUPER 8 GB (Turing sm75); measured CUDA reserved peaks are 5.44–5.96 GiB.
  Z-Image Turbo stays gated at 12 GB.
- **Low-memory install-time quantization** — NF4 preparation now streams ~1 GiB
  tensor shards through short-lived child processes (worker commit peak
  1.95–3.6 GiB), so the install gate drops to available RAM ≥ 8 GiB and commit
  headroom ≥ 6 GiB. 16 GiB machines can install Qwen and MOSS.
- **Windows desktop fixes** — send preflight no longer re-hashes the install
  (60 s+ → ~46 ms), a 60 s turn watchdog bounds stranded sends, cancellation
  settles in 3–7 s, downloads retry per file and repair corrupted parts, no
  console windows appear, and `<data>/logs/app.log` keeps a bounded structured
  log.
- **Verified on real machines** — Windows 11 / RTX 2060 SUPER 8 GB for the
  compact tier and Windows 10 22H2 / RTX 3080 Ti 12 GB for the standard tier,
  with GitHub Actions quality green on Windows x64 and macOS arm64.

See the [latest release notes](https://github.com/Goldloli/image-studio/releases/latest)
for the complete change list and known limitations.

| Model page with the compact tier | Works library |
|---|---|
| ![Compact tier on the model page](verification/windows-8gb-tier/model-compact-tier.png) | ![Works library](verification/windows-installed-app/screenshots/works-library.png) |

## Highlights

- **Seven certified models / eight capabilities** on both macOS and Windows.
- **Two execution tiers**: a standard tier (≥ 12 GB VRAM, 1024², `bfloat16`) and
  a compact tier (≥ 8 GB VRAM, 768², `float16`) selected automatically from the
  device facts.
- **Local execution**: no hosted image API or cloud project storage.
- **Conversation workspace**: drafts, prompts, parameters, progress, results,
  retries, and cancellation remain grouped by conversation.
- **Deterministic model identities**: pinned revisions, per-file SHA-256,
  fixed runtimes, recipes, and worker protocol identities.
- **Durable local library**: SQLite-backed drafts, jobs, attempts, assets, task
  history, and provenance.
- **Strict process supervision**: bounded worker output, cancellation,
  descendant cleanup, execution leases, and crash recovery.
- **Cross-platform releases**: Windows NSIS and macOS arm64 `.app` artifacts
  are built and verified by GitHub Actions.

## Supported Models

| Model | Capabilities | Fixed recipe |
|---|---|---|
| Z-Image Turbo | Text to image | 4-bit loading, 9 steps, 1:1 / 4:3 / 3:4 |
| FLUX.2 Klein 4B distilled | Text to image, single-image edit | 4-bit loading, 4 steps, guidance 1.0 |
| BiRefNet general epoch_244 | Remove background, RGBA + mask | ONNX Runtime CPU, float32 |
| Real-ESRGAN x4plus | Exact 4× upscale | ncnn Vulkan, tile 128 |
| Qwen-Image 2.1 | Text to image | 4-bit NF4, 40 steps, guidance 1.0 |
| Qwen-Image 2.1 Turbo | Text to image | 4-bit NF4, 8 steps, official Turbo sampling |
| MOSS-Image-SFT | Text to image | 4-bit NF4, 50 steps, guidance 3.0 |

### Execution tiers

| Tier | VRAM | Canvas | Compute | Load strategy |
|---|---|---|---|---|
| `standard` | ≥ 12 GB | 1024² | `bfloat16` | staged / component offload |
| `compact` | ≥ 8 GB | 768² | `float16` | staged, one NF4 component resident at a time |

Both tiers share one 25–33 GB model download and one prepared NF4 cache; only
the recipe identity and its exact parameter map differ. A device that meets only
the compact thresholds degrades automatically and the model page states
"compact 768×768". Devices below both tiers keep the model disabled with the
lowest unmet requirement as the reason. BiRefNet and Real-ESRGAN were never
VRAM-bound.

**8 GB support (verified on an RTX 2060 SUPER, Turing sm75)**: Qwen-Image 2.1,
Qwen-Image 2.1 Turbo, MOSS-Image-SFT, and FLUX.2 Klein 4B (text-to-image and
edit) run in the compact tier with measured CUDA reserved peaks of 5.44–5.96
GiB. Z-Image Turbo keeps its 12 GB threshold because the model overflows in
`float16` on BF16-less Turing architectures.

**Performance on the 8 GB reference card (compact 768×768)**: every job starts a
fresh worker, so the total includes component loading.

| Model | Steps | Total | Sampling |
|---|---|---|---|
| Qwen-Image 2.1 Turbo | 8 | 84.5 s | 66.3 s |
| Qwen-Image 2.1 | 40 | 391.3 s | 358.1 s |
| MOSS-Image-SFT | 50 | 752.1 s | 733.8 s |
| FLUX.2 Klein 4B text-to-image | 4 | 145.6 s | 9.8 s |
| FLUX.2 Klein 4B edit | 4 | 166.1 s | 10.3 s |

On the Windows 10 / RTX 3080 Ti 12 GB target machine the standard tier measured
Klein 50.1 s, Z-Image Turbo 55.1 s, BiRefNet 25.6 s, Real-ESRGAN 8.6 s, Klein
edit 46.6 s, and single Qwen-Image 2.1 / MOSS-Image-SFT generations of 84 s /
143 s. Install-time quantization on that machine runs at a worker commit peak of
~2–3 GiB. Evidence:
[`verification/windows-8gb-tier/`](verification/windows-8gb-tier/README.md),
[`verification/windows-12gb-standard-tier/`](verification/windows-12gb-standard-tier/README.md),
[`doc/15-windows-8gb-tier.md`](doc/15-windows-8gb-tier.md), and
[`doc/16-zimage-quantization-investigation.md`](doc/16-zimage-quantization-investigation.md).

The authoritative model catalog is
[`resources/model-catalog/catalog.json`](resources/model-catalog/catalog.json).
Windows uses separately frozen NF4/ONNX/ncnn profiles; macOS uses the verified
MFLUX/MLX and ONNX runtimes.

## System Requirements

- **Windows**: Windows 10 22H2 or Windows 11, x64. The current-user NSIS package
  embeds the WebView2 offline installer. Model eligibility depends on the GPU
  (≥ 8 GB VRAM for the compact tier, ≥ 12 GB VRAM for the standard tier); the
  install-time quantization path needs ≥ 8 GiB available physical RAM and
  ≥ 6 GiB commit headroom, and 16 GiB RAM or more is recommended.
- **macOS**: Apple Silicon (arm64), macOS 11 or newer. Model execution uses the
  verified MFLUX/MLX and ONNX runtimes; the model page reports device
  eligibility.
- **Disk**: each generative model downloads 25–33 GB and keeps a prepared NF4
  cache. Reserve at least 40 GB per generative model and roughly 150 GB for the
  full model set. Model weights and private Python runtimes are not bundled.

## Download

Use the latest published [GitHub Release](https://github.com/Goldloli/image-studio/releases/latest)
and verify the downloaded file against its `SHA256SUMS-*.txt`:

- **Windows x64**: `Image.Studio_<version>_x64-setup.exe`
- **macOS arm64**: `Image-Studio-<version>-macos-arm64.zip`

```bash
sha256sum -c SHA256SUMS-windows.txt
sha256sum -c SHA256SUMS-macos.txt
```

The release is unsigned and not notarized:

- Windows may show SmartScreen. Verify the checksum, then use
  **More info → Run anyway** only if the digest matches.
- macOS may block first launch in Gatekeeper. Verify the checksum, then
  Control-click the app and choose **Open**, or run:

```bash
xattr -dr com.apple.quarantine "Image Studio.app"
```

macOS builds require Apple Silicon and macOS 11 or newer.

## Product Areas

- **Create**: multi-conversation image generation, source-image workflows,
  immediate placeholders, real worker progress, cancellation, and retry.
- **Inspiration**: a searchable prompt library with source attribution.
- **Features**: only capabilities backed by an installed, executable model
  are exposed as startable.
- **Models**: installation, runtime, device eligibility, hardware thresholds,
  execution tier, and license information from the host catalog.
- **Works**: immutable formal assets across conversations, filters, preview,
  provenance, and native PNG/RGBA/mask export.
- **Tasks**: host-side filtering, search, stable pagination, attempt history,
  errors, stages, logs, and recovery instructions.
- **Settings**: persistent appearance, explicit new-conversation defaults,
  storage inspection, diagnostic status, runtime inventory, and a bounded log.

## Build From Source

Prerequisites:

- Rust **1.99.1** from [`rust-toolchain.toml`](rust-toolchain.toml)
- Node.js **24.19.0** and npm **11.17.0**
- Python **3.11 or newer** for repository verification scripts; **3.13 recommended**. Model workers use their own pinned private Python runtimes, not the system interpreter.
- Tauri system prerequisites for the target platform

```bash
npm --prefix apps/frontend ci
npm --prefix apps/frontend run build
cargo build --workspace --locked
cargo run -p image-studio-desktop --locked
```

### macOS development bundle

```bash
python3 scripts/prepare-macos-dev-app.py
open "target/dev/Image Studio.app"
```

### Windows sidecars

`cargo clippy`, tests, and Tauri builds require the Windows sidecars to be
present in `apps/desktop/binaries/`. The CI workflow and
[`scripts/build-windows-internal.ps1`](scripts/build-windows-internal.ps1)
construct and copy them automatically.

### Frontend-only preview

```bash
npm --prefix apps/frontend run dev
```

Browser preview does not provide native installation, local generation,
persistence, or process supervision. Rebuild the frontend and desktop app after
UI changes to refresh the embedded production assets.

## Local-First Architecture

```text
React UI
  ↓ restricted Tauri IPC
Desktop host / StudioService
  ├─ exact model installation and integrity checks
  ├─ SQLite library, drafts, jobs, attempts, and assets
  ├─ generation controller and hardware gates
  └─ inference supervisor
       ↓ bounded JSON protocol
     Python / ONNX / native workers
```

| Workspace area | Responsibility |
|---|---|
| `apps/desktop` | Tauri host, constrained IPC, installers, storage, diagnostics, and process adapters |
| `apps/frontend` | React workspace, libraries, settings, task views, and canvas |
| `crates/studio-core` | Typed identities, plans, recipes, attempt state, and validation |
| `crates/worker-protocol` | Framing, handshakes, identity checks, and generation protocol |
| `tools/inference-supervisor` | Host EOF, execution leases, worker/descendant supervision |
| `tools/*worker*` | MFLUX, ONNX, Diffusers/CUDA, and Real-ESRGAN execution |

The core and protocol crates do not depend on the GUI or inference frameworks.
Worker stdout carries bounded protocol frames; stderr carries bounded logs.
Product processes are launched with argument arrays and never through a shell.

## Local Data and Safety

Default paths:

- macOS data: `~/Library/Application Support/ImageStudio`
- macOS cache: `~/Library/Caches/ImageStudio`
- macOS logs: `~/Library/Logs/ImageStudio`
- Windows root: `%LOCALAPPDATA%\ImageStudio` with `data`, `cache`, `logs`, and
  `work` subdirectories
- Application log: `<data>/logs/app.log` (bounded, 8 MiB rotation)

Use `--data-dir PATH` for an isolated root (`data`, `cache`, `logs`, and `work`
are created under it). One host owns a data root at a time. On Windows the data
path must stay short enough for the private Python runtime to open its longest
files, and the host validates that budget before starting.

SQLite schema v5 stores projects, conversations, draft revisions, turns, jobs,
attempts, formal assets, and provenance. Model installation and worker
activation bind fixed manifests, runtime inventories, and SHA-256 contents.

Image generation succeeds only after output validation and an atomic database
commit. Restart marks unfinished work as interrupted; retry creates a new
Attempt only after explicit user action. Missing or externally modified assets
remain visible as unavailable without deleting their historical records.

## Diagnostics and Verification

Development builds can open **Development → Communication diagnostics** or run:

```bash
cargo run -p image-studio-desktop --locked -- --diagnostics
```

Diagnostics validate the constrained worker channel only. They do not prove GPU,
model, or image-generation success.

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm --prefix apps/frontend test
npm --prefix apps/frontend run check
npm --prefix apps/frontend run build
openspec validate --all --strict
python3 scripts/ci/validate-workflows.py
actionlint -no-color
```

Real model acceptance evidence is stored in [`verification/`](verification/):

- Windows compact tier: [`verification/windows-8gb-tier/`](verification/windows-8gb-tier/README.md)
- Windows standard tier and Windows 10 target machine: [`verification/windows-12gb-standard-tier/`](verification/windows-12gb-standard-tier/README.md)
- Windows desktop coverage and defects: [`verification/windows-desktop-coverage/`](verification/windows-desktop-coverage/2026-11-06-coverage-results.md), [`verification/windows-desktop-defects/`](verification/windows-desktop-defects/2026-11-06-preflight-and-cancel.md)
- Installed Windows all-model acceptance: [`verification/windows-parity-app/`](verification/windows-parity-app/README.md)
- macOS seven-model evidence: [`verification/qwen-moss-20261101/moss-stage-report.md`](verification/qwen-moss-20261101/moss-stage-report.md)

## CI and Releases

The public repository uses three pinned GitHub Actions workflows:

- [quality.yml](.github/workflows/quality.yml): Windows x64 and macOS arm64
  formatting, Clippy, workspace tests, frontend tests/build, and OpenSpec.
- [build.yml](.github/workflows/build.yml): Windows NSIS and macOS arm64 app
  artifacts with checksums and package identity checks.
- [release.yml](.github/workflows/release.yml): version/main provenance checks
  and a draft GitHub Release.

Pushing a `v*` tag whose commit is on `main`, with the tag, workspace Cargo,
Tauri, and frontend versions all equal, builds both platforms and stages a
draft release that the maintainer reviews before publishing. The published
`v0.1.1` release contains the Windows x64 NSIS installer and the macOS arm64
`.app` zip. See [CI and release documentation](doc/14-ci-release.md) and
[`verification/ci-release/`](verification/ci-release/) for run IDs and artifact
digests.

## Contributing and Security

- Report vulnerabilities through [SECURITY.md](SECURITY.md), not a public issue.
- Functional changes follow the OpenSpec and Beads workflows used by the project.
- Keep model/runtime identities pinned; do not substitute a different snapshot
  under an existing install id.

## License

Project source and original distributable assets are licensed under
[Apache-2.0](LICENSE). Third-party libraries, fonts, example prompts/images, and
model materials retain their own licenses. See [NOTICE](NOTICE) and
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
