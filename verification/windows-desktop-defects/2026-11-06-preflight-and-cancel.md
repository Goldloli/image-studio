# Windows desktop defect fixes — measured evidence (2026-11-06)

Machine: Windows 11 Enterprise 22631 / i9-9900K / RTX 2060 SUPER 8 GB (sm75) / 31.9 GiB RAM.
Build: `cargo build --release -p image-studio-desktop`, isolated data root `D:\ist-8gb\appdata`,
driven through WebView2 CDP (`scripts/windows-ui/cdp.mjs`).

## Root cause of the send hang

`ready_paths()` hashed the whole installation (model 30 GB + runtime 4 GB + prepared cache 10 GB)
before every send. A warm app still needed minutes, so the turn sat in `pending` and the new
watchdog (or the user) gave up. Stage traces taken from the app's stderr:

```
[preflight] start   turn-…           ← thread spawned
[preflight] gate ok turn-…           ← hardware/tier check (fast)
[preflight] geometry ok 768x768      ← canvas
[preflight] ready paths begin        ← BLOCKED HERE (35 GB of SHA-256)
```

Fix: the send path verifies existence and sizes only, caches the result until the next install,
and the full digest verification runs once when an install activates.

## After the fix

Sends issued back-to-back through the UI; each preflight is the full `prepare_turn` path
(verification, environment, freeze, bind):

| Send | Preflight | Result |
|---|---|---|
| 1 (cold, after restart) | 4.54 s | bound, generated, `图片已生成 100%` |
| 2 (immediately after) | 46 ms | bound, generated |
| 3 | 46 ms | bound, generated |
| 4 | 60 ms | bound, generated |

No `生成、清理或模型安装仍在进行` rejection, no turn stranded in `正在接收提示词`.

## Cancel latency

Cancelled through the composer stop control on three models:

| Model | Stage at cancel | Click → `已取消` | Leftover processes |
|---|---|---|---|
| Qwen-Image 2.1 Turbo | sampling | 6.69 s | none |
| MOSS-Image-SFT | sampling | 6.38 s | none |
| MOSS-Image-SFT | loading | 6.68 s | none |
| FLUX.2 Klein 4B | sampling | 5.13 s | none |

Previously the same action took about 120 s.

## Install projection

Unit test covers the phase that used to fall through: an installer snapshot with
`preparing_model` now projects as that phase with `busy = true` instead of 
ot_installed`
(`catalog::tests::unknown_installer_phase_never_projects_as_not_installed`).

## Download retry

Unit test drives a two-response server: the first connection truncates the body, the second
returns it; the artifact download retries and completes
(`installation::tests::transient_download_failure_is_retried_before_failing`). Digest mismatches,
explicit cancellation and an aborted install are not retried.

## Sequential sends

Five sends issued one after another, each waiting for the image:

| Send | Preflight | Wall clock |
|---|---|---|
| 1 | 4.48 s (cold) | 115.2 s |
| 2 | 0.10 s | 110.7 s |
| 3 | 0.12 s | 110.3 s |
| 4 | 0.11 s | 105.8 s |
| 5 | 0.14 s | 105.5 s |

Every send reached `图片已生成`; none stalled, none was rejected for a busy controller.

## Console window on every generation

Observed live: generations created new `conhost` / `OpenConsole` / `WindowsTerminal` processes
(child processes were spawned without `CREATE_NO_WINDOW`). After the fix, a generation runs
`inference-supervisor` and `python` with **no window handle** (`MainWindowHandle = 0`, empty
title) and still produces the image. Screenshot: 
o-console-generation.png`.

## Live install (BiRefNet, 972 MB model + shared runtime)

The row moved through the phases the page now projects:

- `正在下载` captured while bytes were arriving (`install-downloading.png`)
- backend phases observed: `downloading` → `verifying` → `probing` → `ready`
- finished `当前可执行` (`install-ready.png`)

## Install phases seen live (beyond BiRefNet)

MOSS-Image-SFT (27 GB) and FLUX.2 Klein 4B (16 GB) were installed from a seeded cache while the
model page was open:

- `downloading` → `verifying` → `probing` → `preparing_model` (MOSS, ≈12 min) → `ready`
- `downloading` → `verifying` → `probing` → `ready` (Klein, 9.2 min total)
- the row read `正在下载` during the download, never `尚未安装`

## Not verified on this machine

- A forced network failure during a real install. Injecting one needs a firewall rule or a hosts
  entry, both of which require administrator rights this session does not have. The retry runs
  through the real download function in
  `installation::tests::transient_download_failure_is_retried_before_failing`, which serves a
  truncated body once and then the full body over a real socket.
