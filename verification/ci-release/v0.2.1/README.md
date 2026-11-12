# v0.2.1 release acceptance (2026-11-13)

Status: **passed and publicly published** —
<https://github.com/Goldloli/image-studio/releases/tag/v0.2.1>.

Tag `v0.2.1` is an annotated maintenance tag on `2b805e6` in `main`. The
release packages the eight merged dependency updates without changing model,
runtime, worker, recipe, schema, or user-workflow identities.

## Runs

| Run | Result | Notes |
|---|---|---|
| [quality 43260424768](https://github.com/Goldloli/image-studio/actions/runs/43260424768) | success | Windows `7m55s`; macOS arm64 retry `3m38s` |
| [release-draft 43264815859](https://github.com/Goldloli/image-studio/actions/runs/43264815859) | success | version provenance `21s`, Windows `13m8s`, macOS retry `7m57s`, draft release `32s` |

The first macOS quality and release jobs waited for a runner without executing
steps and were cancelled at the platform queue limit. Both were recovered with
`gh run rerun <id> --failed`; the Windows artifacts and successful jobs were
retained.

## Published assets

Ten assets, all downloaded again after publication:

| Asset | Bytes | SHA-256 |
|---|---:|---|
| `Image.Studio_0.2.1_x64-setup.exe` | 245,189,562 | `88ff36ab7e900b058e74f2ae9c6e2f9010aa4599dc42833dadd142d5b5e46767` |
| `Image-Studio-0.2.1-macos-arm64.zip` | 32,570,985 | `712f70c7eca2b85f30c9caa71ab03af5dc94acff6103b4b0bea04c83a1d99213` |
| `internal-package.json` | 1,094 | committed copy |
| `SHA256SUMS-windows.txt` | 99 | committed copy |
| `SHA256SUMS-macos.txt` | 101 | committed copy |
| `README.md`, `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, `RELEASE-NOTES.md` | — | release documents |

## Recheck

- Both downloaded packages pass their published checksum files.
- The Windows installer hash and byte count match `internal-package.json`;
  target is `x86_64-pc-windows-msvc`, install mode is `currentUser`, and
  WebView2 is bundled offline.
- The macOS `.app` reports `io.github.goldloli.image-studio`, short/build version
  `0.2.1`, `LSMinimumSystemVersion 11.0`, and four arm64 executables.
- The public release is `draft: false`, `prerelease: false`, and is returned by
  the `releases/latest` endpoint.
- The release body has no version placeholder and retains the macOS quarantine
  command.
- A downloaded macOS app launch smoke test used an isolated data root, remained
  running for eight seconds, and initialized SQLite, locks, and the application
  log before a clean termination.

## Validation scope

No model, runtime, worker-protocol, recipe, or schema code changed. The prior
0.2.0 macOS and Windows real-generation evidence remains the product behavior
baseline. This maintenance release reran:

- frontend `81` tests, typecheck, and Vite production build;
- Rust formatting, Clippy with warnings denied, and workspace tests;
- OpenSpec 1.12 strict validation (`24/24`);
- workflow structure validation and `actionlint`;
- locked release build, macOS app packaging, package identity, architecture,
  checksums, and a downloaded-app startup smoke.

Machine-readable record: [acceptance.json](acceptance.json).
Previous release: [v0.2.0](../v0.2.0/README.md).
