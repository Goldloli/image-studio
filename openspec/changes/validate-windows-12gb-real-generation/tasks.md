# Tasks

状态：**用户已确认执行（2026-11-12）。** 确认时调整：真实下载并安装大型模型；不执行整树禁网验收；真实 IME、显示缩放切换与视觉质量人工复核不测（waived/not_run）。

本 change 不修改产品行为；任务按依赖顺序排列，每项完成后以其证据文件作为验收。执行细节见 [test-plan.md](test-plan.md)。

## 1. 批准与准备

- [x] 1.1 用户于 2026-11-12 确认执行；确认调整：真实下载大模型、不测整树禁网、不测人工项。冻结执行提交。
- [ ] 1.2 在 Beads 建立跟踪 issue（承接目标机复验与失败项），关联本 change；本机库中没有 `b65`，按新 issue 记录。
- [ ] 1.3 记录冻结提交、`git status`、版本号与 CI quality/build 状态（只读）→ `environment.json`。

## 2. 环境与数据安全

- [ ] 2.1 记录机器身份、显示/DPI、WebView2、磁盘、页面文件与内存现状（P02）→ `environment.json`。
- [ ] 2.2 完成基线对照与执行面 diff 分类（P03）→ `environment.json.execution_surface_review`。
- [ ] 2.3 创建 `C:\ist-rg\`，复制真实 v5 子集到 `r-v5`，记录源根只读与复制哈希（P04）→ `data-safety.json`。
- [ ] 2.4 运行两份种子缓存脚本并校验 `missing=0` / `hash_mismatch=0`（P05）→ `seed-cache-parity.json`、`seed-cache-zimage.json`。
- [ ] 2.5 确认 CDP 端口 9222 可用、UI 驱动脚本可用、编码约定（P06）。

## 3. 构建与安装

- [ ] 3.1 运行 `scripts/build-windows-internal.ps1` 质量门并保存日志（I01）→ `build-quality-gates.log`、`build-desktop.log`。
- [ ] 3.2 记录安装包与安装后 exe 的 SHA-256，覆盖安装当前构建（I02）。
- [ ] 3.3 全新根首次启动与模型页投影（I03）→ 截图 + `environment.json`。
- [ ] 3.4 单实例激活与启动失败诊断（I04、I05）→ 文本/截图证据。
- [ ] 3.5 日志结构与无控制台窗口检查（I06）。

## 4. 模型安装

- [ ] 4.1 种子缓存安装七型号并记录耗时/字节/阶段（M01）→ `installs.json`。
- [ ] 4.2 空缓存根真实网络下载并安装 Z-Image Turbo 大模型（约 30.59 GiB + 运行时 2.53 GiB；可选加装 Real-ESRGAN）（M02）→ `install-network.json`；失败记 `blocked`/`fail`。
- [ ] 4.3 量化 prepare 与内存预检记录（M03）→ 安装日志片段。
- [ ] 4.4 重启后型号状态与档位投影（M04、M05）→ 截图 + `models-state.json`。

## 5. 真实出图矩阵

- [ ] 5.1 G01a/G01b Klein 文生图、G02 Z-Image、G03 Qwen base、G04 Qwen Turbo、G05 MOSS → `g01–g05.json` + 截图。
- [ ] 5.2 G06 Klein 编辑、G07 BiRefNet、G08 Real-ESRGAN → `g06–g08.json` + 截图。
- [ ] 5.3 G09 原生导出逐字节一致（中文/空格路径）→ `exports/` + 哈希记录。
- [ ] 5.4 G10 机器完整性检查与用户视觉复核 → `quality-check.json`、`user-review.json`。
- [ ] 5.5 G11 资源采样与汇总（1 s CSV）→ `samples/` + `resources.json`。
- [ ] 5.6 G12 重启后作品可开、可导出。

## 6. 真实作品与会话管理

- [ ] 6.1 W01–W03 查询/筛选/收藏/预览，全部使用本轮真实作品。
- [ ] 6.2 W04 删除普通真实作品（文件移除、历史保留、重启不复活）。
- [ ] 6.3 W05 删除 BiRefNet 双输出（两个 Asset ID 均不可用）。
- [ ] 6.4 W06 引用阻止与解除（当前草稿引用、未激活槽位引用）。
- [ ] 6.5 W07 会话删除、后继选择与作品墓碑。

## 7. 真实 v5 升级

- [ ] 7.1 D01 升级与数据保留（20 assets / 23 jobs / 1 conversation）→ 数据库快照。
- [ ] 7.2 D02 旧作品预览与导出逐字节一致。
- [ ] 7.3 D03 升级后重装 Klein 并真实出图一次。
- [ ] 7.4 D04 重启幂等；D05/D06 无法安全执行时记 `not_run`。

## 8. 鲁棒性

- [ ] 8.1 O01 四类取消 ≤15 s 落定、无残留。
- [ ] 8.2 O02 强杀恢复：interrupted、无资产、显式重试才新建 Attempt。
- [ ] 8.3 O03 看门狗（可安全注入时）。
- [ ] 8.4 O05 整树禁网生成：用户决定不测（waived/not_run，不视为通过）。
- [ ] 8.5 O06–O08 只读/磁盘/日志/无控制台检查。

## 9. 人工复核与结论

- [ ] 9.1 H01 真实 IME / H02 显示缩放 / H04 视觉质量：用户决定不测（waived/not_run）；H03 窗口/主题用 CDP 自动记录。
- [ ] 9.2 机器完整性检查随 5.4（G10）执行，不以人工结论替代。
- [ ] 9.3 逐项 `pass`/`fail`/`blocked`/`not_run` 汇总（E01–E02）→ `results.json`。
- [ ] 9.4 结论标签与报告（E03）：当前构建真实出图验收、执行面分类、复用项、未执行项分别写明。
- [ ] 9.5 用户确认后归档 change、合并 `windows-desktop` delta、更新 `verification/` 索引与 `doc/15-windows-8gb-tier.md`（如适用）、收口 Beads（E04）。
- [ ] 9.6 发现的缺陷按影响面另立 OpenSpec change；本 change 不夹带产品修复。
