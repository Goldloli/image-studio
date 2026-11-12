# 同根第二实例激活实测（2026-11-12）

Change：`activate-running-instance`（Beads `image-studio-an2`）。
本机：Windows 11 企业版 22631 / i9-9900K / RTX 2060 SUPER 8 GiB / 31.9 GiB RAM。
本轮只做原生窗口与进程行为验证：不下载模型、不运行推理。

**结论：通过。** 同一数据根的第二次启动在 186 ms 内激活已有窗口并以 0 退出，不写启动失败记录；
最小化窗口被还原；无响应实例在 5 s 后得到明确提示；不同数据根仍可并行；WebView2 真故障仍走
`startup-failure.log` + 对话框 + 退出码 3。跨 Windows 登录会话的提示未在本机执行（无第二个会话）。

## 构建身份

| 项 | 值 |
|---|---|
| 提交 | `5345d4c290c9e42f8f89c21193f949be1c5e8df2` |
| 安装包 | `Image Studio_0.1.1_x64-setup.exe`，245,119,760 B，SHA-256 `00a348170d45ca842ae75f896f463ceba24f57de6e5693395ee125bd44490818` |
| 安装后 exe | `%LOCALAPPDATA%\Image Studio\image-studio-desktop.exe`，42,508,800 B，SHA-256 `c37485558d1702abc2b0ae0eb4a0cee72be22668dae34466332329b5d8c79898` |
| 安装方式 | current-user 静默安装，退出码 0 |
| 数据根 | `C:\ist-verify\r-single-a`、`r-single-b`、`r-single-fault`（隔离、可丢弃） |

## 结果

| 用例 | 结果 | 实测 |
|---|---|---|
| 同根第二次启动 | pass | 第二个进程 **186 ms** 以退出码 **0** 结束；前台窗口切换为第一个实例（`GetForegroundWindow` 的 PID 等于 A）；未生成 `logs/startup-failure.log`；进程数保持 1 |
| 最小化后激活 | pass | `IsIconic=true` → 第二次启动 → `IsIconic=false` 且 A 为前台窗口 |
| 已运行实例无响应 | pass | 用 `NtSuspendProcess` 暂停 A 后启动 B：5 s 后出现「Image Studio 已在运行 / 已有一个 Image Studio 窗口在使用该数据目录，但 5 秒内没有响应…」，关闭后 B 退出码 0；A 恢复后仍存活 |
| 不同数据根并行 | pass | `r-single-a` 与 `r-single-b` 同时运行，两个根各自存在 `cache/webview2`，C 不激活 A |
| 结束实例后重启 | pass | 结束全部实例后以 `r-single-a` 重启，正常创建窗口，无残留命名对象阻塞 |
| WebView2 真故障 | pass | `WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 指向不存在目录：关闭 WebView2 错误框后写 `startup-failure.log`（`Could not find the webview runtime`）、弹「Image Studio 启动失败」、退出码 **3**，`app.log` 保留 `app.panic` |
| 跨登录会话 | not_run | 本机只有一个交互会话，无法构造；代码路径为 `Unavailable`，提示「可能属于另一个 Windows 登录会话」 |

## 自动化

- 单元（`cargo test -p image-studio-desktop --lib single_instance`）：4 项通过
  - 同一目录的不同写法（尾部分隔符、大小写）得到同一 16 hex 身份；
  - 不同目录身份不同；
  - 同一进程内第二次 `claim` 为 secondary，释放后再次成为 primary；
  - 不同根均为 primary。
- 集成（`cargo test -p image-studio-desktop --test single_instance -- --ignored --test-threads=1`）：2 项通过，1.94 s
  - 真实二进制双开：第二次 0 退出、第一次存活、无 `startup-failure.log`；
  - 不同根并行启动。
- 质量门：`cargo fmt --all -- --check` 通过；`cargo clippy --workspace --all-targets -D warnings` 通过；
  `cargo test --workspace` 354 passed / 0 failed / 9 ignored；前端 81 passed、构建通过。

## 实现要点（供复核）

- 身份：规范化数据根（`canonicalize` → 反斜杠归一 → 小写）的 SHA-256 前 8 字节；
- 命名对象：`Local\image-studio-<id>`（mutex）、`-activate`、`-ack`（auto-reset event）；
- secondary 打开事件需要 `EVENT_MODIFY_STATE | SYNCHRONIZE`，否则 `WaitForSingleObject` 会立即失败——
  该问题由集成测试在首轮实现中暴露并修复；
- 主实例在 Tauri `setup` 后启动独立线程等待激活请求，`unminimize → show → set_focus` 后
  `SetForegroundWindow`，仍不是前台窗口时 `FlashWindowEx` 兜底，完成后 `SetEvent(ack)`；
- 同根但来自另一登录会话时，`Library::open` 返回 InUse，应用在构建 Tauri 窗口前给出明确提示
  并以 0 退出，不再落入 WebView2 profile 冲突。

## 复现

```powershell
# 构建并安装
& scripts/build-windows-internal.ps1 -SkipQualityGates
Start-Process 'target\release\bundle\nsis\Image Studio_0.1.1_x64-setup.exe' -ArgumentList '/S' -Wait

# 同根激活
& "$env:LOCALAPPDATA\Image Studio\image-studio-desktop.exe" --data-dir C:\ist-verify\r-single-a
& "$env:LOCALAPPDATA\Image Studio\image-studio-desktop.exe" --data-dir C:\ist-verify\r-single-a   # 立即退出 0

# 自动化
cargo test -p image-studio-desktop --lib single_instance
cargo test -p image-studio-desktop --test single_instance -- --ignored --test-threads=1
```

## 结论标签

- **同根第二次启动：通过**（窗口激活、最小化恢复、无响应提示、跨根并行）。
- **启动失败诊断未回归**：真的 WebView2 故障仍是日志 + 对话框 + 退出码 3。
- 本轮不含模型下载、推理、图片生成或性能结论。
