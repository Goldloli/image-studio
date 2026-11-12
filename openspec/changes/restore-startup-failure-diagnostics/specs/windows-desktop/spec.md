## ADDED Requirements

### Requirement: Panic 钩子链必须同时保留启动失败报告

桌面进程安装的诊断钩子 SHALL 组成单个链，任何后安装的钩子 MUST NOT 使已安装的启动失败钩子失效。链上每个钩子按安装顺序被调用；启动失败钩子命中后以登记退出码结束，不再继续执行后续钩子。

#### Scenario: 日志钩子安装在启动失败钩子之后

- **WHEN** 启动失败报告钩子先安装，日志钩子随后安装，且 Tauri 因 WebView2 或用户数据目录初始化失败而 panic
- **THEN** 进程先写 `logs/app.log` 的 `app.panic` 记录，再写 `logs/startup-failure.log`，显示中文启动失败对话框，并以退出码 3 结束
- **AND** 不会出现只有 101 的静默退出

#### Scenario: 同一数据根的第二个实例

- **WHEN** 一个实例持有数据根，另一个进程用相同 `--data-dir` 启动
- **THEN** 第二个进程按启动失败处理：写 `logs/startup-failure.log`、显示“Image Studio 启动失败”、关闭对话框后以退出码 3 结束
- **AND** 第一个实例的会话、任务、作品与模型安装记录不被修改

#### Scenario: 与启动无关的 panic

- **WHEN** 运行期间发生与 Tauri setup 无关的 panic
- **THEN** 该 panic 除写入 `app.panic` 外，仍按原有默认行为输出回溯，进程不会被执行启动失败流程提前终止
