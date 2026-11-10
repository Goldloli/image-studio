# v0.1.1 release acceptance (2026-11-10)

Status: **passed and publicly published** —
<https://github.com/Goldloli/image-studio/releases/tag/v0.1.1>.

The tag `v0.1.1` points at `069c6e2` (`main`), the commit that carries the
0.1.1 version bump, the bilingual release notes, and the refreshed README.

## Runs

| Run | Result | Notes |
|---|---|---|
| [quality 42763466161](https://github.com/Goldloli/image-studio/actions/runs/42763466161) | success | Windows `4m11s`, macOS arm64 `2m41s` on `069c6e2` |
| [release-draft 42763614051](https://github.com/Goldloli/image-studio/actions/runs/42763614051) | success | version provenance `17s`, macOS app `3m02s`, Windows installer `6m05s`, draft release `32s` |

## Published assets

Ten assets, all downloaded again after publication:

| Asset | Bytes | SHA-256 |
|---|---|---|
| `Image.Studio_0.1.1_x64-setup.exe` | 245,148,864 | `c749588356d14b01509adfd99e5bf3d8ccb70d54626a9e307ffe6ed1bac50a1e` |
| `Image-Studio-0.1.1-macos-arm64.zip` | 32,516,275 | `7f4981d3419a118415307bac9d1a7460a846ec342bb63cd06cee8fc0a84d2520` |
| `internal-package.json` | 1,094 | committed copy |
| `SHA256SUMS-windows.txt` | 99 | committed copy |
| `SHA256SUMS-macos.txt` | 101 | committed copy |
| `README.md`, `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, `RELEASE-NOTES.md` | — | release documents |

## Recheck

- Both `SHA256SUMS-*.txt` files pass after re-download.
- The Windows installer SHA-256 and byte count match the CI-generated
  `internal-package.json` copied beside this record.
- The macOS `.app` reports `io.github.goldloli.image-studio`, version `0.1.1`,
  `LSMinimumSystemVersion 11.0`, four arm64 Mach-O executables, five embedded
  legal resources, and 15 zip entries.
- The GitHub release body is byte-identical to the rendered
  `RELEASE-NOTES.md` (only the trailing newline differs), and no `{{TAG}}` /
  `{{VERSION}}` placeholder remains.
- The published release is visible through the anonymous GitHub API with
  `draft: false`, `prerelease: false`.

Machine-readable record: [acceptance.json](acceptance.json).
Earlier release: [v0.1.0 record](../github-actions-acceptance-2026-11-05.json).

## Remaining risks

- Windows and macOS installers are unsigned; macOS is not notarized.
- Model weights and private Python runtimes are downloaded separately.
- The 8 GB compact tier is certified on Turing sm75 (RTX 2060 SUPER); other
  8 GB architectures are not certified in this release.
- Z-Image Turbo remains gated at 12 GB VRAM.
- Windows install-time quantization needs ≥ 8 GiB available physical RAM and
  ≥ 6 GiB commit headroom.
