# Windows 12 GB 目标机真实出图验收（2026-11-12）

Change：`validate-windows-12gb-real-generation`  ·  Beads：`image-studio-pze`  ·  构建：`159a0e89d80c`（0.1.1）  ·  数据根：`C:/ist-rg/`

## 结论标签

- **当前构建真实出图（机器证据）：pass (machine evidence; 9/9 cases succeeded in the canonical run, resources within product-level budgets)**
- 功能与管理：pass (install, workspace, works/conversations, v5 upgrade, startup diagnostics, cancellation, forced kill)
- 复用证据：none (execution-surface change); 2026-11-08 and 8 GB supplementary evidence referenced only as context
- waived：whole-tree network denial、real Microsoft IME、display scaling switch、visual acceptance（用户 2026-11-12 决定不测，不视为通过）
- not_run：watchdog injection、migration failure protection、disk-shortage/read-only startup、worker-internal CUDA reserved

执行面审计：当前构建相对 2026-11-08 基线 `b861ae5` **属于 execution-surface change**（`low_memory.py`、`prepare.py`、`qwen.py`、`windows_process.rs`、`installation.rs`、worker wheel 等），因此历史 Gate 0 的 worker CUDA reserved 不能复用；本轮以产品级 host/进程采样为资源证据，worker 内部 CUDA reserved 记为 not_run。

## 构建与安装

| 项 | 值 |
| --- | --- |
| 提交 | `159a0e89d80ca896140d3854fcf2a3c6acecb17e`（本地文档提交，未 push；远端 main `95dd117` 带 `[skip ci]`） |
| 安装包 | `Image Studio_0.1.1_x64-setup.exe`，245,118,946 B，`d23c18c0a107850cd8fc14d305e0bd23064f3632decd219567015ddc4e0cd3b4` |
| 安装后 exe | `%LOCALAPPDATA%\Image Studio\image-studio-desktop.exe`，`aec5d0672a8b56ded736de19afa93486b0d4593fc85c2067aecdf3d921cab539` |
| 静默安装 | 第一次 `/S` 退出码 2 未改文件；立即重试退出码 0 并安装成功（观察 F8） |
| 质量门 | 前端 81 测试 + build；`cargo fmt`/`clippy`/`test --workspace` 全绿（`../build.log`） |

## 真实出图矩阵（八项能力，全部成功）

| 用例 | 型号 / 能力 | 墙钟 s | 输出 | 资源（host 口径） |
| --- | --- | ---: | --- | --- |
| G01a | Klein / 文生图 | 27 | 1024×1024，2,150,170 B，`09c54104c1d2…` | GPU 7590.0 MiB（峰值减基线 6190.0 MiB）· commit 16.68 GiB · 可用内存最低 17.1 GiB |
| G01b | Klein / 文生图 | 26 | 1024×1024，1,684,363 B，`683bd991e993…` | GPU 7607.0 MiB（峰值减基线 6207.0 MiB）· commit 16.60 GiB · 可用内存最低 15.7 GiB |
| G02 | Z-Image / 文生图 | 54 | 1024×1024，1,394,376 B，`db2fc65aee60…` | GPU 10852.0 MiB（峰值减基线 9452.0 MiB）· commit 18.75 GiB · 可用内存最低 13.1 GiB |
| G03 | Qwen-Image 2.1 / 文生图 | 86 | 1024×1024，1,354,564 B，`d8342834f9de…` | GPU 6452.0 MiB（峰值减基线 5052.0 MiB）· commit 13.10 GiB · 可用内存最低 16.5 GiB |
| G04 | Qwen-Image 2.1 Turbo / 文生图 | 41 | 1024×1024，1,504,693 B，`3b779a738338…` | GPU 7907.0 MiB（峰值减基线 6507.0 MiB）· commit 12.99 GiB · 可用内存最低 15.1 GiB |
| G05 | MOSS / 文生图 | 146 | 1024×1024，2,013,223 B，`59954dfaa674…` | GPU 6190.0 MiB（峰值减基线 4790.0 MiB）· commit 12.66 GiB · 可用内存最低 16.0 GiB |
| G06 | Klein / 单图编辑 | 39 | 1024×1024，2,148,717 B，`ce17ae2c2c3d…` | GPU 7827.0 MiB（峰值减基线 6427.0 MiB）· commit 16.66 GiB · 可用内存最低 13.6 GiB |
| G07 | BiRefNet / 去背景 | 23 | 1024×1024，1,565,690 B，`b29ce455af76…`（RGBA + 8-bit mask） | GPU 1515.0 MiB（峰值减基线 115.0 MiB）· commit 10.90 GiB · 可用内存最低 14.3 GiB |
| G08 | Real-ESRGAN / 4× 放大 | 8 | 4096×4096，16,839,218 B，`2e04d2729964…` | GPU 2274.0 MiB（峰值减基线 874.0 MiB）· commit 2.77 GiB · 可用内存最低 20.6 GiB |

预算对照（全部通过）：host GPU 峰值减基线 ≤ 10.5 GiB；进程树 private commit ≤ 24 GiB；可用物理内存最低 ≥ 2 GiB；加载 ≤ 600 s、生成 ≤ 900 s。原始 1 s 采样在 `samples/*.csv`，逐项证据在 `generations/<ID>-evidence.json`。

> 附：G01a 第一次尝试在三个应用实例并行时以进程提交峰值 24.18 GiB 触发预算失败；关闭升级实例后重试通过（16.68 GiB）。首次失败记录保留在 `generations/G01a-attempt1-budget-failure.json`。早先一轮 prompt 因 PowerShell 5.1 编码问题损坏，已整体归档到 `generations/invalid-input/`，不作为验收证据。

## 模型安装

| 型号 | 方式 | 耗时 s |
| --- | --- | ---: |
| BiRefNet | 种子缓存 | 61 |
| Klein | 种子缓存 | 89 |
| MOSS | 种子缓存 | 361 |
| Qwen-Image 2.1 | 种子缓存 | 399 |
| Qwen-Image 2.1 Turbo | 种子缓存 | 403 |
| Real-ESRGAN | 种子缓存 | 11 |
| Z-Image | 种子缓存 | 164 |
| Z-Image Turbo（r-net） | 真实网络下载（ModelScope，35.56 GB，断点续传后完成） | 1012 |

网络安装细节：第一次下载到 33.17/35.56 GB 后停滞，重启应用以“继续安装”续传完成；下载阶段 842 s，校验 + 环境准备 + 设备探针后 ready；证据 `install-network.json`、`network-install-evidence.json`。

## 作品与会话管理（全部使用本轮真实作品）

| 项 | 结果 |
| --- | --- |
| W01 查询/筛选/空态 | pass |
| W02 收藏持久化 | pass |
| W03 预览与制作记录 | pass |
| W04 删除普通真实作品 | pass |
| W05 BiRefNet 双输出删除 | pass |
| W06 引用阻止与解除 | pass |
| W07 会话删除与作品墓碑 | pass |
| C01 新建/切换/重命名 | pass |
| C02 删除非当前会话 | pass |
| C03 删除当前/最后会话 | pass |

作品数变化：27（生成完成）→ 26（W04 删除失败输入轮作品）→ 25（W05 删除 BiRefNet 双输出）→ 24（W06 解除引用后删除）→ 25（无控制台复核再生成 1 件）。W07 删除会话后作品保留并显示“会话已删除”墓碑；导出副本不受删除影响。

## v5 → v6 真实升级（r-v5）

- v5 副本升级后 `user_version` 6、`quick_check` ok，20 件旧作品 / 23 个任务 / 1 个会话 / 1 个草稿保留（`upgrade/db-after-restart.json`，共 21 件资产）。
- 旧作品导出逐字节一致：2,097,618 B，`326a97c6baf175d2…`（`upgrade/export-old-work.json`）；旧 BiRefNet 双输出可解码，mask 为 1024² 8-bit 灰度。
- 升级后从种子缓存重装 Klein（114 s）并真实出图一次（42 s）；重启幂等（21 资产 / 25 turns 前后一致）。
- D05 迁移失败保护、部分故障分支未执行（not_run）。

## 鲁棒性

| 项 | 结果 | 关键数据 |
| --- | --- | --- |
| O01 四类取消 | pass | 落定时间：Klein 6.0 s / Qwen-Turbo 4.5 s / BiRefNet 4.5 s / Real-ESRGAN 1.5 s；无残留 worker |
| O02 宿主强杀恢复 | pass | 重启后 attempt=interrupted、无新资产、不自动重试；显式“重试”产生新 attempt，随后取消 |
| O04/I06 无控制台窗口 | pass | 59 次 1 s 采样：控制台窗口 0，worker 窗口句柄 0 |
| O05 整树禁网 | waived | 用户决定不测 |
| O03 看门狗 / O06 磁盘不足 | not_run | 本轮未执行 |

## 人工复核

| 项 | 结果 | 说明 |
| --- | --- | --- |
| H01 真实微软拼音 | waived | 用户决定不测 |
| H02 显示缩放切换 | waived | 用户决定不测（当前 100%） |
| H03 窗口/主题/键盘 | pass | 1280×800、960×640 客户区精确；浅/深主题切换生效；Tab/Escape 无异常弹层 |
| H04 视觉质量接受/拒绝 | waived | 机器完整性检查通过；视觉结论未由用户给出 |

## 观测与后续

- **F5** 模型页安装完成后的自动刷新可能停住，手动“刷新”才显示“当前可执行”（Z-Image、Qwen Turbo、Qwen base、MOSS、r-v5 Klein 均出现）。
- **F6（工具问题）** Windows PowerShell 5.1 以 ANSI 读取无 BOM 的 UTF-8 脚本，中文字面量损坏；prompts 已改由 UTF-8 JSON 读取。
- **F7** 三个应用实例并行 + 网络下载时，Klein 首次生成触发 24 GiB 私有提交预算失败；关闭升级实例后重试通过，预算余量偏紧。
- **F8** 安装包第一次静默安装退出 2 且无文件变化，立即重试退出 0。
- **F9** `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 指向不存在目录未能复现启动失败；启动诊断通过超长路径与 profile 文件占用路径验证。
- **F10** `stub-supervisor.exe` 仍在安装包内（延续历史 wfy 观察项）。

## 证据索引

| 路径 | 内容 |
| --- | --- |
| `environment.json` | 机器、构建、数据根、执行面审计与观测 |
| `results.json` | 全部逐项状态（pass/not_run/waived/pending_user） |
| `generations/` | 每用例驱动结果、逐项证据、资源汇总；`invalid-input/` 为编码问题轮次 |
| `samples/` | 1 s 资源采样 CSV |
| `screenshots/` | 安装、生成、管理、升级、启动诊断截图 |
| `install/`、`install-network*` | 七型号安装与真实网络安装证据 |
| `upgrade/` | v5→v6 升级前后 DB 快照、旧作品导出、升级后生成 |
| `uimgmt/`、`cancels/`、`forcekill/`、`console-watch/` | 管理、取消、强杀、控制台窗口证据 |
| `quality/` | 输出 PNG 解码、尺寸/通道、IHDR、亮度统计 |
| `build.log`、`logs-*.log` | 构建与运行日志片段 |

## 复现命令（摘要）

```powershell
& scripts/build-windows-internal.ps1
Start-Process -FilePath 'target\release\bundle\nsis\Image Studio_0.1.1_x64-setup.exe' -ArgumentList '/S' -Wait
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9222'
& "$env:LOCALAPPDATA\Image Studio\image-studio-desktop.exe" --data-dir C:\ist-rg\r-fresh
node scripts/windows-ui/drive-install.mjs --model Klein --out install.json
node scripts/windows-ui/drive-generate.mjs --model Klein --kind text --prompt '...' --out case.json
node scripts/windows-ui/drive-works.mjs --action list
```

> 本文件与 `results.json` 均由证据文件生成/核对；手工结论以用户最终确认为准。
