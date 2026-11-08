# Design: fix-spawned-child-console-window

## 1. 现状与证据

生成期间采样（2026-11-08，`target/release/image-studio-desktop.exe --data-dir C:\ist-accept`）：

| 进程 | MainWindowHandle | MainWindowTitle |
|---|---|---|
| image-studio-desktop | 2032698 | Image Studio |
| inference-supervisor | **1443114** | **C:\...\target\release\inference-supervisor.exe** |
| python（worker） | 0 | （空） |

同一构建在 2026-11-06 的 8 GB 机器上曾验证"supervisor 与 python 均无窗口句柄"，说明这是引入 Job Object 挂起/恢复助手后的回归。

## 2. 根因

`std::os::windows::process::CommandExt::creation_flags` 是设置（覆盖）而非按位或。`quiet_command()` 先设 `CREATE_NO_WINDOW`；`spawn_suspended_in_job`/`spawn_suspended_tokio` 为了挂起后入 Job，再设 `CREATE_SUSPENDED`，把 `CREATE_NO_WINDOW` 覆盖掉。`inference-supervisor.exe` 与控制台 Python 都是控制台子系统程序，父进程是 windows 子系统 GUI，于是 Windows 为新控制台子进程分配可见控制台窗口。

影响范围：生成（supervisor）、安装（私有 Python 安装/探测/量化）、诊断 worker 的 tokio 变体。

## 3. 修复方案

两个 helper 内改为：

```rust
command.creation_flags(CREATE_SUSPENDED | CREATE_NO_WINDOW);
```

- 保存挂起→AssignProcessToJobObject→恢复线程的顺序不变；
- 所有经过 helper 的子进程（生成 supervisor、安装 python、诊断 worker）统一无窗口；
- 不修改 spawn 之外的任何业务逻辑，不修改 Job 生命周期与取消/强杀路径。

## 4. 回归测试

新增 flags 组合单测 `suspended_job_spawn_combines_suspend_and_no_window_flags`：断言 helper 的创建标志同时包含 `CREATE_SUSPENDED` 与 `CREATE_NO_WINDOW`。

行为级"窗口句柄为 0"的测试无法在控制台测试进程内复现：测试进程本身有控制台，`cmd` 子进程会继承它而不是获得新窗口。因此最终验收采用产品级实测（GUI 宿主下生成/安装期间 supervisor/python 的 `MainWindowHandle` 均为 0，见证据 JSON）。

## 5. 风险与回滚

| 风险 | 缓解 |
|---|---|
| 某些子进程依赖控制台句柄（stdout 重定向仍有效；CREATE_NO_WINDOW 只影响窗口，不影响管道） | 生成/安装/探测全部沿用既有 stdout/stderr 管道，实测出图与安装通过 |
| tokio 诊断 worker 行为变化 | 仅隐藏窗口，无窗口也不影响 stdin/stdout 协议 |
| 覆盖 helper 调用方已有 flags | 当前调用方未设置其他 flags；单测覆盖 |

回滚：还原两个 helper 的一行 flags 即可。
