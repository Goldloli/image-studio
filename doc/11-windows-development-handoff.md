# Windows 开发交接

交接日期：2026-11-01。私有源码仓库：[Goldloli/image-studio](https://github.com/Goldloli/image-studio)，默认分支 `main`。

## 已完成与当前授权

Mac M3 Max / 128 GiB 已完成七型号、八能力验收，相关变更已归档。现有 macOS 数据目录、模型安装、Python/MLX 环境和个人图片不随 Git 克隆迁移。

用户已确认 Windows 开发机为 **Windows 10 x64 / RTX 3080 Ti 12 GB / 32 GB RAM / Intel Core i7-13700K**，并授权将源码推送 GitHub，转到 Windows 端继续开发。当前 `introduce-windows-foundation` 仍是已写好、待用户确认实施的提案；本次上传不把提案自动改成已批准，也没有实现 Windows 推理。

## 2026-11-01 Windows 目标机预检

目标机只读预检确认：

- Windows 10 Pro 22H2 x64，`10.0.19045.6466`，中文系统区域。
- i7-13700K，16 核 / 24 线程；2 × 16 GB Kingston DDR4-3200，系统可见物理内存 31.73 GiB。
- NVIDIA RTX 3080 Ti `12288 MiB`、driver `596.49`、sm86；系统同时存在 Intel UHD Graphics 770，CUDA 路线只能选择 NVIDIA 设备。
- WebView2 Runtime `156.0.4314.58`；Dell U2723QE 当前 3840×2160 / 100% 缩放；中文和英文键盘布局均已启用。
- 系统盘为 NTFS NVMe，预检时可用约 1576 GiB；分页文件由系统自动管理，当时约 4.75 GiB。

这些信息证明基础机器条件匹配，不证明 Windows 构建、GUI、GPU 算子或 Z-Image Gate 0 通过。详细快照和证据边界见 [Windows sources](../openspec/changes/introduce-windows-foundation/sources.md)。

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
| 2 | [Windows proposal](../openspec/changes/introduce-windows-foundation/proposal.md) | 首轮交付与待确认选择 |
| 3 | [Windows design](../openspec/changes/introduce-windows-foundation/design.md) | 低内存候选、进程树、平台身份、安装和打包边界 |
| 4 | [Windows tasks](../openspec/changes/introduce-windows-foundation/tasks.md) | 28 项尚未开始的实施/验收范围 |
| 5 | [Windows test plan](../openspec/changes/introduce-windows-foundation/test-plan.md) | 44 个尚未运行的 P0 用例 |
| 6 | [研究依据](../openspec/changes/introduce-windows-foundation/sources.md) | 用户配置、当前 Mac 假设与上游一手来源 |

用户在 Windows 会话确认提案后，先通过 Gate 0：在这台设备用固定来源 Z-Image Turbo、单张 1024×1024、NF4/CPU 卸载候选验证完整加载、连续生成与资源预算。首轮只接这个 Windows 组合，其余六型号后续分批适配。

12 GB 显存和 32 GB 主存是否达标尚未实测。不要先复制 Mac runtime、删除历史任务、改小分辨率或把 MFLUX q4 身份重命名成 CUDA 来绕过可行性门槛。

## 开发工具与最初检查

仓库已有版本事实与 Windows 预检结果需要分开阅读：

- `rust-toolchain.toml` 固定 Rust **1.99.1**；目标机还装有 stable，但固定的 1.99.1 当前缺少 manifest/rustc，首次构建前必须先修复。
- 前端在 Mac 验证 Node.js **24.12.0** / npm **11.6.2**；Windows 当前为 Node.js **24.19.0** / npm **11.17.0**。仓库尚无 `.nvmrc`、`engines` 或 `packageManager` 约束，实施前须明确可复现版本。
- Windows 已安装 Visual Studio Build Tools 2022 **17.14.40**、MSVC **19.44.35228**、Windows SDK **10.0.22621.0 / 10.0.26100.0** 和 VC++ Runtime **14.51.36247**。
- OpenSpec 为 **1.12.0**，Beads 为 **1.2.2**；Windows 未安装 GitHub CLI，也没有 Python 3.12 或完整 CUDA Toolkit/`nvcc`。Python 3.12 与 wheel 闭包应由后续受控 Gate 0 环境解决，不修改系统 Python。

这些不是 Windows 完整认证。当前固定 Rust 工具链未就绪，Node/npm 也尚未冻结，因此不能直接宣称 Windows 构建环境已准备完成。

初步可以检查源码状态、前端与无平台依赖的核心：

```powershell
rustup toolchain install 1.99.1 --profile minimal --component rustfmt --component clippy --target x86_64-pc-windows-msvc
npm --prefix apps/frontend ci
npm --prefix apps/frontend test
npm --prefix apps/frontend run build
cargo check -p studio-core -p worker-protocol --target x86_64-pc-windows-msvc --locked
openspec validate introduce-windows-foundation --strict --no-interactive
```

这些命令不证明完整 Windows 应用或模型可用。桌面默认目录、监督实现、运行时、原生路径和 NSIS 打包仍需按提案实施；当前不应直接把 Mac 的启动/离线验收脚本用于 Windows。Python/CUDA/模型依赖的最终版本由 Gate 0 冻结，不先在系统 Python 安装浮动最新版。

## Beads 与跨机任务状态

Git 的 `main` 推送只同步源码提交。Mac 的任务库不会因为源码 `git push` 出现在 Windows；`.beads/metadata.json` 或 hooks 的存在也不证明目标机有可用任务记录。

2026-11-01 已初始化 Windows 本地 Dolt 空库，前缀为 `image-studio`，角色为 contributor；本次没有执行 Dolt pull/push。当前本地文档校正 issue 为 `image-studio-86d`。初始化工具把 `origin` 配置成 Dolt 远程候选，但没有执行远程同步。

Windows 代理仍应先运行 `bd prime` 检查本地任务库，不把 JSONL 当源码真相，不假定 Mac issue ID 已存在。提案获确认后，按 change 范围建立 Windows 本地实施 issue 并认领，再开始代码工作。

旧 Mac 已完成任务无需重新打开，完整产品/验收上下文在 Git 中的规格与报告里。若将来需要跨机共享任务历史，另行明确授权并配置 Dolt 原生同步；本次没有上传 `refs/dolt/data`，也没有执行 `bd dolt push/pull`。

## 给 Windows 新会话的接续提示

可粘贴以下内容开始新会话；这段文字只请求检查与接续准备，不代替用户批准实施：

> 请使用中文，先读取 AGENTS.md 和 doc/11-windows-development-handoff.md。我在 Windows 10 x64，RTX 3080 Ti 12 GB、内存 32 GB、i7-13700K。请读取 introduce-windows-foundation 的 proposal、design、specs、tasks 和 test-plan，检查当前工具链、驱动、磁盘与 Beads 状态。提案尚待确认，请先报告准备情况，等我明确确认后按 Gate 0 开始；不要重做 Mac 接入、复制 Mac runtime 或把编译通过当完整模型认证。

本地小步提交已获项目长期授权。后续 `push`、`pull` 和 Dolt 同步仍按当前用户授权执行；本次首次源码上传不构成未来任意远程操作的默认许可。
