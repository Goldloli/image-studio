# Proposal

## Why

最新一轮 Windows 验收（2026-11-12，change `validate-windows-functional-regression`）在 **Windows 11 / RTX 2060 SUPER 8 GB 补充机**上完成功能回归：不下载模型、不推理、不出图；目标机（本机 Windows 10 Pro 22H2 19045 / i7-13700K / RTX 3080 Ti 12,288 MiB sm86 / 31.7 GiB RAM）的复验被移交 Beads（本机 local Dolt 库中不存在 `b65`，且该库没有可用的 Dolt 远程 refs）。

本机最后一次**完整 12 GB 标准档真实出图认证**是 2026-11-08 的 `verification/windows-12gb-standard-tier/`（主要在 `b861ae5` 上执行，其后仅有 docs-only 提交）。此后主分支又前进了 68 个提交并发布 `v0.1.1`：v6 作品/会话管理、收藏与受控删除、单实例激活、启动诊断修复等已进入当前构建，但**当前构建在目标机上的真实出图与"出图后的作品管理"从未实测**。

`openspec/specs/windows-desktop` 现有要求把完整生图认证绑定到执行面变化或基线缺失；对当前构建，diff 分类仍可能是 `no execution-surface change`，从而允许复用历史生图证据。但所有者明确要求在**这台验收机**上做一系列真实的、包含出图的测试，并且 8 GB 补充机的功能回归与历史生图基线都不能替代目标机当前构建的证据。因此需要一次独立、可审计的当前构建真实出图验收轮。

本机条件已具备，适合一次做完：

- **artifact 齐全**：`C:\ist-accept\cache\downloads` 有 152 个缓存项 / 135.5 GiB；`target/parity-gate0` 143.37 GiB、`target/parity-runtime` 111.89 GiB、`target/windows-gate0` 68 GiB 可支撑零下载种子安装。
- **真实 v5 数据**：`C:\ist-accept\data` 有 20 件真实作品 / 23 个任务 / 1 个会话，`C:\ist-a\data` 有 10 件作品 / 11 个任务，可直接做 v5→v6 真实升级与旧作品保留验证。
- **内存预检满足**：低内存 prepare 门槛（可用物理 ≥8 GiB 且 commit 余量 ≥6 GiB）当前满足（约 20.2 GiB / 22.5 GiB），无需管理员调整页面文件。
- **补齐上次缺口**：上一轮因本机没有真实作品/模型而 `not_run`/`partial` 的项（C04、A05、A06、A09、D01 作品保留、U02/U03/U04、S04）可在本轮用真实出图闭环。

## What Changes

- 新增"Windows 目标机真实出图验收"轮次：冻结当前构建 → 质量门与 NSIS 构建 → 覆盖安装 → 全新隔离数据根 → 种子缓存安装七型号 → **八项能力真实生成**（Klein 文生图×2、Klein 单图编辑、Z-Image 文生图、Qwen-Image 2.1、Qwen-Image 2.1-Turbo、MOSS-Image-SFT、BiRefNet 去背景、Real-ESRGAN 4×）→ 导出逐字节、重启恢复、资源与质量证据。
- 要求真实作品参与当前构建的管理闭环：收藏/筛选/预览/导出/受控删除、BiRefNet 双输出删除、引用阻止与解除、会话删除后作品墓碑与保留；不得再用构造库或旧文件副本冒充本轮生成。
- 使用 `C:\ist-accept\data` 的**可丢弃副本**做 v5→v6 真实升级：20 件旧作品 / 23 个任务 / 1 个会话保留，旧作品导出逐字节一致；随后重装一个型号并真实出图一次。
- 明确资源与证据口径：产品级记录 host 观测 GPU 峰值（1 s 采样并说明桌面基线）、生成进程树私有提交峰值、可用物理内存最低值、加载/生成墙钟；worker 内部 CUDA reserved 由同一执行面的 Gate 0 证据承担并显式标注复用，不伪造产品级新实测。
- 至少一条**真实网络安装路径**（Real-ESRGAN 小构件 + native runtime，空缓存根），其余使用已验证种子缓存并记录 provenance。
- 固定一份可复用的 Windows 目标机验收流程（环境身份、构建身份、数据安全、种子缓存、出图矩阵、证据结构、结论标签），后续轮次按同一结构执行。
- 补充触发条件：所有者明确要求完整真实出图验收时，8 GB 补充机功能回归、历史生图基线与 CI 结果均不得替代目标机当前构建证据。
- 不修改产品行为。执行中发现的缺陷按影响面登记 Beads 并另立 OpenSpec change；既有观察项 F2（stub-supervisor 入包）、F3（功能操作不写日志）、F4（answer-dialog 消息框）可记录但不在此修复。

## Capabilities

### New Capabilities

无。本 change 不新增产品能力。

### Modified Capabilities

- `windows-desktop`：
  - 修改"Windows 12 GB 标准档在当前主分支上有真机回归证据"，增加"所有者明确要求完整真实出图验收"触发条件，并明确其他机器/历史基线不得替代；
  - 新增"Windows 目标机真实出图验收覆盖当前构建的能力矩阵与作品管理"；
  - 新增"Windows 目标机真实出图验收的证据口径与结论标签"。

## Impact

- **文档**：新增本 change 的 proposal、design、spec delta、test-plan、tasks 与 README。
- **验收**：全部在本机（Windows 10 Pro 22H2 build 19045，无管理员权限假设）执行。范围含构建/安装、约 168 GiB 新数据根、七型号安装（预计 15–25 分钟）、八项能力真实生成（预计生成墙钟合计 8–15 分钟，另加加载），以及 v5 升级与故障分支；预计 3–5 小时。
- **数据**：新隔离根 `C:\ist-rg\`（`r-fresh` 新用户、`r-v5` 真实 v5 副本、`r-net` 网络安装与故障短根）；不修改 `C:\ist-accept`、`C:\ist-a`，不使用用户默认数据根。新增磁盘约 170–200 GiB（当前 C: 空闲 925.9 GiB）。
- **系统**：不要求管理员；真实网络安装依赖当前代理 `127.0.0.1:7897` 可用；可选整树禁网验收需要管理员，不可用则记 `blocked`；不修改页面文件。
- **代码**：不修改产品代码；执行时可能新增验收用采样/驱动脚本（如资源采样 PowerShell），只用于验收证据，不进入产品运行时。
- **Beads**：用户确认后建立跟踪 issue，承接目标机复验与本轮失败项。
- **远程操作**：只读查询既有 CI run 与执行本地 git diff 审计；不 push、不触发远程写操作。
