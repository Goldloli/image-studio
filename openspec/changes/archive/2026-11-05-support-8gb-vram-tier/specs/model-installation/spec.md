## ADDED Requirements

### Requirement: 安装期量化准备使用低内存分片路径

安装期的 NF4 量化准备 SHALL 按 safetensors 分片逐个加载、量化、保存并释放，峰值内存 MUST NOT 继续要求"可用 RAM ≥ 28 GiB"。预检 SHALL 使用实测后冻结的分段门槛（目标：可用内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB）；不满足时 SHALL 在开始量化前失败，给出可行动错误，且不得留下半成品缓存。prepared 缓存 SHALL 与执行档位无关：`standard` 与 `compact` 共用同一份缓存与逐组件摘要，compact 档 MUST NOT 重建第二份。

#### Scenario: 低可用内存机器完成 prepare

- **WHEN** 设备总内存 31.9 GiB、可用内存约 19.6 GiB，用户安装 Qwen-Image 2.1-Turbo
- **THEN** 预检通过，分片量化完成，prepared 缓存摘要写入就绪记录，过程中不发生 OOM

#### Scenario: 门槛不足时提前失败

- **WHEN** 可用内存低于冻结门槛
- **THEN** 安装器在量化前报告可行动错误（关闭占用内存的程序或选择更小模型），不留下部分 prepared 文件

#### Scenario: 两个档位共用缓存

- **WHEN** 8 GB 设备已 prepare，随后同一模型在同一安装上以 standard 档执行，或反之
- **THEN** 复用同一份 prepared 缓存，摘要校验通过，不触发重新量化，也不产生第二份缓存目录
