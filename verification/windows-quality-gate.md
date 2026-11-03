# Windows quality gate (R01)

Status: passed on the target Windows 10 machine, 2026-11-03, commit `376a3c3`.
This re-run is at the commit that adds the WebView2 startup-diagnosis module
(`apps/desktop/src/startup.rs`: panic hook, per-data-root WebView2 profile) and
its four unit tests, so the workspace suite is now 234 tests.

| Check | Result | Log |
|---|---|---|
| `cargo fmt --all -- --check` | passed | [fmt](windows-r01-fmt.log) |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | passed | [clippy](windows-r01-clippy.log) |
| `cargo test --workspace --locked` | 234 passed, 0 failed, 8 ignored, 1 doc test (17 suites) | [rust tests](windows-r01-rust-tests.log) |
| `cargo build --workspace --examples --bins --locked` | passed | [build](windows-r01-build.log) |
| `npm --prefix apps/frontend run build` | passed (2057 modules, 503 ms) | [frontend build](windows-r01-frontend-build.log) |
| `npm --prefix apps/frontend test` | 37 passed / 6 files | [frontend tests](windows-r01-frontend-tests.log) |
| `npm --prefix apps/frontend audit --omit=dev --audit-level=moderate` | not applicable: registry mirror lacks the advisory endpoint | [audit](windows-r01-frontend-audit.log) |
| `scripts/verify-generation-lock.py` | passed without network | [lock audit](windows-r01-lock-audit.log) |
| `scripts/verify-planned-model-locks.py` | passed without network (6 locks) | [lock audit](windows-r01-lock-audit.log) |
| `openspec validate introduce-windows-foundation --strict --no-interactive` | valid | [openspec](windows-r01-openspec.log) |

Toolchain: Rust `1.99.1` (fixed by `rust-toolchain.toml`), Node `24.19.0`/npm `11.17.0`
(fixed by `.nvmrc` and `engines`), Rust target `x86_64-pc-windows-msvc`.
Lock digests and machine facts are in [windows-quality-gate.json](windows-quality-gate.json).

The `sha2` `asm` feature remains gated to macOS targets, so the Windows
workspace resolves the non-assembly backend and builds with MSVC.

The eight ignored tests are explicit acceptance paths. The Windows product
acceptance reports are linked from the JSON and from
[Windows Gate 0](windows-gate0/README.md).

## Limitations

- `npm audit` cannot report advisories through the configured registry mirror;
  no vulnerability conclusion is claimed.
- The two Python lock-audit scripts are Mac-oriented. They were made
  Windows-checkout safe (explicit UTF-8 reads, CRLF-normalized worker source
  comparison, archived metadata path) and pass; their checks do not imply
  Mac runtime or model execution.
- This gate is Windows-only. Mac regression (R02-R04), complete native
  UI/IME/DPI, clean-user/WebView2 and network-denied generation remain pending.
