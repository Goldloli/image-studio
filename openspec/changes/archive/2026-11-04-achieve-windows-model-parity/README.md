# achieve-windows-model-parity

在 Windows 目标机（RTX 3080 Ti 12 GB / 32 GB RAM）上补齐 macOS 已有的全部七个型号与八项能力，使 Windows 产品与 macOS 功能对齐；对 12 GB 显存无法满足的模型，通过分阶段量化装载或明确的硬件门槛说明来交付。

**状态：已归档（2026-11-04）。** 用户 2026-11-03 确认，2026-11-04 接受 blocked/not-run 行并批准归档。实施与安装版验收完成：Gate 0 六型号全通过、Windows v2 运行时与 Real-ESRGAN ncnn 包冻结、Windows 身份一致、三个 worker 与安装器/目录/UI 全部实现；安装版在隔离数据根上完成六型号缓存命中安装与七项能力的真实生成/处理、导出与生命周期验收（`verification/windows-parity-app/`）。空缓存真实网络安装受 PyPI 吞吐阻塞、未冒充通过；空间/缺件回归、Z-Image 与 macOS 回归记录为 not_run/waived（`verification/windows-parity-app/not-run.json`）。Gate 0 可行性实验原始记录见 [verification/windows-parity-gate0](../../../verification/windows-parity-gate0/README.md)。用户已确认：Windows 与 macOS 全部对齐；只支持本机 12 GB 目标机；接受 Qwen-Image-2.1 的非商用研究许可；GitHub Actions CI/release 由代理出设计、用户在 Mac 启用（另立 change）。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 范围、用户决定、成功标准、风险与不做项 |
| [design.md](design.md) | 运行时、分阶段装载、模型身份、安装预算与硬件门槛 |
| [specs/](specs/) | `windows-local-generation`、`model-catalog`、`model-installation`、`generation-worker-protocol`、`windows-desktop` 的 delta |
| [tasks.md](tasks.md) | Gate 0、运行时、各模型、UI、验收与归档 |
| [test-plan.md](test-plan.md) | 每个新模型与平台的 P0 验收矩阵 |

可行性基线（2026-11-03 实测）：Qwen-Image-2.1-Turbo 以 diffusers 0.41 + NF4 在 8 步内 **19.15 s** 出 1024² 图；MOSS-Image-SFT 以 bf16 + model offload 在 8 步内 **68.45 s** 出图。两者都因内存超卖（CUDA reserved 15.5/15.7 GiB、进程 commit 40.6/50.2 GiB、可用 RAM 近 0）未通过第一轮 Gate 0 预算，因此本 change 的核心工程是分阶段量化装载与硬件门槛，而不是"能不能跑"。
