# Proposal

## Why

2026-11-12 的 Windows 11 本机功能回归（`verification/windows-functional-regression-20261112`，I03）在
`51d56f1` 上复现：应用已经用 `C:\ist-verify\r-works` 运行时，再以同一 `--data-dir` 启动第二个实例，
进程在 849 ms 内以**退出码 101** 结束，stdout/stderr 为空，`logs/startup-failure.log` 不存在，
也没有原生错误对话框；只有 `app.log` 里一条 `WARN app.panic`。

这与 `openspec/specs/windows-desktop` 的“启动失败可解释且不静默退出”直接冲突，并使 2026-11-03
验收过的 W07 行为（日志 + 中文对话框 + 退出码 3）在 WebView2/profile 初始化失败路径上失效。
用户会看到双击图标后没有任何反应。

根因在钩子所有权，不是诊断代码缺失：

1. `apps/desktop/src/host.rs:713` 调用 `startup::install_panic_hook`，安装“setup panic → 写
   `startup-failure.log` + 对话框 + 退出码 3”的钩子；
2. `apps/desktop/src/host.rs:736` 的 `HostState::new` 又用 `std::panic::set_hook` 安装只写日志的
   钩子（`0a274e8 feat(desktop): log panics and generation-time memory` 引入）；
3. `set_hook` 是覆盖而不是叠加，`app.run` 期间的 Tauri setup panic 只经过日志钩子，退出码为默认 101。

显式路径不受影响：超长数据目录仍在 `host.rs` 内直接调用 `startup::fail_startup`，2026-11-12 复验
（S03）通过。

## What Changes

- 日志 panic 钩子改为**包装已有钩子**：先写 `app.panic`，再把同一个 panic 交给之前安装的钩子。
- `HostState::new` 使用该包装函数，不再直接 `set_hook` 覆盖。
- 启动失败钩子与日志钩子的先后顺序不再影响结果；setup panic 仍走
  `startup::fail_startup`（写 `startup-failure.log`、弹原生对话框、退出码 3），其他 panic 仍交给
  默认钩子并额外留下日志行。

## 成功标准

- 同一数据根启动第二实例：`logs/startup-failure.log` 写出原因、时间、日志路径与建议；中文
  “Image Studio 启动失败”对话框出现；关闭后退出码 3。
- 不同数据根的两个实例仍可同时运行，各自拥有 `cache/webview2`。
- 工作区测试与前端测试通过；现有启动失败单元测试不回归。
- 功能回归文档 I03 由 fail 改为 pass，F1 关闭。

## Non-Goals

- 不新增单实例锁或“第二实例只激活首实例”的产品行为：本轮只恢复“拒绝可解释”，保持每个数据根
  独立所有权。
- 不改退出码约定、不改 `startup.rs` 的日志格式与建议文案。
- 不处理本轮记录的其它观察（F2 stub 二进制随包、F3 功能操作无日志、F4 对话框脚本）。
