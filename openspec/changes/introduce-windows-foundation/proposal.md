## Why

七个型号、八项能力已完成当前 Mac 的实机验收，但 Windows 仍只有核心与协议编译证据。用户希望使用 Windows 10 x64、RTX 3080 Ti 12 GB、32 GB 内存、i7-13700K 开发 Windows 版本，需要先建立真实的平台基础和一个能在该设备运行的生成闭环。

**批准状态：已确认。** 用户已明确要求继续开发并确认上述设备、首轮范围、技术路线和资源门槛。2026-11-01 Gate 0 已在目标机通过，正式产品安装与生成闭环仍待实施。来源与现状核对见 [sources.md](sources.md)，Gate 0 证据见 [Windows Gate 0](../../../verification/windows-gate0/README.md)。

## What Changes

- 首轮目标为 **Windows 10 x64 原生桌面应用**，复用 Rust core + Tauri + React、SQLite 会话/任务/资产和已有交互，不另做一套业务逻辑。验收记录实际系统 build、驱动和 WebView2 版本；Win11、WSL、虚拟机或 CI 成功不能代替目标机结果。
- 补齐 Windows 默认数据/缓存/日志目录、原生窗口操作、中文输入、文件定位、PNG 导出与受控路径行为，以及安装、诊断、推理进程树的停止和异常回收。
- 制作 **当前用户级 NSIS EXE 内部测试安装包**，包含宿主、监督/诊断程序和离线 WebView2 安装依赖。首次启动不下载模型；模型与私有 Python/CUDA 运行时由用户主动安装，不修改系统 Python、不安装 GPU 驱动或要求用户配置完整 CUDA Toolkit。
- 首个模型只做 **Z-Image Turbo 文生图**：固定现有官方 revision `f332072aa78be7aecdf3ee76d5c247082da564a6`，单张 1024×1024 RGB PNG、9 步、guidance 0.0、u32/空种子、16 KiB UTF-8 与含模板 512 token 上限。增加独立 Windows 配方、runtime、worker 和安装身份；MFLUX q4 与 Windows NF4 不视为同一格式。
- 首选官方 Windows wheel 的 **PyTorch + Diffusers + bitsandbytes NF4 + CPU 组件卸载**。先在目标机执行 Gate 0：验证完整加载、真实图像、连续执行和资源预算，再接入正式产品。具体依赖版本及逐文件摘要必须冻结；不以浮动版本安装，也不把量化后的内存估算当实测。
- Gate 0 暂定工程预算：生成进程树 CUDA allocator 的 peak reserved 不超过 **10.5 GiB**，进程树 private commit 峰值不超过 **24 GiB**，系统可用物理内存最低保留 **2 GiB**；同时记录 WDDM 独显/共享内存与系统提交量，不以共享显存或严重换页掩盖失败。这些是待批准的门槛，不是已测成绩。首次加载及后续三个独立进程均须完成，不能靠降低分辨率或增加物理内存通过。
- 模型目录、功能入口、会话默认值和历史重试按当前平台的实际组合判断。Windows 首轮只有 Z-Image 提供安装/生成，其余六型号说明“当前 Windows 版本尚未接入”；不能继承 Mac 的已安装、可执行或认证事实，也不把一个模型的两条后端重复计数。
- 验收覆盖主动安装与续传、完全断网生成、真实取消、宿主/监督强杀恢复、中文输入、窄窗/DPI、作品持久化与导出，并回归现有 Mac 七型号。所有适用 P0 通过后才归档。

现有 Z-Image 原始组件为 `32,848,304,893` bytes，Windows 仍需补齐同 revision 的 pipeline/scheduler 等配置及运行时；NF4 不意味着原始下载量缩为四分之一。安装预算按完整清单、缓存和暂存/最终文件计算，不凭设备内存或模型参数量推断磁盘需求。

批准实施包含固定来源的模型与私有运行时下载、隔离可行性实验和本机验收；目标 Windows 环境可访问后才能运行 Windows 任务。Mac 可先准备源代码和文档，不能据此宣布 Windows 通过。Gate 0 未通过时保留证据并停止后续模型产品化；需要换模型、后端家族、输出尺寸、硬件或缩减范围时，更新本 change 后重新确认，不能将“只有壳可启动”视为全部交付。

后续批次再评估 Klein、BiRefNet、Real-ESRGAN、Qwen/MOSS；本轮不含它们的 Windows 适配，也不含 AMD/Intel GPU、Windows ARM64、批量/队列、LoRA、图像编辑、多 GPU、任意画幅、项目跨机迁移、自动更新、商店上架或正式签名发布。

## Capabilities

### New Capabilities

- `windows-desktop`：Windows 10 x64 内部测试安装包、默认目录、原生日常操作、受管理进程树与分层验收证据。
- `windows-local-generation`：目标 12 GB/32 GB 设备上的可行性门槛、独立 Z-Image CUDA 低内存配方、失败处理和产品闭环。

### Modified Capabilities

- `model-installation`：安装计划、私有运行时和 Ready 记录绑定平台及精确执行组合，补齐 Windows 内容闭包。
- `model-catalog`：按平台投影模型接入/可执行/设备事实和动作，保留旧 ID、正确统计和可解释的不可用状态。
- `generation-worker-protocol`：为 Windows Z-Image 增加独立的生成 v2 配方/身份分支，保留原 v1/v2 的严格校验与历史兼容。

已有 `local-generation` 的持久化接受、幂等、取消终态、资产提交和显式重试合同继续复用；原 Z-Image/MFLUX 的三个尺寸、9 步/q4 与历史 v1 不变，不通过覆盖旧要求来扩展新后端。

## Impact

影响桌面平台 adapter、推理监督器、安装/运行时清单、core 配方登记、生成 v2 载荷校验、Python CUDA worker、目录/能力投影、打包与验收工具。前端仍只传登记 ID 和产品参数，平台与路径由宿主解析。共享代码的变更须覆盖 Mac 回归，不以新增平台为由顺手升级既有模型依赖或改写 SQLite 历史记录。

实施顺序为：**批准 → 目标机 Gate 0 与组合冻结 → Windows 平台/监督 → 安装与生成产品闭环 → 安装包/原生验收 → Mac 回归 → 归档**。前两步已完成；实际跟踪继续使用 Beads；[tasks.md](tasks.md) 声明剩余实施与验收范围。
