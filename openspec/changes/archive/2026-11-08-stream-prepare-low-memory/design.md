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

## 7. 2026-11-08 修订：子进程分片量化（真正降到 16 GiB 级别）

### 7.1 为什么单靠分片装配不够

§4.2 的原型证明：meta 骨架 + 逐张量 bnb 量化能产出与现有缓存**逐字节一致**的产物，但 commit 峰值只从 19.82 GiB 降到 19.79 GiB。原因是单张 64 MB bf16 张量在 bnb CPU 量化过程中会产生约 1 GiB 的临时内存，且 Windows 不把该内存归还系统——内存峰值由"一次处理的张量 + bnb 临时量"决定，而不由组件总量决定。
因此必须让每一次量化的临时内存**随进程退出被回收**：把量化工作拆到子进程，父进程只做编排与字节拼装。

### 7.2 目标架构

```
worker --prepare（coordinator 进程）
  ├─ 读取 source component 的 config / safetensors 张量清单
  ├─ 按源字节预算分组（默认 512 MiB/组，单并发）
  ├─ 对每组派生 prepare_shard 子进程：
  │    - safe_open(...).get_tensor() 只取本组张量
  │    - bnb.functional.quantize_4bit（NF4，double quant，nf4 quant map）
  │    - 非量化参数（embedding/norm/buffer）直通 BF16
  │    - 写 fragment safetensors（与 bnb save_pretrained 相同的键集合）
  │    - 退出，内存全部归还
  └─ 校验 fragment 键覆盖 ≥ 源张量集合
     → 流式拼装最终 component safetensors（自写 safetensors writer：header + 原字节拷贝，不 materialize 张量）
     → 写 config.json（source config + quantization_config）与 generation_config.json
     → 计算 prepared inventory digest
```

峰值预期：

| 进程 | 峰值来源 | 目标 |
|---|---|---|
| prepare_shard 子进程 | 组内源张量 + bnb 量化临时量（实测约 1 GiB/64 MB 张量） | 512 MiB 组约 2–4 GiB |
| coordinator | 流式拼接缓冲（<256 MiB）+ 张量索引 | <1 GiB |
| 合计 | 串行单子进程，不叠加 | **commit ≤ 10 GiB**（验收实测冻结） |

组大小与并发度作为内部参数（默认 512 MiB / 单并发），按实测下调；并发只为性能，默认不开启以避免峰值叠加。

### 7.3 缓存格式、身份与确定性

现有 prepared 缓存是 bnb `save_pretrained` 的合并 safetensors（已核对 Qwen Turbo text encoder：2590 keys；quantized 键含 `.weight`(U8)、`.absmax`、`.nested_absmax`、`.nested_quant_map`、`.quant_map`、`.quant_state.bitsandbytes__nf4`；非量化参数为 BF16；配套 `config.json` 带 `quantization_config`）。

- 子进程产出的 fragment 必须使用同一键名与 dtype 约定；父进程只做字节级拼装，不做数值变换。
- **优先目标**：拼装结果与现有 prepared 缓存逐字节一致（先以 Qwen transformer 的已知 SHA-256 `533cede7…` 做门禁），此时复用现有缓存身份。
- 若无法逐字节一致：登记新的 prepared 缓存身份（模型清单/缓存摘要不再是旧值），必须给出逐参数比较（抽样 1%，bit-identical 或相对误差 < 1e-6）与相同提示词/种子的输出图 PSNR/目视对比；旧缓存与新缓存不得混用。
- 确定性：每个张量独立量化；fragment 按源张量顺序拼接；header JSON 由固定规则生成。多次运行 SHA-256 必须一致。

### 7.4 预检、回退与失败路径

- 低内存路径预检：**可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB**（实测后冻结；若实测峰值更低/更高则按证据调整）。
- 预检失败：量化前失败，给出可行动错误，不留半成品。
- 子进程失败/超时（单组 1800 s 总预算）：终止整棵树，删除未完成 prepared 目录，保留原快照；记录失败组与峰值。
- 旧整体构造路径保留为降级选项：低内存路径失败且当前内存满足旧门槛（28 GiB/26 GiB）时可显式回退并记录原因。
- 取消：沿用现有受管理进程树回收，子进程必须随 prepare 取消被整树回收。

### 7.5 验收（无 16 GiB 实机时的模拟口径）

本机（32 GiB）没有 16 GiB 实机，采用可复核的模拟：

1. **可用内存模拟**：以独立进程提交并驻留内存，把系统可用物理内存压到约 8 GiB（记录前后 `Get-Counter` 值）；
2. **commit 上限模拟**：prepare 进程放入 Windows Job Object（JOB_OBJECT_LIMIT_PROCESS_MEMORY / commit 限制 ≈10 GiB），超出即失败；
3. 在模拟条件下完成 Qwen Turbo 全新安装 → prepare → 标准档生成；记录 prepare 进程 commit 峰值、可用物理内存最低值、耗时、产物 SHA-256；
4. 拒绝路径：把可用内存压到 ~7 GiB 重跑，确认量化前失败、无半成品；
5. 记录限制：这是模拟证据，不是 16 GiB 实机证据；所有者提供 16 GiB 实机后再补端到端实机记录。

同时在本机正常状态（无模拟）跑一次安装 → 生成，确保行为与旧路径一致。

### 7.6 风险

| 风险 | 缓解 |
|---|---|
| bnb 单张量临时量仍偏大（~1 GiB） | 组按源字节预算切小、单张量极端情况一组一张量；实测后冻结组大小 |
| 子进程启动开销（每组 ~5–10 s，约 20–30 组） | 允许 prepare 总时长 ≤ 现状 2 倍；记录耗时 |
| safetensors 拼装错误/键不完整 | 键覆盖校验 + 与现有缓存逐字节比对 + `from_pretrained` 加载测试 |
| 缓存身份变化导致所有机器需重新 prepare | 优先逐字节一致；否则登记新身份并写迁移说明；不静默混用 |
| 取消/崩溃留下子进程 | 受管理进程树 + Job Object 回收；失败清理 prepared 目录 |
| 模拟口径与真实 16 GiB 有差异 | 证据中明确标注模拟；补实机验证为后续项 |

