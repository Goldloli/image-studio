# Windows 首轮适配：依据与证据边界

核对日期：2026-11-01。本文用于提案研究；本轮没有访问目标 Windows 机器、下载完整模型、安装依赖或执行 Windows 推理。

## 用户已确认的设备

| 项目 | 已知事实 | 实施时需补采 |
|---|---|---|
| 系统 | Windows 10 x64 | 版本、build、补丁、WebView2 版本 |
| GPU | GeForce RTX 3080 Ti，12 GB 显存 | 驱动、可用独显内存、实际设备标识 |
| 内存 | 32 GB | 可用物理内存、提交限制、分页文件配置 |
| CPU | Intel Core i7-13700K | 实际系统报告 |
| 存储 | 用户尚未提供 | 安装卷文件系统、可用空间、缓存与最终文件是否同卷 |

这些是用户提供的目标配置，不是兼容认证结果。系统或驱动版本尚未采集不阻止编写提案；它们必须在目标机 Gate 0 中成为可追溯的运行条件。

## 仓库现状

- [当前产品说明](../../../README.md)：七型号、八能力已在 M3 Max / 128 GiB 验收；当前交付仍以 macOS 开发包为主。
- [平台路径](../../../apps/desktop/src/adapters/paths.rs)：默认路径只有 macOS 分支，其他系统要求显式数据根。
- [推理监督](../../../tools/inference-supervisor/src/lib.rs)：实际执行与组回收依赖 Unix，非 Unix 分支返回不支持；Windows 需要真实的平台实现。
- [桌面配置](../../../apps/desktop/tauri.conf.json)：当前 bundle 未启用，窗口配置含 macOS 标题栏参数；ICO 已存在并不构成 Windows 安装包证据。
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
