# Design

## Context

最新两个改变验收口径的 change 已归档：

- `2026-11-12-validate-windows-functional-regression`：把 Windows 验收拆成"功能回归"与"按执行面变化触发的完整生图认证"，并把功能回归实际执行放在 **Windows 11 / RTX 2060 SUPER 8 GB 补充机**；目标机复验移交给 Beads `b65`（本机库无此记录）。
- `2026-11-08-certify-windows-10-target-machine`：在本机（Windows 10 / RTX 3080 Ti 12 GB）建立 12 GB 标准档真实生成基线 `verification/windows-12gb-standard-tier/`，主要构建为 `b861ae5`（其后只有 docs-only 提交）。

当前主分支 `95dd117`（v0.1.1 之后 68 个提交）主要包含 v6 作品/会话管理、受控删除、单实例激活、启动诊断修复等 UI 与本地管理变化；`windows-desktop` 主规格新增了"功能回归不执行模型推理""真实出图按影响面触发"等要求，但**当前构建没有本机真实出图证据**，且上一轮功能回归中若干作品/会话/渲染项因本机无真实模型与真实作品而 `not_run`/`partial`。

本机当前状态（2026-11-12 实测）：

| 项 | 值 |
| --- | --- |
| OS / 架构 | Windows 10 Pro 22H2 build 19045 x64 |
| CPU / RAM | i7-13700K 16C/24T / 31.7 GiB |
| GPU | RTX 3080 Ti 12,288 MiB，sm86，驱动 596.49，空闲约 10.7 GiB |
| 显示 | 2560×1440，系统 DPI 96（100%）；上一轮记录为 3840×2160@150% |
| WebView2 | 157.0.4341.46 |
| 页面文件 / commit | 自动管理 4,864 MiB；commit limit 36.5 GiB，可用 commit 约 22.5 GiB；可用物理约 20.2 GiB |
| 磁盘 | C: 空闲 925.9 GiB |
| 网络 | 系统代理 `127.0.0.1:7897` 已启用 |
| 工具链 | Node 24.19.0 / npm 11.17.0 / Rust 1.99.1；Python 3.11/3.13/3.14 |
| 已安装应用 | `Image Studio 0.1.0`（2026-11-04 构建，位于 `%LOCALAPPDATA%\Image Studio`） |
| 真实 v5 数据 | `C:\ist-accept\data`：20 assets / 23 jobs / 1 conversation / 168.1 GiB（含 159.45 GiB 模型）；`C:\ist-a\data`：10 assets / 11 jobs / 2 conversations（2026-11-04） |
| 种子 artifact | `C:\ist-accept\cache\downloads` 152 项 / 135.5 GiB；`target/parity-gate0` 143.37 GiB；`target/parity-runtime` 111.89 GiB；`target/windows-gate0` 68 GiB |

## Goals / Non-Goals

**目标**

- 在当前构建上、在本机用产品界面完成七型号八能力的真实生成/编辑/去背景/放大，并留下当前构建的可定位证据。
- 用本轮真实生成的作品验证当前构建的 v6 作品与会话管理闭环（收藏、筛选、导出、删除、引用阻止、双输出、会话墓碑）。
- 用真实 v5 库副本验证 v5→v6 升级后的作品保留、查看与导出。
- 建立可复用的 Windows 目标机验收流程与证据口径，明确当前构建实测、复用证据与人工复核的区别。
- 补齐上一轮功能回归在目标机上 `not_run`/`partial` 的项，并如实保留无法执行的项。

**非目标**

- 不覆盖 8 GB 紧凑档（本机 12 GB；8 GB 证据属于补充机 `verification/windows-8gb-tier/`）。
- 不覆盖真实 16 GiB 机器低内存安装（Beads `pwp`，需另一台机器）。
- 不改变模型执行面、配方、运行时、协议、安装器或发布流程，不新增产品能力。
- 不建立持续 GPU CI，不承诺性能指标，不做签名/公证。
- 不在本轮批量真实下载全部模型权重；仅验证一条小构件真实网络安装路径，其余使用已核验种子缓存。
- 不修复执行中发现的缺陷（另立 Beads/OpenSpec change），也不重跑与执行面无关的 Gate 0。

## Decisions

### 1. 验收档位与标签

本轮执行**当前构建的 Windows 12 GB 目标机真实出图验收**。报告必须使用独立标签，至少区分：

| 标签 | 含义 | 本轮处理 |
| --- | --- | --- |
| 当前构建真实出图 | 本轮在产品界面生成、记录资源与哈希 | 必须覆盖八项能力 |
| 复用证据 | 历史 Gate 0 资源口径、8 GB 补充机功能回归 | 显式引用提交与路径，不改记为当前实测 |
| 人工复核 | 视觉质量、真实 IME、显示缩放 | 依赖用户操作，记录 `pending_user` 或结论 |
| 未执行 | 因权限/环境/安全原因未跑 | 保持 `not_run` 或 `blocked` |

不得把本轮结果写成"七模型八能力历史认证重跑"以外的模糊表述，也不得把复用证据混入当前构建实测。

### 2. 触发条件

`windows-desktop` 现有规则把完整生图认证绑定到执行面变化或基线缺失。本 change 增加第三个触发条件：**所有者明确要求完整真实出图验收**。执行时仍要做执行面 diff 审计并记录分类（本轮预期为 `no execution-surface change`），但分类结果不改变"所有者要求的轮次必须执行"这一事实；反之，8 GB 补充机的功能回归、历史基线或 CI 结果不得替代本机当前构建证据。

### 3. 构建与安装来源

- 冻结执行开始时（文档提交之后）的 `main` 提交，记录 `git rev-parse HEAD`、版本号与工作区状态。
- 用 `scripts/build-windows-internal.ps1`（默认质量门）构建 `x86_64-pc-windows-msvc` 版本与 NSIS current-user 安装包；记录 installer/exe SHA-256、构建日志、quality gate 日志。
- 用当前用户静默覆盖安装（旧版本为 0.1.0），验证应用版本、安装目录、图标/许可资源与启动；安装包未签名属于已知事实，不据此扣分。
- 记录冻结提交的 CI quality run 状态（只读）；若 CI 不可查或未成功，如实记录，不伪造。

### 4. 数据根与种子缓存

- **r-fresh（主证据，新用户视角）**：`C:\ist-rg\r-fresh`。用两份既有脚本预置零下载缓存，并在证据目录保存结果 JSON：
  1. `scripts/windows-parity/seed-parity-cache.ps1 -DataRoot C:\ist-rg\r-fresh -OutputPath <EVID>/seed-cache-parity.json`（七型号中的 parity 型号 + runtime-v2 + Real-ESRGAN native）；
  2. 将 `scripts/windows-ui/seed-cache.ps1` 复制到 `target/seed-windows-cache.ps1` 后运行（Z-Image 模型 + v1 运行时），`-DataRoot C:\ist-rg\r-fresh`。
  - 通过条件：两份结果均 `missing=0`、`hash_mismatch=0`，缓存项按 SHA-256 唯一；记录 artifacts/unique/bytes/硬链接数。若 parity 源不完整，回退为从 `C:\ist-accept\cache\downloads` 硬链接既有已验证缓存，并在证据中说明 provenance 差异。
- **r-v5（升级证据）**：`C:\ist-rg\r-v5`。只复制 `C:\ist-accept\data` 的 `library.sqlite3`（含 `-wal`/`-shm`）、`assets/`、`inputs/`、`licenses/`、`application-preferences.json`；不复制 `models/`、`runtimes/`、`installations/`（约 168 GiB）。源根保持只读，不修改。
- **r-net（网络安装证据）**：`C:\ist-rg\r-net`，空缓存启动，**真实下载并安装 Z-Image Turbo**（模型 30.59 GiB + 私有运行时 2.53 GiB），可选加装 Real-ESRGAN（模型 43 MiB + native runtime 43 MiB，同一 archive 去重）验证小构件。记录下载 URL、字节、耗时、吞吐、重试、SHA-256 与安装阶段，并与种子缓存路径分账。若网络不可用或校验失败，主证据仍用种子缓存，网络项记 `blocked`，不冒充。
- **r-fresh 主证据**：七型号中除网络验证型号外全部由种子缓存安装；Z-Image 的网络下载在 `r-net` 完成，`r-fresh` 的 Z-Image 从种子缓存安装，二者分开记录，避免把下载耗时混入缓存安装耗时。
- 其他故障分支使用 `C:\ist-rg\` 下的短根（如 `r-bad-webview`、`r-pathfail`），不触碰用户默认根。
- 新根的模型安装会占用约 168 GiB；当前空闲 925.9 GiB，可容纳旧根 + 新根 + 缓存硬链接。

### 5. 出图矩阵与真实作品顺序

标准档统一为 1024×1024 / `bfloat16`（BiRefNet 1024² RGBA+mask；Real-ESRGAN 1024²→4096²）。生成顺序设计为让后续管理测试都有真实素材：

| ID | 型号 / 能力 | 参考输入 | 产物用途 |
| --- | --- | --- | --- |
| G01a | FLUX.2 Klein 4B / 文生图 | 无 | 收藏、导出、编辑源图 |
| G01b | FLUX.2 Klein 4B / 文生图（第二张） | 无 | 引用阻断源图、删除对象 |
| G02 | Z-Image Turbo / 文生图 | 无 | 双输出/放大源图 |
| G03 | Qwen-Image 2.1 / 文生图 | 无 | 作品库分面 |
| G04 | Qwen-Image 2.1-Turbo / 文生图 | 无 | 作品库分面、取消用例 |
| G05 | MOSS-Image-SFT / 文生图 | 无 | 作品库分面 |
| G06 | FLUX.2 Klein 4B / 单图编辑 | G01a | 编辑能力 |
| G07 | BiRefNet / 去背景 | G02 | 双输出删除、mask 校验 |
| G08 | Real-ESRGAN / 4× 放大 | G01b | 放大能力、精确 4× 校验 |

- 每项固定记录：提示词（中英各至少一条）、seed、尺寸、步数/参数、模型与执行身份、阶段进度、墙钟、输出文件尺寸/字节/SHA-256。
- 允许为保持真实感使用不同提示词；但同一能力至少一次完整成功即可，失败项按 `fail` 记录并保留原因，不用重试结果掩盖首次失败。
- 真实作品超过 8 件后，作品库筛选/删除测试有足够样本；其中 G01b 在删除测试后仍可由 Real-ESRGAN 源图引用覆盖引用/解除分支，必要时补充一次重生成。

### 6. 资源口径

产品级证据采集方式（执行时新增一个只用于验收的采样脚本）：

- `nvidia-smi` 1 s 采样 GPU used，记峰值并注明约 1.4 GiB 桌面基线（与 2026-11-08 方法一致）。
- 每 1 s 汇总生成进程树（宿主 → supervisor → worker）的 `Win32_Process.PrivatePageCount`，记峰值。
- 每 1 s 采样系统可用物理内存，记最低值；同时记录 `app.log` 的 `generation.start available_gib` 与阶段耗时。
- 墙钟取界面开始/结束时间与 `attempts.created_at/completed_at` 差值，并与阶段日志互证。
- 预算对照：host GPU 峰值减基线 ≤10.5 GiB（代理口径）、进程树 private commit ≤24 GiB、可用物理内存最低 ≥2 GiB、加载 ≤600 s、生成 ≤900 s、无 WDDM 共享回退（以 nvidia-smi 与进程行为判断并注明限制）。
- worker 内部 CUDA reserved 未被产品持久化：本轮引用同一执行面的历史 Gate 0 证据（`verification/windows-12gb-standard-tier/` 的 worker/probe 材料）并标注"复用"；若执行面 diff 审计发现执行面变化，则 Gate 0 证据不得复用，该项保持 `not_run` 并另立计划。

### 7. 作品/会话管理与真实数据

- 全部使用本轮真实生成的作品（G01–G08），不使用构造库；旧文件副本只允许用于 v5 升级后的旧作品导出比对。
- 引用阻断：将 G01b 设为当前草稿源图后删除，验证阻止提示与解除路径；再切走能力验证"未激活槽位仍引用"分支；按提示解除后删除成功。
- 双输出：BiRefNet 的 primary 与 mask 作为一组删除，两个 Asset ID 均不可读/不可导出/不可作源图。
- 会话墓碑：删除本轮作品所在会话，作品保留、来源显示"会话已删除"、重启不复活。
- 导出：原生保存对话框，中文/空格路径，导出 SHA-256 与正式资产一致。

### 8. v5→v6 真实升级

- 副本 `r-v5` 首次启动触发 v5→v6：核对 20 assets / 23 jobs / 1 conversation / 1 draft 保留，`is_favorite`、`deletion_state` 等新列默认值正确，`PRAGMA quick_check` 通过。
- 旧作品预览与导出：抽 2 件旧作品核对 SHA-256 与字节一致。
- 升级后闭环保留：从种子缓存安装 Klein（约 15 GiB / 110 s 级），真实出图一次并导出。
- 重启幂等：不重复迁移、不复制作品、不新增默认会话。
- 迁移失败保护：用只读副本或损坏副本尝试启动，确认不重建空库；无法安全构造时记 `not_run`。

### 9. 故障与人工项

- 取消：Klein（CUDA）、Qwen-Turbo（Python 分阶段）、BiRefNet（CPU/ONNX）、Real-ESRGAN（Vulkan）各一次，15 s 内落定、无半成品、无残留进程。
- 强杀恢复：Z-Image 生成中强杀宿主，重启后 Attempt 为 interrupted、无资产，只有显式重试才新建 Attempt。
- 看门狗、磁盘不足等分支沿用历史注入方法，安全可复现时执行，否则 `not_run`。
- **所有者决定不执行（2026-11-12 确认）**：整树禁网生成（不测，不视为通过）、真实微软拼音 IME、显示缩放切换、八张图视觉接受/拒绝。全部记 `waived`/`not_run` 并记录决定来源；机器侧质量检查（PNG 解码、尺寸/通道、非全黑、mask 位深、4× 尺寸）照常执行，报告不得把 waived 项写成通过，结论只能声明"机器证据通过 + 人工项未测"。

### 10. 证据结构与完成标准

- 证据目录：`verification/windows-12gb-real-generation-<YYYYMMDD>/`，至少包含 `README.md`、`environment.json`、`results.json`（逐项状态）、`seed-cache-*.json`、每项生成 JSON、资源采样 CSV、截图、数据库快照、构建/质量门日志、`user-review.json`。
- 状态集合：`pass | fail | blocked | not_run`；任何未执行项不得记为通过。
- 完成标准：八项能力均有当前构建真实输出证据；作品/会话/v5 升级/故障分支按计划执行或如实记录未执行；人工复核项按用户决定记 `waived`；结论标签明确；失败项有复现步骤与 Beads。
- 若八项能力中任一项因当前构建缺陷失败，本轮不宣布"真实出图验收通过"，按失败项另立修复 change 后重跑受影响项。

### 11. 与 CI、其他机器的分工

- CI 继续负责编译、单测、权限清单、安装包构建与哈希；不替代本机真实出图。
- 8 GB 补充机功能回归只承担功能证据；目标机证据以本轮为准。
- 真实 16 GiB 机器低内存证据（Beads `pwp`）不在本轮，由另一台机器单独执行。

## Risks / Trade-offs

- **种子缓存让安装脱离真实下载**：主证据使用缓存安装，只对 Real-ESRGAN 走真实网络；报告必须标注缓存来源，不宣称验证了全量下载链路。
- **资源口径为 host 观测代理**：GPU 峰值包含桌面基线，与 worker CUDA reserved 不完全等价；必须写明方法与限制，并用历史 Gate 0 证据补足，不得伪造产品级 reserved 数字。
- **真实作品依赖生成顺序**：删除/引用测试会消耗作品；按"先生成后管理"的顺序执行，并保留至少一件作品直到结论复核。
- **v6 升级与模型身份变化**：旧安装记录不复制，升级后模型显示未安装是预期；需从缓存重装一个型号验证闭环保留，避免误判为升级失败。
- **强杀/取消可能留下 GPU 资源**：执行后检查进程树与 nvidia-smi 回落，异常时记录并清理，不把残留当通过。
- **人工项已 waived**：真实 IME、显示缩放切换与视觉接受/拒绝本轮不测；结论只能声明机器证据通过并列出 waived 项，不能声明视觉质量已接受。
- **页面文件/内存波动**：当前满足低内存 prepare 门槛，但长时间执行可能波动；若被预检拒绝，记 `blocked` 并给出实测值，不调整系统配置（除非用户明确授权）。

## Migration Plan

1. 用户确认本 change 后，建立 Beads 跟踪 issue，冻结当前提交并记录 CI 状态。
2. 完成环境/执行面/diff/数据安全前置证据，创建 `C:\ist-rg\` 与种子缓存。
3. 构建并安装当前构建，完成安装、启动、单实例与启动诊断检查。
4. 按 [test-plan.md](test-plan.md) 执行种子安装、八项真实出图、作品/会话管理、v5 升级与故障分支，逐项记录证据。
5. 汇总机器证据与 waived 项，用户确认结论标签（人工项本轮不测）。
6. 归档 change、合并 `windows-desktop` delta、关闭/更新 Beads，并在 `verification/` 与 `doc/15-windows-8gb-tier.md`（如适用）更新索引。
