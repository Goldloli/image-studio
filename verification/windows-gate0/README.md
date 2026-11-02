# Windows Gate 0

Status: Gate 0 passed on the target Windows 10 machine, 2026-11-01.
Product installation, controller generation, an empty-cache production install,
an interrupted-then-resumed production install, cancellation during runtime
preparation/probe and activation rejection were subsequently validated as
separate evidence linked below; those results do not change the original Gate
0 measurements.

## Fixed runtime

- Python 3.12.10 Windows embeddable x64
- PyTorch 2.10.0+cu126
- Diffusers 0.36.0
- Transformers 4.57.3
- Accelerate 1.12.0
- bitsandbytes 0.48.2
- RTX 3080 Ti, UUID `GPU-78b715aa-ec51-31af-8813-eac9653458a7`,
  PCI bus `00000000:01:00.0`, compute capability `8.6`, 12288 MiB

## Selected candidate

Candidate A:

- transformer and text encoder: bitsandbytes NF4, BF16 compute, double
  quantization
- VAE: BF16 with tiling
- Diffusers model CPU offload
- PyTorch native memory-efficient SDPA
- FlowMatchEulerDiscreteScheduler
- 1024x1024, one image, 9 steps, guidance 0.0, 512 token limit

Candidate B was loaded successfully but was not needed after candidate A
passed.

## Three independent processes

| Run | Load | Execute | CUDA reserved peak (GiB) | Private commit peak (GiB) | Minimum available RAM (GiB) | Image |
|---|---:|---:|---:|---:|---:|---|
| 1 | 24.36 s | 14.33 s | 8.87 | 18.95 | 8.77 | [cat](candidate-a-run1-cat.png) |
| 2 | 24.38 s | 14.45 s | 8.87 | 23.69 | 9.16 | [teapot](candidate-a-run2-teapot.png) |
| 3 | 23.89 s | 14.45 s | 8.87 | 18.93 | 10.63 | [desk](candidate-a-run3-desk.png) |

All runs are below 10.5 GiB CUDA reserved, 24 GiB private commit, 600 seconds
load, and 900 seconds execution. Every run produced a decodable 1024x1024 RGB
PNG with 9 step callbacks.

## Model content

The 20 downloaded files, including 19 inference components, total
`32,848,305,533` bytes and match the fixed revision
`f332072aa78be7aecdf3ee76d5c247082da564a6`.

ModelScope `Tongyi-MAI/Z-Image-Turbo@master` was independently checked against
all 20 entries. The seven large safetensors files match ModelScope's API
SHA-256 values; every remaining file was downloaded from ModelScope and hashed
locally. See `modelscope-equivalence.json`.

## Retained failure

The initial default-SDPA/float32-VAE run generated a valid image but used
`12.55 GB` (`11.69 GiB`) CUDA reserved, above the 10.5 GiB budget. It remains recorded in
`candidate-a-default-attention-failed.json`. The fixed configuration uses
memory-efficient SDPA and BF16 VAE.

## Measurement limitations

Process private commit and available RAM were sampled at 50 ms. CUDA reserved
was sampled at stage and callback boundaries. `nvidia-smi` did not expose a
per-process compute-apps value under this WDDM setup; global NVML memory and
Torch allocator values are retained. WDDM shared-memory peak is unavailable
from the selected tooling.

## Product closure evidence

The product installer acceptance used the validated Gate 0 cache as immutable
input and then exercised the formal installer instead of the Gate 0 helper:

- all 20 ModelScope-verified model files and the private Python/wheel closure
  were verified again;
- the installed worker reported the fixed runtime, model snapshot, worker
  build, RTX 3080 Ti, sm86 and required package versions;
- all 19 model components were frozen into the Windows generation plan;
- the direct supervisor protocol and `GenerationController` both produced a
  valid 1024x1024 RGB PNG and matching SHA-256 candidate/asset identity.
- loading-time and sampling-time cancellation both ended with zero outputs;
- force-killing the real supervisor ended the job as failed with zero outputs,
  and a new job in the same installed runtime then generated a committed Asset.

Reports:

- [Product install](product-install.json)
- [Installed worker protocol](product-install-worker-protocol.json)
- [Controller generation](product-controller-generation.json)
- [Controller asset](product-controller-primary.png)
- [Cancellation and supervisor-kill recovery](product-cancel-recovery.json)
- [Post-recovery asset](product-recovery.png)

That product closure used immutable validation-cache bytes. A separate
empty-cache acceptance exercised the production ModelScope URLs and runtime
sources without seeding or copying any artifact:

- all `32,848,319,217` declared model download bytes and
  `2,713,862,823` runtime bytes were fetched into a new root;
- all 20 model files and 39 runtime artifacts passed their fixed byte counts
  and SHA-256 identities before installation;
- private runtime installation, full inventory and the real GPU/NF4 probe
  reached the same runtime, worker and model identities as the cache-seeded
  installation;
- the run took `1047.50` seconds and the model source was exclusively
  `www.modelscope.cn`; Hugging Face remained provenance metadata only.

ModelScope closed several long-lived CDN responses before a segmented object
completed. The installer now resumes each incomplete segment from its verified
prefix with bounded retries instead of failing the whole installation.

Report:

- [Empty-cache ModelScope install](product-empty-cache-install.json)

## Interrupted and resumed install

A second acceptance ran the same production ModelScope closure from an empty
root, cancelled it after `310,721,780` bytes (about 296 MiB), and then resumed
in the same root:

- cancellation took effect 9.01 seconds after start: the phase history was
  `not_installed`, `preparing`, `downloading`, `cancelled`, and the cache kept
  33 entries including 16 verified segment files totalling `310,080,876` bytes;
- the same root restarted from the retained prefix and the phase history became
  `cancelled`, `preparing`, `downloading`, `verifying`, `installing_runtime`,
  `probing`, `ready`;
- the resumed run verified the complete `35,562,182,040` bytes, installed the
  private runtime (19,485 inventory entries) and reached the frozen
  `windows-python31210-torch2100-cu126-bnb0482-v1` runtime,
  `studio-zimage-cuda-worker-0.1.0` worker and
  `z-image-turbo-diffusers-cuda-nf4-v1` model identities with an RTX 3080 Ti
  sm86 GPU probe;
- the resumed transfer took `3110.89` seconds because the ModelScope/PyTorch
  CDN trickled the remaining large wheel; the earlier uninterrupted empty-cache
  run of the same closure took `1047.50` seconds, so this is a throughput
  observation, not a functional deviation.

Command:

```powershell
cargo test -p image-studio-desktop --locked `
  actual_windows_product_installs_from_an_empty_cache_and_resumes_after_cancel `
  -- --ignored --nocapture
```

Report:

- [Interrupted and resumed install](product-interrupted-resume.json)

## Runtime cancellation and retry

A cache-seeded acceptance started the formal install without any download and
cancelled it twice before activation:

- during `installing_runtime` (99.33 seconds after start) the phase history
  ended `cancelled`; no Ready record was written and the manager reported idle
  99.94 seconds after start;
- a retry then cancelled during the real GPU/NF4 probe (261.16 seconds): the
  probe child process tree was terminated through the Windows Job, the phase
  history ended `cancelled`, and the partial runtime stayed inactive;
- the third start re-verified the seeded closure and reached `ready` in
  199.48 seconds with the frozen runtime/worker/model identities and the
  RTX 3080 Ti sm86 probe;
- all 59 validation-cache entries came from the Gate 0 artifacts, so this
  proves cancellation and retry behaviour rather than network behaviour.

Report:

- [Runtime cancellation and retry](product-runtime-cancel-retry.json)

## Activation failures

The cache-seeded product install then exercised the activation gate against two
deliberately damaged components:

- a same-length corruption of `scheduler/scheduler_config.json` returned
  `SHA-256 校验失败`, moved the installation to `unavailable`, left the damaged
  file in place (no silent overwrite), and kept the 59-entry cache unchanged;
  restoring the exact bytes recovered readiness without reinstalling or
  downloading;
- deleting `model_index.json` returned `文件缺失`, kept the installation
  `unavailable`, left the other 19 model files and all cache entries intact, and
  no activation path re-fetched the missing component.

Real insufficient-disk-space acceptance still requires a small test volume and
remains pending; the space calculation itself is covered by
`installation_space_accounts_for_separate_and_shared_volumes`.

Report:

- [Activation failures](product-activation-failures.json)

Empty-cache activation, deliberate interruption/resumption, runtime/probe
cancellation with retry, and corrupt/missing-component activation rejection are
now evidenced; space-failure acceptance, network-denied cache installation,
clean-user installation, complete IME/DPI interaction and Mac regression remain
separate pending gates.
