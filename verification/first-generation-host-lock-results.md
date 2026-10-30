# 宿主锁继承回归验收

日期：2026-10-30。对应 Beads `image-studio-b88.7`。

默认并发 workspace 测试曾在旧宿主释放后重开 Library 时得到 `InUse`。原始 `File` 依赖最后一个描述符关闭释放锁；同时运行的子进程若保留继承描述符，会让宿主退出作用域后仍被占用。

现使用私有 `HostLock` RAII 显式解锁，字段顺序保证 SQLite 连接先关闭、排他锁后释放。真实回归子进程只在自己的 `pre_exec` 内清除继承 FD 的 CLOEXEC，并在 exec 后继续持有描述符。测试确认活动 owner 拒绝第二实例，旧 Library drop 后子进程仍活着也可立即重开，旧子进程退出不会解锁新 owner。测试保留 stdout 读取端直至子进程退出，避免 fixture 的 SIGPIPE 干扰。

最终 [默认并发完整测试](first-generation-rust-tests.log) 通过：134 passed、0 failed、2 ignored；两项显式验收另见[质量汇总](first-generation-quality.json)。没有将测试改为串行，没有移除排他断言，也没有对重开操作加盲目重试。

历史失败日志保留：[首次完整测试](first-generation-rust-tests-attempt1.log)、[回归 fixture 修正前](first-generation-rust-tests-attempt2.log)、[早期定向日志](first-generation-host-lock-tests.log)。它们是问题排查记录，不是当前通过证据。
