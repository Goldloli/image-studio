# Tasks: fix-spawned-child-console-window

- [x] 1.1 复现：生成期间 `inference-supervisor` 出现非零 `MainWindowHandle` 与 exe 路径标题；记录证据。
- [x] 1.2 定位根因：`creation_flags(CREATE_SUSPENDED)` 覆盖了 `quiet_command()` 的 `CREATE_NO_WINDOW`。
- [x] 2.1 `spawn_suspended_in_job` 与 `spawn_suspended_tokio` 改为 `CREATE_SUSPENDED | CREATE_NO_WINDOW`。
- [x] 2.2 新增 Windows 单测：std 与 tokio helper 拉起的控制台子进程无可见顶层窗口。
- [x] 2.3 重建桌面应用并重启；产品内一次生成：supervisor/python `MainWindowHandle=0`，无 app 归属新 conhost。
- [x] 2.4 安装路径复验：重装 BiRefNet 全程无窗口/无新控制台进程，安装完成。
- [x] 2.5 回归：`cargo test`（desktop 全量 + forced_reap）、格式与 clippy、`openspec validate --all --strict`。
- [x] 3.1 证据 JSON + 截图写入 `verification/windows-12gb-standard-tier/`。
- [x] 3.2 归档本 change（无规格 delta，`--skip-specs`）。
