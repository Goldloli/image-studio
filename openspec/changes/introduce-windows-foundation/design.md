## Context

动机与范围见 [proposal.md](proposal.md)。目标设备为用户确认的 Win10 x64 / RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K；2026-11-01 的只读预检已确认这些基础硬件和 Windows 10 22H2 条件，但没有据此证明模型可执行。仓库已经具备跨后端协议、安装器和事务资产合同，但当前路径、归档、运行时、监督及验收脚本含 macOS/Unix 假设；前端浏览器测试不能覆盖 WebView2 和 Win32 行为。

现有 Z-Image 清单约 32.85 GB，transformer 原始 shard 合计约 24.62 GB；不能先构造完整未量化模型副本再认为 32 GB RAM 足够。完整 Windows 组件闭包还需固定 revision 的 `model_index.json`、scheduler 配置及该 pipeline 实际读取的文件。具体事实与一手资料见 [sources.md](sources.md)。

## Goals / Non-Goals

设计目标是通过有限的平台分支复用现有业务：Rust 宿主拥有平台事实和执行选择，worker 只处理一个冻结 Attempt；Windows 和 Mac 各有稳定身份及验证证据。资源调优发生在隔离 Gate 0，正式产品只提供一个已冻结配置。

不构建通用后端插件系统、不让用户选择任意 Python/命令/服务器，也不以跨机项目迁移作为首轮测试前提。正式发布签名、更多平台与模型使用后续 change。

## Decisions

### 1. 保留现有 workspace，按实际运行边界扩展

| 边界 | 本轮责任 | 依赖方向 |
|---|---|---|
| `studio-core` | Windows 独立配方、型号/能力/参数与冻结计划校验 | 继续不依赖 Win32、GUI、PyTorch |
| `worker-protocol` | 严格的 Windows Z-Image v2 载荷/握手/事件归属 | 不调用设备或进程 API |
| desktop adapters | 路径、目录投影、安装、能力查询、Win32 文件操作、调度、原生 UI | 使用 core/protocol 及监督入口 |
| `inference-supervisor` | Windows 句柄、Job Objects、租约、父通道、超时和回收 | 平台实现藏在该运行边界，不进入 core |
| 独立 CUDA Python worker | 固定依赖、真实 tokenizer、CUDA/NF4 推理和候选 PNG | 使用本地已核验组件；不访问业务数据库 |
| Windows 构建/验收脚本 | 本机 MSVC 构建、NSIS 组包、证据采集 | 显式隔离数据根，不依赖 Mac sandbox 工具 |

只按 Windows worker 的独立打包/依赖边界增加目录；不因为每个型号或平台就新增业务 crate。可复用的宿主逻辑继续共用，Win32 API 依赖仅在 Windows target 启用。

### 2. Gate 0 先证明 12 GB/32 GB 可行，再做产品接入

Gate 0 在批准后执行，允许隔离实验工具和固定组件下载，不先暴露产品安装/发送按钮。只读预检已经取得 OS build、驱动、sm86、RAM、分页文件、磁盘和 WebView2 等环境快照；Gate 0 仍须在实际执行条件下重新采集并关联用例证据，不能把快照当资源或兼容结果。候选依赖以官方 Windows x64 wheel、Python 3.12、CUDA 12.x 兼容路线为起点，最终 patch 版本由同机实验冻结。

首选实验配置 A：transformer 与 text encoder 的适用 Linear 使用 bitsandbytes 4-bit NF4，计算 dtype 候选 BF16，保持原始模型组件；通过 Accelerate/Diffusers 的组件级 CPU 卸载按编码、采样、VAE 顺序使用单 GPU，启用固定 VAE tiling。低内存 shard 加载必须覆盖“加载时量化”的 CPU 峰值。VAE 与非 Linear 权重不能标成已经全部 4-bit。

仅允许同一路线内的第二个有界候选 B：transformer 仍用 NF4，text encoder 使用 BF16 并按阶段加载/释放，避免量化 text encoder 与卸载 hooks 不兼容；其他输入输出、采样与模型 revision 不变。A、B 每次单独生成报告，不在一次请求中遇错自动切换。通过者按预算内的稳定性和耗时选定一个，冻结组件精度、量化覆盖/排除、卸载顺序、加载方式、VAE tile、attention、scheduler 与依赖身份。候选均失败即停止本轮后续模型产品化并报告，不扩展成无界后端搜索。

首轮不要求 FlashAttention、xformers、Triton、`torch.compile` 或本机编译 CUDA 扩展；使用选定 PyTorch 的内置 attention 路线并记录实际 kernel 策略。Windows 产品运行不依赖完整 CUDA Toolkit 或用户系统 Python；开发构建需要的 MSVC 工具与用户运行依赖分开。

资源门槛为本提案的工程选择：CUDA peak reserved ≤10.5 GiB、worker/监督进程树 private commit 峰值 ≤24 GiB、系统可用物理内存最低 ≥2 GiB，且在 12 GB 独显完成真实运算，无 OOM/崩溃。CUDA 指标按进程采集并在阶段边界同步；CPU/系统采样间隔至多 100 ms，明确这是采样观测值。另录 WDDM dedicated/shared 使用量、系统 commit/limit 和换页观测；若 WDDM 无法提供可信 per-process 值，记录限制，以重复实图和 allocator/系统数据共同判断，不伪造精度。

目标机同时枚举到 RTX 3080 Ti 与 Intel UHD 770。Gate 0 与正式 worker 必须按 CUDA 设备名称、PCI bus、compute capability 和显存核对实际执行设备，不能把 Windows 适配器序号、桌面默认适配器或枚举顺序当作执行身份。Intel GPU 不在本轮推理范围，也不得作为 CUDA 不可用时的回退。

一次首次加载和连续三次独立进程生成均须通过；后续三次允许文件系统缓存，不共享已加载模型，不靠人工清理或扩大硬件。沿用现有加载 600 秒、执行 900 秒的有界失败策略，不先承诺交互速度。若达不到预算或时限，需要调整规范而不是提高限额冒充通过。

### 3. 一个新的 Windows 配方，不重解释 Mac 的 q4 身份

建议 Windows 安装/目录组合 ID 为 `z-image-turbo-diffusers-cuda-nf4-v1`，模型变体与 capability 使用新身份，精确值在 Gate 0 冻结清单中同时登记。旧 `z-image-turbo-mflux-v1`、`z-image-turbo-q4`、v1 capability、runtime 锁和已有 plan/manifest 不改名、不自动迁移。

Windows 同一产品型号仍显示为 Z-Image Turbo。宿主按平台选择有界登记的 profile：Mac 保持七个原目录条目；Windows 将 Z-Image 行投影为新组合，另外六个型号保持可查但明确未接入本平台。同一逻辑型号不会因为多个内部 profile 增加型号数。目录搜索、功能页、composer、设置默认值使用同一能力事实；不另起一套异步设备检测。

Windows 参数固定为单张 1024×1024 RGB PNG、9 步、guidance 0.0、u32/空 seed，无源图/负提示词/用户可调 steps；tokenizer 读取同 revision 本地模板，512 token 预算计入特殊 token，原文 ≤16 KiB。worker 禁用截断并在采样前拒绝超限。9 是本配方传入的参数，不强行要求后端恰好回调 9 次；冻结所选 scheduler 的实际采样曲线与回调 total，禁止将 8 NFE、步数参数和完成百分比混为一谈。

CPU 卸载是明确登记的混合 CPU/CUDA 执行策略，不代表允许失败后纯 CPU 重跑。制作记录保存实际 GPU、驱动、计算 dtype、组件级量化/卸载、worker/runtime/snapshot 身份与真实 seed。量化质量以 Windows 同配置独立上游基线审阅，不要求 Mac 与 CUDA 像素或同 seed 图像完全相同。

### 4. 内容相同可以复用，激活记录不能跨平台复用

模型组件优先保留现有 revision 和摘要，补齐 Diffusers 读取闭包后生成新的 Windows manifest；不能修改现有 Mac manifest 冒充已核验。缓存只按内容摘要共享已验证文件，不复制 Mac venv、可执行文件、软链接或 Ready 记录。

模型传输优先核验 ModelScope 等用户已允许的国内固定来源，正式清单保留官方 revision、逐文件 bytes/SHA-256 和许可。没有同内容证明时不登记为产品源；可使用用户已有代理访问固定官方源，不自动启用未登记镜像。运行时只用固定官方解释器/wheel/必要 VC runtime，不在用户机器解浮动版本或从源码编译。

Ready 绑定平台/架构、install/profile、runtime/worker、模型清单及版本/设备探针。激活前测试 Win10 DLL 导入、CUDA、BF16/NF4 实际小算子，探针不增加整模型认证。驱动不兼容、DLL 缺失、设备不可用和内存不足有不同恢复信息。驱动属于外部前置，不由应用下载安装；受控 VC/WebView 运行依赖要有来源清单和检测/安装策略。

### 5. Windows 默认目录与文件边界

通过 Known Folder API 解析 LocalAppData，默认根为 `%LOCALAPPDATA%\ImageStudio`，下设 `data`、`cache`、`logs`、`work`；`--data-dir PATH` 继续产生与 Mac 相同的隔离子目录语义，不回退源码目录。原生路径保持 `PathBuf`/UTF-16 边界，不经 shell；文件定位只允许宿主已解析的受控目录。

首轮正式验收使用本地 NTFS 数据卷；不承诺网络共享、OneDrive 占位文件或其他文件系统的执行语义。拒绝无效/保留路径并提供可行动错误。处理 canonicalize 后的 `\\?\` 前缀时不能破坏展示、原生对话框或程序启动。

当前 Unix no-follow/rename 假设需要 Windows 对应实现：以打开的句柄核对文件类型和最终路径，拒绝越界 junction/reparse point、ADS 与非普通候选文件；原子导出在目标目录暂存并由 Windows 兼容的替换操作完成，覆盖仍需用户确认。SQLite、manifest 和既有资产提交事务不改成另一套存储协议。已有资产无需加载模型即可查看/导出。

### 6. 用 Job Objects 管理实际进程树

沿用宿主→原生 supervisor→worker 的边界。Windows 宿主持有 outer execution Job，supervisor 管理 inner compute Job；使用 kill-on-last-handle-close，禁止 breakaway，Job 句柄不由计算后代继承。通过 suspended create→加入 Job→resume 消除计算早于归组的窗口；若当前调试器/CI Job 限制导致无法加入，启动失败并解释原因，不放行无监督 worker。

宿主终止时 outer Job 回收监督和计算；监督终止时 inner Job 回收 worker/后代。活跃进程/Job 句柄和 Job 活动进程统计作为回收依据；不从数据库读取旧 PID 再盲杀。跨实例执行租约用 Windows 文件共享/锁语义实现，明确传递必要 lease 句柄并测试计算存活期间仍阻止重入；仅 EOF 或父进程退出不等于回收完成。句柄所有权、继承表和所有失败分支在实现评审中逐项核对。

合作取消仍走协议，超过 3 秒进入整组强制回收，以 5 秒清理截止记录结果；超期保留“正在清理/清理异常”并阻止新任务，不伪报资源已释放。持久化取消/成功裁决、Interrupted 和显式新 Attempt 重试继续复用。安装探针与诊断后代也使用同类受管理生命周期，不能只修正式推理路径。

### 7. generation v2 新分支与历史兼容

Windows worker 使用独立 build/capability/runtime 身份及 generation v2 新配方分支。严格载荷只含冻结计划所需字段；未知键、错平台、错 dtype/recipe、旧事件、错误 runtime 或错误候选角色均拒绝。沿用有界 JSONL、stderr 日志、Attempt 令牌和一个 primary 候选，不把路径或可执行配置交给前端。

保留 Mac Z-Image v1 与原六型号 v2 分支；不让新 worker 解读旧 v1。Mac 历史重试在 Mac 仍用原冻结组合；若在 Windows 看见不适用的历史计划，保留其身份和说明，不自动转成 CUDA。用户明确选择 Windows 型号后新建请求才使用新组合。现有请求/计划/manifest 的通用字段可容纳独立身份，不为新增平台统一抬高所有历史 schema。

### 8. Win10 本机构建和内部安装包

固定 Rust/Node 工具链与锁文件，在目标 Win10 或匹配的 Windows 构建机执行 MSVC 全工作区构建，打包 host/supervisor/diagnostic `.exe`、字体、图标和前端。Rust 由 `rust-toolchain.toml` 固定；Node/npm 已固定为 `24.19.0`/`11.17.0` 并写入 `.nvmrc`、`engines` 与 `packageManager`。NSIS 采用 per-user 安装；Windows 平台配置已完整覆盖 Mac 特有窗口参数，显式使用原生最小化/最大化/关闭与拖拽区域，不绘制装饰性假控制钮。

Windows 预检曾发现根 workspace 对 `sha2` 启用全局 `asm`，其汇编源无法由当前 MSVC 工具链构建。现已把该优化限定到 macOS；特征解析确认 Windows 不启用 `sha2-asm`，macOS 目标仍启用，完整 workspace 已重新构建通过。Mac 上的实际哈希性能与模型回归仍须在 Mac 执行，不能仅由依赖图代替。

采用 offlineInstaller 方式提供缺失 WebView2 时的离线安装依赖；打包时固定依赖版本与内容摘要，实测有/无 WebView2 分支以及权限不足的恢复提示。主程序不捆绑模型权重，首次启动不准备 Python/CUDA。升级覆盖程序而保留数据；卸载只移除程序，保留用户数据、模型与作品。本 change 产物明确标成内部测试包，不宣称已签名发布。

运行验收必须覆盖从 NSIS 安装目录启动，不能只测 `target/debug`。Mac 交叉检查可早做，但目标机 GUI、IME、WDDM 和进程清理必须另有证据。仓库已配置 `origin`，历史源码推送不授权本 change 的后续 push、pull、CI 云账户或 Dolt 远程同步；这些远程操作仍须单独授权。

## Risks / Trade-offs

| 风险/取舍 | 处理 |
|---|---|
| NF4 与卸载 hooks 在所选 Windows 版本组合上不兼容 | Gate 0 只比较 A/B；冻结通过组合后再产品化，不能自动回退 |
| 低显存方案把压力转移到 32 GB RAM 或加载副本 | 同时量测加载、编码、采样、解码的 GPU/CPU/系统峰值，预算不通过就停止 |
| CUDA/WDDM 的共享内存或分页导致看似成功但不可用 | 记录系统与 WDDM 观测、三次独立执行和时限，不能只报 Torch allocated |
| Win10 的实际 build、驱动或 DLL 与 wheel 不匹配 | 先采集并冻结依赖闭包；不可满足时报告条件，不默认要求用户升 Win11 |
| 全局哈希优化引入 MSVC 汇编构建失败 | 将汇编特性限定到支持平台或使用等价受控实现，保持 SHA-256 合约与 Mac 性能回归 |
| 双适配器环境把 Intel 核显或错误枚举项当成执行设备 | 按 CUDA 名称、PCI bus、compute capability 与显存记录并绑定 NVIDIA 设备，不依赖系统适配器序号 |
| 路径重解析、独占文件和替换语义不同 | 原生句柄边界和 NTFS 集成验收，覆盖中文路径/占用/越界/失败原文件保持 |
| Job 句柄泄漏或继承使强杀留下计算 | 启动竞争、宿主/监督/worker 强杀和 descendant 故障测试，保持租约直至确认清理 |
| 共用代码破坏 Mac 七型号 | Mac 质量门加真实 Z-Image 生命周期及其他六型号各一条能力回归；Klein含编辑、BiRefNet含mask |

## Migration Plan

1. 用户批准后在 Beads 建实施任务；Gate 0 先交付精确清单、选定配置和同机基线。
2. 先接平台边界和故障 fixture，再接安装/生成、目录与原生产品路径，逐单元验证并本地提交。
3. 用隔离数据根、空缓存和缓存根验收；保留原 Mac 清单、已装环境和旧任务身份，不复制个人数据库进行迁移实验。
4. 在目标机生成 NSIS 包并执行 [test-plan.md](test-plan.md)；只有当前 Windows 组合所有适用 P0 与 Mac 回归通过才归档。
5. 回滚只停用新 Windows 组合和替换程序版本，保留已有库、资产、模型缓存和失败证据；不以清空数据修复安装或协议问题。

## 实施前提与可延后采集项

2026-11-01 已确认本 change 并建立 Beads 实施记录；目标机 Windows 构建、原生启动、受控文件、Job Object 和 Gate 0 完整模型执行均已有当前证据。正式 Windows 产品安装、worker、原生生成闭环、NSIS 与 Mac 回归仍未完成。

系统 build、驱动、磁盘和工具链的预检值见 [sources.md](sources.md)。Gate 0 已冻结 Python 3.12.10、PyTorch 2.10.0+cu126、Diffusers 0.36.0、Transformers 4.57.3、Accelerate 1.12.0、bitsandbytes 0.48.2，以及固定模型、BF16 VAE、memory-efficient SDPA 和组件 CPU offload。目标机结果见 [Windows Gate 0](../../../verification/windows-gate0/README.md)。实际条件若导致更换模型、后端家族、平台、输出或预算，必须修订提案并重新确认。
