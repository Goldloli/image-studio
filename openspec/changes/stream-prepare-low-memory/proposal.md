# Proposal: stream-prepare-low-memory

## Why

Windows 的 Qwen/MOSS 模型必须在安装期把 bf16 快照量化为 NF4 并持久化，否则生成阶段无法在 12 GB / 8 GB 卡上执行。当前实现（`studio_diffusers_worker/backends/prepare.py`）用 `from_pretrained(..., quantization_config=..., device_map="cpu", low_cpu_mem_usage=True)` 一次性构造整个组件，实测：

| 组件 | commit 峰值 | 可用物理内存最低 |
|---|---|---|
| Qwen text encoder | 23.1 GiB | 0.003 GiB |
| Qwen transformer | 20.1 GiB | 1.5 GiB |
| MOSS text encoder | 22.7 GiB | 0.3 GiB |
| MOSS transformer | 16.4 GiB | 5.1 GiB |

因此安装器要求"可用物理内存 ≥ 28 GiB 或 commit 余量 ≥ 26 GiB"。2026-11-08 在全新数据根上实测：32 GiB 内存的 Windows 10 目标机（默认 4.8 GiB 页面文件）全新安装 Qwen-Image 2.1 Turbo 被预检拒绝（可用物理 22.6 GiB、commit 余量 24.8 GiB），**新用户装不上该型号**；16 GiB 内存机器更无法安装。

`2026-11-05-support-8gb-vram-tier` 曾把"分片流式量化、可用内存 ≥ 8 GiB"写进主规格，但实现从未做到；`2026-11-05-fix-prepare-memory-contract` 已把规格改回现状并登记本项为后续。所有者于 2026-11-08 要求**真正支持 16 GiB 内存**，本 change 重启并交付它。

## 用户决定

**2026-11-08：项目所有者要求"需要真支持 16G 内存"。** 本提案按该目标重写，等待所有者确认后开始实现；此前 2026-11-06 的暂缓决定（"内存不再优化"）已被本次要求取代。原先的"把 26 GiB 门槛降到 24 GiB"小修提案（`fix-prepare-preflight-threshold`）只覆盖 32 GiB 机器，无法支持 16 GiB，已撤回并合并进本 change。

## 范围

- 新增**子进程分片量化**执行路径：coordinator 读取固定快照，按张量分组（目标每组 ≤ 1 GiB 源字节）派生独立子进程；子进程只加载本组张量、用 bitsandbytes 量化为 NF4、按现有 prepared 缓存格式写片段后退出；coordinator 以流式方式把片段拼装成最终 safetensors 与 config，不在父进程持有完整组件。
- 预检按实测冻结为**可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB**；失败在量化前给出可行动错误，不留半成品。
- prepared 缓存格式保持 `models/<snapshot>.prepared/<component>/{config.json,*.safetensors}`，可被现有 worker 的 `from_pretrained` 路径直接加载；优先做到与现有缓存一致，否则登记新缓存身份并给出数值等价证据。
- 在受限内存条件下（模拟 16 GiB 机器：预留内存使可用 ≈8 GiB + Job Object commit 限制）完成安装 → prepare → 生成端到端验收；有 16 GiB 实机后再补实机证据。
- 旧的整体构造路径保留为降级/回滚选项，默认路径切换为低内存路径。

## 成功标准

- 四个组件（Qwen3-VL 文本编码器、Qwen DiT、MOSS transformers 文本编码器、MOSS DiT）在"可用物理内存 ≥ 8 GiB、commit 余量 ≥ 6 GiB"条件下完成 prepare，实测 commit 峰值 ≤ 10 GiB，无 OOM，时间不超过现有实现的 2 倍。
- 产物可被现有 worker 加载并通过 `_component_ready`；若产物与现有缓存逐字节一致则复用现有身份，否则登记新缓存身份并给出逐参数/输出图质量等价证据。
- 32 GiB 默认页面文件机器与 16 GiB 模拟环境都能完成全新安装；安装失败时不留下半成品缓存。
- 现有 12 GB / 8 GB 档位的证据不被破坏；`openspec validate --all --strict`、全量测试与前端构建通过。

## 不做

- 不改动生成阶段的执行策略、画幅、精度或预算。
- 不支持低于 8 GiB 可用内存的机器。
- 不引入 GGUF/INT8 等新量化格式（如需 ComfyUI 式预量化格式属另一条线）。
- 不重写 worker 协议或 recipe 身份。
- 不解决 Z-Image 的 8 GB 支持（已实测不可行，属另一条线）。
