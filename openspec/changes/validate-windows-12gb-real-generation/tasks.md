# Tasks

状态：**2026-11-12 已执行（机器证据部分）。** 结果：真实出图 9/9 通过（八项能力；一次环境干扰失败已保留并重试通过），安装/管理/v5 升级/取消/强杀/启动诊断通过；整树禁网、真实 IME、显示缩放切换、视觉接受/拒绝按用户决定 waived；看门狗、迁移失败保护、磁盘不足与 worker 内部 CUDA reserved 未执行（not_run）。证据：`verification/windows-12gb-real-generation-20261112/results.json` 与 `README.md`。归档 change 待用户确认（E04）。

本 change 不修改产品行为；任务按依赖顺序排列，每项完成后以其证据文件作为验收。执行细节见 [test-plan.md](test-plan.md)。

## 1. 批准与准备

- [x] 1.1 用户于 2026-11-12 确认执行；确认调整：真实下载大模型、不测整树禁网、不测人工项。冻结执行提交 `159a0e8`。
- [x] 1.2 在 Beads 建立跟踪 issue `image-studio-pze`；新观察项另建 `image-studio-10a`（模型页刷新）与 `image-studio-5vb`（Klein 提交预算边界）。
- [x] 1.3 记录冻结提交、版本 0.1.1、远端 main `95dd117` 带 `[skip ci]`、本地质量门结果 → `environment.json`。

## 2. 环境与数据安全

- [x] 2.1 记录机器身份、显示/DPI、WebView2、磁盘、页面文件与内存现状 → `environment.json`。
- [x] 2.2 执行面 diff 审计：相对 `b861ae5` 为 **execution-surface change**（low_memory/prepare/qwen/window process/installation/worker wheel），Gate 0 资源口径不可复用 → `environment.json.execution_surface_review`。
- [x] 2.3 创建 `C:\ist-rg\`，复制真实 v5 子集到 `r-v5`（20 作品/23 任务/1 会话），源根只读 → `data-safety.json`。
- [x] 2.4 种子缓存：parity 132 artifacts / 123 unique / 0 missing / 0 mismatch；Z-Image 25 links / 34 hits；`r-fresh` 零下载 → `seed-cache-parity.json`、`seed-cache-zimage.log`。
- [x] 2.5 CDP 9222、UI 驱动脚本与 UTF-8 JSON case 文件约定（修复 PowerShell 5.1 ANSI 读取问题，见 F6）。

## 3. 构建与安装

- [x] 3.1 质量门与 NSIS 构建通过 → `build.log`、`../windows-installer/internal-package.json`。
- [x] 3.2 安装包 245,118,946 B `d23c18c0…`；安装后 exe `aec5d067…`；第一次静默安装退出 2、重试退出 0（F8）。
- [x] 3.3 全新根首次启动：7 型号 / 0 安装 / 设备满足 7 / 无下载；截图 `screenshots/02-models-fresh.png`。
- [x] 3.4 单实例：同根第二实例 1.1 s 内退出 0，首实例存活，无启动失败日志。启动诊断：超长路径与 profile 文件占用路径均写 `startup-failure.log`、弹“Image Studio 启动失败”、退出码 3（坏 WebView2 环境变量未复现，F9）。
- [x] 3.5 结构化 `app.log`；真实生成期间 59 次采样无控制台窗口、worker 窗口句柄全为 0 → `console-watch/console-watch.json`。

## 4. 模型安装

- [x] 4.1 种子缓存安装七型号：BiRefNet 61 s、Klein 89 s、MOSS 361 s、Qwen-Image 2.1 399 s、Qwen Turbo 403 s、Real-ESRGAN 11 s、Z-Image 164 s；全部 ready → `install/install-summary.json`。
- [x] 4.2 空缓存根真实网络安装 Z-Image：35.56 GB，首次下载 33.17 GB 后停滞，重启续传，第二轮 1012 s 完成；ready 记录 21:57:27 → `install-network.json`、`network-install-evidence.json`。
- [x] 4.3 低内存 prepare 与内存预检记录 → `logs-f5-moss-prepare.log`（MOSS prepare 峰值 1.955 GiB、可用内存最低 16.8 GiB）。
- [x] 4.4 重启后七型号均“当前可执行”；Klein 详情显示“当前执行：标准档 1024×1024 · bfloat16” → `screenshots/M05-klein-standard-tier.png`。

## 5. 真实出图矩阵

- [x] 5.1 G01a/G01b Klein 文生图、G02 Z-Image、G03 Qwen base、G04 Qwen Turbo、G05 MOSS → `generations/G01a–G05.json` 与逐项证据；首次全部成功（G01a 早期环境干扰失败记录另存）。
- [x] 5.2 G06 Klein 编辑、G07 BiRefNet、G08 Real-ESRGAN → `generations/G06–G08.json`，G07 产出 RGBA + 8-bit mask。
- [x] 5.3 G09 原生导出逐字节一致：G03 1,354,564 B、旧 v5 作品 2,097,618 B。
- [x] 5.4 G10 机器完整性检查通过（解码、尺寸/通道、mask 8-bit、4× 4096²、非全黑）；视觉接受/拒绝 waived。
- [x] 5.5 G11 资源采样（1 s CSV）：host GPU 峰值减基线、private commit 峰值、可用物理内存最低值全部在预算内；worker CUDA reserved not_run（F7 记录首次失败）。
- [x] 5.6 G12 重启后作品可开、可导出、收藏保留、墓碑保留。

## 6. 真实作品与会话管理

- [x] 6.1 W01–W03：27 件真实作品、筛选/收藏/预览通过 → `uimgmt/`。
- [x] 6.2 W04 删除普通真实作品（27→26，文件移除，重启不复活）。
- [x] 6.3 W05 BiRefNet 双输出删除（primary + mask 同组，26→25）。
- [x] 6.4 W06 引用阻止与解除：删除被拒并给出会话与能力级解除路径；清理当前与陈旧能力槽位后删除成功。
- [x] 6.5 W07 会话删除：24 件作品保留并显示“会话已删除”墓碑，后继默认会话创建，重启保持。
- [x] C01–C03：新建/切换/重命名、删除非当前、删除当前与最后一个会话均通过。

## 7. 真实 v5 升级

- [x] 7.1 D01：`user_version` 5→6，20 旧作品/23 任务/1 会话/1 草稿保留，`quick_check` ok → `upgrade/db-*-restart.json`。
- [x] 7.2 D02：旧作品导出逐字节一致（`326a97c6…`）。
- [x] 7.3 D03：升级后重装 Klein（114 s）并真实出图一次（42 s）。
- [x] 7.4 D04：重启幂等（21 资产 / 25 turns 前后一致）；D05 迁移失败保护 not_run；D06 旧 mask 8-bit 可解码通过。

## 8. 鲁棒性

- [x] 8.1 O01 四类取消：Klein 6.0 s、Qwen-Turbo 4.5 s、BiRefNet 4.5 s、Real-ESRGAN 1.5 s 落定，无残留进程。
- [x] 8.2 O02 强杀恢复：宿主中断后 attempt=interrupted、无资产、不自动重试；显式重试创建新 attempt，随后取消。
- [ ] 8.3 O03 看门狗：本轮未执行（not_run）。
- [x] 8.4 O05 整树禁网生成：用户决定不测（waived/not_run，不视为通过）。
- [ ] 8.5 O06–O08：O07/O08 由 I05/I06 覆盖通过；O06 磁盘不足未重跑（not_run）。

## 9. 人工复核与结论

- [x] 9.1 H01 真实 IME / H02 显示缩放 / H04 视觉质量：用户决定不测（waived）；H03 窗口 1280×800/960×640、主题、Tab/Escape 已记录。
- [x] 9.2 机器完整性检查随 G10 执行。
- [x] 9.3 逐项状态汇总 → `results.json`（65 pass / 3 not_run / 4 waived / 1 pending_user）。
- [x] 9.4 结论标签（E03）：当前构建真实出图（机器证据）通过；执行面为 execution-surface change，未复用历史 Gate 0；waived 与 not_run 分列。
- [ ] 9.5 用户确认后归档 change、合并 `windows-desktop` delta、更新 `verification/` 索引与文档（E04 待确认）。
- [x] 9.6 未夹带产品修复；观察项登记 `image-studio-10a`、`image-studio-5vb`（其余 F6/F8/F9/F10 记录在环境证据）。
