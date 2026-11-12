# Windows 11 本机功能回归（2026-11-12）

Change：`validate-windows-functional-regression`（本机执行记录；该 change 的测试计划要求不下载模型、不运行推理）。
Beads：`image-studio-bcn`。

**结论：Windows 功能回归失败。** 35 项检查中 22 项通过、7 项按限制部分通过、1 项失败、5 项未运行。
失败项是 **I03 数据根所有权**：同一数据根启动第二个实例时进程以退出码 101 静默结束，
既不写 `logs/startup-failure.log`，也不弹原生错误对话框，与 `startup.rs` 文档化的
“日志 + 对话框 + 退出码 3”不一致（缺陷 F1）。

本机是 **Windows 11 / RTX 2060 SUPER 8 GB 补充机**，不是 Windows 10 / RTX 3080 Ti 目标机。
本轮**没有**执行模型下载、推理或新图片生成；生图结论只引用本机 2026-11-05/06 的 8 GB 基线
（`verification/windows-8gb-tier/`）。因此本文件不得读作“七模型八能力、GPU 资源、图片质量或性能重新认证”。

机器与构建身份、执行面分类见 [environment.json](environment.json)；逐项状态与证据索引见
[results.json](results.json)。

## 0. 构建与安装

| 项 | 值 |
|---|---|
| 提交 | `51d56f1c365aed174fd8554e81965ef1f50eb8e1`（`main`，工作区干净） |
| 应用版本 | 0.1.1 |
| 安装包 | `Image Studio_0.1.1_x64-setup.exe`，245,110,777 B，SHA-256 `43a7b081e08fb8966493c6ccb6fd584d1aea2299c49f5d36e22f6340b9fe99e2` |
| 构建方式 | `scripts/build-windows-internal.ps1 -SkipQualityGates`（NSIS，currentUser，离线 WebView2 安装器） |
| 安装后 exe | `%LOCALAPPDATA%\Image Studio\image-studio-desktop.exe`，42,512,384 B，SHA-256 `40afe5c8f393f806a1dbaaf69adef704183106821ab8394ac08c58358ed1904c` |
| 安装结果 | 静默安装退出码 0，注册表登记 `Image Studio 0.1.1` |
| 数据根 | `C:\ist-verify\r-fresh`、`r-v5`、`r-works`（均为可丢弃隔离根） |

质量门（本机）：前端 `npm test` 81 项通过、`npm run build` 通过；`cargo test --workspace --locked`
350 passed / 0 failed / 7 ignored（19 个套件，输出见 [tests-workspace.txt](tests-workspace.txt)）。
`cargo fmt`/`clippy` 未在本轮重跑。

## 1. 执行面判断

以本机 8 GB 生图基线（`af9e40b`，2026-11-08）为对照执行
`git diff af9e40b..HEAD -- tools resources/windows-parity crates/studio-core crates/worker-protocol apps/desktop/src …`：

- 改动集中在 `apps/desktop/src/adapters/library/*`（v5→v6、会话删除、收藏、受控作品删除）、
  `host.rs`（新增 `conversation_delete`/`asset_set_favorite`/`asset_delete` 命令）、
  `studio.rs`（删除与收藏视图投影）、`capabilities/main.json` 与版本号。
- `tools/*`、`resources/windows-parity/*`、`crates/studio-core` 源码、`crates/worker-protocol` 未改动。

分类：**`no execution-surface change`**。因此本轮可以引用既有 8 GB 生图基线，不重跑推理。
注意基线复用的限制：本机当前没有现存模型安装和真实作品库，本轮无法复跑“查看/导出既有真实作品”，
作品相关检查使用构造库（见第 5 节）。

## 2. 安装、启动与数据根（I01–I03）

| 项 | 结果 | 证据 |
|---|---|---|
| I01 当前用户安装 | pass | 安装器退出码 0；从安装目录启动；启动不下载模型、不准备运行时 |
| I02 安装身份与日志 | partial | 版本/哈希一致，关于页含许可；图标资源未逐项核对 |
| I03 数据根所有权 | **fail** | 同一数据根第二实例退出码 101，无 `startup-failure.log`、无对话框：[02-second-instance-same-root.txt](02-second-instance-same-root.txt)、[03-second-instance-app-log.txt](03-second-instance-app-log.txt) |
| I03 不同数据根共存 | pass | 两实例同时存活，各自 `cache/webview2`，旧共享 profile 未被触碰：[04-two-roots.txt](04-two-roots.txt) |

F1 的根因：`HostState::new`（`apps/desktop/src/host.rs:53`）安装的日志 panic hook 覆盖了
`startup::install_panic_hook`，`Failed to setup app` 只落一条 `app.panic` 日志便以 101 退出。
该行为由 `0a274e8 feat(desktop): log panics and generation-time memory` 引入。
它使 2026-11-03 验收过的 W07 启动失败诊断（日志 + 对话框 + 退出码 3）在 WebView2/profile 失败路径上失效；
超长数据目录路径仍走显式分支，诊断正常（见 S03）。

## 3. 升级与数据库（D01–D03）

- D01 partial：v5 库副本（追加 3 条会话）首次启动后 `user_version` 5→6，4 条会话与 4 条草稿
  （含 prompt 文本）保留，`is_favorite`/`deletion_state` 等新列齐备（[db-v5-upgraded.json](db-v5-upgraded.json)）。
  本机没有 v5 作品/任务/双输出数据，作品字节与任务历史保留未运行。
- D02 pass：重启后 4 会话 / 4 草稿 / `user_version` 6 / `quick_check ok`，未重复迁移、未新建默认会话
  （[db-v5-restarted.json](db-v5-restarted.json)）。
- D03 not_run：未构造不可读/不可写的升级失败，未伪造通过。

## 4. 原生工作区（U01–U06）

| 项 | 结果 | 说明 |
|---|---|---|
| U01 单层导航 | pass | 导航仅品牌/创作/灵感库/模型/作品/主题/关于/设置；无页签、功能页、任务页、开发入口（[01-create-fresh.png](01-create-fresh.png)） |
| U02 窗口尺寸 | partial | 1280×800 与 960×640 下导航、会话列表、输入框、模型状态、发送按钮可达（[05](05-window-1280x800.png)、[06](06-window-960x640.png)）；应用默认窗口本就是 1280×800，故 04 与 05 字节相同；仅 100% DPI；最小化/最大化/拖拽未逐项复核 |
| U03 主题与键盘 | partial | 深色切换即时生效并持久化（[04-theme-dark.png](04-theme-dark.png)）；Tab 遍历、Escape 关闭确认框、Enter 提交重命名通过；按钮 Enter 激活未单独确认 |
| U04 中文输入与草稿 | partial | 合成候选 + IME Enter 不发送、无错误；真实微软拼音未驱动；两会话草稿隔离并跨重启恢复 |
| U05 灵感库与关于 | pass | 541 条、搜索“樱花”得 4 条、标注“上游示例，不代表本机生成”；关于页含版本、平台与七模型许可（[11](11-inspiration.png)、[15](15-settings-about.png)） |
| U06 模型页只读投影 | pass | 7 型号、已安装 0、设备满足 6；Z-Image 显示 12 GB 门槛不满足；Klein 展开显示“紧凑档 768×768 · float16”，无自动下载（[02](02-models-fresh.png)、[03](03-model-klein-expanded.png)） |

## 5. 会话与作品管理（C01–C04、A01–A09）

会话：C01 新建/切换/重命名、C02 非当前会话删除（取消无副作用、确认后墓碑、重启不复活）、
C03 删除当前会话选后继、删除最后一个会话同事务建默认空会话，均 pass；数据库证据见
[db-after-session-deletes.json](db-after-session-deletes.json)，截图见
[07](07-delete-dialog.png)、[08](08-after-delete.png)、[09](09-delete-current-successor.png)、[10](10-delete-last-session.png)。
C04 未运行：本机没有归属这些会话的真实作品（墓碑 `deleted_at` 已在数据库证据中确认）。

作品：本机没有可用的真实作品库，且本轮禁止生成，因此按测试计划允许的“旧库副本 + 既有作品字节”
构造了 3 件作品用于文件行为验证：文件字节复制自本机 2026-11-05 产品输出
`verification/windows-8gb-tier/ui-qwen-turbo-768.png`（1,176,635 B，SHA-256 `0e56258e…`），
会话与任务行由 [make-works-fixture.py](make-works-fixture.py) 写入；**这不是本轮生成结果**。

| 项 | 结果 | 说明 |
|---|---|---|
| A01 查询/筛选/空态 | pass | 区分“尚无正式作品”和“当前条件没有作品”；分面计数正确（[12](12-works-empty.png)、[17](17-works-three-items.png)） |
| A02 收藏持久化 | pass | 收藏不打开预览，“仅收藏”得 1 件，重启后保持，取消后库内归 0（[23](23-works-favorites-filter.png)） |
| A03 预览与导出 | pass | 预览 768×768 与制作记录；原生保存导出 1,176,635 B，SHA-256 与资产一致（[18](18-work-preview.png)） |
| A04 普通作品删除 | pass | 取消无副作用；确认后文件移除、`deletion_state=deleted`、外部导出副本哈希不变、重启不复活（[19](19-work-delete-dialog.png)–[21](21-work-deleted.png)、[db-works-after-deletes.json](db-works-after-deletes.json)） |
| A07 只读文件删除 | pass | 只读属性下确认删除成功，未触碰其他文件 |
| A08 占用与重试 | pass | 非共享句柄占用时显示“删除未完成：另一个程序正在使用此文件 (os error 32)”且保留文件；释放后“重试删除”成功 |
| A05/A06/A09 | not_run | 无 BiRefNet 双输出真实作品、未建立源图引用、未制造删除中断 |

## 6. 设置、路径与诊断（S01–S04）

- S01 pass：五组设置均可打开，主题/侧栏偏好跨重启恢复（[13](13-settings-appearance.png)）。
- S02 partial：显示 data/models/runtimes/cache/assets 的实际路径、占用与可用空间，无任意路径输入或自动迁移
  （[14](14-settings-storage.png)）；未点击“定位”验证文件管理器打开。
- S03 pass：114 字符（>94 UTF-16）数据目录启动被拒绝，写 `logs/startup-failure.log`（含实测长度与建议）、
  弹“Image Studio 启动失败”、点击确定后退出码 3，未创建 data/cache。
- S04 partial：日志行格式 `<epoch_ms> LEVEL event key=value`，轮转上限 8 MiB 保留 3 份；但功能操作本身
  不写事件，本轮 `app.log` 只有 panic 行（F3）；整个验收无 python / supervisor / worker 进程，无模型或运行时目录。

## 7. 缺陷与观察

| 编号 | 严重度 | 摘要 | 位置 / Beads |
|---|---|---|---|
| F1 | high | 第二实例/profile 初始化失败时静默 101，启动失败诊断被日志 hook 覆盖 | `apps/desktop/src/host.rs:53`（由 `0a274e8` 引入）· `image-studio-z1v` |
| F2 | low | 测试专用 `stub-supervisor.exe` 随安装包发布 | `apps/desktop/Cargo.toml` `[[bin]]` · `image-studio-wfy` |
| F3 | low | 功能操作不写 `app.log`，功能回归无法从日志复核操作序列 | `apps/desktop/src/studio.rs` 日志调用点 · `image-studio-t7u` |
| F4 | low | `scripts/windows-ui/answer-dialog.ps1` 无法点击 MB_OK 消息框的“确定”（暴露为 Pane） | `scripts/windows-ui/answer-dialog.ps1` · `image-studio-384` |

## 8. 结论标签

- **Windows 功能回归：失败（I03）**；其余适用项通过或按限制部分通过。
- **8 GB 生图基线复用**：`verification/windows-8gb-tier/`（2026-11-05/06，本机 RTX 2060 SUPER 8 GB）。
- **生图认证未运行**：本轮没有下载模型、没有推理、没有新图片；不得写成七模型八能力、GPU 资源、
  图片质量、取消/强杀恢复或性能重新认证。
- 目标机（Windows 10 / RTX 3080 Ti 12 GB）结论仍以该机器的完整认证为准，本机结果不能替代。

## 9. 复现

```powershell
# 1. 构建并安装当前提交
& scripts/build-windows-internal.ps1 -SkipQualityGates
Start-Process -FilePath 'target\release\bundle\nsis\Image Studio_0.1.1_x64-setup.exe' -ArgumentList '/S' -Wait

# 2. 隔离根 + WebView2 调试端口
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9333'
& "$env:LOCALAPPDATA\Image Studio\image-studio-desktop.exe" --data-dir C:\ist-verify\r-fresh
node scripts/windows-ui/cdp.mjs text   # 逐项检查见 results.json

# 3. v5 升级与构造作品库
python verification/windows-functional-regression-20261112/make-v5-fixture.py "$env:LOCALAPPDATA\ImageStudio" C:\ist-verify\r-v5
python verification/windows-functional-regression-20261112/make-works-fixture.py C:\ist-verify\r-v5 C:\ist-verify\r-works

# 4. 数据库快照（只读副本）
python verification/windows-functional-regression-20261112/inspect-db.py C:\ist-verify\r-works out.json
```


