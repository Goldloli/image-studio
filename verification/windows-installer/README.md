# Windows internal installer

Status: the Windows x64 Tauri application built and produced an NSIS installer
on the target machine, 2026-11-01. The installer completed a silent current-user
installation and the installed WebView2 application started with an isolated
data root.

## Package

- Installer: `target/release/bundle/nsis/Image Studio_0.1.0_x64-setup.exe`
- Size: `244,749,495` bytes
- SHA-256: `085328ab46aed390f22bbc88487552eaed25356315afdab3096e3765f7b91d8e`
- Install mode: current user
- WebView2: embedded offline installer
- VC runtime: bundled
- Sidecars: `inference-supervisor.exe` and `diagnostic-worker.exe`
- Signing: none; this is an internal unsigned build

The installer does not contain model weights or the private Python/CUDA
runtime. A normal first launch created only the four application roots. It did
not create the private runtime or model files.

## Evidence

- [Machine-readable package record](internal-package.json)
- [Install and launch smoke](install-smoke.json)

## Remaining gates

The current test machine already has development tools and WebView2. A clean
Windows 10 test user, missing/offline WebView2 behavior, permission failure,
upgrade/uninstall data retention, installed-package real generation, complete
native IME/DPI interaction and network-denied generation remain pending.
