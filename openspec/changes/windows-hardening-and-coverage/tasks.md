# Tasks: windows-hardening-and-coverage

> 未获用户确认前，不得开始实现代码。P0 为验收缺口，P1 为可观测性与文案，P2 为工具与回归。

## P0 验收覆盖（补齐从未在产品内跑过的能力）

- [x] 0.1 BiRefNet 去背景（12 s，RGBA + 蒙版）：界面导入源图 → 去背景 → 检查透明通道与 mask → 导出，记录耗时。
- [x] 0.2 Real-ESRGAN 放大：安装（33 MB）→ 220×220 → 880×880，4 s；安装过程中修掉了"坏分片永久失败"缺陷。
- [x] 0.3 MOSS-Image-SFT 完整出图（50 步）：约 18.5 分钟（CPU 被其他应用占用；隔离 Gate 0 为 752 s）。
- [x] 0.4 FLUX.2 Klein 4B 图片编辑：36 s 出图，源图导入经 Win32 对话框自动化。
- [x] 0.5 Qwen-Image 2.1 基础版：安装（种子缓存）→ 40 步出图约 14 分钟；具体提示词校验通过（橘猫）。
- [x] 0.6 结果写入 `verification/windows-desktop-coverage/2026-11-06-coverage-results.md`，`doc/15` 增补界面实测表。

## P1 可观测性与文案

- [x] 1.1 宿主写 `<data>/logs/app.log`：结构化行、8 MiB 轮转、保留 3 份。
- [x] 1.2 发送链路各阶段耗时并入失败详情与日志。
- [x] 1.3 `Rejected` 状态文案改为「发送失败」，原因作为主文案。
- [x] 1.4 内存预算拒绝改为中文 + 可行动建议。
- [x] 1.5 逐段 `eprintln!` 追踪降噪为 DEBUG 事件。

## P2 工具、分支覆盖与回归

- [x] 2.1 `cdp.mjs` 增加 `files`（DOM.setFileInputFiles）与 `dialog`（UIAutomation）命令。
- [x] 2.2 端到端脚本：send-and-wait 与 cancel 两个场景：send-export / cancel / kill-recovery / install。
- [x] 2.3 强制回收：整进程级已覆盖——`tests/forced_reap.rs` 用 `stub-supervisor` 二进制（不握手、不读 stdin、不退出）触发截止时间回收，实测 8.4 s 落定、进程消失、尝试为 Cancelled；判定逻辑另有单测。
- [x] 2.4 真实网络故障：用 `HTTPS_PROXY=http://127.0.0.1:9` 启动应用即让下载全部失败，实测 5 次重试后给出明确错误，去掉代理后重装成功。
- [x] 2.5 回归测试：`fast_readiness_check_never_hashes_file_contents`：发送快路径不得读取文件内容（哈希防回归）。
- [x] 2.6 模型选择器已确认正常：点击展开 6 个模型、显示状态与禁用原因，此前是测试时序问题。
- [x] 2.7 960×640 截图回归通过（导航/输入/发送/模型芯片均可见）；150%/200% DPI 未逐档截图。
- [x] 2.8 IME 行为由 `shouldSendOnEnter` 单测覆盖（`isComposing` / `compositionEndedAt`）；未做真实输入法手工验证。
- [x] 2.9 原生导出对话框自动化：`cdp.mjs dialog` 填写保存框，导出文件与资产逐字节一致。
- [x] 2.10 磁盘不足：Windows 目标机（RTX 3080 Ti）已实测——27.74 GiB 可用空间对 88.99 GiB 预算，约 4 s 内报 `磁盘空间不足`、不写缓存、不留安装记录，重试复现（`verification/windows-gate0/README.md`）；本轮另加决策函数单测（同卷重复计数、分卷各自校验）。
- [x] 2.11 非 ASCII/长路径实测通过（`D:\ist-8gb\数据 测试\appdata`）：安装、导入、4× 放大全通；过程中修掉了 ncnn 日志非 UTF-8 导致的探针失败。

## P3 登记项（无需实现）

- [x] 3.1 `stream-prepare-low-memory` 标注为暂缓（所有者决定：内存不再优化）。
- [x] 3.2 Z-Image 8 GB 标注为关闭（实测不可行）。
- [x] 3.3 12 GB 档真机回归交给所有者执行（核对步骤见 test-plan U1）。
- [x] 3.4 分支推送与合并待所有者确认（当前领先 origin/main 50+ 提交）。
- [x] 3.5 保留 bd `image-studio-b65`（Win10 历史复验）不动。
