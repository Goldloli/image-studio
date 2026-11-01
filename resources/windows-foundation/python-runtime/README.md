# Windows Gate 0 runtime candidate

This directory records the fixed Python and wheel candidate used by Windows
Gate 0. It does not assert that the combination imports, exposes the RTX 3080
Ti to CUDA, supports NF4, or completes model inference.

Fixed inputs:

- Python 3.12.10 Windows embeddable x64 from `python.org`. This is the last
  Python 3.12 release with official binary installers; later 3.12 security
  releases are source-only and are not compiled on the target machine.
- PyTorch `2.10.0+cu126` from `download.pytorch.org/whl/cu126`
- Remaining Python wheels in `requirements.lock` from `pypi.org`
- Model content closed by
  `../z-image-diffusers/model-manifest.json`

The target is Windows 10 x64 with an RTX 3080 Ti (`sm86`) and Intel UHD 770.
Intel is not a fallback. The runtime must select CUDA by NVIDIA name, PCI bus,
compute capability, and memory rather than by adapter enumeration.

Gate 0 downloads artifacts into an ignored validation cache, verifies their
size and SHA-256, extracts only into a private root, and never installs into or
modifies the system Python. A failure is retained as evidence and does not
activate a product installation.

## Reproduction

The checked-in manifest fixes 38 compressed artifacts totaling
`2,713,854,538` bytes. A development rebuild uses the host Python only to
download and extract the fixed inputs:

```powershell
python -m pip download --no-deps --only-binary=:all: `
  --platform win_amd64 --implementation cp --python-version 3.12 --abi cp312 `
  --index-url https://pypi.org/simple `
  --find-links https://download.pytorch.org/whl/cu126/torch/ `
  --dest target/windows-gate0/downloads `
  -r resources/windows-foundation/python-runtime/requirements.lock
```

Extract `python-3.12.10-embed-amd64.zip` into
`target/windows-gate0/runtime`, add `Lib\site-packages` and `import site` to
`python312._pth`, and install the fixed wheels with
`pip install --no-index --find-links ... --no-deps --target ...`. The runtime
probe is:

```powershell
target/windows-gate0/runtime/python.exe -I tools/windows-gate0/probe_runtime.py `
  --output verification/windows-gate0/runtime-probe.json
```

On the target machine this probe passed imports, CUDA selection, BF16, and NF4
forward/backward. That is G02-level evidence only; G03-G06 remain unrun until
the complete model executes.
