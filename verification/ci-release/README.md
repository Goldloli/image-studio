# Cross-platform CI/release evidence

Status: **local hardening passed; real GitHub Actions and draft Release pending**
(2026-11-05).

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

The local Node runtime is `24.12.0`, while CI pins `24.19.0`; tests/build were
run through npm `11.17.0`, and the GitHub runner remains the authoritative Node
24.19.0 check.

## Pending real GitHub rows

1. Push implementation commits; quality runs on Windows and `macos-15` arm64.
2. Manually run `build-artifacts`; download and independently verify both
   artifacts and checksums.
3. Confirm Windows installer identity/bytes and macOS arm64/package identity
   from the downloaded files.
4. Push tag `v0.1.0`; verify the draft Release contains both artifacts,
   checksums, README, license/notice files and release notes.
5. Record run URLs, artifact digests and draft Release URL here.

These rows are not passes until the GitHub runs and downloaded artifacts have
been checked. Unsigned/notarization limitations are documented in
[doc/14-ci-release.md](../../doc/14-ci-release.md).
