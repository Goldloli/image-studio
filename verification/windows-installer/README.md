# Windows internal installer

Status: the Windows x64 Tauri application built and produced an NSIS installer
on the target machine, 2026-11-01. It was rebuilt on 2026-11-02 after the
ModelScope URL and segmented-resume fixes, on 2026-11-03 after the
export-overwrite confirmation fix, and again on 2026-11-03 after the WebView2
startup-diagnosis change, then silently installed over the current-user package.
The installed WebView2 application ran the full installed-app UI acceptance (UI
install to ready, three real generations, PNG export, restart persistence,
task/works pages) with an isolated data root and closed gracefully; the rebuilt
package also passed the W07 target-machine startup-diagnosis acceptance.

## Package

- Installer: `target/release/bundle/nsis/Image Studio_0.1.0_x64-setup.exe`
- Size: `244,754,133` bytes
- SHA-256: `9e5ecc402aff21046612c056be28efbb4863bb11a6464549759ef2d9b69a8ba2`
- Install mode: current user
- WebView2: embedded offline installer
- VC runtime: bundled
- Sidecars: `inference-supervisor.exe` and `diagnostic-worker.exe`
- Signing: none; this is an internal unsigned build

The installer does not contain model weights or the private Python/CUDA
runtime. A normal first launch created only the four application roots. It did
not create the private runtime or model files.

A same-root second launch was rejected by the Windows named root mutex. One
normal window remained usable; the second window displayed the explicit
in-use error and disabled its mutating controls. During the 2026-11-03 rebuild,
a second simultaneous instance launched with a **different** `--data-dir`
exited with code 101. The target machine retested that scenario on 2026-11-03
with the pre-fix package and with the rebuilt package (offsets 0 s, 2 s and
25 s): both instances stayed running in every attempt, so the 101 was not
reproduced. Independent-root coexistence is verified for the current package by
[W07-b](../windows-startup-diagnosis/target-machine.json), and the previous
silent-exit class is now reported as a startup failure with a log, a native
dialog and exit code 3.

Silent uninstall removed the program directory while leaving the default data
root and SQLite/WAL bytes intact. Reinstalling restored the existing
cross-platform warning state and its saved Chinese prompt. A stale draft whose
save failed no longer blocks native window close.

## Evidence

- [Machine-readable package record](internal-package.json)
- [Install and launch smoke](install-smoke.json)
- [Latest update and launch smoke](install-update-smoke.json)
- [Installed-app UI acceptance](../windows-installed-app/README.md)
- [Target-machine startup diagnosis (W07)](../windows-startup-diagnosis/target-machine.json)

## Windows model-parity wiring (2026-11-04)

The installer now exposes **seven** Windows install profiles in a fixed order:
`z-image-turbo-diffusers-cuda-nf4-v1`, `flux2-klein-4b-distilled-windows-v1`,
`birefnet-general-epoch-244-windows-v1`, `realesrgan-x4plus-windows-v1`,
`qwen-image-2.1-windows-v1`, `qwen-image-2.1-turbo-windows-v1`,
`moss-image-sft-windows-v1`. The six parity models use the frozen design.md
section 11 model-variant / recipe / snapshot / runtime / worker identities; the
worker snapshot ids are recorded separately from the install ids because the
model manifests are keyed by model variant id.

- Shared v2 Python runtime: Klein, BiRefNet, Qwen base, Qwen Turbo and MOSS all
  install the identical runtime `windows-python31210-torch2100-cu126-diff041-transformers53-v2`.
  Both frozen worker wheels (`studio_diffusers_worker`, `studio_onnx_worker`)
  are embedded in the binary and installed together so the first model that
  installs the runtime cannot strand a later model. A completed runtime is
  reused (activation record bound to the runtime-manifest digest) and Windows
  rebuilds are staged and swapped in only after the wheels and probe succeed.
- Real-ESRGAN uses its own native runtime
  `realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1` and the
  `realesrgan-worker.exe` sidecar (added as an `externalBin` next to
  `inference-supervisor` and `diagnostic-worker`); the packaged sidecar is
  copied as `realesrgan-worker-<triple>.exe` and recorded in the build evidence.
- Staged Qwen/Qwen-Turbo/MOSS installs run the diffusers worker
  `--prepare` after the probe and record `data/models/<install_id>.prepared` in
  the ready record; generation derives the same path from the model directory
  for the staged recipes only.
- BiRefNet and Real-ESRGAN license texts cannot be downloaded from an
  allowlisted URL, so they are embedded in the binary and archived under
  `data/licenses/<install_id>/` before activation, with path/bytes/SHA-256 in
  the ready record and in the install space budget.
- Native runtimes are accepted only when `runtime.platform` matches the host
  (`windows-amd64` on Windows, `macos-universal2` on macOS).

Machine-readable wiring evidence: [windows-parity-wiring.json](windows-parity-wiring.json)
(generated by the `cfg(windows)` installer unit tests; real install and
generation acceptance is run separately).

## Remaining gates

The current test machine already has development tools and WebView2. A clean
Windows 10 test user, missing/offline WebView2 behavior, permission failure,
complete native IME/DPI interaction and network-denied generation remain
pending. Installed-package real generation and export are covered by the
installed-app acceptance above.
