# Image Studio

[English](README.md) | **简体中文**

[![质量门](https://github.com/Goldloli/image-studio/actions/workflows/quality.yml/badge.svg)](https://github.com/Goldloli/image-studio/actions/workflows/quality.yml)
[![最新发布](https://img.shields.io/github/v/release/Goldloli/image-studio)](https://github.com/Goldloli/image-studio/releases/latest)
[![许可证：Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Image Studio 是面向 **macOS arm64** 与 **Windows x64** 的本地优先桌面图片创作
工作台，使用 Rust、Tauri 2 与 React 构建。它支持文生图、基于指令的单图编辑、
带 RGBA/mask 输出的去背景，以及精确 4× 放大；模型推理和项目数据都留在本机。

![Image Studio 生成工作区](verification/windows-installed-app/screenshots/generation-conversation.png)

## 最新版本：v0.1.1

本次发布在首个公开版本的基础上新增 8 GB 显存执行档位、安装期低内存量化路径，
并完成一轮 Windows 桌面可靠性修复：

- **8 GB 紧凑档** —— Qwen-Image 2.1 / Turbo、MOSS-Image-SFT 与 FLUX.2 Klein 4B
  （文生图与编辑）可在 RTX 2060 SUPER 8 GB（Turing sm75）上以 768×768、
  `float16` 运行，实测 CUDA reserved 峰值 5.44–5.96 GiB。Z-Image Turbo 仍要求
  12 GB 显存。
- **安装期低内存量化** —— NF4 量化改为通过短生命周期子进程分片流式处理（约
  1 GiB 一组，worker commit 峰值 1.95–3.6 GiB），安装门槛降到可用物理内存
  ≥ 8 GiB 且 commit 余量 ≥ 6 GiB。16 GiB 内存机器现在可以安装 Qwen 与 MOSS。
- **Windows 桌面修复** —— 发送预检不再重复哈希整个安装（60 s+ → 约 46 ms），
  60 s 回合看门狗兜底滞留发送，取消在 3–7 s 内落定，下载按文件重试并自动修复
  坏分片，生成/安装不再弹控制台窗口，`<data>/logs/app.log` 保存有界结构化日志。
- **真机验证** —— Windows 11 / RTX 2060 SUPER 8 GB 覆盖紧凑档，Windows 10 22H2 /
  RTX 3080 Ti 12 GB 覆盖标准档；GitHub Actions 在 Windows x64 与 macOS arm64 上
  质量门均为绿色。

完整变更与已知限制见[最新发布说明](https://github.com/Goldloli/image-studio/releases/latest)。

| 模型页显示紧凑档 | 作品库 |
|---|---|
| ![模型页紧凑档](verification/windows-8gb-tier/model-compact-tier.png) | ![作品库](verification/windows-installed-app/screenshots/works-library.png) |

## 核心特性

- **七个认证型号、八项能力**，macOS 与 Windows 均已完成真实执行验收。
- **两个执行档位**：标准档（≥ 12 GB 显存、1024²、`bfloat16`）与紧凑档
  （≥ 8 GB 显存、768²、`float16`），由宿主根据设备事实自动选择。
- **本地执行**：不依赖托管图片 API，也不把项目数据上传到云端。
- **会话式创作**：草稿、提示词、参数、进度、结果、重试和取消按会话组织。
- **确定性模型身份**：固定 revision、逐文件 SHA-256、运行时、配方和 worker
  协议身份。
- **持久本地作品库**：SQLite 保存草稿、任务、Attempt、正式资产、任务历史与
  制作记录。
- **严格的进程监督**：有界 worker 输出、取消、后代回收、执行租约和崩溃恢复。
- **双平台发布**：GitHub Actions 构建并验证 Windows NSIS 安装包与 macOS arm64
  `.app`。

## 支持的模型

| 型号 | 能力 | 固定配方 |
|---|---|---|
| Z-Image Turbo | 文生图 | 4-bit 加载、9 步、1:1 / 4:3 / 3:4 |
| FLUX.2 Klein 4B distilled | 文生图、单图编辑 | 4-bit 加载、4 步、guidance 1.0 |
| BiRefNet general epoch_244 | 去背景、RGBA + mask | ONNX Runtime CPU、float32 |
| Real-ESRGAN x4plus | 精确 4× 放大 | ncnn Vulkan、tile 128 |
| Qwen-Image 2.1 | 文生图 | 4-bit（NF4）、40 步、guidance 1.0 |
| Qwen-Image 2.1 Turbo | 文生图 | 4-bit（NF4）、8 步、官方 Turbo 采样 |
| MOSS-Image-SFT | 文生图 | 4-bit（NF4）、50 步、guidance 3.0 |

### 执行档位

| 档位 | 显存 | 画布 | 计算精度 | 加载策略 |
|---|---|---|---|---|
| `standard` | ≥ 12 GB | 1024² | `bfloat16` | 分阶段 / 组件卸载 |
| `compact` | ≥ 8 GB | 768² | `float16` | 分阶段，同一时间只驻留一个 NF4 组件 |

两个档位共享同一份 25–33 GB 模型下载与同一份准备好的 NF4 缓存，只有配方身份
和精确参数表不同。只满足紧凑档门槛的设备会自动降级，模型页标注「紧凑档
768×768」；低于两档门槛的设备保持禁用，并显示未满足的最底要求。BiRefNet 与
Real-ESRGAN 本来就不受显存限制。

**8 GB 支持（已在 RTX 2060 SUPER、Turing sm75 上实测）**：Qwen-Image 2.1、
Qwen-Image 2.1 Turbo、MOSS-Image-SFT 与 FLUX.2 Klein 4B（文生图与编辑）以紧凑档
运行，实测 CUDA reserved 峰值 5.44–5.96 GiB。Z-Image Turbo 因模型在无 BF16 的
Turing 架构上存在 fp16 溢出，仍要求 12 GB 显存。

**8 GB 参考卡性能（紧凑档 768×768）**：每个任务都会新起 worker，总耗时包含组件加载。

| 模型 | 步数 | 总耗时 | 其中采样 |
|---|---|---|---|
| Qwen-Image 2.1 Turbo | 8 | 84.5 s | 66.3 s |
| Qwen-Image 2.1 | 40 | 391.3 s | 358.1 s |
| MOSS-Image-SFT | 50 | 752.1 s | 733.8 s |
| FLUX.2 Klein 4B 文生图 | 4 | 145.6 s | 9.8 s |
| FLUX.2 Klein 4B 编辑 | 4 | 166.1 s | 10.3 s |

在 Windows 10 / RTX 3080 Ti 12 GB 目标机上，标准档实测 Klein 50.1 s、Z-Image
Turbo 55.1 s、BiRefNet 25.6 s、Real-ESRGAN 8.6 s、Klein 编辑 46.6 s，单次
Qwen-Image 2.1 / MOSS-Image-SFT 生成分别为 84 s / 143 s；该机安装期量化的
worker commit 峰值约 2–3 GiB。证据见
[`verification/windows-8gb-tier/`](verification/windows-8gb-tier/README.md)、
[`verification/windows-12gb-standard-tier/`](verification/windows-12gb-standard-tier/README.md)、
[`doc/15-windows-8gb-tier.md`](doc/15-windows-8gb-tier.md) 与
[`doc/16-zimage-quantization-investigation.md`](doc/16-zimage-quantization-investigation.md)。

权威模型目录位于
[`resources/model-catalog/catalog.json`](resources/model-catalog/catalog.json)。
Windows 使用独立冻结的 NF4、ONNX 与 ncnn 配置；macOS 使用已验证的 MFLUX/MLX
和 ONNX 运行时。

## 系统要求

- **Windows**：Windows 10 22H2 或 Windows 11，x64。current-user NSIS 安装包内置
  WebView2 离线安装器。模型可用性取决于显卡（紧凑档 ≥ 8 GB 显存，标准档
  ≥ 12 GB 显存）；安装期量化需要可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB，
  建议 16 GiB 及以上内存。
- **macOS**：Apple Silicon（arm64）、macOS 11 或更高版本。模型执行使用已验证的
  MFLUX/MLX 与 ONNX 运行时；模型页会显示设备可用性。
- **磁盘**：每个生成模型需下载 25–33 GB 并保留一份准备好的 NF4 缓存。建议每个
  生成模型预留至少 40 GB，完整模型集约 150 GB。模型权重与私有 Python 运行时不
  随安装包分发。

## 下载与安装

从最新的已发布 [GitHub Release](https://github.com/Goldloli/image-studio/releases/latest)
下载文件，并使用对应的 `SHA256SUMS-*.txt` 校验：

- **Windows x64**：`Image.Studio_<version>_x64-setup.exe`
- **macOS arm64**：`Image-Studio-<version>-macos-arm64.zip`

```bash
sha256sum -c SHA256SUMS-windows.txt
sha256sum -c SHA256SUMS-macos.txt
```

当前产物未签名、未公证：

- Windows 可能显示 SmartScreen。确认 SHA-256 一致后，才使用
  **更多信息 → 仍要运行**。
- macOS 可能被 Gatekeeper 拦截。确认 SHA-256 一致后，Control-click 应用并选择
  **打开**，或运行：

```bash
xattr -dr com.apple.quarantine "Image Studio.app"
```

macOS 产物需要 Apple Silicon 与 macOS 11 或更高版本。

## 主要页面

- **创作**：多会话文生图、源图工作流、即时占位、真实步骤进度、取消与重试。
- **灵感**：可搜索的提示词库，并保留来源归属。
- **功能**：只展示有真实已安装模型支撑、可立即开始的能力。
- **模型**：显示安装、运行时、设备门槛、硬件适配、执行档位与许可信息。
- **作品**：跨会话浏览不可变正式资产，支持筛选、预览、制作记录以及原生
  PNG/RGBA/mask 导出。
- **任务**：宿主持久库中的状态筛选、检索、稳定分页、Attempt 历史、错误和恢复建议。
- **设置**：持久外观、显式新会话默认值、存储位置、诊断状态、运行时清单与有界日志。

## 从源码构建

前置条件：

- Rust **1.99.1**，由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定
- Node.js **24.19.0**、npm **11.17.0**
- Python **≥3.11**（建议 **3.13**），用于运行仓库验证脚本；模型 worker 使用各自固定的私有 Python 运行时，不依赖系统 Python 版本。
- 目标平台所需的 Tauri 系统依赖

```bash
npm --prefix apps/frontend ci
npm --prefix apps/frontend run build
cargo build --workspace --locked
cargo run -p image-studio-desktop --locked
```

### macOS 开发包

```bash
python3 scripts/prepare-macos-dev-app.py
open "target/dev/Image Studio.app"
```

### Windows sidecar

Windows 的 Clippy、测试和 Tauri 构建要求
`apps/desktop/binaries/` 中已有 sidecar。CI workflow 和
[`scripts/build-windows-internal.ps1`](scripts/build-windows-internal.ps1)
会自动构建并复制。

### 仅预览前端

```bash
npm --prefix apps/frontend run dev
```

浏览器预览不提供原生安装、本地生成、持久化或进程监督。修改界面后，需要重新
构建前端和桌面应用，才能刷新内嵌生产资源。

## 本地优先架构

```text
React 界面
  ↓ 受限 Tauri IPC
桌面宿主 / StudioService
  ├─ 精确模型安装与完整性校验
  ├─ SQLite 本地库、草稿、任务、Attempt 与资产
  ├─ 生成控制器与硬件门槛
  └─ inference supervisor
       ↓ 有界 JSON 协议
     Python / ONNX / 原生 worker
```

| Workspace 区域 | 职责 |
|---|---|
| `apps/desktop` | Tauri 宿主、受限 IPC、安装器、存储、诊断与进程 adapter |
| `apps/frontend` | React 工作区、功能/模型/作品/任务/设置页面与画板 |
| `crates/studio-core` | 类型身份、计划、配方、Attempt 状态与校验 |
| `crates/worker-protocol` | 帧边界、握手、身份检查与生成协议 |
| `tools/inference-supervisor` | 宿主 EOF、执行租约、worker/后代监督与回收 |
| `tools/*worker*` | MFLUX、ONNX、Diffusers/CUDA 与 Real-ESRGAN 执行 |

核心与协议 crate 不依赖 GUI 或推理框架。worker stdout 只传有界协议消息，
stderr 只传有界日志；产品进程使用参数数组启动，不经过 shell。

## 本地数据与安全

默认路径：

- macOS 数据：`~/Library/Application Support/ImageStudio`
- macOS 缓存：`~/Library/Caches/ImageStudio`
- macOS 日志：`~/Library/Logs/ImageStudio`
- Windows 根目录：`%LOCALAPPDATA%\ImageStudio`，内含 `data`、`cache`、`logs` 与
  `work` 子目录
- 应用日志：`<data>/logs/app.log`（有界，8 MiB 轮转）

使用 `--data-dir PATH` 可以创建隔离数据根（其下自动创建 `data`、`cache`、
`logs` 与 `work`）；同一数据根同一时间只允许一个宿主。Windows 上数据路径必须
足够短，保证私有 Python 运行时能打开最长路径文件，宿主会在启动前校验该预算。

SQLite schema v5 保存项目、会话、草稿修订、轮次、任务、Attempt、正式资产与
制作记录。安装和 worker 激活会绑定固定 manifest、运行时清单与 SHA-256 内容。

只有图片验证成功并完成原子数据库提交后，生成任务才成功。重启会把未完成执行
标为中断，只有用户明确重试才创建新 Attempt。图片被外部删除或改写时会显示为
不可用，但不会删除历史记录。

## 诊断与验证

开发构建可打开 **开发 → 通信诊断**，或运行：

```bash
cargo run -p image-studio-desktop --locked -- --diagnostics
```

诊断只验证受限 worker 通道，不代表 GPU、模型或图片生成已经通过。

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm --prefix apps/frontend test
npm --prefix apps/frontend run check
npm --prefix apps/frontend run build
openspec validate --all --strict
python3 scripts/ci/validate-workflows.py
actionlint -no-color
```

真实模型验收证据保存在 [`verification/`](verification/)：

- Windows 紧凑档：[`verification/windows-8gb-tier/`](verification/windows-8gb-tier/README.md)
- Windows 标准档与 Windows 10 目标机：[`verification/windows-12gb-standard-tier/`](verification/windows-12gb-standard-tier/README.md)
- Windows 桌面覆盖与缺陷：[`verification/windows-desktop-coverage/`](verification/windows-desktop-coverage/2026-11-06-coverage-results.md)、[`verification/windows-desktop-defects/`](verification/windows-desktop-defects/2026-11-06-preflight-and-cancel.md)
- Windows 安装版全模型验收：[`verification/windows-parity-app/`](verification/windows-parity-app/README.md)
- macOS 七模型证据：[`verification/qwen-moss-20261101/moss-stage-report.md`](verification/qwen-moss-20261101/moss-stage-report.md)

## CI 与发布

公开仓库使用三个固定版本的 GitHub Actions workflow：

- [quality.yml](.github/workflows/quality.yml)：Windows x64 与 macOS arm64 的
  格式、Clippy、workspace 测试、前端测试/构建与 OpenSpec。
- [build.yml](.github/workflows/build.yml)：Windows NSIS 与 macOS arm64 产物，
  包含 checksum 与包身份检查。
- [release.yml](.github/workflows/release.yml)：版本/main 来源检查，并创建 draft
  GitHub Release。

推送一个 `v*` tag（提交必须在 `main` 上，且 tag、workspace Cargo、Tauri 与前端
版本完全一致）会构建双平台产物并生成 draft Release，由维护者审阅后发布。
已发布的 `v0.1.1` 包含 Windows x64 NSIS 安装包与 macOS arm64 `.app` zip。
运行 ID 与产物摘要见 [CI 与发布文档](doc/14-ci-release.md) 与
[`verification/ci-release/`](verification/ci-release/)。

## 贡献与安全

- 安全漏洞请通过 [SECURITY.md](SECURITY.md) 报告，不要开公开 issue。
- 功能变更遵循项目的 OpenSpec 与 Beads 流程。
- 保持模型/运行时身份固定；不要在既有 install id 下替换为不同快照。

## 许可证

项目源码与原创分发资产采用 [Apache-2.0](LICENSE)。第三方库、字体、示例
提示词/图片与模型材料继续遵循各自许可证，见 [NOTICE](NOTICE) 与
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
