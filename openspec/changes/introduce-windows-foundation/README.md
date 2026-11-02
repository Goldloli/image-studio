# introduce-windows-foundation

为 Windows 10 x64 / RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K 建立桌面基础与首个 Z-Image Turbo 本地生成闭环。

**当前状态：已进入 Windows 产品闭环实施。** 2026-11-01 已在目标机完成 Gate 0、独立 Windows 身份、正式私有运行时/模型安装在环、产品 worker 与 `GenerationController` 实图、加载/采样取消与真实 supervisor 强杀恢复、Windows 目录/功能/设置投影，并生成和安装当前用户级 NSIS 包。2026-11-02 完成正式空缓存 ModelScope 安装、长连接分段续传、主动取消后同一根续传至 Ready、运行准备/探针取消与重试、坏摘要/缺件激活拒绝及 Windows 全量质量门。安装版 WebView2 已启动；空间不足、干净用户、缺失 WebView2、完整 IME/DPI、禁网和 Mac 回归仍未完成。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 待确认范围、用户已知前提、交付与门槛 |
| [design.md](design.md) | 低内存实验、平台/协议/安装身份、目录、进程监督和打包选择 |
| [specs/](specs/) | 两份新能力与三份既有能力的 delta |
| [tasks.md](tasks.md) | 整个 change 的实施与验收范围；复选框只在完整产品验收后更新 |
| [test-plan.md](test-plan.md) | Gate 0、平台、安装、生命周期、产品、安装包和 Mac 回归 |
| [sources.md](sources.md) | 设备事实、仓库现状与上游一手依据 |

本轮先完成目标机的单模型路线，不承诺七型号全部迁移。预检确认基础硬件与目标一致，并发现 NVIDIA RTX 与 Intel 核显并存；Gate 0 已验证 NVIDIA CUDA 执行身份、完整加载、连续生成和资源预算。实现与当前证据由 Beads 和 [Windows 证据](../../../verification/windows-gate0/README.md) 跟踪；不能以缩图、换模型、更大硬件或仅壳可启动替代尚未执行的 P0。
