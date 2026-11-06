# Proposal: fix-windows-desktop-defects

## Why

`2026-11-05-support-8gb-vram-tier` 的界面验收在本机（Windows 11 / RTX 2060 SUPER 8 GB）暴露了四个真实缺陷。它们不影响引擎与 Gate 0 证据，但会让普通用户无法正常完成"安装 → 连续出图"的日常流程：

1. **连续发送会卡死**：第二次（或连续第 N 次）发送后，对话停在「正在接收提示词」，GPU 完全空闲，必须重启应用。本会话中出现至少 3 次，均在重启宿主后恢复。
2. **取消落定约 2 分钟**：点「停止」后界面长时间停在「正在取消」，实测约 120 s 才变「已取消」，期间 worker 进程已消失。
3. **安装期模型页显示"尚未安装"**：安装器实际正在 downloading / preparing / 量化，模型页却显示"尚未安装"，用户会误以为点击无效并重复操作。
4. **应用内下载 wheel 偶发失败**：安装私有运行时期间，从 `files.pythonhosted.org` 下载 `setuptools-84.2.1`、`markupsafe-3.0.4` 等 wheel 时报 `error sending request`，整个安装失败；重试会换另一个 wheel 继续失败。

前两条是用户可见的可靠性缺陷，后两条会造成"装不上/以为没装上"。它们都缺乏自动化回归，本 change 补齐。

## What Changes

- **发送不再可能无限挂起**：`conversation_send` / `prepare_turn` 的每个阶段都有界并在超时后把轮次置为失败（带阶段名与可行动原因），不允许停留在 pending 且无 job 的状态；同时消除"上一轮清理长期持有调度/库互斥锁"的阻塞路径。
- **取消有明确时限**：取消后先合作取消，超过宽限期仍未回收则以"已取消（进程已强制回收）"落定，逐步清理不阻塞 UI；界面显示已等待时长。
- **安装期阶段实时投影**：模型页与设置页在 downloading / verifying / preparing / installing_runtime / probing 各阶段显示真实阶段与已下载字节，不再显示"尚未安装"。
- **下载具备有界重试**：每个 artifact 失败后按退避重试（保留 `.part` 续传），超过次数才失败；错误信息包含 URL 与已尝试次数，并保留已下载进度。
- 为四个缺陷各补一条自动化或脚本化回归（见 test-plan）。

## Capabilities

### Modified Capabilities

- `windows-desktop`：发送必须有界完成或给出可行动失败；取消必须在时限内落定。
- `model-catalog`：安装期各阶段必须实时投影，不能显示成"尚未安装"。
- `model-installation`：artifact 下载必须有界重试且可续传，失败信息可行动。

## 成功标准

- 连续 20 次发送（交替模型，含一次取消）不出现"停在正在接收提示词"；若某次确实无法启动，轮次在 60 s 内以明确错误结束，而不是无限等待。
- 取消从点击到 `cancelled` ≤ 15 s（含强制回收路径），不再出现 2 分钟等待。
- 真实安装一次（Qwen-Image 2.1 Turbo），模型页在每个阶段都能看到对应文案，不出现"尚未安装"。
- 用故障注入让某个 wheel 连续失败 2 次后成功，安装继续完成；失败 5 次时给出含 URL 与次数的可行动错误。
- 现有 234+ Rust 测试、前端 59 测试、`openspec validate --all --strict` 全绿；不改变引擎、协议与档位行为。

## 不做

- 不做安装期内存优化（分片流式量化另见 `stream-prepare-low-memory`）。
- 不改变模型、档位、配方、生成协议或 Gate 0 证据。
- 不实现断点续传跨进程持久化以外的下载加速、镜像切换或 CDN 选择。
- 不修复 Z-Image 的 8 GB 支持（已实测不可行）。
