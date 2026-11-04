# Full review — 2026-11-04 (before any push)

Scope: the complete Windows parity + data-root + cross-platform warning + CI/release
work on branch `main` (HEAD `8d7c881`), reviewed by:

1. a full local quality-gate run,
2. an independent read-only Rust review of the data-root commits (`42764a5`, `d97b5d4`),
3. an independent read-only CI/release workflow review.

No files were edited by the reviewers. This report records findings with severity,
status and evidence; fixes for the critical items are prepared as the follow-up
change `openspec/changes/fix-windows-data-root-review-findings` (waiting for user
confirmation before code changes, per the repository OpenSpec flow).

## Local gates (current HEAD)

| Gate | Result |
|---|---|
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | pass |
| `cargo test --workspace --locked` | pass after the flake fix below (183 desktop tests, 7 ignored; all suites) |
| `npm --prefix apps/frontend test` | 57 pass |
| `npm run check` / `npm run build` | pass |
| `openspec validate --all --strict` | 24/24 |

The supervisor `progress_flood_…` fixture failed twice when the whole workspace
suite ran in parallel ("unexpected flood result Error"). It was caused by the
fixture's 1.5s/2.5s timeouts under CPU contention, not by product code; fixed in
`a40974c` (non-timeout fixtures use 5s/10s/10s) and the full suite is green now.

## Blockers

| ID | Severity | Finding | Status |
|---|---|---|---|
| F1 | **blocker** | `host.rs` matched a Windows-only `PlatformError` variant unconditionally, so the macOS build/CI could not compile. | fixed (`732fac9`): import and branch are `cfg(windows)`-gated; the log now comes from the rejected data path. macOS compile itself remains user-gated (first CI run). |
| C1 | **blocker** | `quality.yml` ran clippy/tests **before** the frontend build. `tauri::generate_context!` embeds `apps/frontend/dist`; a clean checkout has no `dist`, so the desktop crate panics. Reproduced locally by hiding `dist` and running `cargo check -p image-studio-desktop` (`The frontendDist configuration is set to "../frontend/dist" but this path doesn't exist`). | fixed (`8d7c881`): frontend build before all cargo steps |
| C2 | **blocker** | `build.yml` macOS job ran `npm ci` but no frontend build before `cargo build --release --workspace` → same panic, no `.app`/zip. | fixed (`8d7c881`) |
| C3 | **blocker** | Artifact upload mixed repository-root files with `target/release/...`; upload-artifact uses the least common ancestor as the artifact root, so `merge-multiple` produced `artifacts/target/release/...` plus a directory, and `gh release create … artifacts/*` would fail after creating a partial draft. | fixed (`8d7c881`): flat `target/ci-artifacts/{windows,macos}` payload staging, release passes files only, plus checksum verification |
| F2 | **high** | The guard budget came from the staging name, not the activated runtime; mid-implementation testing showed that even a corrected 92-unit budget was unsafe because Python opens a 130-unit `.py` path (real data=88 generation failed with `FileNotFoundError` on a 262-character path). | fixed (`e5150e5`, `e4b6e46`): activated runtimes use `runtimes/rt-<12 hex>` (15 units) with legacy migration, and the guard is 94 units derived from the longest Python-openable path + canonical data re-check. Verified: data=88 and data=94 real generations succeed; data=127 exits 3. |
| F3 | medium | Guard validated a pre-canonical path (junction/8.3 aliases could bypass it). | fixed (`e4b6e46`): canonical data path re-checked with the `\\?\` prefix stripped; unit test covers over-long aliases. |
| F4 | medium | Cleanup only matched new-format `.staging-*`; old-format residue (4.23 GB on disk) was never reclaimed. | fixed (`e446074`): matcher accepts both generations; the real 4.23 GB root-a residue was removed on an install start. Unit test covers both names and non-matching dirs. |
| F5 | medium | Cleanup ran after the disk-space preflight. | fixed (`e446074`): moved into `install()` right after the exclusive lock, before the preflight/downloads; verified on root-a. |
| F6 | medium | Default-root startup failure wrote its log to CWD. | fixed (`732fac9`): `<root>/logs/startup-failure.log` is derived from the rejected data path. |
| C4 | high | Windows build artifact name hardcoded `Image Studio_0.1.0_x64-setup.exe`; a version bump would fail "installer missing" and block releases. | fixed (`8d7c881`): filename derived from `tauri.conf.json` |
| C5 | medium | Release artifacts are verified? The original workflow never verified checksums; Windows checksums would have been written with CRLF (Ubuntu `sha256sum -c` would fail). | fixed (`8d7c881`): LF checksums + `sha256sum -c` before draft |
| C6 | medium | Version/tag check ran only after both 90-minute builds. | fixed (`8d7c881`): `verify-version` job runs first |
| C7 | low | Quality timeout 45 min is tight for a cold Windows tauri build. | fixed (`8d7c881`): 60 min |
| F7 | low | Cleanup warnings are only `eprintln!` in a windows-subsystem GUI; non-UTF-8 names / `entries.flatten()` are silently skipped; check-then-delete has a local TOCTOU window. `activate_staged_runtime` delete-then-rename is pre-existing. | open; document only |
| F8 | low/medium | Cross-platform **tool** drafts (mac BiRefNet/Real-ESRGAN ids on Windows) had no advice and the label fell back to "Z-Image Turbo · 未就绪". | fixed (`527a7ec`): `crossPlatformToolModelAdvice` names the Windows/macOS BiRefNet 去背景 / Real-ESRGAN 放大 target, the combined helper is used by the composer and foreign tool drafts label as `<model> · 当前平台未接入`. Installed-app check: warning shown, label `BiRefNet epoch_244 · 去背景 · 当前平台未接入`, send disabled; native Windows BiRefNet shows no warning. Evidence: verification/cross-platform-model-selection/f8/results.json. |
| C8 | medium | Third-party actions use mutable major-version tags, not commit SHAs. Private repo with no secrets in v1; documented as follow-up. | accepted for v1 (bd follow-up) |
| C9 | low | Docs: README Node/npm versions were stale; the workflow validation script referenced by the evidence lived only under `target/` and was not reproducible. | fixed (`8d7c881`): README aligned, `scripts/ci/validate-workflows.py` committed |
| F9 | info | The 117-upward limit was arbitrary; the final bound is 94 with a short activated runtime directory. | folded into F2 (fixed in `e4b6e46`) |
| F10 | info | Supervisor test flake (workspace parallel load). | fixed (`a40974c`) |

## Evidence

- Local reproduction of C1: hiding `apps/frontend/dist` → `cargo check -p image-studio-desktop` fails with the `frontendDist` panic; restored afterwards.
- Local Windows CI payload simulation: `target/ci-artifacts/windows/` contains the version-derived installer, `SHA256SUMS-windows.txt` (LF) and `internal-package.json`; checksum matches `internal-package.json` (`63bdbf24…`, 245,017,051 B).
- Rust review commands: `cargo test -p inference-supervisor --test windows_supervision` repeated; workspace run before/after the timeout fix.
- Old residue confirmed on disk: `target/windows-parity-accept/root-a/data/runtimes/.windows-python31210-…-v2.staging-27684-…` = 4,326.9 MB.

## Recommended next steps

1. Confirm and implement `fix-windows-data-root-review-findings` (F1–F6): macOS compile gate, correct final-path guard budget + canonical re-check, legacy staging cleanup + early cleanup, startup log path; rebuild installer and refresh the data-root evidence with a deep-root **generation** and an old-format residue test.
2. Fix the cross-platform tool-model advice (F8) as a separate small change or together, then re-run gates.
3. Re-check the CI workflow blockers are gone, then (only with explicit authorization) push and run the first Actions pass; a macOS run can then confirm F1 in CI.

## Fixes landed (2026-11-04)

Commits: `732fac9` (non-Windows startup cfg + log path), `e972da7`/`e4b6e46`
(data-path budget + canonical re-check), `e5150e5` (short activated runtime dir +
legacy migration), `e446074` (legacy staging cleanup before the preflight).

Acceptance evidence: [review-fix/results.json](../windows-data-root-guard/review-fix/results.json) —
installer `3492a8c4…`, installed app `72f38b61…`; real generation succeeds at data
paths 88 and 94, data=127 exits 3 with the log under the rejected root, the real
4.23 GB legacy staging residue was removed at install start, and `C:\ist-a` legacy
runtime directories migrated to `rt-…` with 6/6 models ready. Unit tests: 188
desktop tests passed. F7 (cleanup observability/TOCTOU) stays documented; F8
(cross-platform tool-model advice) remains bd `image-studio-5w0`; action SHA
pinning remains bd `image-studio-06k`.
