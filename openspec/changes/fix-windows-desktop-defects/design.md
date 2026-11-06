# Design: fix-windows-desktop-defects

## 1. 连续发送卡死

**现象**：第二次（或连续第 N 次）发送后，对话停在「正在接收提示词」，GPU 空闲，重启应用后恢复。本会话出现 ≥3 次；`generation_turns` 中该轮 `status='pending'`、`job_id IS NULL`。

**代码事实**（`apps/desktop/src/studio.rs`）：

- `prepare_turn` 在冻结请求后取 `self.scheduling` 互斥锁，随后在**持锁状态**下调用 `library.bind_turn_job()`（SQLite Immediate 事务）。
- `submit` 与 `retry` 也取同一把锁。
- 生成线程在完成/取消路径上会取 `library` 互斥锁与 `generation.inner.state` 锁。

**根因假设**（按可能性排序）：

1. 上一轮的清理/落库线程持有 `library` 或 `scheduling` 锁，且其中某一步无超时（例如等待进程回收或 SQLite 写入），新发送在 `prepare_turn` 取锁处无限阻塞。
2. `bind_turn_job` 的 SQLite Immediate 事务遇到持锁连接，未设置 `busy_timeout`，被阻塞而非快速返回 `BUSY`。
3. 执行租约文件 `execution.lock` 仍被残留进程持有，`generation.start` 返回错误，但该错误路径未把 pending 轮次置为失败。

**修复方向**：

- 给发送链路加**分段计时与结构日志**（prepare / freeze / lock / bind / start 各段耗时），先定位真实阻塞点，再针对性去掉"持锁做慢操作"。
- 不把 `bind_turn_job` 放在 `scheduling` 锁内，或改为有界重试 + `busy_timeout`。
- 无论根因如何，增加**发车看门狗**：pending 且无 job 超过 60 s 的轮次必须转为 `failed`，错误里带阶段名与最近一次阻塞点，界面显示可行动原因并允许重试，而不是无限「正在接收提示词」。
- `generation.start` 的失败必须落到轮次状态（失败原因含租约/进程信息）。

## 2. 取消落定慢

**现象**：点击「停止」后约 120 s 才从「正在取消」变为「已取消」；期间 worker 进程已消失。

**代码事实**：`SupervisorConfig.timeouts` 为 `cancel_grace_ms = 5_000`、`cleanup_deadline_ms = 30_000`；轮次只有在 attempt 被回收（`finish_after_reap`）后才转 `cancelled`。

**根因假设**：

1. worker 侧合作取消只在采样步之间检查；单步耗时较长（MOSS 约 12 s/步），但不足以解释 120 s。
2. worker 退出时释放 6 GB 级张量、torch/进程组拆解耗时，supervisor 等进程真正退出，应用再等 reap。
3. 应用侧把「等待回收」做成了同步阻塞，UI 只能显示等待。

**修复方向**：

- 记录取消各阶段时间戳（请求 → worker 收到 → 进程退出 → 租约释放 → 状态落定），确定耗时集中在哪一段。
- 超过 `cancel_grace_ms + cleanup_deadline_ms` 仍未回收时，允许强制回收并立即把轮次落为 `cancelled`（原因标注"进程已强制回收"），回收清理在后台完成。
- 界面在「正在取消」时显示已等待秒数，超过 15 s 给出说明。

## 3. 安装期模型页显示"尚未安装"

**现象**：安装器实际在 downloading / preparing 时，模型页显示"尚未安装"；数据目录里已有下载与 prepared 目录。

**代码事实**：模型页的 `installPhase` 来自 `InstallationSnapshot`；`project_entry` 只有在 `entry.install_id` 命中 `installations` 时才取快照，否则回退为 `not_installed`/`failed`。安装期间的快照若未及时刷新或 install_id 未被识别，就会显示"尚未安装"。

**修复方向**：

- 安装进行中时，宿主在内存中维护当前安装的 phase/receivedBytes，并由 `model_catalog` 与 `model_view` 共同读取，保证"安装总览 + 行内状态"同源。
- 模型页在安装期轮询（有界）或在 `model_install` 返回后立即刷新，不依赖用户手动刷新。
- 增加逐阶段投影测试：downloading / verifying / preparing / installing_runtime / probing 都必须映射到正确文案，且不得出现 `not_installed`。

## 4. 应用内下载 wheel 偶发失败

**现象**：安装私有运行时期间，`files.pythonhosted.org` 的单个 wheel 报 `error sending request`，整个安装失败；重试换另一个 wheel 继续失败。

**代码事实**：安装器逐 artifact 下载，失败即整体失败；没有按 artifact 的重试/退避；`.part` 续传只在特定路径生效。

**修复方向**：

- 每个 artifact 失败后按 1 s / 2 s / 4 s 退避重试，最多 5 次；保留 `.part` 并按 Range 续传；
- 失败信息包含 URL、已尝试次数与最后错误，并给出"重试安装/检查网络"的可行动建议；
- 只对幂等的 GET 重试；校验失败不重试同一字节，直接标记该文件损坏并重新下载；
- 重试与等待都计入有界进度事件，不伪造百分比。

## 5. 风险

- 发送看门狗可能误杀真正慢的轮次：阈值 60 s 远大于实测发送耗时（<1 s），且看门狗只作用于"pending 且无 job"的轮次。
- 取消强制回收可能与正在写盘的 worker 竞争：回收后由既有 quarantine/事务校验兜底，半成品不进资产库。
- 下载重试会放大日志与进度事件：沿用有界日志与节流。
