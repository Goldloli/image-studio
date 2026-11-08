# Windows 12 GB standard-tier acceptance (2026-11-08)

Machine: Windows 10 Pro 22H2 (19045) / i7-13700K / **RTX 3080 Ti 12,288 MiB (sm86, driver 596.49)** /
31.7 GiB RAM / 3840×2160 @150% / WebView2 157.0.4341.42.
Build: current `main` (`b861ae5`; local docs-only commits do not change code) via
`scripts/build-windows-internal.ps1 -SkipBundle` + `cargo build --release -p image-studio-desktop`.
Data root: **`C:\ist-accept`** — a brand-new isolated root that simulates a new user's first install
(the 2026-11-04 root `C:\ist-a` was not used and not modified).

This round covers the 12 GB **standard tier** on the Windows 10 target machine. Evidence files use the
`S*`/`O1` case ids from `openspec/changes/certify-windows-10-target-machine/test-plan.md`.

## Result summary

| Case | Result | Measured |
|---|---|---|
| S0 environment/build/quality gates | pass | cargo fmt/clippy/test (199 desktop + 46 protocol, 0 failed), frontend test/check/build, exe SHA-256 in [S0-environment-build.json](S0-environment-build.json) |
| S0 seed cache (`C:\ist-accept`) | pass | parity seed 132 artifacts / 123 unique / 0 missing / 0 mismatch, 112.5 GB hard-linked; Z-Image Gate 0 seed 25 links; cache total 135.4 GB, **zero download** |
| S1 fresh-root initial state | pass | 7 models, 0 installed, device meets thresholds for 7; Klein detail shows **标准档 1024×1024 · bfloat16** |
| S2 seeded-cache installs | pass | Klein ≈110 s (18.7 GB), BiRefNet ≈32 s, Real-ESRGAN ≈12 s, Z-Image ≈172 s (35.56 GB) |
| S3 Qwen-Turbo install (fresh user) | **blocked** | install-time NF4 prepare refused before quantization: available RAM 22.6 GiB (<28), commit headroom 24.8 GiB (<26). No ready record, no `.prepared` directory. See [S3-qwen-prepare-preflight.json](S3-qwen-prepare-preflight.json) |
| S4 Klein text-to-image | pass | 50.1 s total; 1024×1024 PNG 2,051,314 B `ad9fdf9c…`; host GPU used peak 11,008 MiB of 12,288 |
| S5 Z-Image text-to-image (≥12 GB-only model) | pass | 55.1 s total, 9 steps; 1024×1024 PNG 1,850,011 B `ac2c725d…`; host GPU used peak 10,707 MiB |
| S6 BiRefNet background removal | pass | 25.6 s; RGBA 1024×1024 1,987,389 B + 8-bit mask 10,375 B; CPU route |
| S7 Real-ESRGAN 4× | pass | 8.6 s; 1024→4096, 23,760,930 B `e9052829…`; ncnn Vulkan |
| S8 Klein single-image edit | pass | 46.6 s; 1024×1024 PNG 1,771,418 B `acf721ac…`; scene preserved, prompt applied |
| S10 export + restart | pass | exported PNG byte-identical to asset (SHA-256 `ad9fdf9c…`); after restart 4/7 models ready, 5 conversation turns persist |
| O1 runtime log | pass | `C:\ist-accept\logs\app.log` structured events incl. `install.verify.*`, `turn.preflight.done ms=…`, `generation.start available_gib=…` |
| S14 MOSS product install + generation | **pass** | low-memory install 364 s (transformer prepare 79.6 s, worker commit peak 1.952 GiB; cache `fe8c7bcb…`), 50-step generation **143 s**, 1024×1024 PNG 1,855,587 B `1e7bc1ac…`, host GPU peak 7,606 MiB |
| S15 Qwen-Image 2.1 product install + generation | **pass** | low-memory install 415 s (text encoder 122.2 s / 2.918 GiB, transformer 102.2 s / 2.902 GiB), 40-step generation **84 s**, 1024×1024 PNG 1,995,903 B `f92b0f93…`, host GPU peak 7,784 MiB |
| S3b low-memory prepare under simulated 16 GiB | pass | Qwen Turbo install through the UI at 11.35 GiB available RAM: preflight 11.75/12.95 GiB, prepare peaks 2.9 GiB per worker, caches byte-identical |
| S3c Qwen standard generation after cleanup fix | pass | Qwen Turbo 1024² 8 steps 40.7 s, PNG 2,010,413 B `92a9d4f8…` |
| W2 window sizes | pass | 960×640 与 1280×800：导航/输入/发送/模型选择可见（`W2-window-sizes.json`） |
| W4 native export dialog | pass | 导出 == 资产逐字节一致（`ff6109ab…`，`W4-native-export-dialog.json`） |
| W6 theme | pass | 浅色/深色切换生效，截图 62/63 |
| O2 watchdog | pass | 注入滞留 pending 回合，60 s 超时错误 + 重试入口（`O2-watchdog-injected-reap.json`） |
| O3 cancel coverage | pass | Qwen Turbo 6.26 s / BiRefNet 3.8 s / Real-ESRGAN 3.32 s 落定，无残留进程 |
| O4 preflight cancel | pass | 6.67 s 落定（bd ipw 可关闭） |
| O5 console windows | pass | 修复后 supervisor/python `MainWindowHandle=0`，生成与安装均无窗口 |
| O6 download retry | pass | 代理故障 21.2 s 失败并给出可重试错误；恢复后 53 s 安装成功 |
| O7 non-ASCII path | pass | `C:\ist-验收 中文` 安装 + 4× 放大 8.7 s，输出 4096² |
| O10 bad checksum | pass | 坏缓存被拒绝且无半成品；恢复缓存后 6.4 s 安装成功 |

Notes:

- GPU numbers are host-observed `nvidia-smi` samples (1 s interval) and include the ~1.4 GiB desktop
  baseline; they are not the worker's CUDA reserved figure.
- Qwen-Image 2.1 / MOSS-Image-SFT standard generation is **blocked** on this machine's current default
  pagefile state, not by an old install: a new user on the same machine hits the same gate. On
  2026-11-04 the same machine passed the gate via a 29.05 GiB commit headroom (pagefile had grown);
  today the auto-managed pagefile is 4.8 GiB and commit headroom is ~24–25 GiB. The environment
  experiment that grew the commit limit to 43.2 GiB (31.4 GiB headroom) is recorded in
  [S3-qwen-prepare-preflight.json](S3-qwen-prepare-preflight.json); Windows shrank it back before the
  retry, so a stable fix needs the owner to set a larger virtual memory size (admin + reboot).
- The Real-ESRGAN install used the seeded cache; the real-network download/retry path was not re-run.
- No console-window, watchdog, cancel, disk-full or network-fault cases were executed in this round;
  see the change test plan for their `not_run` status.

## Screenshots

`01-model-page-klein-expanded.png`, `03-klein-ready.png`, `04-birefnet-ready.png`,
`05-realesrgan-ready.png`, `06-zimage-ready.png`, `07-qwen-preflight.png`,
`10-klein-t2i-result.png`, `11-zimage-t2i-result.png`, `13-birefnet-result.png`,
`14-realesrgan-result.png`, `15-klein-edit-result.png`, `16-restart-model-state.png`,
`17-settings-runtime-probe.png`.
