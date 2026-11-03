# Qwen-Image 2.1 (base) — Windows installer manifest

- `model_variant_id`: `qwen-image-2.1-windows-v1`
- model snapshot id (design §11): `qwen-image-2.1-windows-staged-d26bb61231c349cf6b7896fa83353113880e1ba3`
- upstream repository: `Qwen/Qwen-Image-2.1`
- frozen upstream revision: `d26bb61231c349cf6b7896fa83353113880e1ba3`
- license: Qwen Research License (**non-commercial** — must be shown in catalog/model page)
- installer manifest: `model-manifest.json` (installer `ModelManifest` schema)

## Source policy

Files are served from the official ModelScope repository
`Qwen/Qwen-Image-2.1` with **per-file pinned revisions**. The byte counts and
SHA-256 values are the fixed upstream Hugging Face blob identities; the
ModelScope transport was hash-verified per file (`transfer-qwen21-base.json`,
`reverification-qwen21-base.json`). Each manifest file keeps its ModelScope
resolve URL plus the non-breaking `source_revision` and `upstream_url`
(HF declared URL) fields. `LICENSE` (7,831 bytes) is one of the 25 model files
from the same repository.

## Windows strategy

Staged NF4: NF4 text encoder, release, NF4 transformer sampling, release,
tiled VAE decode (design §10.2). Gate 0 measured 6.00 GiB CUDA reserved /
13.96 GiB process commit / 16.08 GiB available RAM for the 40-step run.

## Space budget

- upstream snapshot (`component_bytes`): 33,131,609,424 bytes (~30.86 GiB)
- prepared quantized cache (install-time CPU prepare, design §10.2): ~10.05 GB
- installer download cache: one compressed copy of the snapshot
- RAM preflight for prepare: recommend available physical RAM ≥ 28 GiB or
  commit headroom ≥ 26 GiB (design §10.2 item 3); not enforced by this resource

## Evidence

- staged Gate 0: `verification/windows-parity-gate0/staged-loader-qwen21-base.md`,
  `staged-qwen21-base-prepare.json`, `staged-qwen21-base-run.json`, `staged-qwen21-base.png`
- transfer/integrity: `verification/windows-parity-gate0/transfer-qwen21-base.json`,
  `reverification-qwen21-base.json`
- manifest verification: `verification/windows-parity-manifests/installer-manifests-verification.json`
