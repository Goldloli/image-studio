# Test plan: certify-windows-10-target-machine

环境（本机，2026-11-08 实测）：Windows 10 Pro 22H2 19045 / i7-13700K / **RTX 3080 Ti 12 GB（sm86，驱动 596.49）** / 31.7 GiB RAM / 3840×2160 @150% / WebView2 157.0.4341.42。
数据根：**`C:\ist-accept`**（全新，模拟新用户首次安装；旧根 `C:\ist-a` 保留未动，仅作可选升级对照）。
构建：`scripts/build-windows-internal.ps1 -SkipBundle` + `cargo build --release -p image-studio-desktop`（commit/exe SHA 记录到每条证据）。
证据目录：`verification/windows-12gb-standard-tier/`。

## 覆盖矩阵

| ID | 能力/项 | 当前状态 | 判定方式 | 证据 |
|---|---|---|---|---|
| S0 | 环境、构建、质量门禁、种子缓存 | **pass** | 质量门禁全绿；132 artifacts / 0 missing / 0 mismatch；零下载 | `S0-environment-build.json`, `build-quality-gates.log`, `seed-parity-cache.json` |
| S1 | 新用户初始状态与档位投影 | **pass** | 7 型号 / 0 已安装 / 设备满足 7；Klein 显示 standard 1024²/bfloat16 | `S1-fresh-root-initial.json` |
| S2 | 种子缓存安装（Klein/BiRefNet/Real-ESRGAN/Z-Image） | **pass** | 安装阶段完整、无下载、记录耗时 | `S2-seeded-install-timings.json` |
| S3 | Qwen/MOSS prepare 预检 | **blocked** | 量化前失败、错误可行动、无 prepared/ready；数值入证据 | `S3-qwen-prepare-preflight.json`, `07-qwen-preflight.png` |
| S4 | 标准档生成：Klein 文生图 | **pass** | 1024²、50.1 s、PNG/SHA-256、host GPU 峰值 | `S4-klein-t2i.json` |
| S5 | 标准档生成：Z-Image 文生图（≥12 GiB、无 8 GB 紧凑档） | **pass** | 1024²、55.1 s（9 步）、PNG/SHA-256、host GPU 峰值 | `S5-zimage-t2i.json` |
| S6 | 标准档生成：BiRefNet 去背景 | **pass** | RGBA + mask 双输出 1024² | `S6-birefnet.json` |
| S7 | 标准档生成：Real-ESRGAN 4× | **pass** | 1024² → 4096²（精确 4×） | `S7-realesrgan.json` |
| S8 | 标准档生成：Klein 单图编辑 | **pass** | 源图导入 → 编辑出图、场景保留 | `S8-klein-edit.json` |
| S9 | Qwen Turbo / Qwen base / MOSS 标准档生成 | **not_run（blocked by S3）** | 待所有者决定是否调大虚拟内存（A）后补测 | 引用 S3 |
| S10 | 导出逐字节 + 重启恢复 | **pass** | 导出 == 资产（SHA-256）；重启后 4/7 就绪、5 轮对话保留 | `S10-export-restart.json` |
| S11 | 空缓存真实联机安装（历史 I2-1） | not_run（历史 blocked） | 若执行 Z-Image 联机安装仅作部分旁证 | 引用 `verification/windows-parity-app/not-run.json` |
| S12 | 旧根升级路径对照（可选） | not_run | `C:\ist-a` 未动，本轮用全新根 | — |
| W1 | 100%/150%/200% DPI | user-accepted（2026-11-03，不重测） | 100%/200% 无本机证据但已由所有者接受；150% 旧证据保留 | 引用 `windows-final-acceptance.json` |
| W2 | 960×640 / 1280×800 | optional（默认跳过） | 此前已有机器证据；如执行则布局与可操作性 | screenshots + JSON |
| W3 | 真实中文 IME | user-accepted（2026-11-03，不重测） | 所有者已接受 W06（仅单测/合成保护证据） | 引用 `windows-final-acceptance.json` |
| W4 | 原生导出对话框 | optional（默认跳过） | 本轮导出已用保存对话框自动化验证字节一致 | `export-klein-t2i.png` |
| W5 | 多显示器/睡眠唤醒 | optional（默认跳过） | 如执行则归位与冒烟发送正常 | JSON |
| W6 | 暗/亮主题人工审阅 | optional（默认跳过） | 如执行则所有者目测结论 | screenshots + 记录 |
| W7 | 7 型号样图质量人工复核 | user-accepted（parity 归档，不重做） | 归档时所有者已接受 4.4 `pending_user` | 引用 `windows-parity-app/not-run.json` |
| O1 | app.log 结构化与阶段耗时 | **pass** | 日志存在、字段齐全、阶段耗时可定位 | `O1-app-log.json` |
| O2 | 看门狗 60 s 卡单 | not_run | 60 s 内 failed + 阶段/原因；释放锁后重试成功 | — |
| O3 | 取消（BiRefNet/Real-ESRGAN/Python 型号） | not_run | ≤15 s 落定、无残留进程 | — |
| O4 | 预检期取消（bd ipw） | not_run | 落定时间 + 与旧 ~90 s 对比 | — |
| O5 | 生成/安装不弹控制台 | not_run | 期间无新 conhost/OpenConsole/WindowsTerminal | — |
| O6 | 安装下载重试（代理故障） | not_run | 5 次重试后明确错误；去代理后重装成功 | — |
| O7 | 非 ASCII/长路径 | not_run | 中文+空格数据根安装+放大正常 | — |
| O8 | 磁盘不足快速回归 | not_run | 约 4 s 报错、无缓存/安装记录 | — |
| O9 | 强制回收测试引用 | not_run | `forced_reap` 本地测试通过 | — |
| O10 | 坏校验/缺失文件拒绝（I2-2） | not_run | 明确错误、可重试、无半成品 | — |
| C1 | HEAD CI run #35 记录 | not_run（API 已确认 success） | Windows + macOS 均 success | 待写入 `verification/ci-release/` |
| C2 | 未发布提交/发布决策登记 | **pass** | 记录 76 commits、owner-gated | `acceptance.json` |
| C3 | v0.1.0 发布包本机安装（可选） | not_run | 安装/启动/卸载成功，标签为旧代码 | — |
| D1–D7 | 文档/规格/归档收口 | not_run | 见 tasks.md 第 5 节 | — |
| B1–B3 | bd 收口 | not_run | 见 tasks.md 第 6 节 | — |

## 判定规则

- 任何未执行项保持 `not_run`；被内存预检/外部条件阻止记 `blocked`；不得写成通过。
- 不得用旧构建（v0.1.0/2026-11-04）、8 GB 参考机、CI 结果替代本机标准档证据。
- 种子缓存安装与空缓存真实联机安装分开记录；本轮全部为种子缓存。
- 每项证据包含：命令/脚本、JSON（含机器身份与构建 commit）、截图或日志、耗时。
- GPU 数字为 host `nvidia-smi` 1 s 采样峰值（含约 1.4 GiB 桌面基线），不是 worker 的 CUDA reserved；方法已标注。
- 历史 `user-accepted`/waived 项（W05 100%/200% DPI、W06 真实 IME、parity 4.4 样图复核等）保持原决定，本 change 不重开、不再询问。
- Qwen/MOSS 的 blocked 是机器级内存状态问题：新用户在相同页面文件状态下同样命中，已在全新根实测证实。

## 执行顺序

已完成：S0 → S1 → S2 → S3 → S4–S8 → S9（blocked 记录）→ S10 → O1。剩余：O2–O10、C1/C3、D、B 收口项，以及（若所有者选择 A）Qwen/MOSS 补测后再归档。
