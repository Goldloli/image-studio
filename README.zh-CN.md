# Image Studio

[English](README.md) | **简体中文**

[![质量门](https://github.com/Goldloli/image-studio/actions/workflows/quality.yml/badge.svg)](https://github.com/Goldloli/image-studio/actions/workflows/quality.yml)
[![许可证：Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Image Studio 是面向 **macOS arm64** 与 **Windows x64** 的本地优先桌面图片创作
工作台，使用 Rust、Tauri 2 与 React 构建。它支持文生图、基于指令的单图编辑、
带 RGBA/mask 输出的去背景，以及精确 4× 放大；模型推理和项目数据都留在本机。

![Image Studio 生成工作区](verification/windows-installed-app/screenshots/generation-conversation.png)

## 核心特性

- **七个认证型号、八项能力**，macOS 与 Windows 均已完成真实执行验收。
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

**显存档位**：五个生成模型登记 `standard`（≥ 12 GB，1024²，bf16）与 `compact`（≥ 8 GB，768²，
fp16）两档，同一份模型下载与 prepared 缓存共用；设备只满足紧凑档时自动降级并在模型页标明。
Z-Image Turbo 实测无法在 8 GB 上运行，保持 12 GB 门槛。详见
[`doc/15-windows-8gb-tier.md`](doc/15-windows-8gb-tier.md)。

权威模型目录位于
[`resources/model-catalog/catalog.json`](resources/model-catalog/catalog.json)。
Windows 使用独立冻结的 NF4、ONNX 与 ncnn 配置；macOS 使用已验证的 MFLUX/MLX
和 ONNX 运行时。

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

macOS 产物需要 Apple Silicon 与 macOS 11 或更高版本。模型权重和私有 Python
运行时不随安装包分发，需要在应用内单独安装。

## 主要页面

- **创作**：多会话文生图、源图工作流、即时占位、真实步骤进度、取消与重试。
- **灵感**：可搜索的提示词库，并保留来源归属。
- **功能**：只展示有真实已安装模型支撑、可立即开始的能力。
- **模型**：显示安装、运行时、设备门槛、硬件适配与许可信息。
- **作品**：跨会话浏览不可变正式资产，支持筛选、预览、制作记录以及原生
  PNG/RGBA/mask 导出。
- **任务**：宿主持久库中的状态筛选、检索、稳定分页、Attempt 历史、错误和恢复建议。
- **设置**：持久外观、显式新会话默认值、存储位置、诊断状态与运行时清单。

## 从源码构建

前置条件：

- Rust **1.99.1**，由 [`rust-toolchain.toml`](rust-toolchain.toml) 固定
- Node.js **24.19.0**、npm **11.17.0**
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

macOS 默认路径：

- 数据：`~/Library/Application Support/ImageStudio`
- 缓存：`~/Library/Caches/ImageStudio`
- 日志：`~/Library/Logs/ImageStudio`

使用 `--data-dir PATH` 可以创建隔离数据根；同一数据根同一时间只允许一个宿主。
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

- Windows 七型号安装版总账：
  [`verification/windows-parity-app/README.md`](verification/windows-parity-app/README.md)
- macOS 七型号整批证据：
  [`verification/qwen-moss-20261101/moss-stage-report.md`](verification/qwen-moss-20261101/moss-stage-report.md)

## CI 与发布

公开仓库使用三个固定 action SHA 的 GitHub Actions workflow：

- [quality.yml](.github/workflows/quality.yml)：Windows x64 与 macOS arm64 的
  fmt、Clippy、workspace tests、前端 test/build 与 OpenSpec。
- [build.yml](.github/workflows/build.yml)：Windows NSIS 与 macOS arm64 `.app`，
  验证 checksum 和包身份。
- [release.yml](.github/workflows/release.yml)：验证版本与 `main` 来源后创建
  GitHub Release draft。

详见 [CI 与发布文档](doc/14-ci-release.md) 和
[`verification/ci-release/`](verification/ci-release/)。

## 贡献与安全

- 漏洞请通过 [SECURITY.md](SECURITY.md) 报告，不要先公开利用细节。
- 功能扩展遵循项目的 OpenSpec 与 Beads 流程。
- 模型/运行时身份必须保持固定；不得用另一快照冒充既有 install id。
- 提交前运行 README 中列出的最小质量门，并保留真实验收证据。

## 许可证

项目源码与自有可分发资源采用 [Apache-2.0](LICENSE)。第三方库、字体、提示词
示例图片与模型材料保留各自上游许可证；见 [NOTICE](NOTICE) 和
[THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md)。
