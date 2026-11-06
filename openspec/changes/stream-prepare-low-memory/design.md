# Design: stream-prepare-low-memory

## 1. 现状为什么吃满内存

`transformers.from_pretrained(..., quantization_config=BitsAndBytesConfig(load_in_4bit=True), device_map="cpu", low_cpu_mem_usage=True)` 已经是"按 shard 读取"，但它在内存中构造完整模型：每个 bf16 分片被读入后由 bnb 量化成 NF4 参数写回模块，而分片本身、量化临时量与非量化的 bf16 参数（embedding、norm 等）在构造期同时存在。实测 6.4 GB 的 NF4 结果对应 23.1 GiB commit。

## 2. 技术路线：meta 骨架 + 逐 shard 量化

对每个组件：

1. `with accelerate.init_empty_weights(): model = AutoModel.from_config(config)` 建立 meta 骨架（零内存）。
2. 用 transformers 的 `replace_with_bnb_linear(model, quantization_config)` 把 `nn.Linear` 换成 meta 上的 `Linear4bit`（不分配数据）。
3. 解析 `model.safetensors.index.json`，逐个 shard：
   - `safetensors.torch.load_file(shard)` 只加载该分片；
   - 对每个参数：`bnb.functional.quantize_4bit(tensor, blocksize=64, quant_type="nf4", compress_statistics=True)`，把 `data`/`quant_state` 装进对应模块的 `Params4bit`；
   - 非量化参数（embedding、norm、buffer）直接落到 CPU 张量；
   - 释放该分片与临时量（`del` + `gc.collect()`）。
4. `model.save_pretrained(prepared_dir)` 写出 bnb 格式 + config（`quantization_config` 与现有缓存一致）。

预期峰值 ≈ 最大 shard（Qwen 文本编码器约 4–5 GB）+ 该分片的 NF4 副本 + 非量化参数，目标 ≤ 10 GiB。

## 3. 逐架构风险与适配

| 组件 | 主要风险 | 处置 |
|---|---|---|
| Qwen DiT（2 shards） | 结构最简单，先做 | 试点，验证 `save_pretrained` 与加载 |
| Qwen3-VL 文本编码器（4 shards） | 视觉塔、`lm_head` 与 `embed_tokens` 的 tied weights、meta 构造期 `Tensor.item()` | 保留 tied 关系；tied 权重只量化一次；若构造期仍碰 meta 计算，退回 `init_empty_weights(include_buffers=False)` 或显式创建缺失 buffer |
| MOSS transformers 文本编码器（4 shards） | 与 Qwen3 类似的 tied/rope buffer | 同上 |
| MOSS DiT（自定义 `MOSSImageTransformer`） | 自定义类，`replace_with_bnb_linear` 未必识别 | 自己遍历 `named_modules()` 替换 `nn.Linear`；保持 `moss_bnb_shim` 依赖的 `weight.dtype` 语义 |

已知的失败模式（来自 8 GB change 的实测，必须在新实现中避开）：

- `Params4bit.quant_state` 不随模块 `.to()` 迁移 → 新实现只在 CPU 上装配，不搬移模块。
- `device_map="auto"` 需要 `llm_int8_enable_fp32_cpu_offload`，且 Qwen3-VL 构造期对 meta 调用 `Tensor.item()` → 不用 `device_map`，用自建骨架。
- `accelerate.cpu_offload` 在 meta 模型上报 `Cannot copy out of meta tensor` → 不依赖运行时 hook。

## 4. 数值等价

prepared 缓存是派生产物，必须可由固定快照确定性重建。量化本身是按参数独立进行的，理论上逐参数确定性一致；但装配顺序/shard 边界可能改变浮点累加顺序。

验收方式：

- 逐参数比较新缓存与现有缓存的反量化权重（抽样 1% 参数，要求 bit-identical 或相对误差 < 1e-6）；
- 用新缓存跑一次 Qwen Turbo compact Gate 0（768²、8 步、固定种子），与现有缓存输出比较：允许逐像素差异，但必须记录 PSNR 与目视结论；
- 若无法达到数值等价，则保留现有缓存路径作为默认，流式路径作为低内存机器的显式选项，并在文档中写明差异。

## 4.1 已测的配置差异（2026-11-06，本机）

在写实现前先验证"去掉 `device_map="cpu"` 就能省内存"这个最简单的假设：

| 组件 | 加载方式 | commit 峰值 | 加载耗时 | 产物 SHA-256 |
|---|---|---|---|---|
| Qwen transformer | `device_map="cpu"`（现状） | 19.821 GiB | 156–195 s | `533cede7…`（与仓库登记的缓存一致） |
| Qwen transformer | 不传 `device_map` | **14.716 GiB** | 17.6–99 s | `cc2247…`（**与现状不同**） |
| Qwen text encoder | `device_map="cpu"`（现状） | 23.1 GiB | 234 s | — |
| Qwen text encoder | 不传 `device_map` | **33.7 GiB** | 120 s | — |

结论：

1. 去掉 `device_map` 对 DiT 确实省 5.4 GiB 且快得多，但它产出的是**另一份 prepared 缓存**：
   1457 个张量中 452 个不同（含 `absmax` 与 quant_state），不是数值等价。用它就等于换了一个
   量化产物，现有 12 GB 机器的缓存会失配并需要重新 prepare。
2. 对文本编码器，去掉 `device_map` 反而更差（33.7 GiB），所以这不是通用优化。
3. 原始加载方式是**确定性**的：用 `device_map="cpu"` 重建 Qwen transformer 两次，SHA-256 都是
   `533cede7…`，与仓库登记值一致。这意味着新路径必须自己保证同样的确定性，否则无法复用现有缓存。

因此"改一行加载参数"不是本 change 的交付方式；真正的分片流式量化（§2）仍是唯一能到 8 GiB 的路径，
且必须重新登记缓存身份并完成数值等价验证。
## 4.2 分片流式原型（2026-11-06）

按 §2 写了一个原型（meta 骨架 + `replace_with_bnb_linear` + 逐张量 `safe_open` +
accelerate 的 `set_module_tensor_to_device`），在 Qwen transformer 上实测：

| 指标 | 现状加载 | 分片流式原型 |
|---|---|---|
| 产物 SHA-256 | `533cede7…` | **`533cede7…`（逐字节一致）** |
| 参数量 | 3.313 GiB | 3.313 GiB（uint8 NF4） |
| commit 峰值（load） | 19.821 GiB | **19.791 GiB** |
| commit 峰值（save） | — | 19.823 GiB |
| 加载耗时 | 156–195 s | 29–31 s |

原型在功能上是成功的：产出与现有缓存逐字节一致，说明分片装配的语义正确。但**内存没有下降**。
逐张量观测显示 RSS 随张量线性增长，量化后模型参数只有 3.31 GiB，而 `gc.collect()` 后 RSS 仍为
18.16 GiB；单个 3355 万参数张量（bf16 64 MB、NF4 17 MB）会让 RSS 增加约 **1 GiB**。

也就是说，占用不是"模型被整体物化"，而是 **bitsandbytes 在 CPU 上量化的临时开销**（远超参数量本身），
并且这部分内存在 Windows 上不会归还给系统。这解释了为什么换加载器、换分片粒度都压不下去。

结论：分片流式装配可以交付（正确且更快），但**单靠它达不到 8 GiB**。要真正降低安装期峰值，
还需把量化工作按分片拆到子进程（进程退出即释放）或在 bnb 之外换量化实现。这两条都超出本提案当前
的验证范围，需要在确认后单独立项。
## 5. 预检与失败

- 量化前做内存预检：可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB（数值由 tasks 3 实测后冻结）。
- 任一 shard 失败即整体失败：删除未完成的 prepared 目录，保留原快照，给出可行动错误。
- 预检失败不得开始量化；不得留下部分缓存。

## 6. 时间与回滚

- 参考机器上现有 prepare 约 723 s；流式路径预计相近（瓶颈是量化计算而非 IO），允许最长 2 倍。
- 回滚策略：`prepare.py` 保留现有 `_build_component`，通过显式开关或自动降级选择；任一组件流式失败时可用旧路径重试并记录原因。
