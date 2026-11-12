# Tasks

用户已指示“修复”，以下任务已在 2026-11-12 执行完毕（除归档外）。

## 1. 实现

- [x] 1.1 `apps/desktop/src/adapters/logging.rs` 增加 `install_panic_hook()`：先记录 `app.panic`，再调用 `take_hook()` 取出的钩子。
- [x] 1.2 `HostState::new` 改用它，删除内联 `set_hook`。
- [x] 1.3 `startup.rs` 的匹配前缀与现有单元测试保持不变并通过。

## 2. 回归

- [x] 2.1 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked` 通过（clippy 1.99.1 对只读删除路径的 `permissions_set_readonly_false` 新增定向 `#[allow]`，见提交 `f54a434`）。
- [x] 2.2 前端 `npm test` 81 项通过、`npm run build` 通过。

## 3. 本机复验

- [x] 3.1 重建 release 与 NSIS 安装包（245,102,349 B，`56dc6bcf…`），覆盖安装当前提交。
- [x] 3.2 同一数据根第二实例：`startup-failure.log`、中文对话框、关闭后退出码 3（`verification/windows-functional-regression-20261112/05-fixed-second-instance.txt`、`07-installed-fix-recheck.txt`）。
- [x] 3.3 不同 `--data-dir` 的两实例共存，各自拥有 `cache/webview2`。
- [x] 3.4 第一个实例全程存活；其数据根仍为 `user_version=6`、`quick_check=ok`、1 会话 / 1 草稿 / 0 作品。
- [x] 3.5 强制 WebView2 失败（`WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 指向不存在目录）：日志 + 对话框 + 退出码 3，`app.log` 保留 `app.panic`（`06-fixed-bad-webview.txt`）。

## 4. 证据与收口

- [x] 4.1 已更新 `verification/windows-functional-regression-20261112` 的 I03、F1、结论与 `results.json`/`environment.json`。
- [x] 4.2 关闭 Beads `image-studio-z1v`，并在 `image-studio-bcn` 记录结论。
- [x] 4.3 用户确认结果后归档本 change，并把 `windows-desktop` delta 合并进主 spec（2026-11-12 归档）。

