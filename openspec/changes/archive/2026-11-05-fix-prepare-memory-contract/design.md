# Design: fix-prepare-memory-contract

## 1. 实测事实（RTX 2060 SUPER 机器，31.9 GiB RAM，75.9 GiB commit limit）

| 模型 | 组件 | load | save | commit 峰值 | 可用物理内存最低 |
|---|---|---|---|---|---|
| Qwen Turbo | text encoder | 234.43 s | 157.59 s | 23.145 GiB | 0.003 GiB |
| Qwen Turbo | transformer | 155.32 s | 37.48 s | 20.275 GiB | 2.075 GiB |
| Qwen base | text encoder | 177.23 s | — | 23.146 GiB | 0.170 GiB |
| Qwen base | transformer | 161.50 s | — | 20.126 GiB | 1.527 GiB |
| MOSS | text encoder | 139.54 s | — | 22.747 GiB | 0.303 GiB |
| MOSS | transformer | 139.54 s | — | 16.358 GiB | 5.138 GiB |

产物体积与 12 GB 机器一致（Qwen 10,049,397,568 B）。安装器在产品内跑同一步骤时实测 commit 23.126 GiB、可用内存 0.426 GiB。

## 2. 为什么流式分片量化没有实现

- transformers 的 `from_pretrained(..., quantization_config=..., device_map="cpu", low_cpu_mem_usage=True)` 已经是"按 shard 加载"，但它会在内存里构造完整模型并保留 bf16 分片与 NF4 结果的重叠峰值，这正是 23 GiB 的来源。
- bitsandbytes 的 `Params4bit.quant_state` 不随模块 `.to()` 迁移，`device_map="auto"` 需要 `llm_int8_enable_fp32_cpu_offload`，而 Qwen3-VL 构造期会对 meta 张量调用 `Tensor.item()`；`accelerate.cpu_offload` 随后报 `Cannot copy out of meta tensor`（三次尝试均失败）。
- 真正的分片流式量化需要自建 meta 骨架 + 逐 shard 的 `quantize_4bit` + `Params4bit` 装配，并按模型架构分别实现；这是一项独立工程，不是本次修正的范围。

## 3. 结论

规格必须描述"组件级量化 + 双条件预检"，并把分片流式量化登记为未交付的后续项。后续项的目标与入口：

- 目标：可用物理内存 ≥ 8 GiB 的机器（例如 16 GiB 内存 + 8 GB 显存）可以完成 prepare。
- 入口：`prepare.py` 的 `_build_component` 与 `installation.rs` 的预检常量；实现后必须重新采集上表同口径的峰值证据，并更新本要求与 `verification/windows-8gb-tier/`。
