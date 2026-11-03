# MOSS-Image-SFT — Windows installer manifest

- `model_variant_id`: `moss-image-sft-windows-v1`
- model snapshot id (design §11): `moss-image-sft-windows-staged-f972bd4b4b9e3852133cc5da954d87bc32cd8fb9`
- upstream repository: `OpenMOSS-Team/MOSS-Image-SFT`
- frozen upstream revision: `f972bd4b4b9e3852133cc5da954d87bc32cd8fb9`
- license: Apache-2.0
- installer manifest: `model-manifest.json` (installer `ModelManifest` schema)

## Source policy

Official ModelScope repository `OpenMOSS/MOSS-Image-SFT` with per-file pinned
revisions; fixed upstream Hugging Face blob byte counts and SHA-256 values. Each
manifest file keeps its ModelScope resolve URL plus the non-breaking
`source_revision` and `upstream_url` fields. `LICENSE` (11,358 bytes) is one of
the 19 model files.

`pipeline_moss_image.py` is **not** a model file and is not part of this
manifest: the Windows worker vendors the reviewed custom pipeline code (fixed
upstream revision, 25,060 B / SHA-256 `1dac1aca…`, review in
`verification/windows-parity-gate0/moss-pipeline-review.md`) and never executes
Python from the model directory. The worker also carries the documented bnb
dtype shim (`tools/windows-parity-staged/moss_bnb_shim.py`) required by the
upstream pipeline's `weight.dtype` compute-precision lookup.

## Windows strategy

Staged NF4 text encoder + NF4 DiT with the bnb shim + float32 VAE, 50 steps /
CFG 3 / negative-branch skip at layer 22 (design §10.2/§11). Gate 0 measured
5.84 GiB CUDA reserved / 9.20 GiB process commit / 21.47 GiB available RAM.

## Space budget

- upstream snapshot (`component_bytes`): 27,219,783,878 bytes (~25.35 GiB)
- prepared quantized cache (install-time CPU prepare, design §10.2): ~8.47 GB
- installer download cache: one compressed copy of the snapshot
- install-time RAM preflight: available physical RAM ≥ 28 GiB or commit
  headroom ≥ 26 GiB recommended by Gate 0 (design §10.2 item 3)

## Evidence

- staged Gate 0: `verification/windows-parity-gate0/staged-loader-moss-image-sft.md`,
  `staged-moss-image-sft-prepare.json`, `staged-moss-image-sft-run.json`,
  `staged-moss-image-sft.png`
- pipeline review: `verification/windows-parity-gate0/moss-pipeline-review.md`
- manifest verification: `verification/windows-parity-manifests/installer-manifests-verification.json`
