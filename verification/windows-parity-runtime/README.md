# Windows v2 runtime freeze and offline-install evidence

Case: change task **1.2** — "冻结 Windows v2 运行时（Python 3.12.10 / torch
2.10.0+cu126 / torchvision 0.25.0 / diffusers 0.41 / transformers 5.3 /
accelerate / bitsandbytes / onnxruntime），登记 wheel 与运行时清单 SHA-256，
证明离线可安装" (`openspec/changes/achieve-windows-model-parity/tasks.md`).

Runtime under test:
`windows-python31210-torch2100-cu126-diff041-transformers53-v2`
(`resources/windows-parity/runtime-v2/`).

Status: **pass** (content frozen, hash re-verified, offline install and imports
proven). Model inference is explicitly out of scope for this case.

## Machine

- Windows 10 Pro 22H2, build `10.0.19045` (target-certified machine)
- 13th Gen Intel Core i7-13700K, 24 logical CPUs, 34,066,575,360 bytes RAM
- NVIDIA GeForce RTX 3080 Ti 12 GB, driver `596.49`
- Disk at run time: 2,048,167,010,304 total / 1,428,426,289,152 free bytes
- Frozen runtime runs on its own Python 3.12.10 embeddable interpreter; the
  host Python 3.14.7 was used only to drive cross-target pip resolution.

## What was done

1. Resolved the closure with `pip install --dry-run --report` for
   `cp312`/`win_amd64`, pinned every distribution into
   `requirements.lock` (49 packages), and downloaded exactly the pinned wheels
   (`torch`/`torchvision` from `https://download.pytorch.org/whl/cu126`,
   everything else from PyPI).
2. Built `resources/windows-parity/runtime-v2/runtime-manifest.json`:
   51 artifacts (49 wheels + Python embeddable archive + `pip` bootstrap),
   per-file URL/bytes/SHA-256. Each local hash was compared with the official
   index metadata during the build.
3. Extracted the frozen Python 3.12.10 embeddable distribution to
   `target/parity-runtime/runtime`, wrote the required `python312._pth`
   (`python312.zip`, `.`, `Lib\site-packages`, `import site`), and bootstrapped
   `pip 26.3` from the local wheelhouse.
4. Ran the offline install **with the frozen interpreter itself**:
   `python.exe -I -m pip install --no-index --find-links <wheelhouse> -r requirements.lock
   --only-binary=:all: --no-cache-dir` — no index URL, no downloads.
5. Ran `probe_runtime_v2.py` on the frozen interpreter to verify every pinned
   distribution and the library imports / CUDA availability.
6. Independently re-hashed every artifact from disk against the checked-in
   manifest (`manifest-reverify.json`).

## Results

| Item | Expected | Actual | Status |
|---|---|---|---|
| Offline install exit code | 0 | 0 | pass |
| Offline install wall time | recorded | 123.955 s | pass |
| pip install log network activity | none | 0 `Downloading` lines, 0 URLs, `Looking in links` only | pass |
| Installed distributions | 49 locked + bootstrap pip | 50 `.dist-info` (49 exact-version pins) | pass |
| `torch` import | 2.10.0+cu126 | 2.10.0+cu126 | pass |
| `torch.version.cuda` | 12.6 | 12.6 (cuDNN 91002) | pass |
| `torch.cuda.is_available()` | true | true, 1 device | pass |
| CUDA device | RTX 3080 Ti, sm86 | NVIDIA GeForce RTX 3080 Ti, capability 8.6 | pass |
| `torch.cuda.is_bf16_supported()` | true | true | pass |
| `diffusers` | 0.41.0 | 0.41.0 | pass |
| `transformers` | 5.3.0 | 5.3.0 | pass |
| `onnxruntime` | 1.31.1 (CPU) | 1.31.1 | pass |
| Other pins (`accelerate`, `bitsandbytes`, `safetensors`, `huggingface_hub`, `numpy`, `PIL`, `psutil`) | exact | exact | pass |
| Manifest re-verification | 51 artifacts, 49 installed dists | 0 mismatches | pass |

Space measured for the installer budget:

- wheelhouse: 2,714,172,955 bytes (50 files) — download/install cache, deletable after install
- Python embeddable archive: 11,133,606 bytes
- installed runtime: 4,778,861,981 bytes (32,580 files); `torch` alone is
  4,160,943,973 bytes and bootstrap `pip` is 16,801,036 bytes
- packaged steady state if bootstrap `pip` is pruned: 4,762,060,945 bytes
- install peak while wheelhouse + runtime + Python zip coexist: 7,504,168,542 bytes
- install time: 123.955 s

The probe deliberately does **not** create a CUDA context (no tensor
allocation), so it is safe while another experiment owns the GPU. The frozen
runtime's own pip version is 26.3; dependency resolution used host pip 26.2.1.

`pip` is kept in the verified tree only because the frozen interpreter installed
its own wheel closure offline. `runtime-manifest.json.installer_contract.packaged_runtime`
records the recommendation (prune `Lib\site-packages\pip`, `pip-26.3.dist-info`
and entry-point scripts from the packaged product runtime, or budget the
16,801,036 bytes if kept for repair/diagnostics).

## Evidence files

| File | Content |
|---|---|
| `install-evidence.json` | offline install facts + checks + space + onnxruntime choice; references the raw pip log and resolver report hashes |
| `runtime-probe.json` | frozen-interpreter probe: pinned dists, imports, versions, CUDA driver-level availability (no context) |
| `manifest-reverify.json` | independent re-hash of all 51 artifacts and the 49 installed distribution versions |
| `dependency-resolution.json` | resolver command/indexes/target, 49 resolved packages with canonical URLs + SHA-256 |
| `runtime-probe.log` (build dir) | raw probe stdout |

Raw build artifacts (gitignored, under `target/parity-runtime/`): wheelhouse,
runtime tree, `offline-install.log`, `offline-install-timing.txt`,
`pip-resolve-report.json`, `pip-resolve.log`, `pip-download-*.log`.

## Reproduction

```powershell
python tools/windows-parity/runtime/build_runtime_manifest_v2.py
# extract+patch runtime, bootstrap pip, then:
target\parity-runtime\runtime\python.exe -I -m pip install --no-index `
  --find-links target\parity-runtime\wheelhouse `
  -r resources\windows-parity\runtime-v2\requirements.lock `
  --only-binary=:all: --no-cache-dir --disable-pip-version-check
python tools/windows-parity/runtime/collect_install_evidence.py
target\parity-runtime\runtime\python.exe -I tools/windows-parity/runtime/probe_runtime_v2.py `
  --output verification/windows-parity-runtime/runtime-probe.json
python tools/windows-parity/runtime/verify_runtime_manifest.py
python tools/windows-parity/runtime/summarize_resolution.py
```

## Not run / limitations

- No model loading or inference: Gate 0 per model is recorded in the
  model-specific evidence directories, not here.
- No `--cuda-context-probe` run: CUDA availability is proven at driver level
  only; a real CUDA operator probe must run inside a granted GPU window and is
  not required for the 1.2 acceptance criteria.
- The wheelhouse is not checked in; the manifest + indexes allow rebuilding it
  (hash-verified), and the local build copy is retained under `target/`.
- Python 3.12.10 is the last 3.12 release with official Windows binaries.
