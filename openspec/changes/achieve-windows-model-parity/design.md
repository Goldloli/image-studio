# Design: achieve-windows-model-parity

## 1. 目标设备与预算

- 唯一认证设备：Windows 10 22H2 / RTX 3080 Ti 12 GB（sm86）/ 32 GB RAM / i7-13700K / driver 596.49。
- 预算沿用第一轮并新增"独占显存"要求：
  - 加载 ≤ 600 s、单次生成/处理 ≤ 900 s（BiRefNet/Real-ESRGAN 沿用各自产品时限）。
  - CUDA reserved ≤ 10.5 GiB **且不触发 WDDM 共享内存回退**（以 `nvidia-smi` 独占占用与固定显存阈值判定；共享回退即失败）。
  - 进程树 private commit ≤ 24 GiB；系统可用物理 RAM 最低 ≥ 2 GiB。
- 12 GB 目标机上 Qwen/MOSS 的基线实测（[parity Gate 0](../../../verification/windows-parity-gate0/README.md)）：朴素 bf16 + 组件 offload 超预算；本 change 以分阶段量化装载尝试达标，达不到则写硬件门槛。

## 2. 运行时矩阵

| 运行时 ID | 内容 | 服务模型 |
|---|---|---|
| `windows-python31210-torch2100-cu126-bnb0482-v1` | 既有 Z-Image 运行时，保持不变 | Z-Image Turbo |
| `windows-python31210-torch2100-cu126-diff041-transformers53-v2` | Python 3.12.10、torch 2.10.0+cu126、torchvision 0.25.0+cu126、diffusers 0.41.x、transformers 5.3.x/5.17+（以 Gate 0 冻结）、accelerate、bitsandbytes、safetensors、onnxruntime（CPU） | Klein、Qwen 基础/Turbo、MOSS、BiRefNet |
| `realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1` | 官方 ncnn Windows 构建 + x4plus 权重，固定 SHA-256 | Real-ESRGAN |

规则：每个模型绑定唯一运行时与 worker 构建 ID；运行时清单逐文件登记 SHA-256；不允许浮动解析或运行时联网补件。不同模型不得共享"已就绪"状态。

## 3. 分阶段量化装载（核心工程）

对 Qwen/MOSS 的 CUDA worker 采用显式阶段化执行，禁止"整模型 offload、让驱动把显存溢出到共享内存"：

1. **文本阶段**：仅文本编码器（NF4/INT8 量化）驻留 GPU，编码提示词后立即释放并清空缓存。
2. **采样阶段**：仅 DiT（NF4/INT8）驻留 GPU 采样；KV cache 与激活计入显存预算；超限时降低分块/序列并行而不是回退共享内存。
3. **解码阶段**：VAE 以分块/tiling 解码，解码后再释放。
4. 每个阶段记录独占显存、进程 commit、耗时；任一阶段超预算即失败并保留可诊断日志。
5. 量化来源优先级：官方 diffusers 仓库 + bitsandbytes NF4（可控、可逐文件校验）→ 官方发布的量化集（如 Comfy-Org INT8/W4A8，需自研加载器）→ GGUF Q4（需要 GGUF 支持）。若三者都无法在 12 GB 内安全执行，则该型号写硬件门槛（≥16 GB 显存或 ≥64 GB 内存），模型页禁用执行入口。
6. MOSS 使用仓库自带 `pipeline_moss_image.py`（`trust_remote_code`）；必须固定 revision + 文件 SHA-256，并审阅上游代码。

## 4. 模型与身份

| 型号 | Windows 后端/worker | 能力 | 许可 | 备注 |
|---|---|---|---|---|
| FLUX.2 Klein 4B distilled | diffusers Flux2 txt2img + edit worker | 文生图、单图编辑 | Apache-2.0 | 单图条件编辑，不做 img2img 伪装 |
| BiRefNet epoch_244 | ONNX Runtime CPU worker | 去背景（RGBA+mask） | MIT | 复用既有双输出事务合同 |
| Real-ESRGAN x4plus | ncnn Vulkan worker（固定 Windows 构建） | 4× 放大 | BSD-3-Clause/MIT | 固定 4×、tile、单 GPU |
| Qwen-Image 2.1 | diffusers QwenImage21Pipeline + NF4 分阶段 | 文生图 | Qwen Research License（非商用） | 40 步、q8/NF4、2048 token 预算 |
| Qwen-Image 2.1-Turbo | 同上，官方 8 步采样曲线 | 文生图 | 同上 | 速度优先 |
| MOSS-Image-SFT | 自定义 pipeline（固定 revision）+ 分阶段 | 文生图 | Apache-2.0 | 50 步/CFG3/负分支跳层；8 步 LoRA 可后续 |

每个型号登记独立 model-variant / recipe / install / runtime / worker ID，扩展有界平台 profile；不重写 Z-Image 与 macOS 历史身份。

## 5. 安装与目录

- 安装清单：ModelScope（Qwen/MOSS 官方源，固定逐文件 revision）与上游官方 release（BiRefNet ONNX、Real-ESRGAN Windows 包、Klein 快照）分开登记；逐文件 bytes/SHA-256 校验，摘要一致才可复用。
- 空间预算：按每个型号的下载 + 目标 + 运行时展开 + 预留计算；沿用第一轮空间预检（含真实小卷验证结论），安装前失败、不激活、不删除已有数据。
- 目录/模型页：新增 `min_vram_gb`、`min_ram_gb`、`quantization`、`license`、`windows_status` 等元数据；低于门槛时禁用安装/生成入口并展示原因。
- 缓存与离线：沿用禁网缓存安装验收（本 change 重新执行），不使用在线兜底。

## 6. Worker 与生命周期

- 新 worker 合同延续 generation v2 的分帧/有界日志/取消令牌/租约，仅新增模型身份与阶段事件；诊断 worker 与监督器不改协议。
- 取消与强杀：每个新 worker 必须可被合作取消、Job 整树回收、宿主/监督强杀恢复；不得在清理确认前允许下一任务。
- 事务：图片候选只在完整校验 + 资产提交后成功；BiRefNet 双输出原子提交；Real-ESRGAN 输出摘要登记。

## 7. UI 与功能投影

- 功能页只展示真实可执行能力；Windows 目录投影七个型号与八项能力，状态区分为"已接入/已安装/运行时就绪/可执行/设备满足"。
- 模型页展示许可与硬件门槛；设置页运行环境展示 v1/v2 运行时与探测结果。
- 历史记录、旧草稿与任务中心保持只读兼容，不跨平台隐式重试。

## 8. 风险与备选

- **R1 32 GB RAM 不足以安全装载 Qwen/MOSS**（已观察）：分阶段装载优先；仍失败则硬件门槛（用户已授权）。
- **R2 12 GB 显存 + 量化质量/速度折衷**：Gate 0 逐模型决定量化方案与预算，必要时限制分辨率/步数（需用户确认）。
- **R3 diffusers/transformers 快速演进**：冻结版本与清单；升级走新 change。
- **R4 上游许可**：Qwen 非商用已由用户接受并在 UI 标注；其他模型许可逐项审计。
- **R5 第三方 custom pipeline 代码安全**：固定 revision、审阅、禁止运行时下载代码。
- 备选：ComfyUI 式 INT8/W4A8 加载器（社区 12 GB 证据）作为 R1 失败时的实现路径；GGUF 作为最后备选。

## 9. 验收策略

- 每个型号：Gate 0（加载/单次资源/质量样图）→ 安装（空缓存/离线）→ 真实生成/处理 → 取消/强杀恢复 → 原生 UI 发送/查看/导出 → 汇总证据。
- 平台：Windows 目标机（主）；Mac 回归由用户验收（R02–R04 已完成模式沿用）。CI/release 另立 change。

## 10. Gate 0 实测与执行策略决定（2026-11-03 补充）

### 10.1 逐模型实测与决定

| 型号 | 实测路径 | CUDA reserved | 进程 commit | 可用 RAM | 结论 |
|---|---|---|---|---|---|
| Z-Image Turbo | v1 运行时 NF4（第一轮 Gate 0） | 8.87 GiB | 23.69 GiB | — | 既有 v1 保持不变 |
| BiRefNet | ONNX Runtime CPU，无 GPU | 0 | 10.14 GiB | 15.52 GiB | 通过，CPU 配方 |
| Real-ESRGAN | ncnn Vulkan 4×（220→880、97→388、NASA 384→1536） | <1 GiB | — | — | 通过，固定 4×/tile128/单 GPU |
| Qwen-Image 2.1 | staged NF4 + prepared cache，40 步 | 6.00 GiB | 13.96 GiB | 16.08 GiB | 通过，staged |
| Qwen-Image 2.1-Turbo | staged NF4 + prepared cache，8 步 | 6.00 GiB | 13.95 GiB | 17.32 GiB | 通过，staged |
| MOSS-Image-SFT | staged NF4 text encoder + NF4 DiT（shim）+ fp32 VAE，50 步/CFG3/skip22 | 5.84 GiB | 9.20 GiB | 21.47 GiB | 通过，staged |
| FLUX.2 Klein | NF4 + 组件 CPU offload（每生成独立新进程），4 步 | 9.21 GiB | 19.45 GiB | 15.30 GiB | 通过，NF4+offload（同进程叠加 bf16+NF4 的 combined 结果 31.75/39.11 GiB 作为失败证据保留） |

证据：`verification/windows-parity-gate0/`（各 staged-loader-*.md、klein-gate0.json、birefnet-gate0.json）、`verification/windows-parity-realesrgan/`、`verification/windows-parity-runtime/`。

### 10.2 决定

1. **Qwen 基础/Turbo 与 MOSS 采用分阶段装载 + 本地 prepared 量化缓存**：install 的 prepare 步骤在 CPU 上把固定上游快照量化并持久化（Qwen：text encoder 6.38 GB + transformer 3.67 GB；MOSS：text encoder 5.85 GB + DiT 2.62 GB），generation 逐阶段加载 prepared 组件，文本编码 → 释放 → DiT 采样 → 释放 → VAE 解码，只有一个大组件驻留 GPU。**Klein 采用 NF4 + 组件 CPU offload**：每次生成在独立 worker 进程中装载一次（`enable_model_cpu_offload`），实测 CUDA 9.21 GiB / commit 19.45 GiB / 可用 RAM 15.30 GiB 全部达标；必须在专用新进程中运行，禁止在同一进程里叠加装载第二种策略（combined 实验因此失败：bf16 commit 31.75 GiB、NF4 commit 39.11 GiB/可用 RAM 0.19 GiB，该失败证据保留在 `verification/windows-parity-gate0/klein-gate0-combined-process-bf16-nf4.json`）。
2. **量化与失败证据**：Qwen 基础/Turbo 用 NF4 text encoder + NF4 transformer；Klein 用 NF4 text encoder + NF4 transformer + bf16 VAE；MOSS 用 NF4 text encoder + NF4 DiT；MOSS DiT 的 INT8 与 NF4 都因上游 custom pipeline 从 `weight.dtype` 取计算精度而失败（int8/uint8），因此产品 worker 必须携带 `tools/windows-parity-staged/moss_bnb_shim.py` 记录的 bf16 计算精度兼容补丁（固定上游 revision，审阅记录见 `verification/windows-parity-gate0/moss-pipeline-review.md`）。
3. **prepare 属于安装阶段**：量化 transient（Qwen text encoder commit 23.38 GiB / RAM 0.58 GiB；MOSS text encoder 23.00 GiB / 0.83 GiB；Klein 类似）不计入 generation 预算，但安装器必须先做 RAM 预检（建议要求可用物理内存 ≥ 28 GiB 或 commit 余量 ≥ 26 GiB，不足时给出可行动错误而不是冒险 prepare）；prepared cache 体积计入安装空间（Qwen 10.05 GB、MOSS 8.47 GB），且必须由固定上游文件确定性重建、按组件摘要复用。
4. **硬件门槛元数据**：所有 staged 大模型只认证当前目标机（RTX 3080 Ti 12 GB + 32 GB RAM）；模型页登记 `min_vram_gb=12`、`min_ram_gb=32`（prepare 需要）与 `quantization`、`license`；BiRefNet 为 CPU 路线（`min_vram_gb=0`、`min_ram_gb=16`），Real-ESRGAN 为 ncnn Vulkan（`min_vram_gb=4`、`min_ram_gb=8`）。低于门槛时禁用安装/生成并显示原因。
5. **MOSS 许可与安全边界**：Apache-2.0；custom pipeline 固定 25,060 B / sha256 `1dac1aca…`，Windows worker vendored 代码、离线运行，不运行时下载或执行模型目录中的 Python。
6. **Qwen 许可**：Qwen Research License（非商用），模型页、制作记录与目录必须标注。

### 10.3 未决与后续

- Klein 的 fresh-process NF4+offload 证据（独立 JSON/输出图）待 gate0-transfer 落库；在证据落库前不得声称 Klein Gate 0 通过。
- prepared cache 的确定性重建、摘要复用与空间预算细节在 tasks 3.2/3.4 落地。
- Real-ESRGAN 未采峰值 VRAM（进程 2–5 秒，只有前后快照），按 ncnn 固定合同记录为局限。

## 11. Windows 身份登记（2026-11-03 冻结，实现按此登记）

不修改 macOS 历史 ID；Windows 使用独立 model-variant / capability / recipe / snapshot / runtime / worker 身份。Z-Image 的既有 Windows 身份 `z-image-turbo-diffusers-cuda-nf4-v1` 保持不变。

| 型号 | model_variant_id | capability_id | recipe_id | model_snapshot_id | runtime_lock_id | worker build |
|---|---|---|---|---|---|---|
| Klein 文生图 | `flux2-klein-4b-distilled-windows-v1` | `image.text-to-image.flux2-klein-4b.windows-v1` | `klein4b-text-windows-v1` | `flux2-klein-4b-windows-e7b7dc27f91deacad38e78976d1f2b499d76a294` | `windows-python31210-torch2100-cu126-diff041-transformers53-v2` | `studio-diffusers-worker-windows-0.1.0` |
| Klein 单图编辑 | 同上 | `image.edit.flux2-klein-4b.windows-v1` | `klein4b-edit-windows-v1` | 同上 | 同上 | 同上 |
| BiRefNet | `birefnet-general-epoch-244-windows-v1` | `image.remove-background.birefnet-general-epoch-244.windows-v1` | `birefnet-general-cpu-windows-v1` | `birefnet-general-epoch-244-onnx-windows-v1` | `windows-python31210-torch2100-cu126-diff041-transformers53-v2` | `studio-onnx-worker-windows-0.1.0` |
| Real-ESRGAN | `realesrgan-x4plus-windows-v1` | `image.upscale.realesrgan-x4plus.windows-v1` | `realesrgan-x4plus-windows-t128-v1` | `realesrgan-x4plus-ncnn-windows-v1` | `realesrgan-x4plus-ncnn-windows-vulkan-v0.2.0-v1` | `studio-realesrgan-worker-windows-0.1.0` |
| Qwen 基础版 | `qwen-image-2.1-windows-v1` | `image.text-to-image.qwen-image-2.1.windows-v1` | `qwen21-windows-txt2img-nf4-40-v1` | `qwen-image-2.1-windows-staged-d26bb61231c349cf6b7896fa83353113880e1ba3` | `windows-python31210-torch2100-cu126-diff041-transformers53-v2` | `studio-diffusers-worker-windows-0.1.0` |
| Qwen Turbo | `qwen-image-2.1-turbo-windows-v1` | `image.text-to-image.qwen-image-2.1-turbo.windows-v1` | `qwen21-turbo-windows-txt2img-nf4-8-v1` | `qwen-image-2.1-turbo-windows-staged-fc13ba1986fcb905854dc8de2f52eee395f49b5b` | 同上 | 同上 |
| MOSS-SFT | `moss-image-sft-windows-v1` | `image.text-to-image.moss-image-sft.windows-v1` | `moss-image-sft-windows-txt2img-nf4-50-v1` | `moss-image-sft-windows-staged-f972bd4b4b9e3852133cc5da954d87bc32cd8fb9` | 同上 | 同上 |

冻结的有效参数（ExecutionPlan.effective_parameters，BTreeMap 精确匹配）：

- Klein 文生图（Windows）：`recipe`, `recipe_version=1`, `seed`, `steps=4`, `guidance=1.0`, `scheduler="flow_match_euler_discrete"`, `token_budget=512`, `thinking=false`, `truncation=false`, `quantization="nf4"`, `load_strategy="nf4-component-offload-v1"`。
- Klein 编辑（Windows）：同上；源图条件由输入角色 `source` 表达，输出单张 1024² RGB。
- BiRefNet（Windows）：`recipe`, `recipe_version=1`, `provider="CPUExecutionProvider"`, `precision="float32"`, `preprocessing="birefnet-1024-rgb-v1"`, `postprocessing="birefnet-logits-sigmoid-v1"`, `intra_op_threads=4`, `inter_op_threads=1`, `execution_mode="sequential"`；双输出 primary/mask。
- Real-ESRGAN（Windows）：`recipe`, `recipe_version=1`, `model="realesrgan-x4plus"`, `scale=4`, `tile=128`, `threads="1:1:1"`, `tta=false`。
- Qwen 基础版（Windows）：`recipe`, `recipe_version=1`, `seed`, `steps=40`, `guidance=1.0`, `sampling="linear"`, `token_budget=2048`, `quantization="nf4"`, `load_strategy="staged-nf4-v1"`。
- Qwen Turbo（Windows）：同上但 `steps=8` 并加 `sampling_schedule="qwen-turbo-model-index-v1"`。
- MOSS（Windows）：`recipe`, `recipe_version=1`, `seed`, `steps=50`, `guidance=3.0`, `sampling="moss_image"`, `token_budget=4096`, `thinking=true`, `negative_prompt=""`, `uncond_skip_layers=[22]`, `quantized_components=["transformer","text_encoder"]`, `text_encoder_precision="nf4"`, `vae_precision="float32"`, `load_strategy="staged-nf4-v1"`。

Klein 的 `quantization`/`load_strategy` 取值以 fresh-process NF4 Gate 0 证据为准；若最终证据改为 bf16 分阶段，则改为 `quantization="bfloat16"` 并更新同一处常量与测试。所有 Windows 型号的 QC 与测试必须同时覆盖：错平台（macOS 身份）拒绝、未知身份拒绝、参数不匹配拒绝。
