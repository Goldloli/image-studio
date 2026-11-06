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

A turn was cancelled during sampling by clicking the composer stop control:

- click → `已取消` in **6.69 s** (previously ~120 s)
- no `inference-supervisor` or `python` process left behind

## Install projection

Unit test covers the phase that used to fall through: an installer snapshot with
`preparing_model` now projects as that phase with `busy = true` instead of `not_installed`
(`catalog::tests::unknown_installer_phase_never_projects_as_not_installed`).

## Download retry

Unit test drives a two-response server: the first connection truncates the body, the second
returns it; the artifact download retries and completes
(`installation::tests::transient_download_failure_is_retried_before_failing`). Digest mismatches,
explicit cancellation and an aborted install are not retried.

## Not yet verified on this machine

- A full install from scratch to watch the phase labels live (the unit test covers the mapping).
- A forced network failure during a real install (the retry path is covered by the test only).
- Twenty consecutive sends (four were measured; the loop used for the test had a broken polling
  predicate).
