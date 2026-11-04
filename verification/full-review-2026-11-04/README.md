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
| F1 | **blocker** | `apps/desktop/src/host.rs:642` matches `PlatformError::DataPathTooLong`, which is `#[cfg(windows)]` (`paths.rs:24-33`). `host` is compiled on every target and the macOS quality job runs `cargo test --workspace` → `E0599`, so the first macOS CI run cannot pass. | **open** (fix in follow-up change) |
| C1 | **blocker** | `quality.yml` ran clippy/tests **before** the frontend build. `tauri::generate_context!` embeds `apps/frontend/dist`; a clean checkout has no `dist`, so the desktop crate panics. Reproduced locally by hiding `dist` and running `cargo check -p image-studio-desktop` (`The frontendDist configuration is set to "../frontend/dist" but this path doesn't exist`). | fixed (`8d7c881`): frontend build before all cargo steps |
| C2 | **blocker** | `build.yml` macOS job ran `npm ci` but no frontend build before `cargo build --release --workspace` → same panic, no `.app`/zip. | fixed (`8d7c881`) |
| C3 | **blocker** | Artifact upload mixed repository-root files with `target/release/...`; upload-artifact uses the least common ancestor as the artifact root, so `merge-multiple` produced `artifacts/target/release/...` plus a directory, and `gh release create … artifacts/*` would fail after creating a partial draft. | fixed (`8d7c881`): flat `target/ci-artifacts/{windows,macos}` payload staging, release passes files only, plus checksum verification |
| F2 | **high** | The data-path guard budget (117 units) was derived from the 26-unit **staging** name, but activation renames it to `runtimes/<runtime.id>` (61 units for v2). A path just under the limit would install/probe from staging and then fail during real generation from the longer final path (`numpy` DLL full path ≈ 275 > 260). The deep-root acceptance at data 88 works, but the upper bound is unsafe. | **open** (follow-up change: budget from final runtime layout, max 92 units, plus canonical re-check) |
| F3 | medium | Guard validates `std::path::absolute` before canonicalization; a junction/short-name alias can canonicalize to a longer path and bypass the guard (verified locally: 61-unit junction path → 146-unit canonical path). | **open** (re-validate canonical data path) |
| F4 | medium | `cleanup_stale_runtime_staging` only matches names starting `.staging-`; the **old** format `.{runtime_id}.staging-{pid}-{nanos}` (the form that left 4.23 GB / 20,621 files in `target/windows-parity-accept/root-a`) is never matched, so "回收历史残留" is not true for old residue. The acceptance D4 seeded a new-format name and therefore under-tested this. | **open** (match both formats + old-name test) |
| F5 | medium | Cleanup runs inside `prepare_python_runtime` **after** the disk-space preflight (and downloads); stale staging can make the preflight fail before anything is cleaned. | **open** (run after the exclusive install lock, before preflight) |
| F6 | medium | Startup failure log path for the default data root: `host.rs` falls back to `.` (CWD) when `--data-dir` is absent, and ignores write errors; only `--data-dir` gets the root's `logs`. | **open** (derive root from the rejected data path) |
| C4 | high | Windows build artifact name hardcoded `Image Studio_0.1.0_x64-setup.exe`; a version bump would fail "installer missing" and block releases. | fixed (`8d7c881`): filename derived from `tauri.conf.json` |
| C5 | medium | Release artifacts are verified? The original workflow never verified checksums; Windows checksums would have been written with CRLF (Ubuntu `sha256sum -c` would fail). | fixed (`8d7c881`): LF checksums + `sha256sum -c` before draft |
| C6 | medium | Version/tag check ran only after both 90-minute builds. | fixed (`8d7c881`): `verify-version` job runs first |
| C7 | low | Quality timeout 45 min is tight for a cold Windows tauri build. | fixed (`8d7c881`): 60 min |
| F7 | low | Cleanup warnings are only `eprintln!` in a windows-subsystem GUI; non-UTF-8 names / `entries.flatten()` are silently skipped; check-then-delete has a local TOCTOU window. `activate_staged_runtime` delete-then-rename is pre-existing. | open; document only |
| F8 | low/medium | Cross-platform **tool** drafts (mac `birefnet-general-epoch-244` / `realesrgan-x4plus` on Windows) are not covered by the new advice helper: `activeTextRecipe` is null, so no warning is shown and the label falls back to "Z-Image Turbo · 未就绪". Same class as the fixed text-model issue. | open (bd, separate small fix) |
| C8 | medium | Third-party actions use mutable major-version tags, not commit SHAs. Private repo with no secrets in v1; documented as follow-up. | accepted for v1 (bd follow-up) |
| C9 | low | Docs: README Node/npm versions were stale; the workflow validation script referenced by the evidence lived only under `target/` and was not reproducible. | fixed (`8d7c881`): README aligned, `scripts/ci/validate-workflows.py` committed |
| F9 | info | Data-path guard now rejects paths above 117 though the short staging name means some 118–137 paths might work; the follow-up recomputes a correct upper bound (92) instead of the current incorrect 117. | folded into F2 |
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
