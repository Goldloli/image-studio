# Cross-platform CI/release evidence

Status: **local and real GitHub Actions acceptance passed; draft Release awaits
manual review** (2026-11-05).

Change: `openspec/changes/introduce-ci-release-pipeline`. Workflows:
`.github/workflows/{quality,build,release}.yml`; operator guide:
[doc/14-ci-release.md](../../doc/14-ci-release.md).

## Current local evidence

| Item | Result | Evidence |
|---|---|---|
| Rust fmt / Clippy `-D warnings` / workspace tests | PASS | [local-hardening-2026-11-05.json](local-hardening-2026-11-05.json) |
| Default parallel workspace tests | PASS, repeated 5 times | same evidence file |
| Frontend test / check / build | PASS, 58 tests | same evidence file |
| npm audit high threshold | PASS, 0 vulnerabilities | same evidence file |
| OpenSpec strict | PASS, 24/24 | same evidence file |
| actionlint / workflow structure | PASS | same evidence file |
| zizmor | 0 high/medium; low/help only | same evidence file |
| macOS runner contract | `macos-15` arm64 + runtime assertion | workflow |
| macOS release bundle | arm64, stable identifier, 15 zip entries | same evidence file |
| macOS archive | 32,457,641 B, SHA-256 `9f275384…` | same evidence file |
| Windows installer command | previously PASS; new SHA-256 download change awaits Windows runner | [local-validation.json](local-validation.json) |
| Final main quality | PASS on Windows + macOS arm64 | [github-actions-acceptance-2026-11-05.json](github-actions-acceptance-2026-11-05.json) |
| Manual dual-platform build | PASS; both artifacts downloaded and rechecked | same evidence file |
| `v0.1.0` draft Release | PASS; 10 assets, checksums and docs present | same evidence file |

The local Node runtime is `24.12.0`, while CI pins `24.19.0`; tests/build were
run through npm `11.17.0`, and the GitHub runner remains the authoritative Node
24.19.0 check.

## Real GitHub rows

1. Final `main` quality: PASS, Windows and macOS arm64.
2. Manual `build-artifacts`: PASS, both artifacts independently downloaded and
   checksum-verified.
3. macOS downloaded package: stable identifier, arm64 core/sidecars and legal
   resources verified.
4. `v0.1.0` tag: version/main provenance check passed.
5. Draft Release: PASS with 10 assets; manual publish remains a user decision.

Unsigned/notarization limitations are documented in
[doc/14-ci-release.md](../../doc/14-ci-release.md).
