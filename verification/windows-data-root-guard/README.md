# Windows data-root path guard acceptance

Status: **passed** (2026-11-04), on the installed NSIS build `58406c92…`
(installed app `e1cd5fb3…`). Source commits: `42764a5` (startup path guard) and
`d97b5d4` (short staging name + stale staging cleanup).

Machine: Windows 10 22H2, RTX 3080 Ti 12 GB, 32 GB RAM, i7-13700K.

## Results

| Case | Result | Evidence |
|---|---|---|
| Deep data root that previously failed (`target/windows-data-root-accept/deep-root-b`; root 83 / data 88 UTF-16 units) | **PASS** — shared v2 runtime installed, BiRefNet probe ready/executable, no `文件名或扩展名太长`, no staging leftover | [results.json](results.json), [ready record](deep-root-birefnet-ready-record.json), [seed](deep-root-seed.json) |
| Stale `.staging-*` cleanup | **PASS** — the legacy 4.23 GB-style staging directory was removed before the new install; only the formal runtime directory remained | results.json (`staging_dirs_remaining=0`) |
| Oversized data root (`C:\ist-root-x…x`, data path 127 > 117 limit) | **PASS** — exit code **3**, Chinese `startup-failure.log` with short-directory suggestion, no `data/` or `runtimes/` created | [results.json](results.json), [startup-failure.log](oversize-startup-failure.log), [dialog screenshot](oversize-dialog.png) |
| Short-root regression (`C:\ist-a`) | **PASS** — 6/6 existing models stay ready and a real Klein text-to-image succeeds | results.json (`short_root_regression`) |
| Unit tests | **PASS** — path boundary/UTF-16 surrogate tests, staging-name length/uniqueness, stale cleanup boundaries | `cargo test -p image-studio-desktop` |

Key measurements from the deep-root runtime layout after the fix:

- longest **load-relevant** path: `Lib\site-packages\onnxruntime\capi\onnxruntime_pybind11_state.pyd`
  full path 225 units (was 252 at the failed staging path before the fix);
- longest extracted file: torch header at 297 units (Rust extracts via `\\?\`;
  never loaded by Python);
- staging name is now `.staging-<hexpid>-<8hexnonce>` (≤ 26 units) instead of
  `.<60-char runtime id>.staging-<pid>-<nanos>` (~95 units).

## Boundaries

- The guard rejects data paths above 117 UTF-16 units with an actionable error;
  it is not a general long-path implementation (no registry `LongPathsEnabled`
  dependency, no `\\?\` injection for third-party Python/onnxruntime).
- macOS path behaviour is unchanged.
