# Proposal: windows-hardening-and-coverage

## Why

`2026-11-05-support-8gb-vram-tier` 与 `2026-11-06-fix-windows-desktop-defects` 交付了 8 GB 档位与五个桌面缺陷的修复，但在收尾盘点时留下三类问题：

1. **验收覆盖不全**：四个已安装模型里，只有 Qwen-Image 2.1 Turbo 与 FLUX.2 Klein 4B 在界面里完整出过图；MOSS 只测了取消，BiRefNet 装了没跑过；Real-ESRGAN 与 Qwen 基础版根本没装；Klein 的**图片编辑**只在引擎层验证，没走产品。
2. **缺陷修复中的分支没被真实触发**：取消的强制回收路径、真实网络故障下的下载重试、看门狗超时（修复后无法复现）都只有代码或单元测试保障。
3. **可观测性不足**：GUI 进程没有日志文件，本轮的根因（每次发送重新哈希 35 GB）是靠临时把 stderr 重定向到文件、再加 `eprintln!` 才定位的；发布版本里这条路径不可用。

另外本轮还观察到几个值得修的产品细节与测试工具缺口（见 design.md）。

## What Changes

- **补齐验收覆盖**：BiRefNet 去背景、Real-ESRGAN 放大、MOSS 完整出图、Klein 编辑、Qwen 基础版，全部在产品界面里跑通并记录时间。
- **补上未触发的分支**：用一个故意忽略取消的 stub worker 验证强制回收；把下载重试的故障注入做成可重复的测试；确认看门狗在人为阻塞下仍能落定。
- **桌面可观测性**：写入 `<data>/logs/app.log`（有界轮转），失败轮次记录阶段、耗时与关键路径；`eprintln!` 逐段追踪降噪为受控的结构化事件。
- **失败文案准确**：`Rejected` 轮次不再显示「未启动」，改为「发送失败」并直接给出原因与恢复建议。
- **测试工具**：`scripts/windows-ui/cdp.mjs` 增加文件选择与对话框命令，让 Klein 编辑与源图导入可脚本化。
- **Windows 专属回归**：高 DPI、小窗口布局、中文 IME、原生导出对话框、磁盘不足、非 ASCII 路径。
- **登记不做的与外部依赖**：`stream-prepare-low-memory` 暂缓、Z-Image 8 GB 关闭、12 GB 档回归由所有者执行、分支推送/合并待全部完成后进行。

## 成功标准

- `test-plan.md` 的覆盖矩阵中，所有「未测」项变为「通过（附证据）」或「明确不做（附理由）」。
- 发布构建在 `<data>/logs/app.log` 里能直接看到每次发送的阶段耗时与失败原因，无需重定向 stderr。
- 失败轮次在界面显示「发送失败：<原因>」而不是「未启动」。
- `openspec validate --all --strict`、全量 Rust/前端测试、生产构建全绿。

## 不做

- 不引入新的模型、协议、档位或量化格式。
- 不重做安装器架构，不改 manifest 身份。
- 不实现 `stream-prepare-low-memory`（另行确认）。
- 不做 Z-Image 的 8 GB 支持（已实测不可行）。
- 不改 12 GB 档的默认行为。
