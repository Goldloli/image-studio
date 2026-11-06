# Z-Image 在 8 GB (sm75) 上的量化调查（2026-11-06）

Status: 调查记录，不是规格。结论以本机实测为准；未实测的部分明确标注。

## 1. 问题

Z-Image Turbo 在 RTX 2060 SUPER（sm75，8 GB）上，用 bitsandbytes NF4 + fp16 会产出全黑图
（NaN），加载峰值 ~7.8 GiB。此前的四种配置见
[`verification/windows-8gb-tier/README.md`](../verification/windows-8gb-tier/README.md)。

## 2. 上游确认：这是已知的 Turing/Volta 限制

- [Tongyi-MAI/Z-Image issue #15](https://github.com/Tongyi-MAI/Z-Image/issues/15)：用户报告
  V100 / RTX 2060 上 FP16 与"BF16 回退到 FP16"都产出 NaN 黑图，只有 FP8 权重 + 强制
  `manual cast: torch.float32` 才正常。作者给出的两个方向是 **upcast attention/norms 到 FP32**
  与 **clamp 激活**。
- [HF discussion #22](https://huggingface.co/Tongyi-MAI/Z-Image/discussions/22)：标题即
  "fp16 is not working (Z-Image base incompatible with RTX 20 series, RDNA2 and older)"。

因此黑图不是本项目的实现缺陷，而是模型在缺少 BF16 的架构上的数值问题。

## 3. 候选量化方案（按与本栈的适配度）

| 方案 | 与 torch 2.10 / diffusers 0.41 / sm75 / Windows 的适配 | 结论 |
|---|---|---|
| bitsandbytes NF4（现状） | 已接入；sm75 上只能 fp16 计算，bf16 无内核 | 保留给其余四个模型 |
| **SDNQ**（`pip install sdnq`） | diffusers ≥ 0.40 原生支持（本项目 v2 运行时是 0.41）；纯 PyTorch，无 Triton 时回退 eager；INT8 量化矩阵乘需 Turing+ 与 Triton | **最佳候选**，见 §4 |
| GGUF（Q4_K 等） | 需要自研 GGUF 加载器；ComfyUI 生态成熟但不在 diffusers 栈内 | 可行但工程量大，收益与 NF4 接近 |
| torchao int4/int8（diffusers 原生） | int4 weight-only 的 tinygemm 内核面向 sm80+；int8 在 sm75 支持有限 | 优先级低于 SDNQ |
| FP8 | 需要 sm89+ | 不可用 |

## 4. SDNQ 实测（本机，RTX 2060 SUPER 8 GB）

环境：`D:\ist-8gb\venv`（Python 3.11、torch 2.10.0+cu126、diffusers 0.41.0、sdnq 0.2.9，
无 Triton）。加载冻结的 Z-Image 快照，`weights_dtype="uint4"`、`dequantize_fp32=True`、
`use_quantized_matmul=False`、`quantization_device="cpu"`、`enable_model_cpu_offload`。

| 配置 | CUDA reserved 峰值 | 512² 每步耗时 | 结果 |
|---|---|---|---|
| SDNQ uint4，fp16 激活，fp16 attention | **4.385 GiB** | ~2.2 s | NaN 黑图 |
| SDNQ uint4，fp16 + fp32 attention upcast | 4.385 GiB | ~2.2 s | 仍 NaN |
| SDNQ uint4，bf16 激活 | — | — | `No available kernel`（SDPA 在 sm75 无 bf16 内核） |
| SDNQ uint4，fp16 + embedding/conditioning 路径改 FP32（未量化） | 6.832 GiB | ~5.3 s | block0 输入已有限，但整图仍 NaN |
| 同上 + fp32 attention upcast | 6.832 GiB | ~5.3 s | 仍 NaN |

关键定位（forward hook，1 步推理）：

- fp16 时 `layers[0]` 的**输入**已经是 NaN → 溢出发生在 embedding/conditioning 路径
  （`all_x_embedder` / `t_embedder` / `cap_embedder` / `noise_refiner` / `context_refiner`）。
- 把这条路径改成 fp32 后，block0 输入变为有限，但 **NaN 随后仍出现在 30 个主层内部**，
  且 fp32 attention 不足以消除。
- 文本编码器本身在 fp16 下是安全的（hidden abs max ≈ 54）。

**结论**：SDNQ 把显存问题解决了（4.4 GiB），但 Z-Image 的 fp16 数值问题遍布
embedding 与主层（norm/FFN 等），不是单点 upcast 能修的。

### 4.1 溢出点的精确定位

在把 conditioning 路径改成 fp32 后，用逐子模块 hook 扫描 `layers[0]`：

| 子模块 | 输出有限 | abs max |
|---|---|---|
| attention | 是 | 8,624 |
| attention_norm2 | 是 | 5.36 |
| ffn_norm1 | 是 | 56.8 |
| **feed_forward** | **否** | 3.4e38（fp32 上限） |

也就是**溢出发生在 SwiGLU 前馈网络**，不在 attention、也不在 embedding。attention 的 8,624
虽然有限，但已经接近 fp16 上限 65,504。

要把 FFN 救回来只有两条路：把它换成 fp32 计算——但 FFN 权重是量化的，fp32 输入会触发
dtype 不匹配；改成不量化的 fp32 FFN 需要约 9 GB 权重，反而更不可行；或者等上游重做数值。
这就是"8 GB 不可行"的最终证据。
## 5. 可行路线（按代价排序）

1. **主层 FP32 计算 + SDNQ uint4 权重**：已知可出有效图像（fp32 全流程实测有效），
   但 512² 每步约 96 s，9 步约 14 min，超出 900 s 预算；768² 更慢。
2. **selective upcast 到 norm/FFN**：需要逐模块定位（本次只做到"主层内部"），
   若命中，速度可能落在每步 10–30 s，值得一次有边界的调查。
3. **SDNQ INT8 量化矩阵乘**：INT8 GEMM 的 int32 累加本身不溢出，但非矩阵乘算子仍是 fp16（FFN 的 SwiGLU 乘法仍在 fp16）；
   且 Windows 需要手工安装 Triton。未实测。
4. **等上游修复**：issue #15 仍开放。

## 6. 对本项目的建议

- Z-Image 继续按 12 GB 门槛登记（现状），不要为了 8 GB 放松预算或伪造通过。
- 若要做 8 GB 支持，需要独立 change，至少包含：
  把 Z-Image 迁到 v2 运行时（diffusers 0.41 + sdnq）、SDNQ uint4 的安装期量化、
  以及一个可验证的数值方案（主层 fp32 或 selective upcast），并在本机重新采集
  Gate 0 与 900 s 预算证据。
- 其余四个模型不需要 SDNQ：它们在 NF4 + fp16 下已实测 5.436–5.961 GiB 且出图正常。

## 7. 参考

- <https://github.com/Tongyi-MAI/Z-Image/issues/15>
- <https://huggingface.co/Tongyi-MAI/Z-Image/discussions/22>
- <https://github.com/Disty0/sdnq>
- <https://github.com/vladmandic/sdnext/wiki/SDNQ-Quantization>
- <https://huggingface.co/docs/diffusers/main/en/quantization/sdnq>
