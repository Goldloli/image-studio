# Tasks

状态：**2026-11-12 已执行。** 验收在 Windows 11 / RTX 2060 SUPER 8 GB 补充机完成；目标机
（Windows 10 / RTX 3080 Ti 12 GB）的复验移交 Beads `image-studio-b65`。执行与证据见
`verification/windows-functional-regression-20261112/`（逐项状态 `results.json`、机器/构建
`environment.json`）。

本 change 只建立验收策略、测试计划与证据口径；不包含产品功能实现。

## 1. 文档确认

- [x] 1.1 用户于 2026-11-12 指示执行；proposal、design、spec delta、test plan 与 tasks 先于执行提交。
- [x] 1.2 全程未下载模型、未启动推理、未生成新图片；模型页与安装入口保持禁用。
- [x] 1.3 执行面 diff 分类为 `no execution-surface change`（对照 `af9e40b`），复用本机
  2026-11-05/06 的 8 GB 生图基线（`verification/windows-8gb-tier/`）。

## 2. 当前构建与前置证据

- [x] 2.1 冻结提交 `51d56f1`，记录本机构建 NSIS `0.1.1`（245,110,777 B / `43a7b081…`）与安装后
  exe 哈希。**偏差**：未使用 CI artifact，也未单独复核该提交的 quality/build workflow 状态。
- [x] 2.2 记录 Windows 11 22631 / i9-9900K / 31.9 GiB / RTX 2060 SUPER 8 GiB（sm75）/ WebView2
  157.0.4341.46 / 1920×1080 @100%（`environment.json`）。
- [x] 2.3 执行面 diff 审查与分类写入 `environment.json` 的 `execution_surface_review`。
- [x] 2.4 使用 `C:\ist-verify\r-fresh`、`r-v5`（真实 v5 库副本 + 追加会话）、`r-works`（构造作品库）
  三个隔离根；未操作用户默认数据根。

## 3. 安装、升级与原生界面

- [x] 3.1 I01–I03 已执行：I01/I02 pass/partial；**I03 首轮 fail**（同根第二实例静默退出 101），
  由 `9600557` 修复后在安装包上复验 pass；`fail` 与修复记录保留在 `results.json` 与
  `verification/windows-functional-regression-20261112/`。
- [x] 3.2 D01–D03 已执行：D01 partial（本机无 v5 作品/任务数据）、D02 pass、D03 not_run。
- [x] 3.3 U01–U06 已执行：U01/U05/U06 pass；U02/U03/U04 partial（仅 100% DPI、按钮 Enter 激活未
  单独确认、真实微软拼音未驱动）。

## 4. 会话、作品与 Windows 文件行为

- [x] 4.1 C01–C03 pass；C04 not_run（本机没有归属这些会话的真实作品）。
- [x] 4.2 A01–A04 pass（A01–A04 使用构造作品库，文件字节取自本机 11-05 产品输出）；A05/A06 not_run。
- [x] 4.3 A07/A08 pass（只读文件、文件占用与重试）；A09 not_run（未制造删除中断）。
- [x] 4.4 S01/S03 pass；S02/S04 partial（未点击“定位”、功能操作本身不写日志，见 `image-studio-t7u`）。

## 5. 证据、结论与归档

- [x] 5.1 逐项状态、23 张截图、4 份数据库快照、启动失败文本与 `tests-workspace.txt` 已归档。
- [x] 5.2 为失败与未覆盖项建立 Beads：`z1v`（已修复关闭）、`wfy`、`t7u`、`384`、`b65`（目标机复验）。
- [x] 5.3 结论分别记录当前构建身份、8 GB 生图基线复用与“生图认证未运行”。
- [x] 5.4 用户确认后归档本 change、合并 `windows-desktop` delta，并关闭 `image-studio-bcn`。
- [x] 5.5 未触发完整生图认证（分类为 `no execution-surface change`）；目标机按 `image-studio-b65`
  另行复验，不据此声称生图通过。
