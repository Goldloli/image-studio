# introduce-windows-foundation

为 Windows 10 x64 / RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K 建立桌面基础与首个 Z-Image Turbo 本地生成闭环；当前仅提案，等待确认。

**当前状态：文档草案，尚未批准实施。** 2026-11-01 已在目标 Windows 机器完成只读环境预检，但没有修改实现代码、执行 Windows 构建、下载模型或运行推理。OpenSpec 校验通过和可 apply 状态均不替代用户确认。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 待确认范围、用户已知前提、交付与门槛 |
| [design.md](design.md) | 低内存实验、平台/协议/安装身份、目录、进程监督和打包选择 |
| [specs/](specs/) | 两份新能力与三份既有能力的 delta |
| [tasks.md](tasks.md) | 批准后实施与验收范围，全部未开始 |
| [test-plan.md](test-plan.md) | Gate 0、平台、安装、生命周期、产品、安装包和 Mac 回归 |
| [sources.md](sources.md) | 设备事实、仓库现状与上游一手依据 |

本轮先完成目标机的单模型路线，不承诺七型号全部迁移。预检确认基础硬件与目标一致，并发现 NVIDIA RTX 与 Intel 核显并存；Gate 0 仍须验证 NVIDIA CUDA 执行身份、完整加载、连续生成和资源预算。Gate 0 未通过时保留失败证据并停止模型产品化，不以缩图、换模型、更大硬件或仅壳可启动替代。实现进度在用户批准后由 Beads 管理；原始文档工作为 `image-studio-f0z`，Windows 本机预检与文档校正由 `image-studio-86d` 跟踪。
