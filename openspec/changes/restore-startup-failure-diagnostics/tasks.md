# Tasks

用户已指示“修复”，以下任务在本轮执行。

## 1. 实现

- [ ] 1.1 `apps/desktop/src/adapters/logging.rs` 增加 `install_panic_hook()`：先记录 `app.panic`，再调用 `take_hook()` 取出的钩子。
- [ ] 1.2 `HostState::new` 改用它，删除内联 `set_hook`。
- [ ] 1.3 检查 `startup.rs` 的匹配前缀与现有单元测试仍然成立。

## 2. 回归

- [ ] 2.1 `cargo fmt --all -- --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、`cargo test --workspace --locked` 通过。
- [ ] 2.2 前端 `npm test`、`npm run build` 通过（无前端改动，作基线确认）。

## 3. 本机复验

- [ ] 3.1 重建 release 与 NSIS 安装包，覆盖安装当前提交。
- [ ] 3.2 在隔离根上启动实例 A，再用相同 `--data-dir` 启动实例 B：确认 `startup-failure.log`、中文对话框、关闭后退出码 3。
- [ ] 3.3 用不同 `--data-dir` 启动实例 B：确认两实例共存且各自拥有 `cache/webview2`。
- [ ] 3.4 确认实例 A 的会话、收藏等数据未被修改。

## 4. 证据与收口

- [ ] 4.1 更新 `verification/windows-functional-regression-20261112` 的 I03、F1 与结论，保存复验截图/文本。
- [ ] 4.2 关闭 Beads `image-studio-z1v`，更新 `image-studio-bcn` 的结论。
- [ ] 4.3 用户确认结果后归档本 change，并把 `windows-desktop` delta 合并进主 spec。
