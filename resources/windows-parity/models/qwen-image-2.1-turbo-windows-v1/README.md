# Qwen-Image 2.1-Turbo — Windows installer manifest

- `model_variant_id`: `qwen-image-2.1-turbo-windows-v1`
- model snapshot id (design §11): `qwen-image-2.1-turbo-windows-staged-fc13ba1986fcb905854dc8de2f52eee395f49b5b`
- upstream repository: `Qwen/Qwen-Image-2.1-Turbo`
- frozen upstream revision: `fc13ba1986fcb905854dc8de2f52eee395f49b5b`
- license: Qwen Research License (**non-commercial** — must be shown in catalog/model page)
- installer manifest: `model-manifest.json` (installer `ModelManifest` schema)

## Source policy

Official ModelScope repository `Qwen/Qwen-Image-2.1-Turbo` with per-file pinned
revisions; fixed upstream Hugging Face blob byte counts and SHA-256 values. Each
manifest file keeps its ModelScope resolve URL plus the non-breaking
`source_revision` and `upstream_url` fields. `LICENSE` (7,831 bytes) is part of
the 16 model files.

## Windows strategy

Same staged NF4 executor as the base model, but with the official 8-step Turbo
sampling schedule (`steps=8`, `sampling_schedule="qwen-turbo-model-index-v1"`),
design §10.2/§11. Gate 0 measured 6.00 GiB CUDA reserved / 13.95 GiB process
commit / 17.32 GiB available RAM.

## Space budget

- upstream snapshot (`component_bytes`): 32,451,607,098 bytes (~30.22 GiB)
- prepared quantized cache (install-time CPU prepare): ~10.05 GB budget per
  design §10.2 (final deterministic rebuild/reuse contract lands in task 3.2/3.4)
- installer download cache: one compressed copy of the snapshot
- install-time RAM preflight as for the base model

## Evidence

- staged Gate 0: `verification/windows-parity-gate0/staged-loader-qwen21-turbo.md`,
  `staged-qwen21-turbo-prepare.json`, `staged-qwen21-turbo-run.json`, `staged-qwen21-turbo.png`
- manifest verification: `verification/windows-parity-manifests/installer-manifests-verification.json`
