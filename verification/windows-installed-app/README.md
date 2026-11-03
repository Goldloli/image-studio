# Windows installed application UI acceptance

Status: partial, 2026-11-02/03, on the target Windows 10 machine.

The NSIS-installed current-user application was launched with an isolated
`--data-dir` root and driven through its real WebView2 UI (remote debugging up
to the page plus Win32 automation for the native save dialog). All generated
images, task/works records and the exported PNG come from that installed
application, not from a helper or a test harness.

## Install from the UI

The isolated root cache was seeded with the 59 Gate 0 artifacts by SHA-256
before the UI action. Clicking 安装模型与运行环境 walked through:

| Time | Observed UI state |
|---|---|
| 23:49:27 | 正在下载 |
| 23:49:39 | 安装预算 53.43 GB, 未就绪 |
| 23:50:40 | 已安装 1, 准备隔离运行环境 |
| 23:51:41 | 已安装 1, 已就绪 (about 134 s total) |

The activation record reports runtime
`windows-python31210-torch2100-cu126-bnb0482-v1`, worker
`studio-zimage-cuda-worker-0.1.0`, model
`z-image-turbo-diffusers-cuda-nf4-v1`, 19,485 runtime inventory entries,
20 model files (`32,848,319,217` bytes), and an RTX 3080 Ti sm86 GPU probe.

## Three consecutive generations

| # | Prompt | Seed | Size | PNG | SHA-256 |
|---|---|---|---|---|---|
| 1 | 雨后的上海街角，一位撑伞的行人，写实摄影，真实自然光 | 3148150817 | 1024x1024 | [generated-1](generated/generated-1-rainy-shanghai.png) | `ee900070…a822` |
| 2 | An English editorial photograph of a ceramic teapot on a wooden table, soft window light | 2001858781 | 1024x1024 | [generated-2](generated/generated-2-teapot.png) | `a67d9f5f…83d6` |
| 3 | 中国城市夜景中的霓虹招牌与雨后街道，电影感构图 | 3070240146 | 1024x1024 | [generated-3](generated/generated-3-neon-street.png) | `39c71d75…6261` |

Each generation showed the real phase sequence in the UI (接收提示词 → 准备/
加载模型 → 编码提示词 → 采样 n/9 → 解码 → 保存候选 → 图片已生成). The frozen
plan recorded recipe `z-image-turbo-nf4-cuda-v1`, 9 steps, guidance 0.0,
512 token budget, NF4 transformer/text encoder, BF16 compute, VAE tiling and
`model_cpu` offload; 19 components; device `nvidia-cuda-sm86`.

## Export, restart and archives

- 导出 PNG opened the native save dialog; the exported file is
  `1,809,462` bytes with SHA-256
  `ee900070e1c51043cbdd078ea8898c677788ac10ecfb946b88da2c2a4d89a822`,
  identical to generated-1.
- Closing the native window and relaunching the installed executable with the
  same `--data-dir` restored all three turns, prompts, seeds and images.
- 任务 shows three completed jobs with job/attempt IDs and the 1024×1024
  request/actual size; 作品 lists three available works.

## Window layout at the current 150% display scale

| Requested client | CSS viewport | Device pixels | Horizontal overflow | Composer/send/model visible |
|---|---|---|---|---|
| 1280x800 | 1280x800 | 1920x1200 | no | yes |
| 960x640 | 961x640 | 1442x960 | no | yes |

Screenshots: [1280x800](screenshots/window-1280x800-150dpi.png),
[960x640](screenshots/window-960x640-150dpi.png). The machine reports window
DPI 144 (150% scaling) and devicePixelRatio 1.5.

## Native window controls and themes

| Control | Observed |
|---|---|
| Native caption | present (`WS_CAPTION`) |
| Maximize | `IsZoomed` true, client `2560x1377`, standard 7-px frame overscan, [screenshot](screenshots/window-maximized-150dpi.png) |
| Restore | window rectangle exactly equals the pre-maximize rectangle |
| Minimize | `IsIconic` true and restore works |
| Drag by caption | requested +120/+80 px, observed +120/+80 px |
| Light theme | `data-theme=light`, body `rgb(244,244,244)`, [screenshot](screenshots/theme-light.png) |
| Dark theme | `data-theme=dark`, body `rgb(32,32,32)`, composer/send visible, [screenshot](screenshots/theme-dark.png) |

## IME guard and UI cancel

- With a CDP composition active (`nihao`), an Enter key at code 229 did not
  create a generation job; after the composition ended, an explicit Enter
  created the next turn.
- Clicking 停止 at 8% moved the turn to `cancelled` with no job, attempt or
  asset created (`本次生成已取消`, [screenshot](screenshots/cancel-settled.png)).
  The cancellation waited for the background preflight's 32.8 GB verification
  to finish before settling; this is recorded as a latency observation, not a
  functional pass for a real IME.

## Settings, preferences and keyboard

- 设置 page sections report real values: storage (37.47 GB data / 32.85 GB model /
  4.61 GB runtime / 35.56 GB cache), runtime (`windows · x86_64`, Diffusers
  CUDA + private Python, model/runtime/probe ready, no known error, diagnostics
  not started) and about (0.1.0 Preview, Windows-only Z-Image capability, no
  auto-update/online model/storage migration). 定位 opened a native Explorer
  window for the isolated data directory.
- Dark theme and an unsent draft both survived a full application restart; the
  theme was reverted to light afterwards.
- Tab from the composer reaches `1:1 · 1 张`, 复制提示词, 模型选择, 发送并生成 and
  创作. Shift+Enter inserts a newline without creating a job.

Screenshots: [general](screenshots/settings-general.png),
[storage](screenshots/settings-storage.png),
[runtime](screenshots/settings-runtime.png), [about](screenshots/settings-about.png).

## Export overwrite confirmation (P08)

Before the fix, exporting a PNG to an existing target silently overwrote it.
`asset_export` now shows a native Yes/No confirmation before replacing an
existing target: declining returns the same cancelled shape and leaves the old
file untouched, confirming goes through the existing atomic overwrite path.
The rebuilt package
(`Image Studio_0.1.0_x64-setup.exe`, SHA-256 `56837792…f047`) shows the dialog;
the manually declined run left both dummy targets byte-identical. Confirmed
overwrite remains covered by the existing library unit test and the decision
unit test in `host::tests`.

## Known open question

While re-installing the rebuilt package, a **second simultaneous instance with
a different `--data-dir` exited with code 101**, while the same instance alone
launches and runs normally. This is unrelated to the export change and needs a
focused look (likely WebView2 user-data-folder contention); the earlier
independent-root smoke used a previous package. Same-root rejection remains the
expected single-instance behaviour.

The current source no longer reproduces it on a Windows 11 development machine
(different roots at 0 ms/500 ms/25 s offsets, the same root, and a launch after
force-killing the first instance all kept both hosts running), but the panic
path is proven: any WebView2 setup failure ends in
`tauri-2.12.5/src/app.rs:1444` with exit code `101`. The application now
reports that class of failure explicitly — see
[windows-startup-diagnosis](../windows-startup-diagnosis/README.md) — and the
target machine still has to re-run W07 with the installed NSIS build.

## Pending

100%/200% OS scaling and a human visual review of both themes remain pending;
the dark/light switch and native window controls were verified programmatically.
Real Microsoft Chinese IME composition, a network-denied variant and the
clean-user/WebView2 gates are not covered by this evidence. The generated
samples are real model output but the visual quality review is still a human
step.

Machine-readable record: [ui-acceptance.json](ui-acceptance.json).
Screenshots: [install ready](screenshots/install-ready.png),
[generation conversation](screenshots/generation-conversation.png),
[task center](screenshots/task-center.png),
[works library](screenshots/works-library.png),
[provenance panel](screenshots/provenance-panel.png).
