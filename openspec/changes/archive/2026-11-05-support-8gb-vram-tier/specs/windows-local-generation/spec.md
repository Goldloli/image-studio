## ADDED Requirements

### Requirement: 8 GB 显存设备使用紧凑档执行

当设备显存满足紧凑档门槛（≥ 8 GB）但不满足标准档门槛时，Windows 生成 SHALL 使用该模型的 compact 配方：画幅 768²（单模型实测超预算时允许登记 512²）、`compute_dtype="float16"`，并保持与标准档相同的"同一时刻只有一个 NF4 大组件驻留"的分阶段策略（实测表明 bitsandbytes 检查点不支持逐层 offload）。compact 执行 SHALL 满足：CUDA reserved ≤ 6.0 GiB、加载 ≤ 600 s、单次生成 ≤ 900 s，且 MUST NOT 触发 WDDM 共享内存回退。若某模型在 512² 下仍无法满足预算，该模型在 8 GB 设备上 SHALL 标记为不可执行并如实说明，不得用降低步数或虚构通过。

#### Scenario: 8 GB 设备生成 Qwen Turbo

- **WHEN** 设备显存 8 GB，用户以紧凑档发送一次 Qwen-Image 2.1-Turbo 文生图
- **THEN** 文本编码器作为单个组件上卡、编码后释放，DiT 以 768² 采样，VAE 分块解码；资源报告显示 CUDA reserved ≤ 6.0 GiB 且无共享内存回退

#### Scenario: 紧凑档超预算

- **WHEN** 某模型 768² 实测超过 6.0 GiB
- **THEN** 该模型登记 512² 重测；仍超预算时 8 GB 设备上的执行入口禁用并显示实测原因，不静默降级到更小画幅或更少步数

#### Scenario: 8 GB 下不可行的型号不登记紧凑档\n\n- **WHEN** 某型号在 8 GB 设备上实测无法满足预算或输出无效（如 Z-Image Turbo 的 fp16 采样产出 NaN、加载峰值约 7.8 GiB）\n- **THEN** 该型号不登记 compact 档，继续按 12 GB 门槛禁用，并在证据中记录失败的实测数据，不得把"已实现 compact 配方"当作可用\n\n#### Scenario: 计算精度不匹配

- **WHEN** compact 配方声明 `compute_dtype="float16"`，而设备或 worker 只能提供其他精度
- **THEN** 请求在推理前被拒绝并给出可行动错误，不用 bf16 软件模拟冒充通过

## MODIFIED Requirements

### Requirement: 大模型分阶段量化装载与资源门槛

对于在目标机上无法整载的模型，Windows 执行 SHALL 采用分阶段执行：文本编码 → 释放并清空缓存 → DiT 采样 → VAE 分块解码；同一时刻只允许一个大组件驻留 GPU，记录独占显存、进程提交与耗时；SHALL 禁止以 WDDM 共享内存回退或页面文件超卖作为通过。该分阶段语义 SHALL 同时适用于标准档（12 GB，1024²，`bfloat16`，CUDA reserved ≤ 10.5 GiB）与紧凑档（8 GB，768²，`float16`，CUDA reserved ≤ 6.0 GiB）：紧凑档 SHALL 保持同一"单大组件驻留"语义：文本编码器、DiT 与 VAE 依次独占显存并在阶段间释放；实测表明 bitsandbytes NF4 检查点无法逐层搬移（quant_state 与 meta 张量限制），因此逐层 offload 不作为 compact 实现。

#### Scenario: Qwen Turbo 在 12 GB 上生成

- **WHEN** 设备可用显存不足以同时容纳文本编码器与 DiT
- **THEN** 应用先编码提示词并释放编码器，再加载 DiT 采样，再分块解码；资源报告显示独占显存不触发共享回退

#### Scenario: Qwen Turbo 在 8 GB 上生成

- **WHEN** 设备显存 8 GB
- **THEN** 文本编码器按层上卡并在编码后释放，DiT 以 768² 采样，VAE 分块解码；两档使用同一模型快照与同一份 prepared 缓存

#### Scenario: 阶段超预算

- **WHEN** 任一阶段超过本档的显存或提交预算
- **THEN** 执行以可诊断错误失败并清理整树，不提交候选、不伪造资产、不切换到更高硬件冒充通过
