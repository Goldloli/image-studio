# Windows internal installer

Status: the Windows x64 Tauri application built and produced an NSIS installer
on the target machine, 2026-11-01. It was rebuilt on 2026-11-02 after the
ModelScope URL and segmented-resume fixes, and again on 2026-11-03 after the
export-overwrite confirmation fix, then silently installed over the
current-user package. The installed WebView2 application ran the full
installed-app UI acceptance (UI install to ready, three real generations, PNG
export, restart persistence, task/works pages) with an isolated data root and
closed gracefully.

## Package

- Installer: `target/release/bundle/nsis/Image Studio_0.1.0_x64-setup.exe`
- Size: `244,745,354` bytes
- SHA-256: `56837792e2110a963fcf003ef591f7b45c3aad6544aeaabf0418eec7c4c5f047`
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
exited with code 101; independent-root coexistence needs re-verification
(likely WebView2 user-data-folder contention) and is recorded as an open
question, not as a pass.

Silent uninstall removed the program directory while leaving the default data
root and SQLite/WAL bytes intact. Reinstalling restored the existing
cross-platform warning state and its saved Chinese prompt. A stale draft whose
save failed no longer blocks native window close.

## Evidence

- [Machine-readable package record](internal-package.json)
- [Install and launch smoke](install-smoke.json)
- [Latest update and launch smoke](install-update-smoke.json)
- [Installed-app UI acceptance](../windows-installed-app/README.md)

## Remaining gates

The current test machine already has development tools and WebView2. A clean
Windows 10 test user, missing/offline WebView2 behavior, permission failure,
complete native IME/DPI interaction and network-denied generation remain
pending. Installed-package real generation and export are covered by the
installed-app acceptance above.
