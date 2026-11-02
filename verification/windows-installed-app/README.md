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

## Pending

100%/200% OS scaling, dark theme, native minimize/maximize/drag, real Microsoft
IME composition, a network-denied variant and the clean-user/WebView2 gates are
not covered by this evidence. The generated samples are real model output but
the visual quality review is still a human step.

Machine-readable record: [ui-acceptance.json](ui-acceptance.json).
Screenshots: [install ready](screenshots/install-ready.png),
[generation conversation](screenshots/generation-conversation.png),
[task center](screenshots/task-center.png),
[works library](screenshots/works-library.png),
[provenance panel](screenshots/provenance-panel.png).
