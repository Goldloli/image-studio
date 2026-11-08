## MODIFIED Requirements

### Requirement: 大模型分阶段量化装载与资源门槛

对于在目标机上无法整载的模型，Windows 执行 SHALL 采用分阶段执行：文本编码 → 释放并清空缓存 → DiT 采样 → VAE 分块解码；同一时刻只允许一个大组件驻留 GPU，记录独占显存、进程提交与耗时；SHALL 禁止以 WDDM 共享内存回退或页面文件超卖作为通过。分阶段语义 SHALL 同时适用于标准档（12 GB，1024²，`bfloat16`，CUDA reserved ≤ 10.5 GiB）与紧凑档（8 GB，768²，`float16`，CUDA reserved ≤ 6.0 GiB）：紧凑档 SHALL 保持同一"单大组件驻留"语义：文本编码器、DiT 与 VAE 依次独占显存并在阶段间释放；实测表明 bitsandbytes NF4 检查点无法逐层搬移（quant_state 与 meta 张量限制），因此逐层 offload 不作为 compact 实现。每个阶段在进入下一阶段前 SHALL 完成组件释放与 `empty_cache`，并校验 CUDA reserved 已降到该阶段的释放阈值（文本阶段 ≤ 2.5 GiB）；未通过 SHALL 以可诊断错误失败，MUST NOT 通过放宽阈值或跳过检查来继续执行。

#### Scenario: Qwen Turbo 在 12 GB 上生成

- **WHEN** 设备可用显存不足以同时容纳文本编码器与 DiT
- **THEN** 应用先编码提示词并释放编码器，再加载 DiT 采样，再分块解码；资源报告显示独占显存不触发共享回退

#### Scenario: Qwen Turbo 在 8 GB 上生成

- **WHEN** 设备显存 8 GB
- **THEN** 文本编码器按层上卡并在编码后释放，DiT 以 768² 采样，VAE 分块解码；两档使用同一模型快照与同一份 prepared 缓存

#### Scenario: 文本阶段释放后显存归零

- **WHEN** 文本编码与提示词编码在标准档或紧凑档完成
- **THEN** 删除文本编码器、执行 `empty_cache` 后，CUDA reserved 降到 ≤ 2.5 GiB 才进入 DiT 采样
- **AND** 若释放后仍高于阈值，执行以可诊断错误失败并清理整树，不放宽阈值、不跳过检查

#### Scenario: 阶段超预算

- **WHEN** 任一阶段超过本档的显存或提交预算
- **THEN** 执行以可诊断错误失败并清理整树，不提交候选、不伪造资产、不切换到更高硬件冒充通过
