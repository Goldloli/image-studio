# Real-ESRGAN x4plus — Windows installer model manifest

- `model_variant_id`: `realesrgan-x4plus-windows-v1`
- model snapshot id (design §11): `realesrgan-x4plus-ncnn-windows-v1`
- upstream repository: `xinntao/Real-ESRGAN`, release `v0.2.5.0`
- frozen asset: `realesrgan-ncnn-vulkan-20220424-windows.zip`
- license: BSD-3-Clause (Real-ESRGAN weights) / MIT (ncnn application)
- installer manifest: `model-manifest.json` (installer `ModelManifest` schema)

## Source policy

The two weight files are archive members of the same official Windows release
zip that also provides the executable:

- archive URL:
  `https://github.com/xinntao/Real-ESRGAN/releases/download/v0.2.5.0/realesrgan-ncnn-vulkan-20220424-windows.zip`
- archive: 45,474,481 bytes, SHA-256
  `abc02804e17982a3be33675e4d471e91ea374e65b70167abc09e31acb412802d`
  (two independent downloads and the GitHub releases API size agree)
- members installed as the model snapshot:
  - `models/realesrgan-x4plus.bin` → `realesrgan-x4plus.bin`
    (33,424,520 bytes, SHA-256 `713ee713…`)
  - `models/realesrgan-x4plus.param` → `realesrgan-x4plus.param`
    (116,029 bytes, SHA-256 `35330ece…`)

`component_bytes` = 33,540,549 bytes = the two installed weight files (the
installer sums archive members and ignores the archive itself). The executable,
`vcomp140.dll` and documentation are installed by the separate native runtime
manifest in `resources/windows-parity/realesrgan-ncnn-vulkan/runtime-manifest.json`.

Both license texts are bundled as application resources (the official Windows
zip does not contain license files); the manifest records them in the
non-breaking `licenses` field with their exact hashes.

## Windows strategy

Fixed ncnn Vulkan contract: model `realesrgan-x4plus`, exactly 4x, tile 128,
single GPU `-g 0`, threads `1:1:1`, TTA disabled. Gate 0 passed on the target
RTX 3080 Ti in three runs (official sample, controlled image, NASA real photo —
all exactly 4x). Hardware floor: `min_vram_gb=4`, `min_ram_gb=8`.

## Space budget

- installed weights (`component_bytes`): 33,540,549 bytes (~32 MiB)
- archive download/cache: 45,474,481 bytes
- no GPU-side prepared cache; ncnn runs directly from the installed files
- the native runtime manifest references the same official zip, so the
  installer's SHA-256-keyed download cache stores it **once**; install-space
  math that counts the archive in both manifests is a conservative upper bound

## Evidence

- Gate 0: `verification/windows-parity-realesrgan/` (`g2-r1-input.json`,
  `g2-r1-controlled.json`, `g2-r1-real-photo.json`, `manifest-reverify.json`)
- frozen bundle: `resources/windows-parity/realesrgan-ncnn-vulkan/asset-manifest.json`
- manifest verification: `verification/windows-parity-manifests/installer-manifests-verification.json`
