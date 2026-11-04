# Windows data-root path guard acceptance

Status: **superseded by the review-fix round** (2026-11-04). The original acceptance below
was on build `58406c92…`; a later independent review showed the 117-unit budget was unsafe
(final runtime directory 61 units, real generation failed) and the legacy staging test used a
new-format name. The corrected acceptance is in
[review-fix/](review-fix/README.md): short activated runtime directories (`rt-<12 hex>`),
94-unit budget derived from the longest **Python-openable** path, canonical data-only
re-check, legacy staging cleanup before the space preflight, and a real data=88/94 generation.
Build: installer `3492a8c4…`, installed app `72f38b61…`; commits `732fac9`, `e972da7`,
`e446074`, `e5150e5`, `e4b6e46`.

Machine: Windows 10 22H2, RTX 3080 Ti 12 GB, 32 GB RAM, i7-13700K.

## Results (original round — retained for history)

| Case | Result | Evidence |
|---|---|---|
| Deep data root that previously failed (`target/windows-data-root-accept/deep-root-b`; root 83 / data 88 UTF-16 units) | **PASS** — shared v2 runtime installed, BiRefNet probe ready/executable, no `文件名或扩展名太长`, no staging leftover | [results.json](results.json), [ready record](deep-root-birefnet-ready-record.json), [seed](deep-root-seed.json) |
| Stale `.staging-*` cleanup | **PASS for the new format only** — the review later found the old-format residue was not matched; corrected in [review-fix/](review-fix/README.md) | results.json (`staging_dirs_remaining=0`) |
| Oversized data root (`C:\ist-root-x…x`, data path 127 > 117 limit) | **PASS** — exit code **3**, Chinese `startup-failure.log` with short-directory suggestion, no `data/` or `runtimes/` created | [results.json](results.json), [startup-failure.log](oversize-startup-failure.log), [dialog screenshot](oversize-dialog.png) |
| Short-root regression (`C:\ist-a`) | **PASS** — 6/6 existing models stay ready and a real Klein text-to-image succeeds | results.json (`short_root_regression`) |
| Unit tests | **PASS** — path boundary/UTF-16 surrogate tests, staging-name length/uniqueness, stale cleanup boundaries | `cargo test -p image-studio-desktop` |

Key measurements from the deep-root runtime layout after the original fix:

- longest **load-relevant** path: `Lib\site-packages\onnxruntime\capi\onnxruntime_pybind11_state.pyd`
  full path 225 units (was 252 at the failed staging path before the fix);
- longest extracted file: torch header at 297 units (Rust extracts via `\\?\`;
  never loaded by Python);
- staging name was shortened to `.staging-<hexpid>-<8hexnonce>` (≤ 26 units) instead of
  `.<60-char runtime id>.staging-<pid>-<nanos>` (~95 units).

## Boundaries

- The original 117-unit limit was wrong; the corrected limit is 94 UTF-16 units with the
  short activated runtime directory. See [review-fix/results.json](review-fix/results.json).
  This is not a general long-path implementation (no registry `LongPathsEnabled`
  dependency, no `\\?\` injection for third-party Python/onnxruntime).
- macOS path behaviour is unchanged.
