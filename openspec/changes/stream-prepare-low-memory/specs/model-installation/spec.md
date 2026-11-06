## MODIFIED Requirements

### Requirement: 安装期量化准备使用低内存分片路径

安装期的 NF4 量化准备 SHALL 以 safetensors 分片为单位流式执行：在 meta 骨架上逐分片读取、逐参数量化为 NF4、装配后立即释放该分片，任一时刻不得在内存中持有完整 bf16 组件。预检 SHALL 在实测后冻结为**可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB**（数值以实测为准）；不满足时 SHALL 在开始量化前失败并给出可行动错误，且不得留下半成品缓存。产出的 prepared 缓存 SHALL 与现有缓存语义等价，可由同一固定快照确定性重建，并写入逐组件摘要。旧的整体构造路径 MAY 保留为降级/回滚选项，但不得作为默认路径。

#### Scenario: 低可用内存机器完成 prepare

- **WHEN** 设备可用物理内存 8 GiB、commit 余量 6 GiB，用户安装 Qwen-Image 2.1-Turbo
- **THEN** 预检通过，分片流式量化完成，prepared 缓存摘要写入就绪记录，过程中 commit 峰值不超过冻结上限且不发生 OOM

#### Scenario: 门槛不足时提前失败

- **WHEN** 可用物理内存或 commit 余量低于冻结门槛
- **THEN** 安装器在量化前报告可行动错误，不留下部分 prepared 文件

#### Scenario: 两个档位共用缓存

- **WHEN** 8 GB 设备已 prepare，随后同一模型在同一安装上以 standard 档执行，或反之
- **THEN** 复用同一份 prepared 缓存，摘要校验通过，不触发重新量化，也不产生第二份缓存目录

#### Scenario: 分片流式量化不得冒充已交付

- **WHEN** 有人声称 16 GiB 内存机器已可 prepare，或引用"可用内存 ≥ 8 GiB"的门槛
- **THEN** 必须以参考机器上的实测资源报告与端到端安装证据支持该声明；仅有实现代码或单元测试不算交付
