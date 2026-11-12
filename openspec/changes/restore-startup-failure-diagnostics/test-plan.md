# 启动失败诊断回归测试计划

状态：本机可执行；不下载模型、不运行推理。

## 0. 前置

- [ ] 记录当前提交、安装包/可执行文件 SHA-256 与数据根。
- [ ] 使用隔离数据根（例如 `C:\ist-verify\r-fix`），不操作用户默认数据根。

## 1. 同一数据根的第二个实例（缺陷复现项）

- [ ] 用安装后的可执行文件在隔离根启动实例 A，等待主窗口出现。
- [ ] 用**相同** `--data-dir` 启动实例 B；记录耗时、stdout/stderr 与退出码。
- [ ] 期望：B 写出 `<root>/logs/startup-failure.log`（含原因、时间、日志路径、建议），显示“Image Studio 启动失败”对话框；关闭后退出码 3。
- [ ] 关闭 B 的对话框后确认 A 仍在运行，A 的会话/收藏/数据库与失败前一致。
- [ ] 确认 `app.log` 同时留下一条 `app.panic` 记录（链式顺序）。

## 2. 不同数据根共存（不得回归）

- [ ] A 仍在运行时，用另一个 `--data-dir` 启动实例 C。
- [ ] 期望：A、C 同时存活，各自存在 `cache/webview2`，旧共享 profile 未被触碰。

## 3. 超长路径（既有显式分支）

- [ ] 用超过 94 UTF-16 units 的 `--data-dir` 启动。
- [ ] 期望与修复前一致：写 `startup-failure.log`、弹窗、退出码 3，不创建 `data/`、`cache/`。

## 4. 质量门

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] 前端 `npm test`、`npm run build`

## 不在本计划范围

- 单实例激活既有窗口（本轮不实现）。
- WebView2 缺失安装场景的重新打包验证（沿用 2026-11-03 W07 证据）。
