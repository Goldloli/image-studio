# Test plan: certify-windows-10-target-machine

环境（本机，2026-11-08 实测）：Windows 10 Pro 22H2 19045 / i7-13700K / **RTX 3080 Ti 12 GB（sm86，驱动 596.49）** / 31.7 GiB RAM / 3840×2160 @150% / WebView2 157.0.4341.42。
隔离数据根：`C:\ist-a`（104.9 GiB 缓存，6 个型号，2026-11-04 就绪记录）；不触碰默认数据根。
构建：`scripts/build-windows-internal.ps1`（HEAD 记录到每条证据）。

## 覆盖矩阵

| ID | 能力/项 | 当前状态 | 判定方式 | 证据 |
|---|---|---|---|---|
| S0 | 环境、构建、质量门禁 | pending | 质量门禁全绿 + HEAD 记录 | `verification/windows-12gb-standard-tier/quality/` |
| S1 | 旧就绪记录激活身份拒绝 | pending | 当前构建拒绝 2026-11-04 记录并给可行动错误 | 截图 + 错误 JSON |
| S2 | 种子缓存重装（Klein/BiRefNet/Real-ESRGAN） | pending | 安装阶段完整、无下载、记录 `seeded_cache` | install JSON |
| S3 | Qwen/MOSS prepare 预检 blocked | pending | 量化前失败、错误可行动、无半成品；数值入证据 | `prepare-preflight.json` + 错误文本 |
| S4 | 标准档生成：Klein t2i | pending | 1024² 输出、资源合同、PNG/SHA-256 | generate JSON + PNG |
| S5 | 标准档生成：Klein 编辑 | pending | 源图导入 → 编辑出图、场景保留 | generate JSON + PNG |
| S6 | 标准档生成：BiRefNet | pending | RGBA + mask，双输出 | JSON + PNG |
| S7 | 标准档生成：Real-ESRGAN | pending | 220×220 → 880×880（4×） | JSON + PNG |
| S8 | 标准档生成：Qwen Turbo / Qwen base / MOSS（按决策） | pending | 若解锁：1024² 输出 + 资源合同；否则 `blocked` | JSON + PNG |
| S9 | Z-Image 标准档（可选） | pending | 1024² 9 步、12 GB 门槛、资源合同 | JSON + PNG |
| S10 | 导出逐字节 + 重启恢复 | pending | 导出 == 资产；重启后就绪且可重开旧作品 | JSON + SHA-256 |
| W1 | 100%/150%/200% DPI | pending | 每档截图 + 元素可见性 | screenshots + JSON |
| W2 | 960×640 / 1280×800 | pending | 布局与可操作性 | screenshots + JSON |
| W3 | 真实中文 IME | pending（manual） | 候选确认不发送、显式发送才创建任务 | 所有者确认记录 |
| W4 | 原生导出对话框 | pending | 文件写出、覆盖确认、旧文件不损坏 | JSON + 截图 |
| W5 | 多显示器/睡眠唤醒 | pending（optional） | 归位与冒烟发送正常 | JSON |
| W6 | 暗/亮主题人工审阅 | pending（manual） | 所有者目测结论 | screenshots + 记录 |
| O1 | app.log 结构化与阶段耗时 | pending | 日志存在、字段齐全、失败可定位 | 日志摘录 |
| O2 | 看门狗 60 s 卡单 | pending | 60 s 内 failed + 阶段/原因；释放锁后重试成功 | JSON + 日志 |
| O3 | 取消（BiRefNet/Real-ESRGAN/Python 型号） | pending | ≤15 s 落定、无残留进程 | JSON |
| O4 | 预检期取消（bd ipw） | pending | 落定时间 + 与旧 ~90 s 对比 | JSON |
| O5 | 生成/安装不弹控制台 | pending | 期间无新 conhost/OpenConsole/WindowsTerminal | 轮询 JSON |
| O6 | 安装下载重试（代理故障） | pending | 5 次重试后明确错误；去代理后重装成功 | JSON + 日志 |
| O7 | 非 ASCII/长路径 | pending | 中文+空格数据根安装+放大正常 | JSON + 截图 |
| O8 | 磁盘不足快速回归 | pending | 约 4 s 报错、无缓存/安装记录 | JSON |
| O9 | 强制回收测试引用 | pending | `forced_reap` 本地测试通过 | 测试日志 |
| C1 | HEAD CI run #35 记录 | pending | Windows + macOS 均 success | `verification/ci-release/` |
| C2 | 未发布提交/发布决策登记 | pending | 记录 75 commits、owner-gated | README 更新 |
| C3 | v0.1.0 发布包本机安装（可选） | pending | 安装/启动/卸载成功，标签为旧代码 | JSON |
| D1 | windows-hardening test-plan 矩阵更新 | pending | C3–C7 状态与事实一致 | diff |
| D2 | windows-8gb-tier known roughness 更新 | pending | 表述与 11-06 测量一致 | diff |
| D3 | Real-ESRGAN asset-manifest 状态修复 | pending | `verified` 与证据一致 | diff |
| D4 | windows-hardening spec delta 同步并归档 | pending | 主规格含日志要求；change 归档；strict 通过 | openspec 输出 |
| D5 | doc/15 增补 12 GB 标准档表 | pending | 表格含本机实测 | diff |
| D6 | 本 change 证据 README + acceptance.json + 归档 | pending | strict 通过、归档完成 | `openspec list` |
| B1 | bd 06k/q2y 关闭证据 | pending | uses 全 SHA、pipeline 已归档 | bd 输出 |
| B2 | bd ipw 验证/关闭 | pending | O4 证据；通过则关闭，否则保留 | bd 输出 |
| B3 | bd b65 缺失与 dolt 同步登记 | pending | 记录本地 DB 无 b65；同步需授权 | bd 输出 |

## 判定规则

- 任何未执行项保持 `not_run`；被内存预检/外部条件阻止记 `blocked`；不得写成通过。
- 不得用旧构建（v0.1.0/2026-11-04）、8 GB 参考机、CI 结果替代本机标准档证据。
- 种子缓存重装与空缓存真实联机安装分开记录；后者若未执行，下载链路在本轮记 `not_run`。
- 每项证据包含：命令/脚本、JSON（含机器身份与构建 commit）、截图或日志、耗时；时间以墙钟为准并标注是否包含组件加载。
- 手工项（真实 IME、主题审阅、多显示器）必须由所有者确认；代理只能记录与整理，不代替判定。
- 数据根与非 ASCII 用例的产物位置、进程组残留、导出哈希必须可复核。

## 执行顺序

1. S0 构建与质量门禁 → 2. S1 激活拒绝 → 3. S3 预检 blocked 证据 → 4. 与所有者确认 §9 决策 → 5. S2 重装 → 6. S4–S10（按耗时从短到长）→ 7. W/O 系列 → 8. C/D/B 收口 → 9. 汇总并归档。

每完成一组即写 JSON/日志，避免长会话中断丢失证据。
