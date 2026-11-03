# BiRefNet general epoch_244 — Windows installer manifest

- `model_variant_id`: `birefnet-general-epoch-244-windows-v1`
- model snapshot id (design §11): `birefnet-general-epoch-244-onnx-windows-v1`
- upstream repository: `ZhengPeng7/BiRefNet`
- frozen revision: `v1` release, asset id `186740004`
- license: MIT
- installer manifest: `model-manifest.json` (installer `ModelManifest` schema)

## Source policy

Single ONNX artifact from the official GitHub release:
`https://github.com/ZhengPeng7/BiRefNet/releases/download/v1/BiRefNet-general-epoch_244.onnx`
— 972,666,916 bytes, SHA-256
`58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c`
(GitHub redirects the download to `objects.githubusercontent.com`, which is in
the installer allowlist).

The MIT license text is **bundled as an application resource**
(`LICENSE-BiRefNet.txt`, 1,087 bytes, SHA-256 `d7f5aa6c…`) instead of being a
downloaded artifact: the GitHub raw URL redirects to `raw.githubusercontent.com`,
which is outside the installer URL allowlist, and the `v1` release carries no
license asset. The manifest records it under the non-breaking `licenses` field;
the installer ignores that field and installs only the ONNX file.

## Windows strategy

ONNX Runtime **CPU** worker (`CPUExecutionProvider`), float32 NCHW
`1x3x1024x1024` input, one sigmoid on the final logits, straight-alpha RGBA +
8-bit mask dual output. Targeted hardware floor: `min_vram_gb=0`,
`min_ram_gb=16` (design §10.2).

## Space budget

- `component_bytes`: 972,666,916 bytes (~928 MiB)
- download cache: 972,666,916 bytes
- no GPU memory and no quantized prepared cache

## Evidence

- Gate 0 CPU dual output: `verification/windows-parity-gate0/birefnet-gate0.json` (+ RGBA/mask PNGs)
- transfer/integrity: `verification/windows-parity-gate0/transfer-birefnet.json`, `reverification-birefnet.json`
- manifest verification: `verification/windows-parity-manifests/installer-manifests-verification.json`
