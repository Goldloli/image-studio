# FLUX.2 Klein 4B distilled — Windows installer manifest

- `model_variant_id`: `flux2-klein-4b-distilled-windows-v1`
- model snapshot id (design §11): `flux2-klein-4b-windows-e7b7dc27f91deacad38e78976d1f2b499d76a294`
- upstream repository: `black-forest-labs/FLUX.2-klein-4B`
- frozen upstream revision: `e7b7dc27f91deacad38e78976d1f2b499d76a294`
- license: Apache-2.0
- installer manifest: `model-manifest.json` (installer `ModelManifest` schema)

## Source policy: HF declared revision vs ModelScope transport

The content identity is the fixed upstream Hugging Face blob manifest: every
file's byte count and SHA-256 come from that manifest and were frozen before
transfer. The transfer itself used the reachable ModelScope mirror
`black-forest-labs/FLUX.2-klein-4B` (per-file revisions, listing revision
`master`); `transfer-klein.json` proves every mirrored file's size and SHA-256
equal the declared HF values. `model-manifest.json` therefore uses the
ModelScope resolve URLs that the transfer actually reached and keeps the
declared HF URL/revision per file in the non-breaking extra fields
`declared_url`, `declared_revision` and `source_revision`. The installer only
reads `path/url/bytes/sha256`, so the extra fields do not change behavior.

`LICENSE.md` is an addition on top of the transfer (9,584 bytes, SHA-256
`ca02bc51…`), taken from the same ModelScope mirror revision `a6e0561…`; the HF
`LICENSE.md` HEAD reports the same size.

## Windows strategy

NF4 text encoder + NF4 transformer + bf16 VAE with component CPU offload, in a
dedicated fresh worker process per generation (design §10.2). Gate 0 measured
9.21 GiB CUDA reserved / 19.45 GiB process commit / 15.30 GiB available RAM, all
within budget. Both text-to-image and single-image edit capabilities share this
snapshot.

## Space budget

- modal file bytes (`component_bytes`): 15,980,141,329 bytes (~14.88 GiB) including `LICENSE.md`
- installer download cache: one compressed copy of the same files
- no prepared quantized cache for Klein (NF4 conversion happens in the worker)

## Evidence

- Gate 0 (fresh-process NF4): `verification/windows-parity-gate0/klein-gate0-fresh-process-nf4.json`
- combined-process failure record: `verification/windows-parity-gate0/klein-gate0-combined-process-bf16-nf4.json`
- transfer/integrity: `verification/windows-parity-gate0/transfer-klein.json`, `reverification-klein.json`
- manifest verification: `verification/windows-parity-manifests/installer-manifests-verification.json`
