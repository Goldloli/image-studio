## MODIFIED Requirements

### Requirement: 启动前校验 Windows 数据目录路径长度

Windows 桌面 SHALL 在打开 WebView2、创建数据子目录或启动任何安装/下载之前，按**最终**运行时布局（`data/runtimes/<runtime id>` + 最长装载相关文件）校验数据目录绝对路径长度，并在 canonicalize 之后再次校验实际路径；超出支持边界时 SHALL 写入 `<数据根>/logs/startup-failure.log`、展示中文可操作提示并以文档化退出码 3 退出，不得进入下载/解压阶段。

边界由最终布局推导：`data + 10 (\runtimes\) + 61 (冻结 v2 runtime id) + 1 + 86 (最长装载相关相对路径) + 10 (Win32 余量) <= 260`，即数据路径上限为 **92 UTF-16 units**。

#### Scenario: 路径超限拒绝启动
- **WHEN** 用户以 `--data-dir` 指定 data 路径超过 92 UTF-16 units 的 Windows 数据根
- **THEN** 应用拒绝启动，日志与对话框说明当前长度、上限，并建议改用更短目录（如 `C:\ImageStudioData`）
- **AND** 不创建运行时暂存目录、不发起任何网络下载

#### Scenario: 支持范围内路径正常启动
- **WHEN** 数据路径在支持范围内（含默认目录与常规短目录）
- **THEN** 启动、安装、探针与生成行为与加固前一致

#### Scenario: 边界数据根的真实生成
- **WHEN** 数据根 data 路径为 92 units 的边界值
- **THEN** 安装、probe 与真实生成都使用最终运行时路径成功，不出现长路径 DLL 错误

#### Scenario: 别名路径不被绕过
- **WHEN** 数据根通过 junction/8.3 短名指向更长的实际路径
- **THEN** canonicalize 后的实际路径同样参与校验，超限时按超限拒绝

### Requirement: 启动失败诊断覆盖数据目录路径错误

数据目录路径长度的启动失败 SHALL 复用现有启动失败诊断（`startup-failure.log`、原生对话框、退出码 3），且日志 SHALL 写入被拒绝的数据根下的 `logs/`（包括未显式指定 `--data-dir` 的默认根），使 release 构建不出现无解释的退出。

#### Scenario: release 构建的可见失败
- **WHEN** 路径超限发生在 release/无控制台构建
- **THEN** 用户能看到中文原因与修复建议，日志保留在对应数据根的 `logs/startup-failure.log` 供排查
