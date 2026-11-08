# Tasks: certify-windows-10-target-machine

> **未获项目所有者确认前，不得开始构建或执行验收。** 本 change 不修改产品行为；所有任务以"取得证据 + 文档收口"为完成标志。每完成一组即写 JSON/日志到 `verification/windows-12gb-standard-tier/`。

## 0. 批准与准备

- [ ] 0.1 取得所有者对本 change 的明确实施确认，并记录 §9 三个决策（prepare 预检 A/B/C、Z-Image 可选回归、手工项配合安排）。
- [ ] 0.2 记录本机环境 JSON（OS/CPU/RAM/GPU/驱动/显示缩放/WebView2/页面文件 commit 上限/可用内存/磁盘/网络可达性/工具链版本），确认与 proposal 事实一致。
- [ ] 0.3 备份 `C:\ist-a\data\installations\models\*.json` 与 `C:\ist-a\logs\*` 为"2026-11-04 旧构建状态"证据，再开始任何重装。
- [ ] 0.4 `pwsh -File scripts/build-windows-internal.ps1`（或先 `-SkipBundle`）构建 HEAD 并保存构建日志、exe/安装包 SHA-256 与 commit。
- [ ] 0.5 运行本地完整质量门禁（`cargo fmt --check`、`clippy -D warnings`、workspace 测试、前端 `npm test`/`npm run check`/`npm run build`、`npm audit`、`openspec validate --all --strict`）并保存日志。

## 1. 12 GB 标准档回归（P0）

- [ ] 1.1 以当前构建 + `--data-dir C:\ist-a` 启动，确认旧就绪记录被激活身份校验拒绝并给出可行动错误；保存截图、错误文本与日志（S1）。
- [ ] 1.2 确认设备探针与档位投影：12288 MiB / sm86；模型页 7 型号、已安装 6 个显示 `standard · 1024² · bfloat16`；Z-Image 显示满足 12 GB 门槛；不出现 compact 投影（截图 + DOM 文本 JSON）。
- [ ] 1.3 用种子缓存重装 Klein、BiRefNet、Real-ESRGAN（无安装期 prepare），记录阶段、耗时、缓存命中、`seeded_cache` 类型；确认不使用网络下载（S2）。
- [ ] 1.4 对 Qwen-Image 2.1-Turbo 触发安装，验证 prepare 内存预检在量化前以可行动错误阻止：记录 `prepare-preflight.json`、`installation-process.log` 片段、错误文本、无半成品缓存/安装记录（S3，`blocked` 预期）。
- [ ] 1.5 如所有者选择 A/B 且环境准备成功，重跑 1.4 并完成 Qwen Turbo（8 步）标准档生成；否则记录 `blocked` 并把 1.6 的范围限定为已解锁型号（S8）。
- [ ] 1.6 逐能力产品内标准档生成：Klein 文生图、Klein 单图编辑、BiRefNet 去背景、Real-ESRGAN 4×；每项记录墙钟、阶段、CUDA reserved/allocated、进程 commit、可用内存最低值、PNG 尺寸/字节/SHA-256（S4–S7）。
- [ ] 1.7 若 1.5 解锁：补 Qwen-Image 2.1 Turbo、Qwen-Image 2.1、MOSS-Image-SFT 标准档生成（串行、按预算）；失败/超预算如实记录（S8）。
- [ ] 1.8 （可选，按决策）Z-Image Turbo 真实联机安装（~32.85 GB，ModelScope）与 1024² 9 步标准档生成；记录安装阶段与资源（S9）。
- [ ] 1.9 导出 PNG 与资产逐字节比对；重启应用确认 6/7 型号仍就绪、旧作品可重开并再次导出（S10）。
- [ ] 1.10 汇总标准档资源口径，与 `windows-local-generation` 合同（CUDA ≤10.5 GiB、commit ≤24 GiB、可用内存 ≥2 GiB、加载 ≤600 s、生成 ≤900 s、无共享内存回退）逐项对照；写 `verification/windows-12gb-standard-tier/` README 与 acceptance JSON。

## 2. Windows 桌面遗留项（P0/P1）

- [ ] 2.1 100%/150%/200% DPI：每档主界面 + 对话页截图 + 元素可见性检查；记录切换方式与是否需要重新登录（W1）。
- [ ] 2.2 960×640 与 1280×800 布局截图与可操作性检查（W2）。
- [ ] 2.3 真实 Microsoft 中文 IME：所有者实际输入并向候选后按 Enter，确认不发送；组合结束后显式发送才创建任务；记录为 `manual` 并附所有者结论（W3）。
- [ ] 2.4 原生导出对话框：`save-ui-dialog.ps1` 保存并与资产逐字节比对；覆盖确认与旧文件保护分支（W4）。
- [ ] 2.5 （可选）多显示器/睡眠唤醒：切换显示器/睡眠唤醒后截图与一次冒烟发送（W5）。
- [ ] 2.6 暗/亮主题截图与所有者目测结论（W6）。
- [ ] 2.7 所有者对 7 型号样图做一次质量人工复核（来源 `verification/windows-parity-app/not-run.json` 的 4.4 项 `pending_user`），记录结论（W7）。

## 3. 可观测性与故障分支（P1）

- [ ] 3.1 验证 `<data>\logs\app.log` 的结构化行与发送阶段耗时（预检/冻结/取锁/绑定/启动）；记录轮转上限的代码/单测依据（O1）。
- [ ] 3.2 看门狗 60 s：独占持有 `<data>\data\execution.lock` 后发送，断言 60 s 内变为"发送失败"并含阶段与原因、不无限停留；释放后重试成功（O2，B2 证据）。
- [ ] 3.3 取消覆盖：BiRefNet、Real-ESRGAN 与一个 Python 型号各取消一次，记录落定时间、强制回收分支、残留进程（O3，补 T12 5/5）。
- [ ] 3.4 预检期取消（bd ipw）：记录取消到落定的时间并与旧 ~90 s 观察对比，判断是否可关闭 ipw（O4，B2）。
- [ ] 3.5 生成与安装期间轮询控制台窗口进程增量，应为 0（O5）。
- [ ] 3.6 代理故障注入验证安装下载重试（`HTTPS_PROXY=http://127.0.0.1:9`，Real-ESRGAN 或可用小模型），并验证去代理后重装成功（O6）。
- [ ] 3.7 非 ASCII/长路径：`D:\ist-12gb\数据 测试\appdata` 新根完成 Real-ESRGAN 安装+放大（O7）。
- [ ] 3.8 磁盘不足快速回归：受限卷/配额下安装预检约 4 s 报错、不写缓存/安装记录（O8）。
- [ ] 3.9 本地运行 `cargo test -p image-studio-desktop --test forced_reap` 并保存日志，作为强制回收证据引用（O9）。
- [ ] 3.10 坏校验/缺失文件拒绝回归（I2-2）：在隔离根缓存中放入损坏 artifact 或删缺失文件后安装，验证明确错误、可重试、不写半成品（O10）。

## 4. CI 与发布记录（P1）

- [ ] 4.1 记录 HEAD 的 GitHub Actions quality run #35（run id、commit、Windows/macOS 结果、URL），写入 `verification/ci-release/` 并更新 README（C1）。
- [ ] 4.2 登记 v0.1.0 之后未发布提交与发布决策（tag/push 需所有者授权），不改远程（C2）。
- [ ] 4.3 （可选）下载 v0.1.0 发布包在本机安装/启动/卸载一次，记录为旧代码发布产物可用性证据（C3）。

## 5. 文档、规格与归档（P1）

- [ ] 5.1 更新 `windows-hardening-and-coverage/test-plan.md`：C3–C7 改为通过并引用 `verification/windows-desktop-coverage/`；2.7/2.8 状态与本轮事实一致（D1）。
- [ ] 5.2 更新 `verification/windows-8gb-tier/README.md` 的 "Known roughness"（发送卡顿与 ~2 min 取消已被修复并有后续测量）（D2）。
- [ ] 5.3 修复 `resources/windows-parity/realesrgan-ncnn-vulkan/asset-manifest.json` 的 `content_verified_pending_gate0` 状态并引用 Gate 0 证据（D3）。
- [ ] 5.4 同步 `windows-hardening-and-coverage` spec delta（桌面运行日志与失败诊断）到 `openspec/specs/windows-desktop/spec.md`，按 OpenSpec 流程归档该 change，并 `openspec validate --all --strict`（D4）。
- [ ] 5.5 `doc/15-windows-8gb-tier.md` 增补 12 GB 标准档本机实测表（明确机器与档位不同）（D5）。
- [ ] 5.6 写 `verification/windows-12gb-standard-tier/README.md` + `acceptance.json`，汇总所有 pass/blocked/not_run/manual 行，归档本 change（D6）。
- [ ] 5.7 清理 `verification/` 中已被后续证据取代的 `not_run`/过时行（`windows-ui-projection/README.md`、`windows-parity-worker/README.md`、`windows-parity-app/test-plan-matrix.md`、`windows-parity-review` 审计 R16/R19 遗留），标注取代关系（D7）。

## 6. bd 收口（P2）

- [ ] 6.1 `image-studio-06k`：确认三个 workflow 的 `uses:` 均为 40 位 SHA，带证据关闭；`image-studio-q2y`：pipeline 已归档、v0.1.0 已发布，带证据关闭（B1）。
- [ ] 6.2 `image-studio-ipw`：按 3.4 结果关闭或保留并更新笔记（B2）。
- [ ] 6.3 `image-studio-ba1` 保留（原生窗口级截图/坐标点击仍需人工或工具探索）；本地 DB 无 `b65`，登记 `bd dolt pull` 需授权（B3）。
