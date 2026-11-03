# introduce-windows-foundation

为 Windows 10 x64 / RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K 建立桌面基础与首个 Z-Image Turbo 本地生成闭环。

**当前状态：Windows 首轮交付已由用户于 2026-11-03 验收收口并归档。** 2026-11-01 至 2026-11-03 在目标机完成 Gate 0、独立 Windows 身份、私有运行时/模型安装在环、产品 worker 与 `GenerationController` 实图、加载/采样取消与真实 supervisor 强杀恢复、平台目录/功能/设置投影、NSIS 当前用户安装、正式空缓存 ModelScope 安装与分段续传、运行准备/探针取消重试、坏摘要/缺件/空间不足拒绝、Windows 全量质量门、安装版原生 UI 完整路径（UI 安装到 Ready、中英文连续三次生成、PNG 导出、重启恢复、任务/作品、明暗主题、原生窗口控件、合成 IME 保护、导出覆盖双分支）、W07 启动失败诊断与每数据根 WebView2 目录隔离；不同数据根第二实例 101 在开发机与目标机均未复现。整树禁网 I07/P06 由用户 2026-11-03 决定本轮跳过（waived/not_run）；100%/200% DPI、真实中文 IME、干净用户/缺失 WebView2 与 Mac R02–R04 未附本轮独立机器证据，按用户验收收口记为 user-accepted（不得读作代理实测通过）。收口记录见 `verification/windows-final-acceptance.json`。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 待确认范围、用户已知前提、交付与门槛 |
| [design.md](design.md) | 低内存实验、平台/协议/安装身份、目录、进程监督和打包选择 |
| [specs/](specs/) | 两份新能力与三份既有能力的 delta |
| [tasks.md](tasks.md) | 整个 change 的实施与验收范围；复选框只在完整产品验收后更新 |
| [test-plan.md](test-plan.md) | Gate 0、平台、安装、生命周期、产品、安装包和 Mac 回归 |
| [sources.md](sources.md) | 设备事实、仓库现状与上游一手依据 |

本轮先完成目标机的单模型路线，不承诺七型号全部迁移。预检确认基础硬件与目标一致，并发现 NVIDIA RTX 与 Intel 核显并存；Gate 0 已验证 NVIDIA CUDA 执行身份、完整加载、连续生成和资源预算。实现与当前证据由 Beads 和 [Windows 证据](../../../../verification/windows-gate0/README.md) 跟踪；不能以缩图、换模型、更大硬件或仅壳可启动替代尚未执行的 P0。
