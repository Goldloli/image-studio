## MODIFIED Requirements

### Requirement: 安装期量化准备使用低内存分片路径

安装期的 NF4 量化准备 SHALL 在受限内存下执行，使可用物理内存 ≥ 8 GiB 的机器（典型为 16 GiB 内存 + 8 GB 显存的 Windows 机器）可以完成 Qwen/MOSS 安装。量化工作 SHALL 拆到独立子进程（或等价的隔离执行单元）中执行：任一进程在同一时刻 MUST NOT 持有完整 bf16 组件；每个子进程只加载本次负责的张量、量化为 NF4、写出与现有 prepared 缓存格式兼容的产物后退出，其内存随进程退出归还系统。coordinator SHALL 以不物化完整组件的方式（字节级流式拼装）生成最终 safetensors，并写入逐组件 inventory 摘要。预检 SHALL 使用实测冻结的门槛：**可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB**；不满足时 SHALL 在开始量化前失败并给出可行动错误，且不得留下半成品缓存。产出的 prepared 缓存 SHALL 与现有格式兼容并可被现有 worker 加载；优先与既有缓存语义等价（逐字节一致或给出数值等价证据），否则 MUST 登记新的缓存身份并禁止与旧缓存混用。旧的整体构造路径 MAY 保留为降级选项，但 MUST NOT 作为默认路径。

#### Scenario: 低可用内存机器完成 prepare

- **WHEN** 设备总内存 31.9 GiB、可用物理内存约 19.6 GiB、commit 余量约 60 GiB，用户安装 Qwen-Image 2.1-Turbo
- **THEN** 预检以 commit 余量通过，组件级量化完成，prepared 缓存摘要写入就绪记录；低内存路径下的 commit 峰值按实测记录（旧整体构造路径约 23 GiB、可用物理内存最低可降至接近 0）

#### Scenario: 默认虚拟内存的 32 GiB 机器完成首次安装

- **WHEN** 32 GiB 内存、Windows 自动管理页面文件约 4.8 GiB（commit 余量约 24 GiB）的机器首次安装需要量化准备的型号
- **THEN** 低内存路径预检通过（commit 余量 ≥ 6 GiB），安装完成且不再要求用户调整虚拟内存

#### Scenario: 16 GiB 内存机器完成 prepare

- **WHEN** 16 GiB 内存的 Windows 机器（可用物理内存约 8–10 GiB、commit 余量 ≥ 6 GiB）安装 Qwen-Image 2.1-Turbo 或 MOSS
- **THEN** 预检通过，子进程分片量化完成，prepared 缓存摘要写入就绪记录
- **AND** 记录的 prepare 峰值 commit ≤ 10 GiB、无 OOM、耗时不超过旧实现的 2 倍
- **AND** 证据明确标注是 16 GiB 实机还是"预留内存 + commit 限制"的模拟口径

#### Scenario: 门槛不足时提前失败

- **WHEN** 可用物理内存低于 8 GiB 或 commit 余量低于 6 GiB
- **THEN** 安装器在量化前报告可行动错误，不留下部分 prepared 文件

#### Scenario: 缓存身份与数值等价

- **WHEN** 低内存路径产出的 prepared 缓存与既有缓存比较
- **THEN** 若逐字节一致则复用既有缓存身份；否则登记新身份，并给出逐参数比较（抽样 1%）与相同提示词/种子的输出图对比证据
- **AND** 新旧缓存不得在同一安装中被混用

#### Scenario: 两个档位共用缓存

- **WHEN** 8 GB 设备已 prepare，随后同一模型在同一安装上以 standard 档执行，或反之
- **THEN** 复用同一份 prepared 缓存，摘要校验通过，不触发重新量化，也不产生第二份缓存目录

#### Scenario: 分片流式量化不得冒充已交付

- **WHEN** 有人声称 16 GiB 内存机器已可 prepare，或引用"可用内存 ≥ 8 GiB"的门槛
- **THEN** 必须以受限内存条件下的实测资源报告（commit 峰值、可用内存最低值、耗时）与端到端安装 → 生成证据支持该声明；仅有实现代码或单元测试不算交付，模拟口径必须明确标注
