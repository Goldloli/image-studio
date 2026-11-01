# Windows internal installer

Status: the Windows x64 Tauri application built and produced an NSIS installer
on the target machine, 2026-11-01. The installer completed a silent current-user
installation and the installed WebView2 application started with an isolated
data root.

## Package

- Installer: `target/release/bundle/nsis/Image Studio_0.1.0_x64-setup.exe`
- Size: `244,750,015` bytes
- SHA-256: `d86db18ad8d35741087bc8c517257b4ddd0604daf5479996429ba1d208aaf99f`
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
in-use error and disabled its mutating controls.

Silent uninstall removed the program directory while leaving the default data
root and SQLite/WAL bytes intact. Reinstalling restored the existing
cross-platform warning state. A stale draft whose save failed no longer blocks
native window close.

## Evidence

- [Machine-readable package record](internal-package.json)
- [Install and launch smoke](install-smoke.json)

## Remaining gates

The current test machine already has development tools and WebView2. A clean
Windows 10 test user, missing/offline WebView2 behavior, permission failure,
installed-package real generation, complete native IME/DPI interaction and
network-denied generation remain pending.
