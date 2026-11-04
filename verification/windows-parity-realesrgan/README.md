# Real-ESRGAN ncnn Vulkan Gate 0 evidence (G2-R1)

Case **G2-R1** from
`openspec/changes/archive/2026-11-04-achieve-windows-model-parity/test-plan.md`: "Real-ESRGAN ncnn
Vulkan 4× — 精确 4×、tile 固定、单 GPU；预算与时限". Implements task 1.3 of the
change. Frozen bundle: `resources/windows-parity/realesrgan-ncnn-vulkan/`.

Status: **pass** for the exact-4x / fixed-tile / single-GPU contract on the
target machine, including a real-photograph run. A separate
`manifest-reverify.json` re-hashes the official archive, every archive member,
the extracted install files and the pinned licenses against the checked-in
`asset-manifest.json`.

## Machine and bundle

- Windows 10 Pro 22H2, build `10.0.19045`; i7-13700K (24 logical CPUs);
  32 GB RAM; NVIDIA GeForce RTX 3080 Ti 12 GB, driver `596.49`
- Executable `realesrgan-ncnn-vulkan.exe` SHA-256
  `07e49f7cbb4ede01ae4dd4c399d3a7e5846e3d2085c3128eff881e55cb7b1a0c`
  from the official `xinntao/Real-ESRGAN` `v0.2.5.0` Windows zip
  (45,474,481 bytes, SHA-256
  `abc02804e17982a3be33675e4d471e91ea374e65b70167abc09e31acb412802d`)
- Models `realesrgan-x4plus.bin` / `.param` from the same archive; extracted
  members were re-hashed against the archive table and matched
- GPU window granted by the Lead explicitly; `nvidia-smi` was checked before
  and after each run and the GPU was released immediately after (~8 s total)

## Runs

All runs use the fixed command:

```
realesrgan-ncnn-vulkan.exe -i <input.png> -o <output.png> -n realesrgan-x4plus \
  -s 4 -t 128 -g 0 -j 1:1:1 -f png -v
```

`-x` (TTA) is never passed.

| Run | Input | Input dims / bytes | Output dims / bytes | Output SHA-256 | Wall time | Status |
|---|---|---|---|---|---|---|
| G2-R1 run 1 (official release sample) | `input-official-sample-220x220.png` | 220x220 / 117,553 | 880x880 / 1,191,677 | `33a47bc5c838a8bca29118e156672cfe8a89442153ffc6351a8d6ae8f9e15895` | 4.863 s | pass |
| G2-R1 run 2 (controlled) | `input-controlled-97x61.png` | 97x61 / 14,643 | 388x244 / 124,974 | `368946826d6c9bf1d6b94ed8c958b75a807a65ffa8559d3c547092a1a811d9b4` | 2.207 s | pass |
| G2-R1 run 3 (real photograph) | `input-nasa-real-photo-384x288.png` | 384x288 / 330,926 | 1536x1152 / 3,413,801 | `73e2d464097fa538c925c3fca9d4c9c5adaf0e1512316d070445bd0f7b1bb394` | 2.337 s | pass |

All three outputs are exactly 4x in both dimensions; run 2 uses dimensions that
are not multiples of the tile size or of 4, so the check is not a coincidence of
the sample. Run 3 uses a public-domain NASA photograph (Apollo 17 view of
Africa and Madagascar, `as17-148-22717`), center-cropped and resized to 384x288
before the frozen command — source URL, license and derivation are recorded in
`real-photo-input-source.json`. The 1536x1152 output has no visible tiling
seams at inspection scale and shows plausible restored cloud/coast detail.

Device identity, from the frozen executable's own `-v` stderr output:

- `[0 NVIDIA GeForce RTX 3080 Ti]  queueC=2[8] queueG=0[16] queueT=1[2]`
- `[1 Intel(R) UHD Graphics 770]` enumerated, not selected (`-g 0`)
- `nvidia-smi`: RTX 3080 Ti `GPU-78b715aa-ec51-31af-8813-eac9653458a7`,
  driver `596.49`, 12,288 MiB total; 746 MiB before run 1, 777 MiB after run 2,
  and 821 MiB after run 3, with GPU utilization back to 3–5% once the window
  ended. Run 3's stderr also confirms the device parser: the run JSON lists both
  Vulkan devices, with `[0 NVIDIA GeForce RTX 3080 Ti]` selected.

`g2-r1-input.json` / `g2-r1-controlled.json` / `g2-r1-real-photo.json` contain
the full command, the nvidia-smi snapshots, input/output records and the
pass/fail evaluation. `g2-r1-vulkan-device-identity.json` re-parses the captured
stderr logs of runs 1-2 into a standalone device record (post-run, no GPU work;
run 3's run-time parser already captured the device lines).
`manifest-reverify.json` independently re-hashes the downloaded official
archive, all 17 archive-member records, the 5 extracted install files and the 2
license files against `asset-manifest.json`. `.stdout.log` / `.stderr.log` files
are the raw program output.

## Evidence files

| File | Content |
|---|---|
| `g2-r1-input.json` | run 1 evidence (official sample image) |
| `g2-r1-input-220x220-4x.png` | run 1 output (880x880) |
| `g2-r1-controlled.json` | run 2 evidence (controlled image) |
| `g2-r1-controlled-97x61-4x.png` | run 2 output (388x244) |
| `g2-r1-vulkan-device-identity.json` | Vulkan device identity parsed from runs 1-2 logs |
| `input-official-sample-220x220.png`, `input-controlled-97x61.png` | committed inputs (byte-identical to run inputs) |
| `real-photo-input-source.json` | NASA public-domain real-photo input (Apollo 17 view of Africa) with source URL, license, derivation and run record |
| `input-nasa-real-photo-384x288.png` | real-photo input used by run 3 (384x288) |
| `g2-r1-real-photo.json` | run 3 evidence (real photograph) |
| `g2-r1-real-photo-4x.png` | run 3 output (1536x1152) |
| `manifest-reverify.json` | independent re-hash of archive, members, install files and licenses |
| `*.stdout.log`, `*.stderr.log` | raw program output for each run |

## Not run / limitations

- Peak VRAM during the run was not sampled: the process finishes in a few
  seconds and only `nvidia-smi` before/after snapshots were taken. No
  shared-memory fallback was observed (GPU memory returned to baseline).
- Single-run evidence only; this is not a throughput benchmark.
- ncnn tiles the input, so block-boundary differences from a PyTorch
  implementation are expected and accepted by the frozen contract.
