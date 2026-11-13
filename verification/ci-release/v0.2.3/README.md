# v0.2.3 release acceptance (2026-11-14)

Status: **published**

<https://github.com/Goldloli/image-studio/releases/tag/v0.2.3>

Tag `v0.2.3` is an annotated tag on
`9d637f63d287aef57d3992d2295e82cba8f99667` in `main`.

| Run | Result | Notes |
|---|---|---|
| [quality 43496331031](https://github.com/Goldloli/image-studio/actions/runs/43496331031) | success | macOS `4m0s`; Windows `8m28s` |
| [release 43497865280](https://github.com/Goldloli/image-studio/actions/runs/43497865280) | success | version `19s`; macOS `5m35s`; Windows `14m12s`; DMG verification `46s`; draft `45s` |

## Published Assets

| Asset | Bytes | SHA-256 |
|---|---:|---|
| `Image.Studio_0.2.3_x64-setup.exe` | 245,491,502 | `f8eb6087879c25607298f678402985a43d43b24ea6e65b17e868fe78a626a2da` |
| `Image-Studio-0.2.3-macos-arm64.dmg` | 34,904,410 | `adc60855dac807766dac37bd3c7762fb07f3291860e0f7ee9a024dcc112e9332` |
| `SHA256SUMS-windows.txt` | 99 | `9a6e4b3dfdfaffb5898bdcb91695fdaca37014c00b4a09a1a37a731a895be6de` |
| `SHA256SUMS-macos.txt` | 101 | `3bb164f04f218391e0605dcf44596dc8e8cf24a21a50dcf11ebcb8e1196f6868` |

The published checksum files match the installer and DMG asset digests returned
by GitHub. The release workflow independently downloaded the macOS artifact,
passed `SHA256SUMS-macos.txt`, and mounted and verified the DMG before creating
the draft.

## Coverage Boundary

- Windows installer and macOS DMG artifacts were built and verified by CI.
- Physical Windows installation/restart and macOS Gatekeeper first-launch UI are
  `not_run`.
- Model, runtime, worker, recipe, and local schema identities are unchanged, so
  existing real-model acceptance remains the inference baseline.
- Real download and installation inside the new updater UI remain `not_run`;
  the release itself is now available as the next stable update target.

Machine-readable details: [acceptance.json](acceptance.json).
