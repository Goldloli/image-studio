# Windows 12 GB 目标机真实出图验收测试计划

状态：**已确认执行（2026-11-12）。** 用户确认时调整：真实下载并安装大型模型；不执行整树禁网验收；真实 IME、显示缩放切换与视觉质量人工复核不测（记 waived/not_run）。

- 机器：Windows 10 Pro 22H2 build 19045 x64 / i7-13700K / RTX 3080 Ti 12,288 MiB（sm86、驱动 596.49）/ 31.7 GiB RAM / 2560×1440@100% / WebView2 157.0.4341.46。
- 证据目录：`verification/windows-12gb-real-generation-<YYYYMMDD>/`（下称 `<EVID>`；执行开始前创建并写入 `environment.json`）。
- 数据根：`C:\ist-rg\r-fresh`（新用户）、`C:\ist-rg\r-v5`（真实 v5 副本）、`C:\ist-rg\r-net`（真实网络安装）、`C:\ist-rg\r-*`（故障分支）。不使用用户默认数据根，不修改 `C:\ist-accept`、`C:\ist-a`。
- 本计划是"当前构建 + 目标机 + 真实出图"的完整验收；功能回归、8 GB 紧凑档、真实 16 GiB 机器（Beads `pwp`）不在范围内。

## 0. 前置条件

### P01 构建冻结与 CI

- [ ] 冻结执行时的 `main` 提交，记录 `git rev-parse HEAD`、`git status --porcelain`、版本号（Cargo / Tauri / 前端应一致）。
- [ ] 只读查询该提交的 GitHub `quality` / Windows 构建状态并记录 run id 与结论；不可查或未成功时如实记录，不伪造。
- [ ] 记录构建方式：`scripts/build-windows-internal.ps1`（默认质量门，NSIS current-user，离线 WebView2 installer）。
- [ ] 记录质量门日志：前端 `npm test` / `npm run build`、`cargo fmt --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked`。
- [ ] 记录安装包/安装后 exe 的字节数与 SHA-256，写入 `<EVID>/environment.json`。

### P02 机器身份

- [ ] 记录 OS 版本/build、架构、CPU、RAM、GPU/显存/compute capability/驱动、显示分辨率与系统 DPI、WebView2、磁盘可用、页面文件/commit limit/可用 commit、可用物理内存、代理设置、Node/npm/Rust/Python 版本。
- [ ] 本机当前参考值（执行时重新实测并覆盖）：Windows 10 19045 / i7-13700K / 31.7 GiB / RTX 3080 Ti 12288 MiB sm86 驱动 596.49 / 2560×1440@96dpi / WebView2 157.0.4341.46 / C: 空闲 925.9 GiB / 页面文件 4,864 MiB / commit limit 36.5 GiB / 可用 commit 约 22.5 GiB / 可用物理约 20.2 GiB / 代理 127.0.0.1:7897。

### P03 基线对照与执行面分类

- [ ] 引用历史基线：`verification/windows-12gb-standard-tier/`（2026-11-08，构建 `b861ae5` 级别，12 GB 标准档真实生成）与 `verification/windows-functional-regression-20261112/`（2026-11-12，Windows 11 / RTX 2060 SUPER 8 GB 补充机，功能回归、无推理）。
- [ ] 执行 `git diff <基线提交>..HEAD -- tools resources/windows-parity crates/studio-core crates/worker-protocol apps/desktop/src` 审计，将结论写为 `no execution-surface change` / `execution-surface change` / `ambiguous`。
- [ ] 本轮预期为 `no execution-surface change`；若为 `execution-surface change`，产品级资源口径不得复用历史 Gate 0，相关项记 `not_run` 并另立计划。

### P04 数据安全

- [ ] 创建 `C:\ist-rg\`；确认源根 `C:\ist-accept`、`C:\ist-a` 在验收期间保持只读（不启动、不写入）。
- [ ] 复制 v5 子集到 `r-v5`：`library.sqlite3`（连同 `-wal`、`-shm`）、`assets/`、`inputs/`、`licenses/`、`application-preferences.json`；不复制 `models/`、`runtimes/`、`installations/`。
- [ ] 记录复制前后文件清单/大小与至少 2 个文件的 SHA-256，写入 `<EVID>/data-safety.json`。
- [ ] 所有导出测试文件使用 `<EVID>/exports/` 或 `C:\ist-rg\exports\`，记录 SHA-256。

### P05 种子缓存 provenance

- [ ] 运行 `pwsh -File scripts/windows-parity/seed-parity-cache.ps1 -DataRoot C:\ist-rg\r-fresh -OutputPath <EVID>/seed-cache-parity.json`。
- [ ] 复制 `scripts/windows-ui/seed-cache.ps1` 到 `target/seed-windows-cache.ps1` 后运行（`-DataRoot C:\ist-rg\r-fresh`），结果写入 `<EVID>/seed-cache-zimage.json`。
- [ ] 通过条件：两份结果 `missing=0`、`hash_mismatch=0`，缓存项按 SHA-256 唯一；记录 artifacts / unique shas / bytes / hardlink / copy 数。
- [ ] 若 parity 源不完整：回退为从 `C:\ist-accept\cache\downloads`（152 项 / 135.5 GiB，2026-11-08 已验证）硬链接既有缓存，并在证据中标注 provenance 差异。

### P06 工具、端口与编码

- [ ] UI 驱动使用 `scripts/windows-ui/`（`cdp.mjs`、`resize-window.ps1`、`save-ui-dialog.ps1`、`answer-dialog.ps1`）。
- [ ] WebView2 CDP 端口选 **9222**（本机 Hyper-V 保留 9259–9358，勿用 9333），启动时设置 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9222` 并确认监听。
- [ ] 辅助脚本统一用 `pwsh` 运行；自动生成的 JSON/日志写 UTF-8 无 BOM，避免 PowerShell 5.1 ANSI 读取导致中文乱码。
- [ ] 记录仓库工作区状态：除验收产生的证据文件外保持干净。

### P07 内存预检现状

- [ ] 记录页面文件、commit limit、可用 commit、可用物理内存。
- [ ] 低内存 prepare 门槛为"可用物理 ≥8 GiB 且 commit 余量 ≥6 GiB"；当前满足，**不修改系统页面文件**。
- [ ] 若执行中门槛不再满足且安装被预检拒绝：记 `blocked` 并保存实测值，不调整系统配置（除非用户明确授权）。

## 1. 构建与安装

### I01 质量门与构建

- [ ] 运行 `scripts/build-windows-internal.ps1` 完成质量门、sidecar 构建与 NSIS 打包；保存 `build-quality-gates.log`、`build-desktop.log` 到 `<EVID>`。
- [ ] 确认 `target/release/bundle/nsis/` 下安装包为当前提交构建。

### I02 覆盖安装与安装身份

- [ ] 以当前用户静默安装当前构建（覆盖旧 0.1.0）：`Start-Process -FilePath '<installer>' -ArgumentList '/S' -Wait`。
- [ ] 记录安装退出码、注册表 `DisplayVersion`、安装目录、应用版本页、图标与许可证资源。
- [ ] 确认启动不下载模型、不准备受管运行时、不安装 GPU 驱动。

### I03 全新根首次启动（新用户）

- [ ] 用 `--data-dir C:\ist-rg\r-fresh` 启动，确认主窗口可见、日志写入 `<root>\logs\app.log`。
- [ ] 模型页：7 型号、0 已安装、设备身份 RTX 3080 Ti / 约 12,288 MiB / sm86；设备满足 7；无紧凑档投影。
- [ ] Z-Image Turbo 显示 12 GB 门槛满足，未安装状态如实显示；无自动下载。

### I04 单实例激活

- [ ] 同根第二次启动（空闲时）：5 s 内已运行窗口被激活/前置，第二个进程退出码 0，不写 `startup-failure.log`。
- [ ] 同根第二次启动（生成中）：激活且不干扰当前生成；不打开第二套 WebView2 数据目录。
- [ ] 不同根两实例可同时运行，各自 `cache/webview2` 独立。

### I05 启动失败诊断

- [ ] 超长数据路径（生成 >94 UTF-16 units 的根）：拒绝启动，写 `logs/startup-failure.log`（含实测长度与建议），弹中文对话框，退出码 3，不创建 data/cache。
- [ ] `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 指向不存在目录：同样写日志、弹框、退出码 3，且 `app.log` 保留 panic 行。
- [ ] 失败后环境修复可正常启动，原数据不受影响。

### I06 日志与无控制台

- [ ] `app.log` 每行含时间、级别、事件、关键字段；生成/安装期间轮询确认无新 conhost/OpenConsole/WindowsTerminal 进程。
- [ ] 记录日志轮转上限（8 MiB × 3）核对方式。

## 2. 种子缓存与模型安装

### M01 种子缓存安装七型号

- [ ] 通过模型页依次安装：BiRefNet、Real-ESRGAN、Klein、Z-Image、Qwen-Image 2.1-Turbo、Qwen-Image 2.1、MOSS-Image-SFT。
- [ ] 每项记录：安装类型（种子缓存）、字节数、阶段耗时、就绪记录、无网络下载。
- [ ] 历史参考值（2026-11-08，非通过标准）：Klein ≈110 s / 18.7 GB 下载、BiRefNet ≈32 s、Real-ESRGAN ≈12 s、Z-Image ≈172 s / 35.56 GB、Qwen/MOSS 低内存 prepare ≈364–415 s。

### M02 真实网络下载与安装（大型模型）

- [ ] 新建 `C:\ist-rg\r-net`，不用种子缓存，真实下载并安装 **Z-Image Turbo**（模型约 30.59 GiB + 私有运行时约 2.53 GiB）；若源站/网络不可用，改以 Klein（约 14.88 GiB）作为备选并记录选择原因。
- [ ] 记录每个下载 artifact 的 URL、声明字节、实际字节、SHA-256、耗时、平均吞吐、重试/失败与恢复、代理设置；记录安装阶段耗时、prepared 缓存与就绪状态。
- [ ] 可选：同根加装 Real-ESRGAN（模型 archive 约 43 MiB + native runtime archive 约 43 MiB，同 SHA 去重），验证小构件下载路径。
- [ ] 失败/校验不符/超时：保留错误与部分文件状态，记 `blocked` 或 `fail`，不冒充安装或出图通过；主证据仍用种子缓存并在报告分账。
- [ ] 不得把 `r-net` 的下载耗时混入 `r-fresh` 的缓存安装耗时。

### M03 安装期量化与内存预检

- [ ] Qwen-Image 2.1 / Turbo / MOSS 走低内存 prepare：记录 `logs\installation-process.log` 的 `[prepare-preflight]` 行、prepared 目录大小（参考：Qwen prepared 9.36 GiB、MOSS prepared 7.89 GiB）、prepare 阶段与耗时。
- [ ] 若预检拒绝：保存错误文本与实测值，记 `blocked`，不得记通过。

### M04 重启后型号状态

- [ ] 重启应用后核对：型号仍就绪或按激活身份变化需重装（如实记录并重装）；旧作品可打开；无自动重装/自动下载。

### M05 模型页投影

- [ ] 已安装型号显示 `standard · 1024² · bfloat16`；设备 7/7 满足；Qwen 系列显示非商用研究许可；许可与门槛原因可见。

## 3. 真实出图矩阵（八项能力）

统一标准档：文生图/编辑 1024×1024；BiRefNet 1024² RGBA+mask；Real-ESRGAN 1024²→4096²。每项均从产品界面发起，记录提示词、seed、参数、模型/执行身份、阶段进度、墙钟、输出与资源。

| ID | 型号 / 能力 | 参考输入 | 提示词建议（可调整，记录实际值） |
| --- | --- | --- | --- |
| G01a | Klein 4B / 文生图 | 无 | 中文："雨后的上海街角，一位撑伞的行人，霓虹倒影，写实摄影" |
| G01b | Klein 4B / 文生图 | 无 | English: "a red fox sitting in a snowy forest at dusk, cinematic lighting, photorealistic" |
| G02 | Z-Image Turbo / 文生图 | 无 | 中文："雪山下的湖泊与晨雾，远景，超广角风景摄影" |
| G03 | Qwen-Image 2.1 / 文生图 | 无 | 中文："一盆粉色樱花放在木质窗台上，晨光，浅景深"（40 步） |
| G04 | Qwen-Image 2.1-Turbo / 文生图 | 无 | English: "a cozy bookstore cafe with warm lights, rain outside the window"（8 步） |
| G05 | MOSS-Image-SFT / 文生图 | 无 | 中文："赛博朋克风格的雨夜街道，霓虹灯牌，电影感"（50 步） |
| G06 | Klein 4B / 单图编辑 | G01a | 中文："把背景换成夜晚的东京街道，保留人物与伞" |
| G07 | BiRefNet / 去背景 | G02 | 无提示词；输出 RGBA primary + 8-bit mask |
| G08 | Real-ESRGAN / 4× 放大 | G01b | 无提示词；1024² → 4096² |

### G01–G08 逐项要求

- [ ] G01–G05 文生图：进度阶段可见（准备/加载/采样/保存），成功提交为正式作品；每项记录输出 PNG 尺寸/字节/SHA-256、墙钟、资源采样。
- [ ] G06 编辑：使用 G01a 作为参考图，保留源图；输出记录同文生图。
- [ ] G07 去背景：输出 primary RGBA + mask，两者均为正式资产；记录尺寸/通道/mask 位深/字节/SHA-256。
- [ ] G08 放大：输出精确 4096×4096（4×）；记录尺寸/字节/SHA-256。
- [ ] 失败项：保存错误文本、阶段与日志，记 `fail`；不得用重试结果掩盖首次失败，重试结果单独记录。

### G09 导出与逐字节一致

- [ ] 对 G01a、G08 与至少一件 Qwen 输出，通过原生保存对话框导出到中文/空格路径。
- [ ] 导出文件 SHA-256 与正式资产一致；取消/拒绝覆盖分支无副作用。

### G10 质量与完整性

- [ ] 机器检查：PNG 可解码、尺寸/通道符合能力、非全黑/全白、mask 为 8-bit 单通道、4× 为 4096²。
- [ ] 人工视觉检查（提示词符合度、构图、伪影、编辑保留度）：**用户决定本轮不测（waived/not_run）**；不得据此声明视觉质量通过，也不得把机器检查写成视觉接受。

### G11 资源口径

- [ ] 执行时新增/使用验收采样脚本，在每次生成期间以 1 s 间隔记录：`nvidia-smi` GPU used、生成进程树 `PrivatePageCount` 峰值、可用物理内存最低值；保存 CSV 到 `<EVID>/samples/`。
- [ ] 汇总每项：host GPU 峰值（并注明约 1.4 GiB 桌面基线）、进程树 private commit 峰值、可用物理内存最低值、加载/生成墙钟、`app.log` 的 `generation.start available_gib` 与阶段耗时。
- [ ] 对照标准档预算：host GPU 峰值减基线 ≤10.5 GiB（代理口径）、private commit ≤24 GiB、可用物理内存最低 ≥2 GiB、加载 ≤600 s、生成 ≤900 s、无共享内存回退（说明观测限制）。
- [ ] worker CUDA reserved 复用同执行面 Gate 0 证据并标注"复用"；执行面变化时该项记 `not_run`。

### G12 重启恢复

- [ ] 重启应用后：8 项作品均可打开、预览、导出；未加载模型时仍可查看旧作品与制作记录。

## 4. 真实作品与会话管理

全部使用本轮真实生成作品；不得使用构造库或旧文件副本冒充。

### W01 查询、筛选与空态

- [ ] 作品总数、分页、日期/模型/尺寸/会话/仅收藏组合筛选正确；区分无作品/无收藏/组合无结果/读取失败。
- [ ] 迟到查询不覆盖当前筛选或删除状态。

### W02 收藏持久化

- [ ] 收藏/取消收藏不打开预览、不误触删除；“仅收藏”按全库查询；重启后保持。

### W03 预览与制作记录

- [ ] 预览显示真实尺寸与制作记录（提示词、模型、seed、配方、环境）；记录与 G 项证据一致。

### W04 删除普通真实作品

- [ ] 取消确认无副作用；确认后原图/缓存文件移除、作品/收藏/计数更新，外部导出副本字节不变。
- [ ] 重启后不复活；原 Job/Attempt/Manifest 历史仍可查。

### W05 删除 BiRefNet 双输出

- [ ] 对 G07 确认 primary 与 mask 作为一组删除；两个 Asset ID 均不可读取、导出或继续作为源图；其他作品保持。

### W06 引用阻止与解除

- [ ] 将 G01b 设为当前草稿源图后尝试删除：被拒并给出会话与解除动作；不要求启动活动生成任务。
- [ ] 切走能力后未激活槽位仍引用时，消息包含对应能力名与解除路径。
- [ ] 按提示移除/更换源图后删除成功；引用状态与作品库一致。

### W07 会话删除与作品墓碑

- [ ] 删除本轮作品所属会话：作品保留、收藏与来源保持；来源显示原会话标题及"会话已删除"；返回会话入口不可用。
- [ ] 删除当前会话时选择最近活动的未删除后继；删除最后一个会话时同事务创建默认空会话，不产生新 Turn/Job/Asset。
- [ ] 重启后已删除会话不复活，旧保存/分页响应不能复活。

## 5. v5 → v6 真实升级（r-v5）

### D01 升级与数据保留

- [ ] 首次启动 `r-v5`：`user_version` 5→6；核对 20 assets / 23 jobs / 1 conversation / 1 draft 保留；`is_favorite`、`deletion_state` 等新列默认正确；`PRAGMA quick_check` 通过。
- [ ] 保存升级前后数据库快照（只读副本）到 `<EVID>/db/`。

### D02 旧作品查看与导出

- [ ] 抽 2 件 v5 旧作品：预览、制作记录、原生导出；导出 SHA-256 与资产一致。

### D03 升级后闭环保留

- [ ] 从种子缓存安装 Klein，完成一次真实文生图并导出（逐字节一致）；记录升级后仍可执行。

### D04 重启幂等

- [ ] 重启后不重复迁移、不复制作品、不创建额外默认会话；会话/作品数稳定。

### D05 迁移失败保护

- [ ] 构造只读或损坏数据库副本尝试启动：不重建空库、不覆盖旧数据，给出可诊断错误；无法安全构造时记 `not_run`。

### D06 旧双输出与引用数据

- [ ] 若 `r-v5` 中存在 BiRefNet 双输出或旧源图引用：验证 mask 可读、导出与引用提示正常；无法定位记 `not_run`。

## 6. 鲁棒性与故障

- O01 取消四类：Klein（CUDA）、Qwen-Turbo（Python 分阶段）、BiRefNet（CPU/ONNX）、Real-ESRGAN（Vulkan）各一次；≤15 s 落定、无半成品、无残留进程与显存未回落。
- O02 强杀恢复：Z-Image 生成中强杀宿主进程 → 重启后 Attempt 为 interrupted、无资产、无自动重试；显式重试才新建 Attempt。
- O03 看门狗：按 2026-11-08 方法注入滞留 pending 回合，60 s 内 failed 并给出阶段与重试入口；无法安全注入记 `not_run`。
- O04 激活与生成并发：生成中第二次启动激活窗口且生成不受影响（与 I04 合并记录）。
- O05 整树禁网生成：**用户决定本轮不测（waived/not_run，不视为通过，也不计入失败）**。
- O06 磁盘不足/只读路径：安全可复现时验证快速失败与不留下半成品；仅引用历史证据时明确标注"未在本轮重跑"。
- O07 启动诊断与日志（与 I05/I06 互证）。
- O08 无控制台窗口（与 I06 互证）。

## 7. 人工复核（用户决定不测）

- [ ] H01 真实微软拼音：**waived/not_run**（用户决定本轮不测，不视为通过）。
- [ ] H02 显示缩放切换：**waived/not_run**（用户决定本轮不测）。
- [ ] H03 窗口最小化/最大化/还原/拖动、亮暗主题、Tab/Escape/Enter 主要操作：用 CDP/截图自动记录为机器证据，不依赖用户操作。
- [ ] H04 视觉质量接受/拒绝：**waived/not_run**（用户决定本轮不测）；G10 机器完整性检查照常，报告不得写成"视觉质量已接受"。

## 8. 证据、结论与归档

### E01 证据归档

- [ ] `<EVID>/` 至少包含：`README.md`、`environment.json`、`results.json`（逐项状态）、`seed-cache-*.json`、`samples/*.csv`、`g*.json`（每项生成）、`db/*.json`、`exports/*.sha256`、截图、`build-quality-gates.log`、`app.log` 片段、`user-review.json`。
- [ ] 每项证据包含构建提交、安装包/EXE 哈希、数据根、时间与执行方式。

### E02 逐项状态与失败项

- [ ] `results.json` 使用 `pass`/`fail`/`blocked`/`not_run`，失败项写复现步骤、原因与 Beads id。
- [ ] 不得把未执行项写成通过；不得用其他型号/历史图片补位。

### E03 结论标签

- [ ] 明确写出："当前构建真实出图验收（机器证据）：通过/未通过"，并列出覆盖的型号与能力。
- [ ] 分别标注：执行面 diff 分类；复用的历史 Gate 0 资源口径（提交/路径）；8 GB 补充机功能回归仅作参考；未执行项清单。
- [ ] 明确列出 waived 项：整树禁网生成、真实 IME、显示缩放切换、视觉质量接受/拒绝（用户 2026-11-12 决定）。
- [ ] 不得写成"七模型八能力历史认证重跑"以外的模糊表述。

### E04 归档与收口

- [ ] 用户确认结论后归档 change、合并 `windows-desktop` delta；更新 `verification/` 索引与 `doc/15-windows-8gb-tier.md`（如适用）；关闭/更新 Beads 跟踪 issue。
- [ ] 缺陷项按影响面另立 OpenSpec change；本 change 不夹带产品修复。

## 判定规则

- 任何未执行项保持 `not_run`；被权限/环境/缺陷阻止记 `blocked` 或 `fail`；不得伪造通过。
- 用户明确放弃的项记 `waived`（results 中状态可为 `not_run`，备注写 waived 与决定来源），不得记 `pass`，也不得据此声明相关能力已通过。
- 当前构建实测、复用证据、人工复核必须分开记录；其他机器/历史基线不能替代目标机当前构建证据。
- 资源数字必须标注观测方法与限制；host GPU 峰值不得直接写成 worker CUDA reserved。
- 一次能力项的首次失败保留原记录，重试成功单独记录，不覆盖首次失败。
- 真实网络路径失败不阻断主体验收，但必须如实记录 `blocked` 与原因。

## 执行顺序

P01 → P02 → P03 → P04 → P05 → P06 → P07 → I01 → I02 → I03 → I04 → I05/I06 → M01 → M02 → M03/M04/M05 → G01–G08 → G09 → G10（机器部分）→ G11 → G12 → W01–W07 → D01–D06 → O01–O08（O05 waived）→ H03 → E01–E04（H01/H02/H04 waived）。

## 不在本计划范围

- 8 GB 紧凑档、真实 16 GiB 机器低内存证据（Beads `pwp`）、新硬件/新 Windows 版本。
- 性能承诺、签名/公证、持续 GPU CI。
- 全部七个型号的真实网络批量下载（只验证 Z-Image Turbo 大模型下载，其余走种子缓存）。
- 整树禁网生成、真实微软拼音 IME、显示缩放切换、八张图视觉质量接受/拒绝（用户 2026-11-12 决定不测，记 waived）。
- 产品行为修复（缺陷另立 change）。
- 用户默认数据根的升级或清理。
