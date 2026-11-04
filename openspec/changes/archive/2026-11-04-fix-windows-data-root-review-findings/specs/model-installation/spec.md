## MODIFIED Requirements

### Requirement: 运行时暂存目录使用短名并原子激活

Windows 共享运行时安装 SHALL 使用短暂存目录名 `.staging-<hexpid>-<8位十六进制随机>`（不超过 26 字符）在 `data/runtimes/` 内准备，并在成功后通过同卷原子 rename 激活为**确定性的短暂存目录** `rt-<12 hex>`（由 runtime id 派生，15 字符）；runtime 身份 SHALL 保留在激活记录与 worker 身份中。由旧构建按原始 runtime id 命名的正式运行时目录 SHALL 在启动/读取就绪状态时迁移为短目录（同卷 rename，best effort）。

#### Scenario: 深数据根安装运行时
- **WHEN** 数据根接近支持边界（但仍在范围内）时安装共享 v2 运行时
- **THEN** 运行时安装、激活与 probe 成功，不出现 onnxruntime 长路径 DLL 导入失败

#### Scenario: 边界数据根的真实生成
- **WHEN** 数据根接近上限时进行真实生成
- **THEN** Python 打开的最长运行时模块路径仍在 Win32 限制内，生成成功

#### Scenario: 旧目录迁移
- **WHEN** 数据根中存在旧构建以原始 runtime id 命名的正式运行时目录且短目录不存在
- **THEN** 启动/读取时将其重命名为短目录，已安装模型继续可用；若短目录已存在则不覆盖、不删除旧数据

#### Scenario: 激活失败清理
- **WHEN** 暂存运行时准备或 rename 激活失败
- **THEN** 失败路径不产生部分激活的正式运行时，并尽量清理本次暂存目录

### Requirement: 安装前回收历史暂存目录

运行时安装 SHALL 在取得独占安装锁之后、磁盘空间预检与下载之前，清理 `data/runtimes/` 中的历史残留暂存目录：新格式 `.staging-<hexpid>-<8hex>` 与旧格式 `.{runtime_id}.staging-<hexpid>-<hexnonce>`（两段后缀均为十六进制；旧十进制是十六进制子集）。清理 SHALL 仅作用于普通目录、拒绝符号链接/联接点，且清理失败只记录警告、不阻塞安装。

#### Scenario: 回收失败尝试的残留
- **WHEN** 上一次安装/取消/崩溃留下新格式或旧格式的暂存目录
- **THEN** 下一次安装开始前该残留被删除并释放磁盘
- **AND** 正式运行时目录与当前安装输入不受影响

#### Scenario: 空间预检之前完成回收
- **WHEN** 旧的暂存残留占用了磁盘
- **THEN** 运行时安装取得独占安装锁后立即回收，再执行磁盘空间预检与下载

#### Scenario: 非暂存目录不误删
- **WHEN** `data/runtimes/` 中存在正式运行时目录或不匹配命名规则的目录
- **THEN** 它们不被删除；清理只处理匹配的普通目录

#### Scenario: 清理失败不阻塞
- **WHEN** 某个残留目录无法删除（占用/权限）
- **THEN** 安装继续，日志记录警告，最终结果不因此失败
