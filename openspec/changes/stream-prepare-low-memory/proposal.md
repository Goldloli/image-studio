# Proposal: stream-prepare-low-memory

## Why

Windows 的 Qwen/MOSS 模型必须在安装期把 bf16 快照量化为 NF4 并持久化，否则生成阶段无法在 12 GB / 8 GB 卡上执行。当前实现（`studio_diffusers_worker/backends/prepare.py`）用 `from_pretrained(..., quantization_config=..., device_map="cpu", low_cpu_mem_usage=True)` 一次性构造整个组件，实测：

| 组件 | commit 峰值 | 可用物理内存最低 |
|---|---|---|
| Qwen text encoder | 23.1 GiB | 0.003 GiB |
| Qwen transformer | 20.1 GiB | 1.5 GiB |
| MOSS text encoder | 22.7 GiB | 0.3 GiB |
| MOSS transformer | 16.4 GiB | 5.1 GiB |

因此安装器要求"可用物理内存 ≥ 28 GiB 或 commit 余量 ≥ 26 GiB"。这意味着 16 GiB 内存的 8 GB 显存机器（一类常见的低成本配置）**无法安装**这些模型，即使显存档位已经支持。

`2026-11-05-support-8gb-vram-tier` 曾把"分片流式量化、可用内存 ≥ 8 GiB"写进主规格，但实现从未做到；`2026-11-05-fix-prepare-memory-contract` 已把规格改回现状并明确登记本项为后续。现在需要真正交付它。

## 用户决定

尚无。本提案等待项目所有者确认后开始实现。

## 范围

- 为 Qwen3-VL 文本编码器、Qwen DiT、MOSS transformers 文本编码器、MOSS DiT 各实现一条**分片流式量化**路径：meta 骨架 + 逐 shard 读取 + 逐参数 `quantize_4bit` + `Params4bit` 装配 + 及时释放，峰值目标为"最大单 shard + NF4 结果"。
- 让安装器预检在**实测**后降到可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB（数值以实测为准）。
- prepared 缓存必须与现有实现语义等价：可由同一固定快照确定性重建，逐组件摘要写入就绪记录；如果无法逐字节一致，必须给出数值等价证据（同提示词同种子的输出与现有缓存一致或差异可解释）。
- 在参考 8 GB 机器上重新采集同口径证据（commit / 可用内存 / 时长），并更新 `doc/15-windows-8gb-tier.md` 与 `verification/`。

## 成功标准

- 四个组件的 prepare 在"可用物理内存 8 GiB"约束下完成，实测 commit 峰值显著低于现状（目标 ≤ 10 GiB），时间不劣于现状的 2 倍。
- 用新缓存生成的图像与现有缓存生成的图像在相同提示词/种子下判定为质量一致（允许逐像素差异，但必须有对比证据）。
- 现有 12 GB / 8 GB 档位的 Gate 0 证据不被破坏；`openspec validate --all --strict`、全量测试与前端构建通过。
- 参考机器上完整跑一次安装 → prepare → 生成，证明端到端可用。

## 不做

- 不改动生成阶段的执行策略、画幅、精度或预算。
- 不支持低于 8 GiB 可用内存的机器。
- 不引入 GGUF/INT8 等新量化格式。
- 不重写 worker 协议或 recipe 身份。
- 不解决 Z-Image 的 8 GB 支持（已实测不可行，属另一条线）。
