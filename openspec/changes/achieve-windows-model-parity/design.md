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
