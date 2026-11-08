## MODIFIED Requirements

### Requirement: 安装期量化准备使用低内存分片路径

安装期的 NF4 量化准备 SHALL 以**组件**为单位（文本编码器与扩散主干分别）在 CPU 上量化、保存并释放，不得在同一进程内叠住两个组件。预检 SHALL 使用实测冻结的双条件门槛：**可用物理内存 ≥ 28 GiB 或 commit 余量 ≥ 24 GiB**（2026-11-04 同机实测 prepare 进程 commit 峰值 23.003 GiB，24 GiB = 峰值 + 约 1 GiB 安全余量；旧的 26 GiB 门槛会把 commit 余量 24.4–24.8 GiB、实测可完成量化的 32 GiB 默认虚拟内存机器误拒）。当 prepared 缓存可复用（模型清单摘要与完整 inventory 摘要匹配）时，安装 SHALL 跳过内存预检与重新量化并直接复用缓存。预检不通过时 SHALL 在开始量化前失败并给出可行动错误（关闭占用内存的程序或选择更小模型），且不得留下半成品缓存。逐 safetensors 分片的流式量化 MUST NOT 被表述为已交付能力：当前实现会在内存中构造完整组件，实测 commit 峰值为 16.4–23.1 GiB。prepared 缓存 SHALL 与执行档位无关：`standard` 与 `compact` 共用同一份缓存与逐组件摘要，compact 档 MUST NOT 重建第二份。

#### Scenario: 低可用内存机器完成 prepare

- **WHEN** 设备总内存 31.9 GiB、可用物理内存约 19.6 GiB、commit 余量约 60 GiB，用户安装 Qwen-Image 2.1-Turbo
- **THEN** 预检以 commit 余量通过，组件级量化完成，prepared 缓存摘要写入就绪记录；记录 commit 峰值约 23 GiB 且可用物理内存最低可降至接近 0，不得被描述为"低内存分片路径"

#### Scenario: 默认虚拟内存的 32 GiB 机器完成首次安装

- **WHEN** 32 GiB RAM、Windows 自动管理页面文件约 4.8 GiB 的机器（可用物理内存约 22.6 GiB、commit 余量 24.4–24.8 GiB）首次安装需要量化准备的型号
- **THEN** 预检以 commit 余量 ≥ 24 GiB 通过，组件级量化完成并写入就绪记录
- **AND** 记录的 prepare 进程 commit 峰值不超过实测最坏值 23.1 GiB，可用物理内存最低值允许接近 0（依赖分页）

#### Scenario: 可复用 prepared 缓存跳过预检

- **WHEN** 目标型号已有匹配的 prepared 缓存（模型清单摘要与 inventory 摘要一致），本次安装只需重建运行时或重新激活，且当前内存余量低于量化门槛
- **THEN** 安装跳过内存预检与重新量化，复用缓存并通过摘要校验，不因内存状态被拒绝

#### Scenario: 门槛不足时提前失败

- **WHEN** 可用物理内存低于 28 GiB 且 commit 余量低于 24 GiB
- **THEN** 安装器在量化前报告可行动错误，不留下部分 prepared 文件

#### Scenario: 两个档位共用缓存

- **WHEN** 8 GB 设备已 prepare，随后同一模型在同一安装上以 standard 档执行，或反之
- **THEN** 复用同一份 prepared 缓存，摘要校验通过，不触发重新量化，也不产生第二份缓存目录

#### Scenario: 分片流式量化不得冒充已交付

- **WHEN** 有人声称 16 GiB 内存机器已可 prepare，或引用"可用内存 ≥ 8 GiB"的门槛
- **THEN** 以实测数据反驳：当前实现需要 16.4–23.1 GiB commit，分片流式量化是独立后续项，尚未实现
