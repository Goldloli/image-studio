# 同根第二实例激活测试计划

状态：未执行。用户确认本 change 后执行；不下载模型、不运行推理。

## 0. 前置

- [ ] 记录提交、安装包 SHA-256、安装后可执行文件 SHA-256。
- [ ] 使用隔离数据根（`C:\ist-verify\r-single-a` 与 `r-single-b`），不操作用户默认数据根。
- [ ] 关闭所有已运行的 Image Studio 实例。

## 1. 同根激活（主路径）

- [ ] 以 `--data-dir C:\ist-verify\r-single-a` 启动实例 A，等待主窗口出现并记录 `pid`。
- [ ] 用同一 `--data-dir` 启动实例 B；记录起止时间与退出码。
- [ ] 期望：B 在 ~1.5 s 内以 0 退出；A 的窗口位于前台；A 的 `pid` 不变；不生成 `logs/startup-failure.log`；`data/library.sqlite3-wal` 大小在 B 前后不增长。
- [ ] 期望：任务管理器中 `image-studio-desktop.exe` 与 `msedgewebview2.exe` 数量不因 B 增加。

## 2. 最小化后激活

- [ ] 最小化 A 的窗口。
- [ ] 启动 B（同根）。
- [ ] 期望：A 的窗口恢复可见并位于前台；B 以 0 退出。

## 3. 激活超时（无响应）

- [ ] 人为让 A 无响应（例如在调试器中断 A，或用 PowerShell `Suspend-Process`）。
- [ ] 启动 B（同根）。
- [ ] 期望：B 在 1.5 s 后显示“Image Studio 已在运行但未响应”的原生提示，关闭后以 0 退出；A 未被结束。
- [ ] 恢复 A（`Resume-Process`），再次启动 B：期望回到第 1 节的激活路径。

## 4. 不同数据根并行（不得回归）

- [ ] 以 `r-single-b` 启动实例 C，同时 A 仍在运行。
- [ ] 期望：A、C 同时存活；各自存在 `cache/webview2`；C 不激活 A、A 不退出。

## 5. 结束首实例后重启

- [ ] 结束 A 与 C。
- [ ] 再次以 `r-single-a` 启动。
- [ ] 期望：正常创建窗口（无残留命名对象阻塞）；日志出现常规启动记录。

## 6. 真 WebView2 故障（既有诊断不得回归）

- [ ] 设置 `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER=C:\ist-verify\no-such-webview` 后启动（新根）。
- [ ] 期望：关闭 WebView2 自身错误框后，写出 `logs/startup-failure.log`、弹“Image Studio 启动失败”、关闭后退出码 3；`app.log` 保留 `app.panic`。

## 7. 跨会话（可选）

- [ ] 若环境允许第二个 Windows 登录会话：在会话 1 运行实例，会话 2 用同一数据根启动。
- [ ] 期望：不弹 WebView2 错误；显示“数据目录正在由另一个 Image Studio 使用”，退出码 0，数据库未被改写。

## 质量门

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --workspace --all-targets --locked -- -D warnings`
- [ ] `cargo test --workspace --locked`
- [ ] 前端 `npm test`、`npm run build`

## 不在本计划范围

- 托盘、开机自启、跨会话窗口激活。
- macOS 双开行为。
