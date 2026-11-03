# Windows startup diagnosis

Status: the startup failure path and the per-data-root WebView2 profile were
implemented and verified on a Windows 11 **development machine** on 2026-11-03,
then verified again on the Windows 10 22H2 / RTX 3080 Ti **target machine** with
the rebuilt NSIS package on 2026-11-03.

- Development machine: [startup-diagnosis.json](startup-diagnosis.json)
- Target machine (W07 acceptance plus the second-instance retest):
  [target-machine.json](target-machine.json)

## What changed

- `apps/desktop/src/startup.rs` installs a panic hook before Tauri builds the
  window. When Tauri fails its `setup` (`tauri-2.12.5/src/app.rs:1444`), the
  application now writes `<data root>/logs/startup-failure.log`, shows a native
  `Image Studio 启动失败` message box and exits with the documented code `3`
  instead of disappearing with the bare panic code `101`.
- The same module points `WEBVIEW2_USER_DATA_FOLDER` at
  `<data root>/cache/webview2` before the webview is created, so two instances
  with different `--data-dir` roots no longer share
  `%LOCALAPPDATA%\local.image-studio.foundation.dev\EBWebView`.
- `--help` documents the exit code and log path. Other panics keep the default
  hook, so existing crash behaviour is unchanged.

## Target machine acceptance (2026-11-03)

Package: `Image Studio_0.1.0_x64-setup.exe`, SHA-256 `9e5ecc40…a8ba2`,
`244,754,133` bytes, rebuilt from commit `376a3c3` and silently installed over
the previous current-user package. Installed executable SHA-256
`7270ba22…842c7`.

| Case | Setup | Result |
|---|---|---|
| W07-a | `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` points at an empty directory | The WebView2 runtime shows its own `Error` TaskDialog (`Could not find the WebView2 Runtime`, with the download link); after dismissing it the application writes `logs/startup-failure.log`, shows `Image Studio 启动失败` and exits with code `3` |
| W07-a2 | a regular file is placed at `<data root>/cache/webview2` so the profile directory cannot be created | The application reports `无法创建 WebView2 用户数据目录` in the log, shows the same native dialog and exits with code `3`; no loader dialog is involved |
| W07-b | two simultaneous instances with different `--data-dir` roots | both stay running, each root created `cache/webview2/EBWebView`, the legacy shared profile was not touched |

The application-level contract (log, dismissible native error dialog, exit code
`3`) therefore holds on the target machine. The only difference from the
development-machine record is the extra WebView2-runtime TaskDialog that
appears before the setup panic when the runtime itself is unavailable. W07-a2
covers the application path on its own.

## Reproduction attempts for the original 101

The 2026-11-03 note in
[windows-installer](../windows-installer/README.md) reported that a second
instance with a different `--data-dir` exited with code `101`.

| Machine | Executable | Offsets tried | Outcome |
|---|---|---|---|
| Windows 11 development | current source | 0 ms, 500 ms, 25 s, same root, launch after force-killing the first instance | not reproduced |
| Windows 10 target | pre-fix installed package (`2ccbadbf…1e37`) | 0 s, 2 s, 25 s | not reproduced |
| Windows 10 target | rebuilt package (`7270ba22…842c7`) | 0 s, 2 s, 25 s | not reproduced |

The panic path itself is proven: any WebView2 setup failure ends in
`tauri-2.12.5/src/app.rs:1444` and exit code `101`. W07 therefore addresses the
observable defect (silent 101) rather than a confirmed trigger. Both automated
runs start the first instance with an isolated root; the original manual record
used the default product root, which the handoff forbids automating.

## Current state

- W07 is verified on both the development and target machines.
- The different-root second-instance 101 was not reproduced on the target
  machine with either the pre-fix or the rebuilt package.
- Independent-root coexistence (W03) is covered by W07-b on the target machine.
