# Windows startup diagnosis

Status: the startup failure path and the per-data-root WebView2 profile were
implemented and verified on a Windows 11 **development machine** on 2026-11-03.
This is not target-machine acceptance: the Windows 10 22H2 / RTX 3080 Ti /
i7-13700K machine still has to re-run W07, and the original 101 from the
2026-11-03 rebuild remains unexplained.

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

## Evidence

Machine-readable record: [startup-diagnosis.json](startup-diagnosis.json).

| Case | Setup | Result |
|---|---|---|
| W07-a | `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` points at an empty directory | `logs/startup-failure.log` written, dialog `Image Studio 启动失败` shown and dismissed, exit code `3` |
| W07-b | two simultaneous instances with different `--data-dir` roots | both stay running, each root created `cache/webview2/EBWebView`, the legacy shared profile was not touched |

The failure log contains the underlying Tauri message
(`Failed to setup app: runtime error: Could not find the webview runtime, make
sure it is installed`), a UTC timestamp, the log path and remediation steps.

## Reproduction attempts for the original 101

The 2026-11-03 note in
[windows-installer](../windows-installer/README.md) reported that a second
instance with a different `--data-dir` exited with code `101`. With the current
source on the development machine this was not reproduced: different roots at
0 ms, 500 ms and 25 s offsets, same-root launch, and a launch after force-killing
the first instance (leaving 12 orphaned `msedgewebview2.exe` processes) all kept
both hosts running.

The panic path itself is proven: any WebView2 setup failure ends in
`tauri-2.12.5/src/app.rs:1444` and exit code `101`. W07 therefore addresses the
observable defect (silent 101) rather than a confirmed trigger.

## Pending on the target machine

- Re-run W07 on Windows 10 22H2 with the installed NSIS build.
- Repeat the different-root second instance attempt and record whether the
  original 101 still appears; if it does, attach `logs/startup-failure.log`.
