# Windows parity installer manifest verification

Validates the installer-consumable resources for the Windows model parity change
(`openspec/changes/achieve-windows-model-parity`, task 3.2 partial) against the
real transferred artifacts and the installer rules implemented in
`apps/desktop/src/adapters/installation.rs` (lines 271-324 schemas, 570-720
component-byte/validation rules, 2194-2217 URL allowlist, 2342-2371 artifact
validation).

Status of the current run: **pass** for the six model manifests. The
Real-ESRGAN native runtime manifest and the v2 product runtime manifest are
verified in the same report once present (this file is updated by their
commits).

## What is checked

For every manifest:

1. **Shape**: `ModelManifest` requires `id`, `revision`, `component_bytes`,
   optional `files` / `archives`; `RuntimeManifest` requires `id` plus the
   fields of the selected kind (python/interpreter/wheels for the Python
   runtime, `executable`/`archives` for the native runtime).
2. **Artifact rules**: every artifact has a safe relative path, positive
   `bytes`, 64 lowercase hex `sha256`, and an `https` URL on the installer
   allowlist (`huggingface.co`, `*.cdn.hf.co`, `*.xethub.hf.co`,
   `modelscope.cn`, `*.modelscope.cn`, `github.com`,
   `release-assets.githubusercontent.com`, `objects.githubusercontent.com`,
   `files.pythonhosted.org`, `www.python.org`, `download.pytorch.org`,
   `download-r2.pytorch.org`; port 443, no credentials).
3. **Archive rules**: archive path ends with `.zip`, members are non-empty,
   member `archive_path`/`path` are safe relative paths, target paths are
   unique, member bytes/hashes are valid.
4. **Component bytes**: `component_bytes == sum(files bytes except README.md) +
   sum(archive member bytes)`, exactly as the installer computes it.
5. **Real content**: every file is re-hashed from the local artifact that was
   actually transferred (`target/parity-gate0/models/<model>/`), for the
   runtime from `target/parity-runtime/{downloads,wheelhouse}`, and for the ncnn
   package from the downloaded official zip. Missing local files are reported
   as `missing` rather than treated as passing.
6. **Runtime packages**: `packages` must cover every `requirements.lock` pin
   with the exact version (the installer probes
   `importlib.metadata.distributions()` and compares each entry).

## Result

| Manifest | Kind | URL hosts | File checks | Component bytes | Status |
|---|---|---|---|---|---|
| `flux2-klein-4b-distilled-windows-v1` | model | www.modelscope.cn | 19/19 | 15,980,141,329 | pass |
| `birefnet-general-epoch-244-windows-v1` | model | github.com | 1/1 (+ bundled MIT license) | 972,666,916 | pass |
| `qwen-image-2.1-windows-v1` | model | www.modelscope.cn | 25/25 | 33,131,609,424 | pass |
| `qwen-image-2.1-turbo-windows-v1` | model | www.modelscope.cn | 16/16 | 32,451,607,098 | pass |
| `moss-image-sft-windows-v1` | model | www.modelscope.cn | 19/19 | 27,219,783,878 | pass |
| `realesrgan-x4plus-windows-v1` | model (archive) | github.com | 1 archive + 2 members | 33,540,549 | pass |
| `realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1` | native runtime | github.com | pending build | n/a | pending |
| `windows-python31210-…-v2` | product runtime | pypi/pytorch/python.org | pending build | n/a | pending |

The generated JSON (`installer-manifests-verification.json`) contains the
per-file bytes/SHA-256 comparison, schema errors (none), allowlist hosts and
component-byte computations.

## Local content sources

- models: `target/parity-gate0/models/{klein,birefnet,qwen-image-2.1,qwen-image-2.1-turbo,moss-image-sft}`
  (the exact files transferred in `verification/windows-parity-gate0/transfer-*.json`)
- Klein `LICENSE.md` was not part of the Gate 0 transfer; it was fetched from
  the same ModelScope mirror revision `a6e0561…` for hash verification
  (`target/parity-runtime/manifests-aux/klein-LICENSE.md`, build artifact).
- runtime: interpreter `target/parity-runtime/downloads/python-3.12.10-embed-amd64.zip`,
  wheels from `target/parity-runtime/wheelhouse`
- Real-ESRGAN: `target/parity-runtime/downloads/realesrgan-ncnn-vulkan-20220424-windows.zip`
  and resource licenses under `resources/windows-parity/realesrgan-ncnn-vulkan/`

## Known deviations (documented, not hidden)

- **BiRefNet MIT license**: no allowlisted download URL exists for the license
  text (GitHub raw redirects to `raw.githubusercontent.com`; the `v1` release
  has no license asset). The license ships as an application resource
  (`resources/windows-parity/models/birefnet-general-epoch-244-windows-v1/LICENSE-BiRefNet.txt`)
  and is recorded in the manifest's non-breaking `licenses` field.
- **Real-ESRGAN licenses**: the official Windows zip contains no license files.
  The two texts are committed resources and recorded in the native runtime
  manifest's `licenses` field; archive members only cover files that really
  exist in the zip.
- `worker_wheels_pending` in the product runtime manifest: Windows worker
  wheels are produced by the worker streams and will be appended to `wheels`
  later; the file is explicitly marked incomplete on that field.

## Reproduction

```powershell
python tools/windows-parity/manifests/build_model_manifests.py
python tools/windows-parity/manifests/build_product_runtime_manifest.py
python tools/windows-parity/manifests/verify_installer_manifests.py
```

Evidence: `verification/windows-parity-manifests/installer-manifests-verification.json`.
