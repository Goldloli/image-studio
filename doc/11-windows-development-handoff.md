# Windows 开发交接

交接日期：2026-11-01。私有源码仓库：[Goldloli/image-studio](https://github.com/Goldloli/image-studio)，默认分支 `main`。

## 已完成与当前授权

Mac M3 Max / 128 GiB 已完成七型号、八能力验收，相关变更已归档。现有 macOS 数据目录、模型安装、Python/MLX 环境和个人图片不随 Git 克隆迁移。

用户已确认 Windows 开发机为 **Windows 10 x64 / RTX 3080 Ti 12 GB / 32 GB RAM / Intel Core i7-13700K**，明确要求继续开发，并确认了 `introduce-windows-foundation` 的实施范围。Gate 0、正式产品身份/安装在环、产品 worker/Controller 实图、平台投影与首次 NSIS 安装烟测已完成；干净用户/缺失 WebView2、完整 UI/IME/DPI、真实取消/强杀、禁网与 Mac 回归仍未完成。

## 2026-11-01 Windows 目标机预检

目标机只读预检确认：

- Windows 10 Pro 22H2 x64，`10.0.19045.6466`，中文系统区域。
- i7-13700K，16 核 / 24 线程；2 × 16 GB Kingston DDR4-3200，系统可见物理内存 31.73 GiB。
- NVIDIA RTX 3080 Ti `12288 MiB`、driver `596.49`、sm86；系统同时存在 Intel UHD Graphics 770，CUDA 路线只能选择 NVIDIA 设备。
- WebView2 Runtime `156.0.4314.58`；Dell U2723QE 当前 3840×2160 / 100% 缩放；中文和英文键盘布局均已启用。
- 系统盘为 NTFS NVMe，预检时可用约 1576 GiB；分页文件由系统自动管理，当时约 4.75 GiB。

这些信息证明基础机器条件匹配，不证明 Windows 构建、GUI、GPU 算子或 Z-Image Gate 0 通过。详细快照和证据边界见 [Windows sources](../openspec/changes/introduce-windows-foundation/sources.md)。

## 2026-11-01 Gate 0 结果

Gate 0 已在目标机通过：

- 固定 Python 3.12.10、PyTorch 2.10.0+cu126、Diffusers 0.36.0、Transformers 4.57.3、Accelerate 1.12.0、bitsandbytes 0.48.2。
- RTX 3080 Ti 按 UUID、PCI bus、sm86 与显存绑定；BF16 和 NF4 小算子通过。
- Z-Image revision `f332072aa78be7aecdf3ee76d5c247082da564a6` 的 20 个文件、19 个推理组件、`32,848,305,533` bytes 全部校验。
- ModelScope `Tongyi-MAI/Z-Image-Turbo@master` 已对全部 20 个文件做等价性验证。
- 候选 A 使用双 NF4、BF16 计算、BF16 VAE tiling、组件 CPU offload 与 memory-efficient SDPA，通过三个独立进程的 1024×1024、9 步、guidance 0.0 实图。
- 三次最大 CUDA reserved 为 `9,523,167,232` bytes，最大进程 private commit 为 `25,440,862,208` bytes，最低可用 RAM 为 `9,417,654,272` bytes；加载低于 25 秒，执行低于 15 秒。

完整报告、资源原始记录与三张样图见 [Windows Gate 0](../verification/windows-gate0/README.md)。Gate 0 通过后，独立 Windows recipe/runtime/worker、正式私有安装在环与 `GenerationController` 实图已分别验证；缓存预置安装、正式空缓存下载计时和禁网仍是不同事实。

## 2026-11-01 产品闭环与安装包进展

- Windows 目录仅暴露一个已认证产品 profile：`z-image-turbo-diffusers-cuda-nf4-v1`；另外六个逻辑型号保留说明但不可安装或执行。
- 产品安装器从 Gate 0 缓存预置 20 个 ModelScope 等价模型文件和完整运行时，重新校验后安装私有 Python/wheel，执行 worker/CUDA 探针并写入 Ready 记录。
- 产品 worker 与 `GenerationController` 均生成真实 1024×1024 RGB PNG；Controller 路径还验证了 v2 载荷、`primary.png`、PNG 校验、SQLite Job/Asset 提交与资产读取。
- Tauri 已生成 `244,750,015` 字节的当前用户 NSIS 内部测试包，嵌入离线 WebView2、VC runtime、`inference-supervisor.exe` 和 `diagnostic-worker.exe`。
- 安装包已静默安装到 `%LOCALAPPDATA%\Image Studio`；安装版 WebView2 使用隔离数据根启动，可访问性树确认中文创作页和 Windows `Z-Image Turbo · 未就绪` 状态，首次启动没有创建私有运行时或模型文件。

证据见 [Gate 0/产品闭环](../verification/windows-gate0/README.md) 和 [Windows 安装包](../verification/windows-installer/README.md)。

## 在 Windows 获取源码

Git 是克隆、构建和本地提交的前置；GitHub CLI 只在需要 GitHub 账号交互或后续经明确授权的远程工作流时使用。用拥有该私有仓库权限的账号执行：

```powershell
gh auth login
gh auth setup-git
git clone --config core.longpaths=true --config core.autocrlf=false https://github.com/Goldloli/image-studio.git
cd image-studio
git status
git switch -c codex/windows-foundation
```

克隆会取得当前 `main` 及其完整祖先提交、源码、规格和已跟踪验收材料。仓库包含字体、样图和日志，首次传输需要一定时间；不包含数十 GB 模型权重，也不包含 `target`、`node_modules`、本地 `.codegraph` 索引或 Mac 的 Beads/Dolt 数据库。

GitHub 登录凭据由 Windows 自身凭据管理保存，不需要从 Mac 复制 token、SSH 私钥或配置文件。

当前 Windows 工作区已经克隆到 `C:\Users\XIAOKA-PC\Desktop\image-studio`，默认位于 `main`。`origin` 已配置不等于未来 push/pull 已获授权。

## 先阅读这些文件

| 顺序 | 文件 | 用途 |
|---|---|---|
| 1 | [AGENTS.md](../AGENTS.md) | 中文沟通、OpenSpec 批准、Beads、小步提交与远程授权规则 |
| 2 | [Windows proposal](../openspec/changes/introduce-windows-foundation/proposal.md) | 已确认的首轮交付与边界 |
| 3 | [Windows design](../openspec/changes/introduce-windows-foundation/design.md) | 低内存候选、进程树、平台身份、安装和打包边界 |
| 4 | [Windows tasks](../openspec/changes/introduce-windows-foundation/tasks.md) | Gate 0 已完成、其余实施/验收范围 |
| 5 | [Windows test plan](../openspec/changes/introduce-windows-foundation/test-plan.md) | Gate 0 已通过，产品 P0 尚未运行 |
| 6 | [研究依据](../openspec/changes/introduce-windows-foundation/sources.md) | 用户配置、当前 Mac 假设与上游一手来源 |

用户在 Windows 会话确认提案后，先通过 Gate 0：在这台设备用固定来源 Z-Image Turbo、单张 1024×1024、NF4/CPU 卸载候选验证完整加载、连续生成与资源预算。首轮只接这个 Windows 组合，其余六型号后续分批适配。

12 GB 显存和 32 GB 主存的 Gate 0 可行性已实测通过。不要复制 Mac runtime、删除历史任务、改小分辨率或把 MFLUX q4 身份重命名成 CUDA 来绕过正式产品边界。

## 开发工具与最初检查

仓库已有版本事实与 Windows 预检结果需要分开阅读：

- `rust-toolchain.toml` 固定 Rust **1.99.1**；预检发现的组件缺失已修复，`rustc/cargo/rustfmt/clippy` 与 `x86_64-pc-windows-msvc` 目标均已核实。目标机还装有 stable。
- 前端在 Mac 曾验证 Node.js **24.12.0** / npm **11.6.2**；仓库现固定 Windows 已通过的 Node.js **24.19.0** / npm **11.17.0**，约束位于 `.nvmrc`、`engines` 和 `packageManager`。
- Windows 已安装 Visual Studio Build Tools 2022 **17.14.40**、MSVC **19.44.35228**、Windows SDK **10.0.22621.0 / 10.0.26100.0** 和 VC++ Runtime **14.51.36247**。
- OpenSpec 为 **1.12.0**，Beads 为 **1.2.2**；Windows 未安装 GitHub CLI，也没有系统 Python 3.12 或完整 CUDA Toolkit/`nvcc`。Gate 0 使用 `target/windows-gate0/runtime` 下的受控 Python 3.12.10 与固定 wheel，不修改系统 Python。

这些不是 Windows 完整认证。Rust、Node/npm、Windows Known Folder、受控文件边界、outer/inner Job Object、Gate 0、正式安装在环、产品 worker/Controller 实图与首次 NSIS 安装烟测已就绪；干净用户/缺失 WebView2、完整 GUI/IME/DPI、真实取消/强杀、禁网与 Mac 回归仍未完成。

初步可以检查源码状态、前端与无平台依赖的核心：

```powershell
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy --target x86_64-pc-windows-msvc
npm --prefix apps/frontend ci
npm --prefix apps/frontend test
npm --prefix apps/frontend run build
cargo check -p studio-core -p worker-protocol --target x86_64-pc-windows-msvc --locked
openspec validate introduce-windows-foundation --strict --no-interactive
```

这些命令不证明完整 Windows 应用可用。桌面默认目录、受控文件、Job Object、Gate 0、Windows 专用正式 recipe/runtime/worker、安装在环与 NSIS 已实现；剩余工作是完整 P0 验收和缺陷修复，而不是重新建立产品身份。Python/CUDA/模型依赖按已冻结版本使用，不在系统 Python 安装浮动最新版。

2026-11-01 已执行的非模型检查结果：前端 46 个包安装成功，6 个测试文件/37 项测试通过，生产构建通过；固定 Rust 的 Windows 核心/协议 `cargo check`、fmt、66 项测试、1 项文档测试和 clippy 均通过。检查中发现并修复了生成协议测试夹具硬编码 POSIX 路径的问题。

`sha2` 的 `asm` 已限定到 macOS，Windows 不启用汇编后端，macOS 目标仍保留。随后 `cargo check/build --workspace --locked`、全 workspace 206 项测试 + 1 项文档测试以及 `cargo clippy --workspace --all-targets -- -D warnings` 均通过，4 项固定网络/运行时缓存测试为显式忽略。

宿主还分别用显式隔离根和无参数默认路径启动成功。Windows 默认目录通过 Known Folder API 解析为 `%LOCALAPPDATA%\ImageStudio`，四个子目录正确创建；设置页目录定位使用 Explorer 且不经过 shell。Windows 平台配置使用原生窗口，release 构建隐藏控制台。生成、安装探针与诊断已使用 outer/inner Job Object、suspended launch 和整树回收。NSIS 已组包、安装并启动；完整 UI/IME/DPI、干净用户/缺失 WebView2、真实取消/强杀、禁网和 Mac 回归仍未完成。

原生 Computer Use 已完整读取 `Image Studio` 的 WebView2 可访问性树，确认中文创作页、导航、输入框、模型状态与原生窗口控件存在，并成功写入/清空中文提示词。Graphics Capture 截图仍超时、元素点击缺少坐标、Tab 未移动焦点，因此目前没有视觉截图或完整键盘证据；限制记录为 `image-studio-ba1`，不能据此假定 UI 全部通过或失败。

## Beads 与跨机任务状态

Git 的 `main` 推送只同步源码提交。Mac 的任务库不会因为源码 `git push` 出现在 Windows；`.beads/metadata.json` 或 hooks 的存在也不证明目标机有可用任务记录。

2026-11-01 已初始化 Windows 本地 Dolt 空库，前缀为 `image-studio`，角色为 contributor；本次没有执行 Dolt pull/push。当前本地文档校正 issue 为 `image-studio-86d`。初始化工具把 `origin` 配置成 Dolt 远程候选，但没有执行远程同步。

Windows 代理仍应先运行 `bd prime` 检查本地任务库，不把 JSONL 当源码真相，不假定 Mac issue ID 已存在。提案获确认后，按 change 范围建立 Windows 本地实施 issue 并认领，再开始代码工作。

旧 Mac 已完成任务无需重新打开，完整产品/验收上下文在 Git 中的规格与报告里。若将来需要跨机共享任务历史，另行明确授权并配置 Dolt 原生同步；本次没有上传 `refs/dolt/data`，也没有执行 `bd dolt push/pull`。

## 给 Windows 新会话的接续提示

可粘贴以下内容开始新会话；这段文字只请求检查与接续准备，不代替用户批准实施：

> 请使用中文，先读取 AGENTS.md 和 doc/11-windows-development-handoff.md。我在 Windows 10 x64，RTX 3080 Ti 12 GB、内存 32 GB、i7-13700K。请读取 introduce-windows-foundation 的 proposal、design、specs、tasks、test-plan 和当前证据，运行 `bd prime` 后从剩余 P0 继续开发；不要重做已完成的 Gate 0/产品身份，也不要复制 Mac runtime 或把安装包烟测当完整模型认证。

本地小步提交已获项目长期授权。后续 `push`、`pull` 和 Dolt 同步仍按当前用户授权执行；本次首次源码上传不构成未来任意远程操作的默认许可。
