# Design: support-8gb-vram-tier

## 1. 目标设备与预算

- **验收设备（本机）**：Windows 11 Enterprise 22631 / i9-9900K / RTX 2060 SUPER 8192 MiB（**sm75 Turing**，driver 560.94，CUDA 12.6）/ 31.9 GiB RAM / C: 58.8 GB 空闲、D: 499.4 GB 空闲。
- **8 GB 预算**：总量 8192 MiB 减去桌面与 WebView2 常驻（实测 1910 MiB，按 2048 MiB 规划）后约 **6.0 GiB** 可用；compact 档要求 **CUDA reserved ≤ 6.0 GiB**，并为显示合成保留 ≥ 1.5 GiB，仍禁止把 WDDM 共享内存回退当作通过。
- **时间预算**沿用既有：加载 ≤ 600 s、单次生成/处理 ≤ 900 s。
- **12 GB 档（standard）预算不变**：CUDA reserved ≤ 10.5 GiB、进程 commit ≤ 24 GiB、可用 RAM ≥ 2 GiB。

## 2. 档位模型与选择规则

档位是**模型条目的执行维度**，不是新的模型型号：

| 档位 | 显存条件 | 画幅 | 计算精度 | 装载策略 |
|---|---|---|---|---|
| `standard` | ≥ 条目登记的 standard 门槛（现有 12 GB） | 1024² | bfloat16 | 现有 staged / 组件 offload |
| `compact` | ≥ 8 GB 且 < standard 门槛 | 768²（单模型实测超预算时降到 512²） | float16 | 逐层 offload / staged + 顺序装载 |

选择规则：

1. 设备事实未知（无 `nvidia-smi`、显存读不到）时，任何档位都不可执行，保持现有"无法确认"行为。
2. 设备显存满足 standard 门槛 → 选 `standard`，行为与现在完全一致。
3. 满足 compact 门槛但不满足 standard → 选 `compact`，目录与生成请求都使用 compact 配方。
4. 都不满足 → 不可执行，并显示两档中门槛最低的那条原因。

身份规则：`model_variant_id`、`capability_id`、`model_snapshot_id`、`runtime_lock_id`、worker build 全部不变；**只有 `recipe_id` 与 `effective_parameters` 随档位不同**。这样同一模型只需要一次 25–33 GB 下载和一份 prepared 缓存。

## 3. 逐模型 compact 策略

现有峰值来自 [verification/windows-parity-gate0/](../../../verification/windows-parity-gate0/) 在 12 GB 卡上的实测；下表 compact 列为**待实测的工程目标**，不是已通过结论。

| 模型 | 现有峰值 | 峰值来源 | compact 手段 | 目标峰值 | 风险 |
|---|---|---|---|---|---|
| Qwen-Image 2.1-Turbo | 6.00 GiB | 文本编码器整体上卡（6.38 GB NF4） | 768² 采样 + fp16 + 分阶段释放 | **实测 5.961 GiB** | 已通过 |
| Qwen-Image 2.1 | 6.00 GiB | 同上（40 步） | 同上 | 待实测 | 中 |
| MOSS-Image-SFT | 5.84 GiB | 文本编码器 + fp32 VAE 解码 | 768² + fp16 + VAE 分块 | 待实测 | 中 |
| FLUX.2 Klein 4B | 9.21 GiB | 组件级 offload 下的单组件驻留与量化瞬时 | prepared 预量化 + 顺序 offload + 768² | ≤ 6.0 GiB | 中 |
| Z-Image Turbo | 8.87 GiB | v1 运行时 NF4 全驻留 | 768²/512² + fp16 实测均失败（NaN 与加载峰值 7.8 GiB） | **不可行** | 已排除 |

实现要点：

- **Qwen / MOSS**：compact 沿用"单大组件驻留"的分阶段执行，但画幅降到 768²、计算精度改为 fp16。**2026-11-05 实测否定了逐层 offload**：bnb 检查点的 `Params4bit.quant_state` 不随 `.to()` 搬移（`mat2 is on cpu`），`device_map="auto"` 需要 `llm_int8_enable_fp32_cpu_offload` 而模型构造期会对 meta 张量调用 `Tensor.item()`，`accelerate.cpu_offload` 随后报 `Cannot copy out of meta tensor`。整块搬移在 8 GB 卡上实测 CUDA reserved **5.961 GiB**，落在 6.0 GiB 预算内。
- **Klein**：`enable_model_cpu_offload` 仍是组件级；compact 改用逐层 offload，并复用安装期 prepared 量化缓存消除"bf16 分片在卡上量化"的瞬时。Klein 只有 4 步，逐层搬运的速度代价可以接受。
- **Z-Image**：v1 运行时（`windows-python31210-torch2100-cu126-bnb0482-v1`）在 sm75 上探针通过，但 compact 实测不可行：768² fp16 出 NaN 黑图、512² bf16 无 bnb 内核、512² fp16+fp32 VAE 仍为黑图，且加载峰值恒为 ~7.8 GiB（v1 加载器先把组件物化到卡上再搬回 CPU）。因此本 change **不给 Z-Image 登记 compact 档**，它继续要求 12 GB；协议中的 compact 身份保留给未来硬件。
- 画幅降级只改变画布与 token 数，不改变步数、guidance、调度器与种子语义；输出尺寸随档位变化，`accepts_output` 与输出规格必须按档位校验。

## 4. 计算精度：sm75 没有原生 BF16

- 本机 compute capability = 7.5；cuBLAS/cuDNN 的 BF16 GEMM 需要 sm80+。现有 worker 把 `bnb_4bit_compute_dtype`、`torch_dtype`、探针自检全部固定为 `bfloat16`（`tools/diffusers-worker-windows/studio_diffusers_worker/backends/*.py`、`__main__.py` 的 probe）。
- compact 档默认 `compute_dtype="float16"`；逐模型允许 `vae_precision` 取 `float16` / `bfloat16` / `float32`，由 Gate 0 的输出质量（非全黑、非 NaN、与 standard 档同提示词可比较）决定。
- `--probe` 的 NF4 自检必须按档位的 `compute_dtype` 构造张量，不得固定 bf16；探针失败必须给出可行动错误。
- 运行时版本不变：PyTorch 2.10.0+cu126 的构建覆盖 7.5（官方 cu126 架构表为 5.0/6.0/7.0/7.5/8.0/8.6/9.0），因此不需要换运行时，只需在代码里正确选择精度。

## 5. 安装期量化准备的低内存路径

- 现状：prepare 在 CPU 上把 bf16 分片搬到内存再量化，Qwen 文本编码器实测 commit 峰值 23.4 GiB、可用 RAM 0.58 GiB；安装器预检要求可用 RAM ≥ 28 GiB。本机实测可用 19.6 GiB，**当前直接无法 prepare**。
- 新路径：按 safetensors 分片顺序处理——加载单个分片 → 量化该分片内模块 → 保存为 prepared 分片 → 释放；峰值约为"最大分片 + 一个量化副本"，目标 commit ≤ 8 GiB。
- 预检改为两段：`可用 RAM ≥ 8 GiB` 且 `commit 余量 ≥ 6 GiB`；具体数值在任务 6 实测后冻结到代码常量与文档。
- prepared 缓存格式、逐组件摘要与 `ReadyRecord` 校验不变；两个档位共用同一份缓存，compact 不得重建另一份。
- 安装空间预算不变（模型 + 运行时 + prepared cache），不得因新路径重复占用。

## 6. 协议与身份

新增五个 compact 配方（`generation v2`，参数为精确匹配）：

| 能力 | standard recipe | compact recipe |
|---|---|---|
| Qwen 基础版文生图 | `qwen21-windows-txt2img-nf4-40-v1` | `qwen21-windows-txt2img-nf4-40-compact-v1` |
| Qwen Turbo 文生图 | `qwen21-turbo-windows-txt2img-nf4-8-v1` | `qwen21-turbo-windows-txt2img-nf4-8-compact-v1` |
| MOSS 文生图 | `moss-image-sft-windows-txt2img-nf4-50-v1` | `moss-image-sft-windows-txt2img-nf4-50-compact-v1` |
| Klein 文生图 | `klein4b-text-windows-v1` | `klein4b-text-windows-compact-v1` |
| Klein 单图编辑 | `klein4b-edit-windows-v1` | `klein4b-edit-windows-compact-v1` |
| Z-Image 文生图 | `zimage-turbo-windows-nf4-v1`（现有） | `zimage-turbo-windows-nf4-compact-v1` |

compact 参数在现有键之外固定增加：

- `resolution`：`768` 或 `512`（整数，方图）。
- `compute_dtype`：`float16`（compact 默认）。
- `load_strategy`：`staged-nf4-sequential-v1`（Qwen/MOSS/Z-Image）或 `nf4-sequential-offload-v1`（Klein）。
- 逐模型可选的 `vae_precision`（MOSS 现有 `float32` 保持不变，Klein/Qwen 由 Gate 0 决定）。

协议校验仍按 `(model_id, capability_id, recipe_id)` 三元组闭集匹配，参数多一个键或少一个键都拒绝；standard 配方的参数映射**不得改动**。

## 7. 目录与 UI 投影

- 目录条目新增 `tiers`：每档包含 `id`、`minVramGb`、`minRamGb`、`resolution`、`computeDtype`、说明。既有 `minVramGb/minRamGb` 保留为"可执行的最低门槛"（compact 值），另加 `standardMinVramGb` 供展示。
- `hardwareEligible` = 任一档满足；`canInstall` 在有任一档满足时为真；`canUse` 另加"运行时就绪 + 模型就绪"。
- 目录与模型页必须展示当前设备将使用的档位、画幅与精度；设备只满足 compact 时明确写"紧凑档 768²"及原因（例如"显存 8 GB < 12 GB，自动使用紧凑档"）。
- 发送区不得显示不可用档位的参数；参数面板显示的画幅必须与实际请求契约一致。

## 8. 安装与空间

compact 不新增 `InstallProfile`：同一 `installId`、同一快照、同一运行时、同一 prepared 缓存。空间公式沿用现有预算，只需确认 prepared cache 已计入。

## 9. 风险与未决

- **R1（高）** Z-Image 的 v1 运行时 + bnb 0.48.2 在 sm75 的可行性未验证；失败则需迁移到 v2 运行时并登记新运行时身份，工作量与范围扩大。
- **R2（中）** fp16 数值稳定性：Klein 的 FLUX VAE、Qwen VAE 在 fp16 下可能溢出成黑图；逐模型实测决定 `vae_precision`。
- **R3（中）** 768² 仍超 6.0 GiB 的模型降到 512²；若 512² 也超，则该模型在 8 GB 档标记为不可执行并如实说明（不得虚构通过）。
- **R4（中）** 逐层 offload 的速度：Klein 4 步可能仍需数分钟/张；必须在 UI 中说明预计耗时。
- **R5（低）** 下载与 prepare 时间：五个模型合计约 134 GB 下载、四份 prepared 缓存，按本机实测带宽估算 5–8 小时，加实测预计 1–2 个工作日。
- **R6（高）** 12 GB 档回归：standard 参数映射、配方身份与预算不得改动；任何共享代码改动都要用现有测试与证据复核。
- **R7（中）** 8 GB 设备常配 16–24 GiB 内存：本 change 只保证 32 GiB 机器通过；16 GiB 是否可行由 prepare 实测决定，不提前承诺。

## 10. 执行顺序

1. 设备事实（compute capability）、档位选择、目录与 UI 投影（无 GPU 依赖，可先用单测验证）。
2. 协议与 recipe 身份（compact 五条 + Z-Image）与参数映射。
3. prepare 低内存分片路径（先在本机验证可用内存 19.6 GiB 下可完成）。
4. Qwen-Image 2.1-Turbo compact worker + Gate 0（先打通"下载→prepare→生成→度量"整条链路）。
5. MOSS、Qwen 基础版 compact。
6. Klein（含 prepared 预量化与顺序 offload）。
7. Z-Image compact（R1 决定走 v1 改造还是 v2 迁移）。
8. 全流程界面验收：发送、查看、导出、取消、强杀恢复。
9. 质量门、证据汇总、文档与归档。
