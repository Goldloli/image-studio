# Proposal

## Why

Windows 上用户从开始菜单、桌面快捷方式或安装完成页再次启动 Image Studio 时，如果同一数据根已有实例在运行，当前行为是：

1. 第二个进程照常创建宿主，打开同一个 `cache/webview2` 用户数据目录；
2. WebView2 因 profile 被占用而创建失败，Tauri setup panic；
3. 修复 `9600557` 之后至少不再静默：会写 `logs/startup-failure.log`、弹“Image Studio 启动失败”、以退出码 3 结束。

但对普通用户来说，这仍然是“应用打不开”，而且错误信息是 WebView2 的
`HRESULT(0x8007139F)`，与真实原因“已经有一个窗口在运行”无关。2026-11-12 本机功能回归
（`verification/windows-functional-regression-20261112`，I03/F1）复现并记录了该路径。

现有规格只要求“返回明确占用原因”，没有要求把已有窗口交还给用户；本 change 把这一体验补齐。

## What Changes

- 启动时按**数据根**建立单实例身份：同一数据根的第二次启动不创建宿主、不打开数据库、不创建第二套 WebView2 profile。
- 第二次启动向已运行实例发送激活请求：还原最小化窗口、置于前台并聚焦；成功后以退出码 0 退出。
- 若已运行实例在约定时限内未确认激活（无响应，或它属于另一个 Windows 会话、命名对象不可见），第二次启动显示明确的中文原生提示后以 0 退出，不把 WebView2 错误当成原因。
- 数据根仍被占用但未走激活路径时（例如跨会话），在创建 Tauri 窗口前以“数据目录正在由另一个 Image Studio 使用”说明并退出，避免再次落入 WebView2 panic。
- 不同 `--data-dir` 的并行运行保持不变；真正的 WebView2/运行时故障仍走既有启动失败诊断（`startup-failure.log` + 对话框 + 退出码 3）。

## 成功标准

- 同一数据根第二次启动：已有窗口被还原并置于前台，第二个进程 1.5 s 级时间内以 0 退出；不产生 `startup-failure.log`，数据库 WAL 不增长。
- 已运行实例最小化时，第二次启动后窗口恢复可见。
- 两个不同 `--data-dir` 的实例可同时运行，各自拥有 `cache/webview2`。
- 结束第一个实例后再启动可正常创建新实例；无残留锁。
- WebView2 故障注入（`WEBVIEW2_BROWSER_EXECUTABLE_FOLDER` 指向不存在目录）仍得到日志 + 对话框 + 退出码 3。

## Non-Goals

- 不引入托盘、开机自启或“退出时最小化到托盘”。
- 不做跨 Windows 会话（不同登录用户）的窗口激活；该场景只要求明确提示。
- 不改变 macOS 行为（macOS 的 `.app` 双开语义与数据根隔离不在本 change 内）。
- 不改数据库、模型运行时、生成链路或安装包内容。
