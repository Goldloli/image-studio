## Purpose

为 Windows 原生桌面增加数据目录路径长度边界与可操作的启动失败提示。

## ADDED Requirements

### Requirement: 启动前校验 Windows 数据目录路径长度

Windows 桌面 SHALL 在打开 WebView2、创建数据子目录或启动任何安装/下载之前，按冻结运行时布局的最坏情况校验数据目录绝对路径长度；超出支持边界时 SHALL 写入 `<数据根>/logs/startup-failure.log`、展示中文可操作提示并以文档化退出码 3 退出，不得进入下载/解压阶段。

#### Scenario: 路径超限拒绝启动
- **WHEN** 用户以 `--data-dir` 指定超过边界（117 字符，UTF-16 code units）的 Windows 数据路径
- **THEN** 应用拒绝启动，日志与对话框说明当前长度、上限，并建议改用更短目录（如 `C:\ImageStudioData`）
- **AND** 不创建运行时暂存目录、不发起任何网络下载

#### Scenario: 支持范围内路径正常启动
- **WHEN** 数据路径在支持范围内（含默认目录与常规短目录）
- **THEN** 启动、安装、探针与生成行为与加固前一致

### Requirement: 启动失败诊断覆盖数据目录路径错误

数据目录路径长度的启动失败 SHALL 复用现有启动失败诊断（`startup-failure.log`、原生对话框、退出码 3），使 release 构建不出现无解释的退出。

#### Scenario: release 构建的可见失败
- **WHEN** 路径超限发生在 release/无控制台构建
- **THEN** 用户能看到中文原因与修复建议，日志保留供排查
