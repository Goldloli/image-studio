# fix-spawned-child-console-window

**状态：2026-11-08 缺陷已复现（O5 回归），按下列方案修复并用本机实测验证。规格已有"生成不弹终端"场景，不需要修改主规格。**

## 问题

当前构建（含低内存 prepare 与 Qwen 修复）在本机发起一次生成时，`inference-supervisor.exe` 会带出**可见控制台窗口**：

- 实测：生成期间 `Get-Process inference-supervisor` 的 `MainWindowHandle=1443114`、`MainWindowTitle=C:\...\target\release\inference-supervisor.exe`；`python` worker 为 0/空。
- 安装路径同样经 `spawn_suspended_in_job` 拉起私有 Python，理论上同样弹窗。

## 根因

`adapters::quiet_command()` 用 `creation_flags(CREATE_NO_WINDOW)` 创建 supervisor 命令；但随后 `windows_process::spawn_suspended_in_job()`（以及 `spawn_suspended_tokio`）再次调用 `creation_flags(CREATE_SUSPENDED)`。Windows 上 `CommandExt::creation_flags` 是**覆盖**而不是叠加，于是 `CREATE_NO_WINDOW` 被清掉，控制台子进程由 GUI 宿主拉起时获得新的可见控制台窗口。

这是 `2026-11-06-fix-windows-desktop-defects` 修复后的回归：当时验证走的是生成路径，但 `spawn_suspended_*` 是后来为 Job Object 生命周期边界引入的，覆盖了既有标志，且没有窗口级回归测试。

## 修复

- `windows_process::spawn_suspended_in_job` / `spawn_suspended_tokio` 统一使用 `CREATE_SUSPENDED | CREATE_NO_WINDOW`，保留挂起→入 Job→恢复的顺序与所有取消/强杀语义。
- 新增 Windows 单测：用 helper 拉起控制台程序（`cmd /C ping`），枚举该 PID 的顶层可见窗口断言不存在；tokio 变体同样覆盖。
- 不改变任何模型/安装/生成行为，只恢复"子进程不弹终端"。

## 验证

1. 单测：`spawned_console_child_has_no_visible_window`（std + tokio）先红后绿。
2. 本机产品内：重建并重启应用，一次生成期间 `inference-supervisor` 与 `python` 的 `MainWindowHandle` 均为 0、无空标题；系统层面无 app 归属的新 conhost/OpenConsole/WindowsTerminal。
3. 安装路径：重装 BiRefNet 期间同样无窗口/无新控制台进程，安装照常完成。
4. 取消与强制回收回归：`tests/forced_reap.rs`、取消实测（Qwen/Turbo 等）仍通过。
5. 证据写入 `verification/windows-12gb-standard-tier/O5-console-window-current-build.json` 与截图。
