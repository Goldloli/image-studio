# v0.2.2 DMG release acceptance (2026-11-13)

Status: **passed and publicly published**
<https://github.com/Goldloli/image-studio/releases/tag/v0.2.2>.

Tag `v0.2.2` is an annotated tag on `5ada451` in `main`. This is the first
release that publishes one macOS arm64 DMG instead of an application zip. The
DMG contains `Image Studio.app` and an Applications shortcut. Historical
releases remain unchanged.

## Runs

| Run | Result | Notes |
|---|---|---|
| [quality 43376989947](https://github.com/Goldloli/image-studio/actions/runs/43376989947) | success | macOS `3m02s`, Windows `8m59s` on `5ada451` |
| [release 43378152775](https://github.com/Goldloli/image-studio/actions/runs/43378152775) | success | version check `21s`, Windows `11m32s`, macOS DMG `5m33s`, independent DMG verification `39s`, draft `35s` |

## Published assets

Ten assets were downloaded again after publication:

| Asset | Bytes | SHA-256 |
|---|---:|---|
| `Image.Studio_0.2.2_x64-setup.exe` | 245,388,429 | `2feceb8df119e555b7cbaf65001a17a339eb01ef07a8ff96ae569455ec702950` |
| `Image-Studio-0.2.2-macos-arm64.dmg` | 34,650,435 | `0ec4282780ab03539dff7ce7c8312765756c1f9d2bd4591176283857737025c6` |
| `internal-package.json` | 1,094 | committed copy |
| `SHA256SUMS-windows.txt` | 99 | committed copy |
| `SHA256SUMS-macos.txt` | 101 | committed copy |
| `README.md`, `LICENSE`, `NOTICE`, `THIRD_PARTY_NOTICES.md`, `RELEASE-NOTES.md` | - | release documents |

## DMG recheck

- The downloaded DMG passes `SHA256SUMS-macos.txt`, `hdiutil verify`, read-only
  mount and detach.
- The volume contains `Image Studio.app` and `Applications -> /Applications`;
  it contains no zip entry.
- `Info.plist` reports `io.github.goldloli.image-studio`, version `0.2.2`,
  build version `0.2.2`, and `LSMinimumSystemVersion 11.0`.
- The desktop, diagnostic, inference supervisor, and Real-ESRGAN executables
  are all arm64.
- LICENSE, NOTICE, third-party notices, font license, and prompt-library
  license are present.
- The downloaded app was copied out of the DMG and launched with an isolated
  data root. It remained running for eight seconds and initialized SQLite,
  host/execution locks, and `logs/app.log`.
- The Release has exactly one macOS DMG and zero macOS zip attachments.

The local pre-release DMG was 34,661,923 bytes with SHA-256 `56d2f161...`; the
GitHub rebuild legitimately produced different bytes. Both passed the same
structure, identity, architecture, mount, checksum, and startup checks.

## Windows recheck

- The installer checksum and byte count match `SHA256SUMS-windows.txt` and
  CI-generated `internal-package.json`.
- Target is `x86_64-pc-windows-msvc`; install mode is current-user and
  WebView2 is bundled offline.

## Historical release

The `v0.2.1` Release still exposes its original macOS zip, digest
`712f70c7eca2b85f30c9caa71ab03af5dc94acff6103b4b0bea04c83a1d99213`,
and Windows installer digest `88ff36ab...`. No historical tag or attachment
was modified.

Machine-readable records:
[acceptance.json](acceptance.json) and
[local-dmg-validation.json](local-dmg-validation.json).

## Remaining limitations

- Builds remain unsigned and not notarized.
- Gatekeeper first-launch UI was not exercised; `not_run`.
- CI does not download model weights or execute GPU inference.
