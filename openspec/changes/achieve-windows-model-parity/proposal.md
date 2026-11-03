# Proposal: achieve-windows-model-parity

## Why

2026-11-03 归档的 `introduce-windows-foundation` 只交付了 Windows 上的 Z-Image Turbo 单模型闭环，proposal 明确把 Klein、BiRefNet、Real-ESRGAN、Qwen/MOSS 留给后续批次。用户现在要求 Windows 端与 macOS **完全对齐**：七个型号、八项能力全部在 Windows 上可安装、可执行、可验收；随后在 Mac 上接入 GitHub Actions CI 与双平台 release。

Gate 0 可行性实验（[verification/windows-parity-gate0](../../../verification/windows-parity-gate0/README.md)）证明：Qwen-Image-2.1-Turbo 与 MOSS-Image-SFT 在 12 GB 目标机上**可以真实出图**（8 步 19 s / 68 s），但官方 bf16 + diffusers 组件 offload 会内存超卖（共享显存 + 页面文件），不能作为产品运行时。因此本 change 交付的是"可安全执行的 Windows 全模型运行时与产品闭环"，包含分阶段量化装载或明确的硬件门槛。

## 用户决定（2026-11-03）

1. Windows 端与 macOS 全部对齐，七个型号都要做；不缩减。
2. 硬件范围：只认证当前目标机（RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K）；其他显卡/内存档位不承诺。
3. 允许使用量化/社区版本（GitHub/Hugging Face/ModelScope）；若某模型在 12 GB 上无法安全执行，则在模型页明确写出所需显存/内存门槛。
4. 接受 Qwen-Image-2.1 的 Qwen Research License（仅非商用研究/评估），仍接入 Windows；产品页面必须展示该限制。MOSS-Image-SFT 为 Apache 2.0。
5. GitHub Actions CI 与双平台 release 不在本 change：由代理出设计与 workflow 文件、用户在 Mac 启用，另立 change（`introduce-ci-release-pipeline`）。

## What Changes

- 新增 Windows 第二套冻结运行时（Python 3.12 / torch 2.10 cu126 / torchvision / diffusers 0.41 / transformers 5.3 / accelerate / bitsandbytes），与 Z-Image 的 v1 运行时并存；每个模型绑定确定性运行时身份。
- 新增六个型号的 Windows 配方、模型变体、安装清单（ModelScope/HF 固定 revision + 逐文件 SHA-256）、空间预算与设备探针：
  - FLUX.2 Klein 4B distilled：文生图 + 单图指令编辑（diffusers Flux2 系列 pipeline）；
  - BiRefNet epoch_244：去背景，ONNX Runtime CPU（复用 macOS worker 合同，Windows 运行时）；
  - Real-ESRGAN x4plus：4× 放大，官方 ncnn Windows Vulkan 构建；
  - Qwen-Image 2.1 / 2.1 Turbo：文生图（QwenImage21Pipeline；非商用许可标注）；
  - MOSS-Image-SFT：文生图（仓库自带 custom pipeline，trust_remote_code；Apache 2.0）。
- 引入**分阶段量化装载**执行器：文本编码 → 释放 → DiT 采样（NF4/INT8）→ VAE 分块解码；同一时刻只在 GPU 驻留一个大组件；记录独占显存与进程提交，禁止共享内存回退作为通过。
- 模型目录新增 Windows 可达性、量化方式、许可与硬件门槛元数据；模型页/设置页对不满足门槛的设备显示可执行说明，而非静默失败。
- 功能页、作品页、任务中心、导出与生命周期（取消/强杀/租约）覆盖全部新能力；每型号独立证据。

## 成功标准

- 七个型号在 Windows 目标机上都能完成：空缓存安装 → 激活探针 → 真实生成/处理 → 取消/强杀恢复 → 原生 UI 发送/查看/导出；每个型号有独立的可复核证据。
- 12 GB 目标机上，Qwen/MOSS 通过分阶段装载达到**独占显存不触发共享回退**；若最终无法达到，则模型页硬件门槛必须明确并通过 UI 验收（用户已授权此降级路径）。
- 安装空间预检、坏摘要/缺件拒绝、禁网缓存安装（沿用第一轮 waive 策略）按本 change 的范围重新验收。
- 现有 Z-Image、macOS 行为与历史记录保持兼容；不重写历史身份。

## 不做 / 超出范围

- 不承诺 AMD/Intel GPU、Windows ARM64、多 GPU、LoRA、批量队列、任意画幅、模型编辑器等新能力。
- 不在本 change 做 CI/release、代码签名、商店上架、自动更新（另立 change）。
- 不改变 macOS 的 MFLUX 配方与已认证证据；Windows 端不继承 macOS Ready/安装事实。

## Impact

- 代码：`apps/desktop`（目录/安装器/worker 适配）、`tools/`（Windows ONNX/ncnn/diffusers worker）、`crates/studio-core`（型号/配方/元数据）、前端模型页/功能页。
- 运行时与资源：新增约 40 GB 运行时与 85+ GB 模型下载；（用户机器磁盘充足）。
- 规格：`windows-local-generation`、`model-catalog`、`model-installation`、`generation-worker-protocol`、`windows-desktop`。
- 证据：`verification/windows-parity-*`。风险：32 GB 内存与 12 GB 显存的组合是本 change 最大不确定性，Gate 0 任务先行，失败时按用户授权记录硬件门槛。
