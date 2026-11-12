# Design

## Context

`host::run`（`apps/desktop/src/host.rs`）依次做四件事：解析 `--data-dir`、安装启动失败钩子、
构造 `HostState`（其中 `Library::open` 用 `host.lock` 抢占数据根）、构建 Tauri 窗口。
WebView2 的 profile 目录固定在 `<root>/cache/webview2`（`startup::webview_profile_directory`），
因此同一数据根的第二个进程必然在 WebView2 初始化阶段失败。

现有可用的原子设施只有操作系统命名对象：互斥体存在性天然是“谁先创建谁赢”，且随最后一个句柄关闭
而销毁，不需要清理崩溃残留。

## Goals / Non-Goals

**目标**

- 同根第二次启动：不构造第二个宿主，激活已有窗口，第二个进程以 0 退出。
- 激活不可确认时给出与真实原因一致的提示，而不是 WebView2 错误。
- 不同数据根并行、真 WebView2 故障诊断都保持现状。

**非目标**

- 跨会话激活、托盘、自启；macOS 行为；生成/模型链路。

## Decisions

### 1. 身份由数据根决定，命名对象承载

- 对规范化的数据根路径（`canonicalize` 后转小写）做 SHA-256，取前 16 个十六进制字符作为 `root-id`。
- 三个命名对象，全部放在 `Local\` 命名空间（同会话可见，避免不同登录用户互相干扰）：

| 对象 | 类型 | 用途 |
|---|---|---|
| `Local\image-studio-root-<id>` | Mutex | 所有权：先创建成功者为 primary |
| `Local\image-studio-root-<id>-activate` | Event（auto-reset） | secondary → primary 的激活请求 |
| `Local\image-studio-root-<id>-ack` | Event（auto-reset） | primary 完成激活的回执 |

- 选择 `Local\` 而不是 `Global\`：不同 Windows 会话的用户不该被另一个会话的窗口打断；跨会话场景
  由第 4 节的回退路径负责解释。
- 选择 SHA-256 截断而不是完整路径拼名字：Windows 对象名有长度限制，路径含中文或空格也不适合直接
  出现在内核对象名里；16 hex（64 bit）在“同一台机器、同一用户”的规模下碰撞概率可忽略。

### 2. 启动协议

`single_instance::claim(root)` 在 `host::run()` 中、`HostState::new` 之前执行：

1. `CreateMutexW(None, false, name)`。
2. `GetLastError() != ERROR_ALREADY_EXISTS` → **primary**：创建 activate/ack 事件，把 mutex 句柄
   保存在本进程生命周期内（随 `run` 栈帧存活），继续正常启动。
3. `ERROR_ALREADY_EXISTS` → **secondary**：`OpenEventW` 两个事件，`SetEvent(activate)`，
   `WaitForSingleObject(ack, 1500 ms)`：
   - 等到 → 直接 `std::process::exit(0)`，不构造宿主、不打开数据库、不创建 WebView2。
   - 超时或事件打不开 → 原生提示“Image Studio 已在运行但未响应”后 `exit(0)`；提示里给出
     “结束该进程后重试”的动作，不写 `startup-failure.log`（这不是启动失败）。

`CreateMutexW` 的“已存在”判定是原子的，因此两个同时启动的进程只会有一个成为 primary，
不需要额外的启动锁或重试循环。

### 3. 激活：在 primary 的独立线程里完成

- primary 在 Tauri `setup` 完成后取出 `AppHandle`，启动一个命名线程 `single-instance`，
  循环 `WaitForSingleObject(activate, INFINITE)`。
- 收到信号：`unminimize()` → `show()` → `set_focus()`；随后用 Win32 `SetForegroundWindow` 兜底，
  仍失败时 `FlashWindowEx(FLASHW_ALL)` 让任务栏闪烁（前台锁的既有限制，不强行抢焦点）。
- 完成（成功或兜底）后 `SetEvent(ack)`。
- 信号可能在监听线程启动前到达：auto-reset 事件会保持已信号状态，监听线程第一次等待就会立即返回，
  窗口此时已经存在，因此不需要额外的“启动中”状态机。

### 4. 回退：数据根仍被占用时不要撞 WebView2

secondary 路径覆盖不到的情况（另一个 Windows 会话、命名对象命名空间差异、或 primary 正卡在
初始化）会让第二个进程继续走正常启动流程，`Library::open` 返回 `LibraryError::InUse`，目前表现为
“窗口内错误提示”，随后仍可能因 WebView2 profile 冲突 panic。

处理：`host::run` 在 `HostState::new` 之后、`tauri::Builder::build` 之前检查
`StudioService::startup_error()`；若为 `InUse`，用 `startup` 模块已有的 Win32 消息框能力显示
“数据目录正在由另一个 Image Studio 使用（数据根：…；如已退出请重试）”，然后 `exit(0)`。
其他启动错误（schema、IO）保持现有窗口内提示行为，不改变语义。

### 5. 与既有启动失败诊断的边界

| 情形 | 退出码 | 日志 | 对话框 |
|---|---|---|---|
| 同根第二实例并激活成功 | 0 | 无（第二个进程不写日志） | 无 |
| 同根第二实例，激活超时 | 0 | 无 | “已在运行但未响应” |
| 跨会话占根（InUse 回退） | 0 | 无新增失败记录 | “数据目录正在由另一个实例使用” |
| WebView2 缺失/损坏 | 3 | `startup-failure.log` + `app.panic` | “Image Studio 启动失败” |
| 超长数据目录 | 3 | `startup-failure.log` | “Image Studio 启动失败” |

### 备选方案

- **`tauri-plugin-single-instance`**：按应用标识而非数据根去重，会把“不同 `--data-dir` 并行”这条
  既有规格场景一起挡掉；不用。
- **只靠 `host.lock` 抢占并改进提示**：能消除 101，但用户仍然只能看到“另一个实例在使用”，窗口不会
  被交还；不满足本次目标。
- **激活后不等待回执**：实现更短，但无法区分“已激活”和“对方无响应”，也无法给用户确定的提示；
  采用 1.5 s 回执。

## Risks / Trade-offs

- **前台锁**：Windows 可能拒绝 `SetForegroundWindow`；以 `FlashWindowEx` 兜底并在文档中说明，
  不采用 `AttachThreadInput` 等侵入手段。
- **secondary 不写日志**：第二个进程在初始化日志系统前就退出；诊断价值由 primary 的
  `app.activate` 记录承担（收到激活请求与结果）。
- **对象名碰撞**：64 bit 截断在单机单用户范围可忽略；若未来需要多用户共享数据根，再扩展为
  带 SID 的身份。
- **句柄生命周期**：mutex 句柄必须活到进程结束；实现中由 `run` 栈帧持有的守卫结构保证，
  另加单元测试覆盖“守卫存活期间第二次 claim 为 secondary”。
- **测试可及性**：窗口焦点在自动化里只能断言“前台窗口标题/进程”，闪烁兜底无法自动断言，
  在验收记录里写明人工步骤。
