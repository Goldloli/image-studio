# Tasks

状态：等待项目所有者确认后执行。每项附完成验证；实现与验收证据一并提交。

## 1. 单实例身份与启动协议

- [ ] 1.1 新增 `apps/desktop/src/single_instance.rs`：`root_identity(root) -> String`（规范化 + 小写 + SHA-256 前 16 hex）与命名对象名常量。
- [ ] 1.2 实现 `claim(root)`：Windows 上用 `CreateMutexW` 判定 primary/secondary；primary 创建 activate/ack 事件并返回持有 mutex 句柄的守卫；非 Windows 一律返回 primary（no-op）。
- [ ] 1.3 实现 `request_activation()`：`SetEvent(activate)` + `WaitForSingleObject(ack, 5000 ms)`，返回是否确认。
- [ ] 1.4 `host::run` 在 `HostState::new` 之前调用 `claim`；secondary 分支不发写日志、直接以 0 退出；`--diagnostics` 模式同样适用。

## 2. 窗口激活

- [ ] 2.1 primary 在 Tauri `setup` 后启动 `single-instance` 线程，等待 activate 事件。
- [ ] 2.2 收到信号后 `unminimize()`→`show()`→`set_focus()`，再用 `SetForegroundWindow` 兜底，失败时 `FlashWindowEx`。
- [ ] 2.3 完成后 `SetEvent(ack)`；在 `app.log` 记录 `app.activate`（结果字段：focused / flashed / missing-window）。
- [ ] 2.4 验证信号在监听线程启动前到达时仍会被消费（auto-reset 事件保持已信号状态）。

## 3. 占用回退提示

- [ ] 3.1 `host::run` 在 `tauri::Builder::build` 前检查 `studio.startup_error`；`InUse` 时用原生消息框显示“数据目录正在由另一个 Image Studio 使用（数据根：…）”并 `exit(0)`。
- [ ] 3.2 其他启动错误（schema/IO）保持窗口内提示，不改为退出。
- [ ] 3.3 确认该路径不再触发 WebView2 panic（不出现 `app.panic` 行与 101/3）。

## 4. 回归测试

- [ ] 4.1 单元：`root_identity` 对同一目录的不同写法（尾部分隔符、大小写）产生同一 id；不同目录不同 id。
- [ ] 4.2 单元（Windows）：同一进程内两次 `claim` → 第一次 primary、第二次 secondary；不同根均 primary；守卫释放后可再次成为 primary。
- [ ] 4.3 集成（Windows，`#[ignore]`）：用 `CARGO_BIN_EXE_image-studio-desktop` 起第一个实例，第二次启动返回 0 且 ≤3 s，第一个实例存活，未生成 `startup-failure.log`；不同根两实例均存活。
- [ ] 4.4 `cargo fmt --check`、`cargo clippy --workspace --all-targets -D warnings`、`cargo test --workspace`、前端 `npm test` / `npm run build` 通过。

## 5. 本机验收与文档

- [ ] 5.1 重建 NSIS 安装包并覆盖安装，按 [test-plan.md](test-plan.md) 记录同根激活、最小化恢复、无响应提示、不同根并行、结束首实例后重启、WebView2 故障注入。
- [ ] 5.2 把结果写入 `verification/` 新目录（机器身份、构建哈希、逐项 pass/fail、截图与日志）。
- [ ] 5.3 更新 `verification/windows-functional-regression-20261112` 的 I03 说明，标注单实例激活已实现。
- [ ] 5.4 关闭 Beads `image-studio-an2`；用户确认后归档本 change 并合并 `windows-desktop` delta。

