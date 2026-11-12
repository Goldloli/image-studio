# v0.2.0 release acceptance (2026-11-12)

Status: **passed and publicly published** —
<https://github.com/Goldloli/image-studio/releases/tag/v0.2.0>.

Tag `v0.2.0` is an annotated tag on `bc50fd3`
(`chore(release): inherit workspace version in studio-core`) in `main`. The
release contains the bilingual README and release notes, explicit macOS
Gatekeeper damage/quarantine guidance, and the 0.2.0 single-rail workspace and
library-management changes.

## Runs

| Run | Result | Notes |
|---|---|---|
| [quality 43242113193](https://github.com/Goldloli/image-studio/actions/runs/43242113193) | success | Windows and macOS arm64 both passed on `bc50fd3`; macOS `2m49s`, Windows `5m20s` |
| [release-draft 43243134087](https://github.com/Goldloli/image-studio/actions/runs/43243134087) | success | version provenance `32s`, Windows installer `12m58s`, macOS app retry `6m50s`, draft release `36s` |

The first macOS release attempt was cancelled after `15m02s` without acquiring
a runner and executed no steps. `gh run rerun <id> --failed` retained the
successful Windows artifact, acquired a `macos-15` arm64 runner, and completed
the macOS build. The final run and all jobs are successful.

## Published assets

Ten assets, all downloaded again after publication:

| Asset | Bytes | SHA-256 |
|---|---:|---|
| `Image.Studio_0.2.0_x64-setup.exe` | 245,195,809 | `2796da216555e74c9a1a89a32a54557a25fe26144b36a24d5d0795ae3d98833f` |
| `Image-Studio-0.2.0-macos-arm64.zip` | 32,566,832 | `e09e5336c051fe25209a10dfd1f3039ddc9b66b056cefaff656310ad35c0ca70` |
| `internal-package.json` | 1,094 | committed copy |
| `SHA256SUMS-windows.txt` | 99 | committed copy |
| `SHA256SUMS-macos.txt` | 101 | committed copy |
| `README.md`, `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, `RELEASE-NOTES.md` | — | release documents |

## Recheck

- Both downloaded packages pass their published `SHA256SUMS-*.txt`.
- The Windows installer SHA-256 and byte count match the CI-generated
  `internal-package.json`; target is `x86_64-pc-windows-msvc`, install mode is
  `currentUser`, WebView2 is bundled offline, and the package is unsigned.
- The macOS `.app` reports `io.github.goldloli.image-studio`, short/build version
  `0.2.0`, `LSMinimumSystemVersion 11.0`, and four arm64 Mach-O executables.
- The public release is `draft: false`, `prerelease: false`, and is returned by
  the repository's `releases/latest` endpoint.
- The release body has no `{{TAG}}` / `{{VERSION}}` placeholder and includes the
  macOS damage explanation and `xattr` quarantine command.
- Repository About is bilingual with English first and points its homepage to
  `https://github.com/Goldloli/image-studio/releases/latest`.

## Local pre-release gates

- Frontend: 81 tests, typecheck, and production build passed.
- Rust: formatting, Clippy with warnings denied, workspace tests, locked release
  build, and macOS release bundle validation passed.
- OpenSpec 1.12.0: 24/24 strict validation passed, matching CI.
- Workflow structure validation and `actionlint` passed.

Machine-readable record: [acceptance.json](acceptance.json).
Previous release: [v0.1.1](../v0.1.1/README.md).

## Known limitations

- Windows and macOS installers are unsigned; macOS is not notarized.
- Model weights and private Python runtimes are downloaded separately.
- The 8 GB compact tier is certified on Turing sm75 (RTX 2060 SUPER); other
  8 GB architectures are not certified in this release.
- Z-Image Turbo remains gated at 12 GB VRAM.
- CI does not download model weights or execute GPU inference.
