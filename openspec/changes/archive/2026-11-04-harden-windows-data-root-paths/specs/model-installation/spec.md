## Purpose

加固 Windows 共享运行时安装的暂存目录命名与残留清理，避免长路径失败与磁盘残留。

## ADDED Requirements

### Requirement: 运行时暂存目录使用短名并原子激活

Windows 共享运行时安装 SHALL 使用短暂存目录名 `.staging-<pid>-<8位十六进制随机>`（不超过 26 字符）在 `data/runtimes/` 内准备，并在成功后通过同卷原子 rename 激活为冻结的正式运行时目录；正式运行时目录名不得为路径长度而改变。

#### Scenario: 深数据根安装运行时
- **WHEN** 数据根接近支持边界（但仍在范围内）时安装共享 v2 运行时
- **THEN** 运行时安装、激活与 probe 成功，不出现 onnxruntime 长路径 DLL 导入失败

#### Scenario: 激活失败清理
- **WHEN** 暂存运行时准备或 rename 激活失败
- **THEN** 失败路径不产生部分激活的正式运行时，并尽量清理本次暂存目录

### Requirement: 安装前回收历史暂存目录

运行时安装 SHALL 在创建新暂存目录前清理 `data/runtimes/` 中匹配 `.staging-` 前缀的历史残留目录；清理 SHALL 仅作用于普通目录、拒绝符号链接/联接点，且清理失败只记录警告、不阻塞安装。

#### Scenario: 回收失败尝试的残留
- **WHEN** 上一次安装/取消/崩溃留下了 `.staging-*` 目录
- **THEN** 下一次运行时安装开始前该残留被删除并释放磁盘
- **AND** 正式运行时目录与当前安装输入不受影响

#### Scenario: 清理失败不阻塞
- **WHEN** 某个 `.staging-*` 目录无法删除（占用/权限）
- **THEN** 安装继续，日志记录警告，最终结果不因此失败
