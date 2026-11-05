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

## 5. 预检与失败

- 量化前做内存预检：可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB（数值由 tasks 3 实测后冻结）。
- 任一 shard 失败即整体失败：删除未完成的 prepared 目录，保留原快照，给出可行动错误。
- 预检失败不得开始量化；不得留下部分缓存。

## 6. 时间与回滚

- 参考机器上现有 prepare 约 723 s；流式路径预计相近（瓶颈是量化计算而非 IO），允许最长 2 倍。
- 回滚策略：`prepare.py` 保留现有 `_build_component`，通过显式开关或自动降级选择；任一组件流式失败时可用旧路径重试并记录原因。
