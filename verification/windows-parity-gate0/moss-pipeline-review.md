# MOSS-Image-SFT custom pipeline review (task 1.5)

Status: **reviewed on 2026-11-03**, fixed identity and hash verified against the
upstream ModelScope repository. The Windows worker must vendor these exact bytes
instead of executing remote code at runtime.

## Frozen identity

| Item | Value |
|---|---|
| Repository (ModelScope) | `OpenMOSS/MOSS-Image-SFT` |
| Hugging Face revision (content identity) | `f972bd4b4b9e3852133cc5da954d87bc32cd8fb9` |
| Custom pipeline file revision (ModelScope) | `01feb93b6661de9ceb29851092e232336b34c240` |
| File | `pipeline_moss_image.py`, 25,060 bytes |
| SHA-256 | `1dac1aca9feb1bfdfc727a401e4b67d61a505d3190706d1b7d2d784f88678b31` |
| Local copy | `target/parity-gate0/models/moss-image-sft/pipeline_moss_image.py` (hash matches) |
| License | Apache-2.0 (`LICENSE`, SHA-256 `cfc7749b...23d30`) |
| `model_index.json` | `_class_name: MOSSImagePipeline`, scheduler/text_encoder/tokenizer/transformer/vae bindings |

Upstream verification: the ModelScope file listing API returns the same 25,060
bytes and SHA-256 `1dac1aca...b31` for path `pipeline_moss_image.py` at revision
`01feb93b...c240`. The local file's SHA-256 equals it, so the reviewed bytes are
the fixed upstream bytes.

## Code review

Reviewed the whole file (440 lines). Findings:

- **Imports**: `math`, `pathlib.Path`, `torch`, `torch.nn`, `torch.nn.functional`,
  `diffusers` (pipeline/model/scheduler/image-processor/utils), `transformers`
  (`AutoModel`, `AutoTokenizer`). No `os`, `sys`, `subprocess`, `socket`,
  `urllib`, `requests`, `pickle`, `eval`, `exec`, `__import__`, or dynamic
  import logic.
- **No I/O or network**: the module itself performs no file writes, environment
  access or network calls. `MOSSImagePipeline.from_components_pretrained` only
  calls `from_pretrained` on the explicit local paths a caller passes, with
  `local_files_only` forwarded; the product worker will not use that classmethod
  with remote ids.
- **Weights**: every component is loaded from `safetensors` through diffusers /
  transformers; no pickle or arbitrary deserialization path is introduced by
  the custom module.
- **Math reviewed**: RMSNorm, SiTU activation, RoPE, SDPA attention, block
  modulation, 50-step CFG loop with negative-branch block-22 skipping, fp32
  CFG/Euler updates, FLUX.2 VAE batch-norm rescale and pixel shuffle. All
  parameter values come from the fixed `config.json` files; the defaults match
  the model card's released model (not MoE/CFG-distilled/video/reference
  variants).
- **Dtype hazard** (functional, not security): the custom forward derives its
  compute dtype from `<module>.weight.dtype`; bitsandbytes reports storage dtype
  there (`int8`/`uint8`), which breaks quantized execution unless the documented
  shim is applied. Recorded in
  [staged-loader-moss-image-sft.md](staged-loader-moss-image-sft.md) and carried
  as a product-worker requirement.

## Security boundary for the product worker

1. Vendor the reviewed file into the Windows worker package with the SHA-256
   above in the worker identity; **do not** load Python code from the model
   directory at runtime (`trust_remote_code=False` in the product path).
2. Run the worker with offline environment (`HF_HUB_OFFLINE=1`,
   `TRANSFORMERS_OFFLINE=1`, no proxy fallback); the installer is the only
   network consumer and only fetches fixed manifest URLs.
3. `configuration.json` contains `"allow_remote": true` (ModelScope SDK
   metadata). The product does not use the ModelScope SDK at runtime; the flag
   must not be treated as permission to fetch anything.
4. The reviewed component weights are safetensors; the installer verifies every
   file's bytes and SHA-256 before activation.
5. Any upstream revision change requires a new review and a new change; the
   file hash is part of the install manifest and worker identity.

## Finding for the install manifest

The macOS planned-model manifest
(`resources/planned-models/moss-image-sft/model-manifest.json`) lists the
components but **not** `pipeline_moss_image.py`. The Windows install manifest
must add it (25,060 bytes, SHA-256 `1dac1aca...b31`, Apache-2.0) or a vendored
copy must ship in the worker package, otherwise the model cannot be constructed.
Decision for the Windows change: ship the reviewed module in the worker package
and keep the model manifest limited to weights/components; add
`configuration.json` to the manifest only if the loader needs it (it does not).

## Artifacts

- Upstream listing evidence captured in this review (ModelScope API response for
  `pipeline_moss_image.py`).
- Local hash: `Get-FileHash target/parity-gate0/models/moss-image-sft/pipeline_moss_image.py -Algorithm SHA256`.
