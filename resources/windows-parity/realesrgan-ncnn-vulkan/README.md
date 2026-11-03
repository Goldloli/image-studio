# Real-ESRGAN x4plus — Windows ncnn Vulkan bundle

This directory freezes the Windows Real-ESRGAN 4x upscaler used by the
model-parity change (`openspec/changes/achieve-windows-model-parity`, task 1.3).
It binds one official portable release, one fixed weight file set, and one
fixed inference contract; it does not redistribute a rebuilt binary.

Fixed inputs:

- Program: `Real-ESRGAN-ncnn-vulkan`, Windows x64 build dated 2022-04-24,
  published in the official `xinntao/Real-ESRGAN` release `v0.2.5.0`
  (`realesrgan-ncnn-vulkan-20220424-windows.zip`), 45,474,481 bytes,
  SHA-256 `abc02804e17982a3be33675e4d471e91ea374e65b70167abc09e31acb412802d`.
  The 2022 asset has no GitHub-published digest; the size matches the GitHub
  releases API and the SHA-256 was reproduced by two independent downloads.
- Program file: `realesrgan-ncnn-vulkan.exe` 6,161,408 bytes, SHA-256
  `07e49f7cbb4ede01ae4dd4c399d3a7e5846e3d2085c3128eff881e55cb7b1a0c`
  (not Authenticode-signed; integrity rests on the fixed SHA-256).
- Runtime dependency shipped in the archive: `vcomp140.dll` (Microsoft OpenMP
  runtime, 182,704 bytes) — it must stay next to the executable.
- Model: `realesrgan-x4plus.bin` (33,424,520 bytes) and
  `realesrgan-x4plus.param` (116,029 bytes) extracted from
  `models/`, SHA-256 `713ee713b0353afaa27976f0563a64a5043bd70b9bd8936c2e26e25ebcdbcddf`
  and `35330ececcea33b6c397a72548e788d5d53becee4734c50b7fada36e89f10a86`.
- Licenses: BSD-3-Clause (`LICENSE-RealESRGAN.txt`, pinned to the
  `v0.2.5.0` tag) and MIT (`LICENSE-ncnn-app.txt`, pinned to
  `Real-ESRGAN-ncnn-vulkan` `v0.2.0`).

`asset-manifest.json` records the archive URL, all 18 archive-member hashes,
the install members, the excluded members, the license records and the
inference contract. The installer extracts only the listed members; anime/x2/x3
weights, the sample images/video and the debug `vcomp140d.dll` are not
installed and are never treated as defaults.

## Fixed inference contract

- model name `realesrgan-x4plus`, scale exactly **4x**
- tile size **128**, single GPU (`-g 0`), threads `1:1:1` (`-j 1:1:1`)
- TTA **disabled** (no `-x`), PNG output
- command:
  `realesrgan-ncnn-vulkan.exe -i <input.png> -o <output.png> -n realesrgan-x4plus -s 4 -t 128 -g 0 -j 1:1:1 -f png`

## Installation layout

The portable archive layout is kept, with the licenses appended:

```
realesrgan-ncnn-vulkan.exe
vcomp140.dll
models/realesrgan-x4plus.bin
models/realesrgan-x4plus.param
README_windows.md
LICENSE-RealESRGAN.txt
LICENSE-ncnn-app.txt
```

App content is 39,886,520 bytes; with the licenses the installed total is
39,890,267 bytes.

## Gate 0

Case **G2-R1** (real ncnn Vulkan 4x on the Windows target machine) passed in
three runs: 880x880 from a 220x220 input in 4.863 s, 388x244 from a 97x61
controlled input in 2.207 s, and 1536x1152 from a 384x288 public-domain NASA
real photograph in 2.337 s — all exactly 4x, all on `[0 NVIDIA GeForce RTX
3080 Ti]` (Vulkan device 1, Intel UHD 770, was enumerated but not selected).
Full evidence is in `verification/windows-parity-realesrgan/`, including an
independent archive/member/license hash re-verification.

## Installer manifests

This directory now carries the installer-consumable view of the frozen bundle:

- `runtime-manifest.json`: native `RuntimeManifest` (kind `native`, platform
  `windows-amd64`, `executable: realesrgan-ncnn-vulkan.exe`) whose archive
  members are exactly the files that exist in the official zip —
  `realesrgan-ncnn-vulkan.exe`, `vcomp140.dll`, `README_windows.md`.
- `../../models/realesrgan-x4plus-windows-v1/model-manifest.json`: the model
  manifest (`realesrgan-x4plus-windows-v1`) uses the same official zip as an
  archive and installs only the two `realesrgan-x4plus` weight members
  (`component_bytes = 33,540,549`).

Because both manifests reference the same 45,474,481-byte zip, the installer's
SHA-256-keyed download cache stores it once; any space calculation that counts
the archive per manifest is a conservative upper bound.

Neither manifest can list license files as archive members: the official
Windows zip does not contain them. The two license texts are committed here
(`LICENSE-RealESRGAN.txt`, `LICENSE-ncnn-app.txt`) and recorded with exact
bytes/SHA-256 in the `licenses` field of both manifests; the installer is
expected to copy those resource texts into `<data>/licenses/<install_id>/` and
record their hashes in the ready record.

## Limitations

- The executable is an upstream 2022 portable build: no vendor support
  statement for newer GPUs, no code signing, and ncnn tiling can produce
  block-boundary differences from the PyTorch implementation.
- Weight files are distributed under the Real-ESRGAN BSD-3-Clause license; the
  license records here must ship with the installed files.
