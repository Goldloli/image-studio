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
- [ ] 2.3 stub supervisor 集成测试：强制回收分支被真实触发。
- [ ] 2.4 诊断构建的下载故障开关（第 N 次尝试失败），验证真实重试路径。
- [x] 2.5 回归测试：`fast_readiness_check_never_hashes_file_contents`：发送快路径不得读取文件内容（哈希防回归）。
- [x] 2.6 模型选择器已确认正常：点击展开 6 个模型、显示状态与禁用原因，此前是测试时序问题。
- [ ] 2.7 DPI 100/150/200%、960×640 / 1280×800 截图回归。
- [ ] 2.8 中文 IME 组合确认不误触发送。
- [x] 2.9 原生导出对话框自动化：`cdp.mjs dialog` 填写保存框，导出文件与资产逐字节一致。
- [ ] 2.10 磁盘不足路径。
- [ ] 2.11 非 ASCII / 长路径下的安装与生成。

## P3 登记项（无需实现）

- [ ] 3.1 `stream-prepare-low-memory` 标注为暂缓（所有者决定：内存不再优化）。
- [ ] 3.2 Z-Image 8 GB 标注为关闭（实测不可行）。
- [ ] 3.3 12 GB 档真机回归交给所有者执行，附核对步骤。
- [ ] 3.4 分支推送与合并待所有者确认。
- [ ] 3.5 保留 bd `image-studio-b65`（Win10 历史复验）不动。
