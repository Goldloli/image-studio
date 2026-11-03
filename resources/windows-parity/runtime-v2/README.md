# Windows v2 Python runtime

This directory freezes the second Windows runtime used by the model-parity
change (`openspec/changes/achieve-windows-model-parity`, task 1.2). It does not
assert model quality: it fixes byte content, proves that the closure installs
offline and that every pinned library imports on this machine.

Runtime identity:

- `id`: `windows-python31210-torch2100-cu126-diff041-transformers53-v2`
- Python `3.12.10` Windows embeddable x64
- torch `2.10.0+cu126` / torchvision `0.25.0+cu126` / CUDA `12.6`
- diffusers `0.41.0` / transformers `5.3.0`
- accelerate `1.12.0` / bitsandbytes `0.50.2` / safetensors `0.8.0`
- huggingface-hub `1.33.0` / psutil `7.1.3`
- onnxruntime `1.31.1` (CPU package, used through `CPUExecutionProvider` for BiRefNet)
- 49 locked distributions in total (`requirements.lock`)

Serves: FLUX.2 Klein 4B distilled, Qwen-Image 2.1, Qwen-Image 2.1-Turbo,
MOSS-Image-SFT, BiRefNet epoch_244 (ONNX Runtime CPU). The Z-Image Turbo runtime
(`windows-python31210-torch2100-cu126-bnb0482-v1`) is unchanged.

## Fixed inputs

- `runtime-manifest.json` records every artifact with per-file URL, bytes and
  SHA-256: 49 wheels, the `pip` bootstrap wheel, and the Python embeddable
  archive. The manifest builder re-hashed each local artifact and compared it
  with the official index metadata (PyPI JSON API for PyPI files, the
  `download.pytorch.org/whl/cu126` index for torch/torchvision).
  `verification/windows-parity-runtime/manifest-reverify.json` is a second,
  independent pass over the checked-in hashes.
- `requirements.lock` pins the full resolved closure; the resolution is
  summarized in `verification/windows-parity-runtime/dependency-resolution.json`.
- `license-records.json` records the license expression/classifiers and any
  bundled license files from each wheel's own `.dist-info/METADATA`.
- `LICENSE-Python-3.12.10.txt` is the CPython license extracted from the
  embeddable archive (`LICENSE.txt` member).

The interpreter archive is re-downloaded from
`https://www.python.org/ftp/python/3.12.10/python-3.12.10-embed-amd64.zip` and
checked against the v1 repository record (11,133,606 bytes, SHA-256
`4acbed6dd1c744b0376e3b1cf57ce906f9dc9e95e68824584c8099a63025a3c3`).

## Offline install contract

The wheelhouse (`target/parity-runtime/wheelhouse`, 50 files, 2,714,172,955
bytes) is a build artifact and is not checked in. A frozen install is:

```powershell
# 1. extract the fixed embeddable distribution and write python312._pth
#    (python312.zip, ., Lib\site-packages, import site)
# 2. install the frozen closure with the frozen interpreter itself, offline
target\parity-runtime\runtime\python.exe -I -m pip install --no-index `
  --find-links target\parity-runtime\wheelhouse `
  -r resources\windows-parity\runtime-v2\requirements.lock `
  --only-binary=:all: --no-cache-dir --disable-pip-version-check
```

The installer must keep the interpreter paths hardcoded: `python.exe` and
`python312._pth` at the runtime root, plus `Lib\site-packages` on the path and
an active `import site` line. Without those two `_pth` entries the embeddable
interpreter cannot see the wheel closure. Console scripts land in `Scripts\`;
workers do not need them and they are not added to `PATH`.

Measured on the target machine (2026-11-04):

| Fact | Value |
|---|---|
| Offline install exit code / wall time | `0` / `123.955 s` |
| Wheelhouse (compressed wheels + pip) | 2,714,172,955 bytes, 50 files |
| Interpreter archive | 11,133,606 bytes |
| Installed runtime (`target/parity-runtime/runtime`) | 4,778,861,981 bytes, 32,580 files |
| Network used during install | none (`--no-index`, no `Downloading` / URL lines in the pip log) |
| Locked packages / installed distributions | 49 / 50 (49 + bootstrap `pip` 26.3) |

Evidence: `verification/windows-parity-runtime/` (`install-evidence.json`,
`runtime-probe.json`, `manifest-reverify.json`, `dependency-resolution.json`,
`README.md`). Probe result: every pinned distribution present at the exact
version; `torch`, `torchvision`, `diffusers`, `transformers`, `accelerate`,
`bitsandbytes`, `safetensors`, `huggingface_hub`, `onnxruntime`, `numpy`,
`PIL`, `psutil` import; `torch.version.cuda == 12.6`,
`torch.cuda.is_available() == true`, device `NVIDIA GeForce RTX 3080 Ti`
(capability 8.6). The probe does not create a CUDA context, so no VRAM is
reserved while another experiment owns the GPU.

## onnxruntime choice

`onnxruntime==1.31.1` is the latest stable release with an official
`cp312`/`win_amd64` wheel at freeze time
(`onnxruntime-1.31.1-cp312-cp312-win_amd64.whl`, SHA-256
`ae8a9a351efda685cc59a66226cdefaa37a126c1d66872ca23a2de9425326130`). Only the
CPU execution provider is part of the contract for BiRefNet, so the CPU package
is pinned; GPU execution providers are intentionally out of this closure.

## Limitations

- Content + offline-install/import proof only; no model inference is certified
  here. Per-model Gate 0 evidence lives in the model-specific verification
  directories.
- Python 3.12.10 is the last 3.12 release with official Windows binaries; later
  3.12 security releases are source-only.
- The runtime closure is frozen as-is; upgrades require a new change and a new
  runtime identity.
