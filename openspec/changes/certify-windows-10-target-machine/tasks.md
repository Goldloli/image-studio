# Tasks: certify-windows-10-target-machine

> 状态（2026-11-08）：标准档回归已在**全新数据根** `C:\ist-accept`（模拟新用户）上执行完毕；证据在 `verification/windows-12gb-standard-tier/`。Qwen/MOSS 的安装期 prepare 预检在本机默认页面文件状态下 blocked（新用户同样命中），已记录实测值与选项分析。剩余 O/C/D/B 收口项未执行。本 change 不修改产品行为。

## 0. 批准与准备

- [x] 0.1 取得所有者实施确认：按所有者要求改用**全新数据根、当新用户**验收（不删除/不依赖 `C:\ist-a`）；Z-Image 执行；Qwen/MOSS 采用"先记录 blocked、再评估环境"（A 需管理员+重启，B 实验见 1.5）。
- [x] 0.2 记录本机环境 JSON（OS/CPU/RAM/GPU/驱动/显示缩放/WebView2/页面文件 commit 上限/可用内存/磁盘/网络/工具链）→ [S0-environment-build.json](../../../verification/windows-12gb-standard-tier/S0-environment-build.json)。
- [x] 0.3 全新根方案下无需改动旧根：`C:\ist-a`（2026-11-04 状态）原样保留、未作为本轮证据来源。
- [x] 0.4 构建 HEAD 并保存日志、commit、exe SHA-256 → [S0-environment-build.json](../../../verification/windows-12gb-standard-tier/S0-environment-build.json)、`build-quality-gates.log`、`build-desktop.log`。
- [x] 0.5 本地质量门禁：cargo fmt/clippy/workspace 测试、前端 test/check/build 全绿；日志见 `build-quality-gates.log`。

## 1. 12 GB 标准档回归（P0，新用户视角）

- [x] 1.1 创建全新隔离根 `C:\ist-accept`，构建当前 main 并按新用户方式启动（`--data-dir C:\ist-accept`，CDP 9333）。
- [x] 1.2 种子缓存：parity seed 132 artifacts / 123 unique / 0 missing / 0 mismatch / 112.5 GB 硬链接；Z-Image Gate 0 seed 25 links；总缓存 135.4 GB，**零下载**（[seed-parity-cache.json](../../../verification/windows-12gb-standard-tier/seed-parity-cache.json)）。
- [x] 1.3 新用户初始状态：7 型号、0 已安装、设备满足 7；Klein 详情显示**标准档 1024×1024 · bfloat16**；设备探针（安装后）RTX 3080 Ti / 12,288 MiB / compute 8.6（[S1-fresh-root-initial.json](../../../verification/windows-12gb-standard-tier/S1-fresh-root-initial.json)）。
- [x] 1.4 种子缓存首次安装：Klein ≈110 s（18.7 GB）、BiRefNet ≈32 s、Real-ESRGAN ≈12 s、Z-Image ≈172 s（35.56 GB）；无网络下载（[S2-seeded-install-timings.json](../../../verification/windows-12gb-standard-tier/S2-seeded-install-timings.json)）。
- [x] 1.5 （历史记录，已被低内存 prepare 取代）Qwen-Image 2.1 Turbo 安装曾被 prepare 内存预检拒绝；2026-11-08 已实现低内存量化并在模拟 16 GiB 条件下安装通过（S3b），见 [stream-prepare-low-memory](../stream-prepare-low-memory/tasks.md)。
- [x] 1.6 标准档逐能力生成：Klein 文生图 50.1 s / 1024²、Z-Image 文生图 55.1 s / 1024²（9 步）、BiRefNet 25.6 s（RGBA+mask 1024²）、Real-ESRGAN 8.6 s（1024²→4096²）、Klein 单图编辑 46.6 s / 1024²；输出 PNG 字节/尺寸/SHA-256 均在 S4–S8 JSON。
- [x] 1.7 Qwen 标准档生成：Qwen-Image 2.1 Turbo 1024² 8 步已通过（40.7 s，PNG 2,010,413 B，SHA-256 `92a9d4f8…`；依赖 `fix-qwen-standard-text-cleanup`）；Qwen-Image 2.1 与 MOSS 的产品级安装/生成本轮未跑（MOSS 低内存 prepare 组件已逐字节验证）。
- [x] 1.8 导出 PNG 与资产逐字节一致（SHA-256 `ad9fdf9c…`）；重启后 4/7 型号仍就绪、5 轮对话与作品全部保留（[S10-export-restart.json](../../../verification/windows-12gb-standard-tier/S10-export-restart.json)）。
- [x] 1.9 汇总标准档资源口径并写 `verification/windows-12gb-standard-tier/` README + acceptance.json（GPU 为 host 采样含桌面基线，方法已在证据中标注）。
- [ ] 1.10 （可选）旧根升级路径对照：本轮未执行；`C:\ist-a` 保持原样，未删除。
- [ ] 1.11 （可选）Real-ESRGAN 真实联机下载路径：本轮仍用种子缓存，未重跑下载/重试（O6 未执行）。

## 2. Windows 桌面遗留项（历史手工项已由所有者接受，不重测；仅保留廉价可选检查）

> `[x]` 表示"已由所有者决定接受/不执行"的登记项，不是本轮实测结果。

- [x] 2.1 100%/150%/200% DPI：所有者 2026-11-03 已接受 W05 的 100%/200%（无本机证据），150% 沿用旧证据；本轮不重测（W1）。
- [ ] 2.2 （可选）960×640 与 1280×800 布局截图与可操作性检查（W2，默认跳过）。
- [x] 2.3 真实 Microsoft 中文 IME：所有者 2026-11-03 已接受 W06（仅有单测/合成组合保护）；本轮不重测（W3）。
- [ ] 2.4 （可选）原生导出对话框：`save-ui-dialog.ps1` 保存并与资产逐字节比对；覆盖确认与旧文件保护分支（W4，默认跳过）。
- [ ] 2.5 （可选）多显示器/睡眠唤醒：切换显示器/睡眠唤醒后截图与一次冒烟发送（W5，默认跳过）。
- [ ] 2.6 （可选）暗/亮主题截图与所有者目测结论（W6，默认跳过）。
- [x] 2.7 7 型号样图质量人工复核：parity 轮归档时所有者已接受 not-run 行（4.4 `pending_user`）；本轮不重做（W7）。

## 3. 可观测性与故障分支（P1）

- [x] 3.1 app.log 结构化日志：`C:\ist-accept\logs\app.log` 含 `install.verify.*`、`turn.preflight.done ms=…`（28/69/2319 ms）、`generation.start available_gib=…`；轮转上限引用代码与单测（[O1-app-log.json](../../../verification/windows-12gb-standard-tier/O1-app-log.json)）。
- [ ] 3.2 看门狗 60 s：未执行。
- [ ] 3.3 取消覆盖（BiRefNet/Real-ESRGAN/Python 型号）：未执行。
- [ ] 3.4 预检期取消（bd ipw）：未执行。
- [ ] 3.5 生成/安装期间控制台窗口轮询：未执行。
- [ ] 3.6 安装下载重试（代理故障注入）：未执行。
- [ ] 3.7 非 ASCII/长路径：未执行。
- [ ] 3.8 磁盘不足快速回归：未执行。
- [ ] 3.9 强制回收测试引用：未执行。
- [ ] 3.10 坏校验/缺失文件拒绝（I2-2）：未执行。

## 4. CI 与发布记录（P1）

- [ ] 4.1 记录 HEAD 的 quality run #35 到 `verification/ci-release/`：未写入（API 已确认 run 35 success）。
- [x] 4.2 登记 v0.1.0 之后 76 个提交未发布、tag/发布需所有者授权（C2，见 acceptance.json）。
- [ ] 4.3 （可选）v0.1.0 发布包本机安装：未执行。

## 5. 文档、规格与归档（P1）

- [ ] 5.1–5.3 更新 `windows-hardening` test-plan 矩阵 / `windows-8gb-tier` known roughness / Real-ESRGAN asset-manifest：未执行。
- [ ] 5.4 同步 `windows-hardening-and-coverage` spec delta 并归档：未执行。
- [ ] 5.5 `doc/15` 增补 12 GB 标准档表：未执行（本文件与 verification README 已含实测值，可作输入）。
- [x] 5.6a 写 `verification/windows-12gb-standard-tier/README.md` + `acceptance.json`（含 pass/blocked/accepted/not_run 全量行）。
- [ ] 5.6b 归档本 change：待剩余收口项完成后执行。
- [ ] 5.7 清理 `verification/` 过时 not_run 行：未执行。

## 6. bd 收口（P2）

- [ ] 6.1 `06k`（uses 已全部 SHA）与 `q2y`（pipeline 已归档、v0.1.0 已发布）带证据关闭：未执行。
- [ ] 6.2 `ipw`：未执行 O4，保留。
- [ ] 6.3 `ba1` 保留；`b65` 本地 DB 不存在，`bd dolt pull` 需授权：已记录，未同步。
