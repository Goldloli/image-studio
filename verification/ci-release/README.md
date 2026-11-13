# Cross-platform CI/release evidence

Status: **local and real GitHub Actions acceptance passed; `v0.2.2` is
publicly published** (2026-11-13). `v0.2.1`, `v0.2.0`, `v0.1.1` and `v0.1.0`
remain previous public releases.

Change: `openspec/changes/archive/2026-11-05-introduce-ci-release-pipeline`.
Workflows:
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
| Final main quality after dependency upgrades | PASS on Windows + macOS arm64 | [github-actions-acceptance-2026-11-05.json](github-actions-acceptance-2026-11-05.json) |
| Manual post-upgrade dual-platform build | PASS; both artifacts downloaded and rechecked | same evidence file |
| Dependabot PRs | PASS; 7/7 merged, 0 open PRs | same evidence file |
| `v0.1.0` public Release | PASS; 10 assets, checksums and bilingual notes present | same evidence file |
| `v0.1.1` quality on the release commit | PASS on Windows + macOS arm64 (run 37) | [v0.1.1/acceptance.json](v0.1.1/acceptance.json) |
| `v0.1.1` release workflow | PASS; version provenance, both platform builds, draft release | same `v0.1.1` record |
| `v0.1.1` public Release | PASS; 10 assets, both platform packages, checksums re-verified after download | [v0.1.1/README.md](v0.1.1/README.md) |
| `v0.2.0` quality on the release commit | PASS on Windows + macOS arm64 (run 43242113193) | [v0.2.0/acceptance.json](v0.2.0/acceptance.json) |
| `v0.2.0` release workflow | PASS; one macOS runner-capacity retry, then both packages and draft release | same `v0.2.0` record |
| `v0.2.0` public Release | PASS; 10 assets, checksums and package identities re-verified after download | [v0.2.0/README.md](v0.2.0/README.md) |
| `v0.2.1` quality on the release commit | PASS on Windows + macOS arm64 after one runner-capacity retry (run 43260424768) | [v0.2.1/acceptance.json](v0.2.1/acceptance.json) |
| `v0.2.1` release workflow | PASS; dependency maintenance build, one macOS runner-capacity retry, then draft release | same `v0.2.1` record |
| `v0.2.1` public Release | PASS; 10 assets, checksums, package identities and startup smoke re-verified after download | [v0.2.1/README.md](v0.2.1/README.md) |
| `v0.2.2` quality on the release commit | PASS on Windows + macOS arm64 (run 43376989947) | [v0.2.2/acceptance.json](v0.2.2/acceptance.json) |
| `v0.2.2` release workflow | PASS; macOS build produced and independently mounted a DMG before draft creation | same `v0.2.2` record |
| `v0.2.2` public Release | PASS; 10 assets, exactly one DMG and zero macOS zip, checksums and startup smoke re-verified | [v0.2.2/README.md](v0.2.2/README.md) |

The local Node runtime is `24.12.0`, while CI pins `24.19.0`; tests/build were
run through npm `11.17.0`, and the GitHub runner remains the authoritative Node
24.19.0 check.

## Real GitHub rows

1. Final `main` quality: PASS, Windows and macOS arm64, including the merged
   action/runtime dependency updates.
2. Manual `build-artifacts`: PASS, both artifacts independently downloaded and
   checksum-verified.
3. macOS downloaded package: stable identifier, macOS 11 minimum, arm64
   core/sidecars and legal resources verified.
4. `v0.1.0` tag: version/main provenance check passed.
5. Public Release: PASS with 10 assets and bilingual install/release notes:
   <https://github.com/Goldloli/image-studio/releases/tag/v0.1.0>.
6. `v0.1.1` tag: version/main provenance check passed on `069c6e2`; the
   Windows installer, macOS arm64 `.app` zip and draft release jobs all
   succeeded (release run 3).
7. Public `v0.1.1` Release: PASS with 10 assets; the Windows NSIS installer
   (`245,148,864 B`) and macOS arm64 zip (`32,516,275 B`) checksums were
   re-verified after download, the installer SHA-256 matches the CI
   `internal-package.json`, and the release body matches the rendered
   bilingual notes:
   <https://github.com/Goldloli/image-studio/releases/tag/v0.1.1>.
8. `v0.1.1` acceptance archive: [v0.1.1/](v0.1.1/) with the machine-readable
   record, the CI-built Windows package evidence and both published
   checksum files.
9. `v0.2.0` tag: version/main provenance passed on `bc50fd3`; quality
   `43242113193` passed on both platforms. Release `43243134087` initially
   waited 15m02s for a macOS runner without executing a step, then a failed-job
   rerun completed macOS in 6m50s and created the draft.
10. Public `v0.2.0` Release: PASS with 10 assets. The Windows installer
    (`245,195,809 B`) and macOS arm64 zip (`32,566,832 B`) were downloaded again,
    both checksums passed, the Windows internal package hash/bytes matched, and
    the macOS bundle reported identifier, version `0.2.0`, macOS 11 minimum and
    four arm64 executables:
    <https://github.com/Goldloli/image-studio/releases/tag/v0.2.0>.
11. `v0.2.0` acceptance archive: [v0.2.0/](v0.2.0/) with the machine-readable
    record, CI-built Windows package evidence and both published checksum files.
12. `v0.2.1` maintenance tag: quality `43260424768` and release
    `43264815859` ultimately passed on both platforms after macOS jobs that
    never acquired runners were rerun. The Windows installer and macOS zip were
    rebuilt from the updated dependency set.
13. Public `v0.2.1` Release: PASS with 10 assets. The Windows installer
    (`245,189,562 B`) and macOS arm64 zip (`32,570,985 B`) passed published
    checksums; package identity, arm64 sidecars, release notes and a downloaded
    app startup smoke were rechecked:
    <https://github.com/Goldloli/image-studio/releases/tag/v0.2.1>.
14. `v0.2.1` acceptance archive: [v0.2.1/](v0.2.1/) with the maintenance
    dependency record, both published checksum files and Windows package
    evidence.
15. `v0.2.2` tag: quality `43376989947` passed on both platforms. Release
    `43378152775` built Windows NSIS and a macOS DMG, re-downloaded and mounted
    the DMG in an independent macOS job, then created the draft Release.
16. Public `v0.2.2` Release: PASS with 10 assets. The DMG-only release
    downloaded again and passed checksum, `hdiutil verify`, mount, identity,
    arm64, Applications shortcut, no-zip and startup smoke checks:
    <https://github.com/Goldloli/image-studio/releases/tag/v0.2.2>.
17. `v0.2.2` acceptance archive: [v0.2.2/](v0.2.2/) with the release record,
    published checksum files, Windows package evidence and local DMG validation.

Unsigned/notarization limitations are documented in
[doc/14-ci-release.md](../../doc/14-ci-release.md).
