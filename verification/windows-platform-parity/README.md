# Windows platform parity (supplementary machine)

Status: quality gates and desktop host behaviour were re-run on 2026-11-03 on a
second machine — **Windows 11 Enterprise 22631 / i9-9900K / RTX 2060 SUPER
8 GiB** — to check that the shared code behaves the same as on the Windows 10
target machine. This is a **supplementary** record: Windows 11 results cannot
replace the target-machine acceptance, and model installation, real generation
and resource budgets were **not** run here.

Machine-readable record: [platform-parity.json](platform-parity.json).

## What matched

| Check | Windows 10 / RTX 3080 Ti (target) | Windows 11 / RTX 2060 SUPER | Same result |
|---|---|---|---|
| Rust tests | 230 passed / 8 ignored / 17 suites | 234 passed / 8 ignored / 17 suites | yes (+4 new startup tests) |
| `cargo fmt --check`, `clippy -D warnings`, workspace build | passed | passed | yes |
| Frontend tests / production build | 6 files, 37 tests, built | 6 files, 37 tests, built | yes |

## What was verified on the supplementary machine

| Case | Result |
|---|---|
| Release build starts, creation page shows `Z-Image Turbo · 未就绪`, tasks and works show their empty states, model page lists one connected model, settings opens | passed |
| Theme switch is saved in `application-preferences.json` and restored after a restart | passed |
| Two instances with different `--data-dir` roots coexist, each creates `cache/webview2/EBWebView`, the legacy shared `%LOCALAPPDATA%` profile stays untouched | passed |
| Release build with an unusable WebView2 (injected `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER`) writes `logs/startup-failure.log`, shows `Image Studio 启动失败` and exits `3` | passed |

Note: when WebView2 itself cannot start, the runtime shows its own English
`Error` dialog before wry returns the setup error. Dismissing it leads to the
Chinese `Image Studio 启动失败` dialog with the log path; both were dismissed by
`scripts/windows-ui/answer-dialog.ps1` in this run.

## Why the model cannot be verified here

The frozen Windows recipe is bound to the 12 GiB target card:

- Gate 0 measured a **8.87 GiB** CUDA reserved peak (budget 10.5 GiB) on the
  RTX 3080 Ti. An 8 GiB card cannot hold that, and the change forbids shrinking
  resolution, steps or quantization to make it fit.
- The recipe uses **BF16 compute on sm86**; the RTX 2060 SUPER is **sm75
  (Turing)** and does not provide that path, so running it would mean an
  unregistered recipe.
- The production install is **35.56 GB** of model and runtime; this machine had
  63 GB free and a throttled link, so a real install was not attempted.

## Not covered here

Model installation, empty-cache download, real Z-Image generation and quality,
CUDA/RAM budgets, network-denied runs, kill recovery during sampling and the
macOS regression all remain with the target machine and a Mac.
