# Windows parity installer manifest verification

Validates the installer-consumable resources for the Windows model parity change
(`openspec/changes/achieve-windows-model-parity`, task 3.2 partial) against the
real transferred artifacts and the installer rules implemented in
`apps/desktop/src/adapters/installation.rs` (lines 271-324 schemas, 570-720
component-byte/validation rules, 2194-2217 URL allowlist, 2342-2371 artifact
validation).

Status of the current run: **pass** for all eight manifests (six model
manifests + Real-ESRGAN native runtime + Windows v2 product runtime):
8/8 manifests, 131 per-file/member checks, 0 failures.

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
   with the exact version. The verifier additionally replays the installer's
   own probe command against the frozen interpreter
   (`python.exe -I -c "import importlib.metadata,json; print(...)"`) and
   compares the resulting normalized name→version map with the manifest:
   50 distributions probed, 49/49 manifest entries match.
7. **Embedded worker wheels**: `worker_wheels_embedded_by_installer` must hold
   the two fixed worker-wheel identities; the verifier re-hashes the committed
   `resources/windows-parity/workers/*.whl` files against them and asserts the
   two wheels are not present in the `wheels` download list (the installer
   embeds them with `include_bytes!`).

## Result

| Manifest | Kind | URL hosts | File checks | Component bytes | Status |
|---|---|---|---|---|---|
| `flux2-klein-4b-distilled-windows-v1` | model | www.modelscope.cn | 19/19 | 15,980,141,329 | pass |
| `birefnet-general-epoch-244-windows-v1` | model | github.com | 1/1 (+ bundled MIT license) | 972,666,916 | pass |
| `qwen-image-2.1-windows-v1` | model | www.modelscope.cn | 25/25 | 33,131,609,424 | pass |
| `qwen-image-2.1-turbo-windows-v1` | model | www.modelscope.cn | 16/16 | 32,451,607,098 | pass |
| `moss-image-sft-windows-v1` | model | www.modelscope.cn | 19/19 | 27,219,783,878 | pass |
| `realesrgan-x4plus-windows-v1` | model (archive) | github.com | 1 archive + 2 members | 33,540,549 | pass |
| `realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1` | native runtime | github.com | 1 archive + 3 members + 2 licenses | n/a | pass |
| `windows-python31210-torch2100-cu126-diff041-transformers53-v2` | product runtime | pypi/pytorch/python.org | 50/50 | n/a | pass |

The generated JSON (`installer-manifests-verification.json`) contains the
per-file bytes/SHA-256 comparison, schema errors (none), allowlist hosts and
component-byte computations.

### Product runtime manifest

`resources/windows-parity/runtime-v2/product-runtime-manifest.json` is the
installer `RuntimeManifest` view of task 1.2:

- `kind: python-embeddable-windows`, `id:
  windows-python31210-torch2100-cu126-diff041-transformers53-v2`,
  `python: 3.12.10`, `platform: windows-amd64`
- `interpreter`: Python 3.12.10 embeddable archive (11,133,606 bytes)
- `wheels`: 49 artifacts (torch/torchvision from the PyTorch cu126 index, the
  rest from PyPI, all allowlisted)
- `packages`: 49 name→version pairs, read from the installed runtime's
  `.dist-info/METADATA` and normalized exactly like the installer probe; every
  value equals its `requirements.lock` pin (verified here)
- `archives: []`, `install_layout:
  {interpreter_root: python.exe, site_packages: Lib/site-packages,
  path_file: python312._pth}`
- `worker_wheels_embedded_by_installer`: the two Windows worker wheels
  (`studio-diffusers-worker`, `studio-onnx-worker`) are embedded by the installer
  (`include_bytes!` / `WINDOWS_SHARED_WORKER_WHEELS`) and installed directly into
  the v2 runtime's `Lib/site-packages`, following the v1 `bundled:worker-wheel`
  precedent. The manifest records their fixed bytes/SHA-256 but deliberately
  does **not** list them in `wheels`, so the installer never downloads them

### Real-ESRGAN native runtime manifest

`resources/windows-parity/realesrgan-ncnn-vulkan/runtime-manifest.json`:

- `kind: native`, `id: realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1`,
  `platform: windows-amd64` (the planned platform-gate change must accept this)
- `executable: realesrgan-ncnn-vulkan.exe`
- one archive: the official `realesrgan-ncnn-vulkan-20220424-windows.zip`
  (45,474,481 bytes, SHA-256 `abc02804…`), members exactly the files that exist
  in the zip: `realesrgan-ncnn-vulkan.exe`, `vcomp140.dll`,
  `README_windows.md`
- the same archive is referenced by the Real-ESRGAN model manifest; the
  installer's SHA-256-keyed download cache stores it once, so any space math
  counting it twice is a conservative upper bound
- two bundled license resources (`LICENSE-RealESRGAN.txt` BSD-3-Clause,
  `LICENSE-ncnn-app.txt` MIT) recorded in the non-breaking `licenses` field
  with exact bytes/SHA-256; the installer copies those texts into
  `<data>/licenses/<install_id>/` and records the hashes in the ready record

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
  and is recorded in the manifest's non-breaking `licenses` field. Accepted
  approach: the installer writes license texts from `resources/windows-parity`
  into `<data>/licenses/<install_id>/` and records their SHA-256 in the ready
  record instead of downloading from a non-allowlisted host.
- **Real-ESRGAN licenses**: the official Windows zip contains no license files.
  The two texts are committed resources and recorded in the native runtime
  manifest's `licenses` field; archive members only cover files that really
  exist in the zip. Same install-time copy rule as above.
- **Real-ESRGAN shared archive**: the model manifest and the native runtime
  manifest reference the same official zip, so the installer download cache
  (keyed by SHA-256) stores it once; counting it per manifest is a conservative
  space upper bound.
- **Worker wheels are installer-embedded, not downloads**: the v2 product
  runtime manifest records `worker_wheels_embedded_by_installer` with the fixed
  identities of `studio_diffusers_worker-0.1.0-py3-none-any.whl` (41,530 B,
  SHA-256 `79799fd9…`) and `studio_onnx_worker-0.1.0-py3-none-any.whl`
  (10,190 B, SHA-256 `45375753…`). The verifier re-hashes the committed wheel
  files, checks both records, and asserts neither wheel appears in the `wheels`
  download list. The runtime manifest therefore has no missing download item for
  activation.

## Reproduction

```powershell
python tools/windows-parity/manifests/build_model_manifests.py
python tools/windows-parity/manifests/build_runtime_manifests.py
python tools/windows-parity/manifests/verify_installer_manifests.py
```

Evidence: `verification/windows-parity-manifests/installer-manifests-verification.json`.
