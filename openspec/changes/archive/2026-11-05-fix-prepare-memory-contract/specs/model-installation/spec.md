## MODIFIED Requirements

### Requirement: 安装期量化准备使用低内存分片路径

安装期的 NF4 量化准备 SHALL 以**组件**为单位（文本编码器与扩散主干分别）在 CPU 上量化、保存并释放，不得在同一进程内叠住两个组件。预检 SHALL 使用实测冻结的双条件门槛：**可用物理内存 ≥ 28 GiB 或 commit 余量 ≥ 26 GiB**；不满足时 SHALL 在开始量化前失败并给出可行动错误（关闭占用内存的程序或选择更小模型），且不得留下半成品缓存。逐 safetensors 分片的流式量化 MUST NOT 被表述为已交付能力：当前实现会在内存中构造完整组件，实测 commit 峰值为 16.4–23.1 GiB。prepared 缓存 SHALL 与执行档位无关：`standard` 与 `compact` 共用同一份缓存与逐组件摘要，compact 档 MUST NOT 重建第二份。

#### Scenario: 低可用内存机器完成 prepare

- **WHEN** 设备总内存 31.9 GiB、可用物理内存约 19.6 GiB、commit 余量约 60 GiB，用户安装 Qwen-Image 2.1-Turbo
- **THEN** 预检以 commit 余量通过，组件级量化完成，prepared 缓存摘要写入就绪记录；记录 commit 峰值约 23 GiB 且可用物理内存最低可降至接近 0，不得被描述为"低内存分片路径"

#### Scenario: 门槛不足时提前失败

- **WHEN** 可用物理内存低于 28 GiB 且 commit 余量低于 26 GiB
- **THEN** 安装器在量化前报告可行动错误，不留下部分 prepared 文件

#### Scenario: 两个档位共用缓存

- **WHEN** 8 GB 设备已 prepare，随后同一模型在同一安装上以 standard 档执行，或反之
- **THEN** 复用同一份 prepared 缓存，摘要校验通过，不触发重新量化，也不产生第二份缓存目录

#### Scenario: 分片流式量化不得冒充已交付

- **WHEN** 有人声称 16 GiB 内存机器已可 prepare，或引用"可用内存 ≥ 8 GiB"的门槛
- **THEN** 以实测数据反驳：当前实现需要 16.4–23.1 GiB commit，分片流式量化是独立后续项，尚未实现
