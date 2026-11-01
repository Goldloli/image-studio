# Windows 首轮适配：依据与证据边界

核对日期：2026-11-01。本文用于提案研究。最初编写提案时没有目标机访问；同日在目标 Windows 机器完成了环境预检、已有源码检查和 Windows 基础构建启动，但没有生成 NSIS 安装包、下载完整模型、安装模型运行时或执行推理。

## 用户确认的目标与目标机预检

| 项目 | 用户确认的目标 | 目标机只读预检 |
|---|---|---|
| 系统 | Windows 10 x64 | Windows 10 Pro 22H2 x64，`10.0.19045.6466`，简体中文 |
| GPU | GeForce RTX 3080 Ti，12 GB 显存 | `12288 MiB`、driver `596.49`、compute capability `8.6`、PCI bus `0000:01:00.0`；另有 Intel UHD Graphics 770，仅按显示适配器记录 |
| 内存 | 32 GB | 2 × 16 GB Kingston DDR4-3200；系统可见物理内存 `31.73 GiB` |
| CPU | Intel Core i7-13700K | 16 物理核 / 24 逻辑处理器 |
| 主板 | 未提供 | MSI MPG Z690 EDGE TI WIFI DDR4；BIOS `A.N1`，发布日期 2025-05-12 |
| 存储 | 未提供 | 系统为 NTFS；预检时 C: 容量约 `1907.50 GiB`、可用约 `1576.09 GiB`，底层为 NVMe SSD |
| 显示与输入 | 未提供 | Dell U2723QE，`3840×2160`，预检时 100% 缩放；已启用简体中文和英文键盘布局 |
| WebView2 | 未指定 | Evergreen Runtime `156.0.4314.58` 已安装 |

预检快照中的系统、驱动和设备身份可用于确认目标机匹配，不是兼容认证结果。内存、分页、提交量和 GPU 可用量会随运行状态变化；Gate 0 仍须在批准后使用正式用例和可追溯原始记录重新采集执行条件。

目标机同时存在 NVIDIA CUDA 适配器和 Intel 显示适配器。Windows 产品路线只面向 NVIDIA CUDA；运行时必须按实际设备身份核对 RTX 3080 Ti、PCI bus、compute capability 与显存，不得根据 Windows 适配器序号或“第一个 GPU”推断执行设备，也不得回退到 Intel GPU。

### 目标机预检快照

只读预检期间观察到：

- `nvidia-smi` 报告 12288 MiB 显存，其中约 1516 MiB 已由桌面应用占用；CUDA 驱动兼容上限显示为 13.2。该数值不是本应用 CUDA wheel 或 Toolkit 版本结论。
- 系统自动管理 `C:\pagefile.sys`，当时分配约 4.75 GiB；commit limit 约 36.48 GiB，已提交约 15.06 GiB（16.17 GB），空闲物理内存约 19.71 GiB。
- 这些是空载或桌面应用运行中的瞬时值，不是生成过程的峰值，也不能替代 Gate 0 的采样与阶段报告。
- 当前 100% 缩放和 3840×2160 显示器只确认了现有桌面条件；1280×800、960×640 与 100%/150%/200% DPI 组合仍须在原生应用中逐项验收。

### Windows 开发工具预检

| 工具 | 2026-11-01 目标机状态 |
|---|---|
| MSVC | Visual Studio Build Tools 2022 `17.14.40`；MSVC `14.44.35207` / compiler `19.44.35228`；Windows SDK 版本 `10.0.22621.0`、`10.0.26100.0` |
| VC++ Runtime | x64 Redistributable `14.51.36247` 已安装 |
| Rust | 仓库固定 `1.99.1`；预检发现安装不完整后已修复，`rustc/cargo/rustfmt/clippy` 与 Windows x64 目标均已核实 |
| Node/npm | Node `24.19.0`、npm `11.17.0`；已由 `.nvmrc`、`engines` 与 `packageManager` 固定，前端门禁在该组合通过 |
| Python | `3.11`、`3.13`、`3.14` 可用；没有 Python `3.12`，也没有 CUDA Toolkit/`nvcc` |
| Git | Git `2.55.0.windows.3`；`core.longpaths=true` |
| 文档工具 | OpenSpec `1.12.0`；Beads `1.2.2` |
| GitHub CLI | 未安装；当前源码已通过 Git 克隆，不代表后续 push/pull 获授权 |

开发工具存在或版本接近只说明前置条件值得继续验证。Node/npm 版本冻结、Windows 全工作区构建和 Tauri 原生启动仍未通过。

### 已有源码初始检查

2026-11-01 在目标 Windows 机器完成了不涉及模型和 Tauri GUI 的源码检查：

- `npm ci` 安装 46 个包；`npm test` 的 6 个测试文件、37 项测试全部通过；生产前端构建通过，仅有单个 catalog chunk 超过 500 KiB 的非阻断提示。
- 固定 Rust `1.99.1` 与 `x86_64-pc-windows-msvc` 目标可用；`cargo check -p studio-core -p worker-protocol --target x86_64-pc-windows-msvc --locked` 通过。
- `cargo fmt --all -- --check`、`cargo check --workspace --locked`、`cargo build --workspace --locked` 和 `cargo clippy --workspace --all-targets --locked -- -D warnings` 均通过。
- `cargo test --workspace --locked` 通过：206 项测试和 1 项文档测试通过，4 项需要固定网络/运行时缓存的显式验收测试被忽略，0 失败。
- Windows 检查发现生成协议测试夹具硬编码 POSIX 绝对路径；已改为由测试按当前平台生成受控绝对路径，生产协议校验和 Mac 路径语义未改写。
- 首次 `cargo check --workspace --locked` 曾在 `sha2-asm` 的 MSVC 汇编处失败；现已把 `asm` 限定到 macOS。`cargo tree` 确认 Windows 不启用 `sha2-asm`，`aarch64-apple-darwin` 仍启用该特性，Mac 实际构建性能回归仍待执行。
- 桌面宿主已用显式隔离根和无参数默认路径分别启动成功；Windows Known Folder API 解析到 `%LOCALAPPDATA%\ImageStudio`，创建 `data/cache/logs/work` 后进程稳定运行，未启动 worker 或模型。
- 原生 Computer Use 可枚举并读取 `Image Studio` 的完整 WebView2 可访问性树：中文创作页、导航、会话、输入、模型状态和原生窗口控件均存在；`set_value` 成功写入并清空中文提示词且草稿保存更新。Graphics Capture 截图仍超时，元素点击缺少几何信息，键盘 Tab 未移动焦点，保留为 `image-studio-ba1`。
- Windows 已增加完整替换的 `tauri.windows.conf.json`，显式使用原生装饰与控件；release 构建隐藏控制台。
- Windows 目录 flush、进程内同根互斥、只读候选提交、ADS/reparse 普通文件校验和只读导出替换已实现并通过测试。原生 Job Object 监督、完整 UI/IME/DPI、安装包和模型执行仍未完成。

这些结果证明当前 Windows 构建、常规测试、原生窗口渲染和部分目录/文件基础可重复；Windows 监督器仍有 Unix-only 分支，视觉截图/坐标交互、快速键盘导航、IME 组合、DPI、安装包和模型执行尚未验收。前端结果只适用于已固定的 Node/npm 组合。

## 仓库现状

- [当前产品说明](../../../README.md)：七型号、八能力已在 M3 Max / 128 GiB 验收；当前交付仍以 macOS 开发包为主。
- [平台路径](../../../apps/desktop/src/adapters/paths.rs)：默认路径只有 macOS 分支，其他系统要求显式数据根。
- [推理监督](../../../tools/inference-supervisor/src/lib.rs)：实际执行与组回收依赖 Unix，非 Unix 分支返回不支持；Windows 需要真实的平台实现。
- [桌面配置](../../../apps/desktop/tauri.conf.json)：基础 bundle 仍未启用，macOS 配置保留 Overlay/traffic-light 参数；Windows 平台配置已另行覆盖为原生装饰。ICO 已存在并不构成 Windows 安装包证据。
- [Z-Image 模型清单](../../../resources/local-generation/model-manifest.json)：固定 `Tongyi-MAI/Z-Image-Turbo` revision `f332072aa78be7aecdf3ee76d5c247082da564a6`，现有组件合计 `32,848,304,893` bytes。现清单不含 `model_index.json` 和 `scheduler/scheduler_config.json`，不能直接当作完整 Diffusers 快照。
- [型号目录](../../../resources/model-catalog/catalog.json)：现有 Z-Image 行 ID 为 `z-image-turbo-mflux-v1`，其他多条 ID 也包含运行时身份。Windows 不能直接把这些旧身份重命名成 CUDA。
- [平台主规格](../../specs/platform-foundation/spec.md)：Windows 现有证据仅限核心与协议目标编译，不包含 GUI、进程、安装或模型认证。

## 上游一手依据

| 编号 | 来源 | 支持的判断 | 不支持的推论 |
|---|---|---|---|
| S01 | [Tauri Windows 前置条件](https://v2.tauri.app/start/prerequisites/) | Windows 桌面使用 MSVC 开发工具与 WebView2 | 仓库现在就能在 Win10 完整构建 |
| S02 | [Tauri Windows 安装包](https://v2.tauri.app/distribute/windows-installer/) | 可生成 NSIS EXE，并选择离线 WebView2 安装方式；官方优先 Windows 本机构建 | Mac 交叉编译等于 Win10 真机验收 |
| S03 | [Microsoft Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects) | Job 可管理进程树，提供关闭最后句柄时终止、嵌套 Job 与整组终止机制 | 只设置一个标志就能自动解决句柄继承、启动竞争与资源释放 |
| S04 | [NVIDIA CUDA GPU 表](https://developer.nvidia.com/cuda/gpus) | RTX 3080 Ti 的 compute capability 为 8.6 | 所有 CUDA wheel、精度和模型都能直接运行 |
| S05 | [PyTorch Windows 安装](https://pytorch.org/get-started/locally/) | Windows 有官方 CPU/CUDA 安装路线 | 任何最新版均适合本机 Win10、现有驱动及所需量化依赖 |
| S06 | [Diffusers Z-Image pipeline](https://huggingface.co/docs/diffusers/api/pipelines/z_image) | 存在 ZImagePipeline 和 CUDA 路线 | 全精度示例证明 12 GB 显存可用，或其他任务已在本产品接入 |
| S07 | [bitsandbytes 安装矩阵](https://huggingface.co/docs/bitsandbytes/installation) | 存在 Windows x86-64 CUDA 构建，矩阵含 sm86；提供 NF4/FP4 | 任意版本的 Z-Image、CPU 卸载与量化 hooks 均兼容 |
| S08 | [Diffusers bitsandbytes](https://huggingface.co/docs/diffusers/quantization/bitsandbytes) | 支持符合条件组件的 4-bit 量化，Diffusers 与 Transformers 各自配置 | NF4 与 MFLUX q4 是同一格式或质量完全相同 |
| S09 | [Diffusers 内存优化](https://huggingface.co/docs/diffusers/optimization/memory) | 提供组件卸载、顺序卸载和 VAE tiling，且各有适用限制 | CPU 卸载不占 RAM、不影响速度，或能无条件解决 32 GB 主存限制 |
| S10 | [固定 revision 的 model_index](https://huggingface.co/Tongyi-MAI/Z-Image-Turbo/blob/f332072aa78be7aecdf3ee76d5c247082da564a6/model_index.json)、[scheduler config](https://huggingface.co/Tongyi-MAI/Z-Image-Turbo/blob/f332072aa78be7aecdf3ee76d5c247082da564a6/scheduler/scheduler_config.json) | 同一模型 revision 有 Diffusers 所需的额外配置，可在实施时补齐内容闭包 | 已核验这两个文件的 SHA-256，或现有 Mac 清单已包含它们 |

上游网页用于判断路线是否值得验证。实施时仍须冻结具体 Python、PyTorch/CUDA、Diffusers、Transformers、Accelerate、bitsandbytes、wheel 与系统依赖身份，保存各自来源、版本、平台、字节数和 SHA-256；不把网页的 `pip install -U` 用作正式产品安装命令。

## 本提案的推断与约束

1. 现有 UI、领域、持久化和协议可以复用；Windows 特有工作集中在平台适配、运行时、进程监督、打包与真机认证。这是根据仓库依赖边界作出的工程判断。
2. 12 GB 显存与 32 GB 内存要求先验证加载峰值，首选 NF4 量化加受控 CPU 卸载。是否能稳定完成 1024×1024 必须由目标机 Gate 0 决定。
3. 首轮只交付一个 Z-Image Windows 组合，不承诺七型号平移。Windows 10 是当前用户确认的首轮系统，Windows 11、其他显卡或更多内存设备的结果不能替代它。
4. 正式模型默认优先核验国内固定来源，保留上述官方 revision 与内容摘要；同内容来源未核验前只记录候选地址，不激活安装入口。来源或模型身份变化须更新文档，不能借网络回退更换模型。

## 预检证据边界

本次记录覆盖本机系统、硬件、驱动、WebView2、磁盘、开发工具、完整 workspace 构建/测试，以及默认和隔离目录的宿主短时启动。没有生成 NSIS 安装包，没有完成 Windows 原生 UI/IME/DPI，没有实现 Windows Job Object 监督，也没有执行 CUDA/PyTorch/BF16/NF4 探针、模型安装或推理。因此 G02–G06、完整安装/生命周期/P0、安装包和 Mac 回归仍保持未运行。
