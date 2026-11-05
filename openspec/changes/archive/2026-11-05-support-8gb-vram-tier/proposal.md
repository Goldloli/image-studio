# Proposal: support-8gb-vram-tier

## Why

当前产品对五个生成模型中每一个都硬性要求 `min_vram_gb=12`、`min_ram_gb=32`（[resources/windows-foundation/model-catalog.json](../../../resources/windows-foundation/model-catalog.json)）。12 GB 这个数字来自唯一认证机（RTX 3080 Ti 12 GiB）：冻结配方按 1024²、BF16 计算、单组件整卡驻留标定，Gate 0 实测峰值为 Z-Image 8.87 GiB、Klein 9.21 GiB、Qwen/Qwen-Turbo 6.00 GiB、MOSS 5.84 GiB，于是产品把 8 GB 设备整体挡在安装与生成之外。

项目所有者的开发机就是 8 GB 设备。现有证据表明 12 GB 不是模型的物理下限，而是当前配方的标定值：Qwen 与 MOSS 的峰值来自**文本编码器整体上卡**，而不是扩散主干采样；Klein 的 9.21 GiB 来自组件级 offload 下的单组件驻留与量化瞬时。把文本编码器改为逐层上卡、并把画幅从 1024² 降到 768² 后，有理由把峰值压到 6.0 GiB 以内（本 change 必须以实测确认，不得只按推理结论放行）。

同时，本机存在两个 8 GB 特有的硬约束，必须在同一 change 内解决：

1. **Turing 无原生 BF16**：本机 RTX 2060 SUPER 的 compute capability 为 7.5，现有 worker 的 `bnb_4bit_compute_dtype`、VAE、transformer 与探针自检全部固定 `torch.bfloat16`，在 sm75 上没有硬件加速路径。
2. **安装期 prepare 的 28 GiB 可用内存预检**：本机总内存 31.9 GiB、实测可用仅 19.6 GiB，当前预检直接失败；而 8 GB 显卡常配 16–32 GiB 内存，这一门槛会把多数目标用户挡在安装阶段。

## 用户决定（2026-11-05）

1. 本项目要支持 8 GB 显存设备；现有 12 GB 配方、身份与证据保持不变。
2. 验收就在当前这台 8 GB 机器上完成，且要求**全流程**：安装、量化准备、真实生成、取消/强杀恢复、界面发送/查看/导出。
3. 五个生成模型**全部都做**，不采用"只放行一个"的临时方案。
4. 允许为 8 GB 档在画幅/精度/装载策略上折衷；默认目标 768²，单模型实测仍超预算时可降到 512²。
5. 不更换模型快照、不引入 GGUF/INT8 新加载器、不改 macOS 历史身份。

## What Changes

- 引入**执行档位**`standard`（12 GB+，沿用现有配方）与 `compact`（8 GB+，新配方），档位由设备显存与计算能力共同决定；同一模型的两个档位共用模型快照、运行时、安装与 prepared 缓存。
- 设备事实新增 `compute_capability`（major.minor）与可用于档位判断的显存事实；未知事实仍然按"未满足"处理。
- 权威目录的硬件门槛从单一 `min_vram_gb/min_ram_gb` 扩展为按档位登记，并投影"当前设备将使用哪个档位"。
- 四个 Windows 生成 worker（diffusers、Z-Image CUDA、以及 compact 下新增的逐层 offload 路径）支持参数化画幅、计算精度与装载策略；`--probe` 的 NF4 自检按档位精度执行。
- 生成 v2 新增五个 compact 配方身份与精确参数映射；`effective_parameters` 仍为精确匹配。
- 安装期 NF4 prepare 增加**低内存分片路径**，预检从"可用 RAM ≥ 28 GiB"改为可实测的更低门槛；prepared 缓存保持与档位无关并可复用。
- 8 GB 档的运行时预算、Gate 0 证据、取消/强杀恢复与界面验收全部记录在 `verification/`；12 GB 档不得回归。
- 文档更新：`doc/` 增加 8 GB 档说明，README 的支持范围明确列出 8 GB 设备。

## Capabilities

### New Capabilities

无。本 change 只修改既有能力。

### Modified Capabilities

- `model-catalog`：硬件门槛按档位登记，目录投影当前设备适用的档位与降级原因。
- `model-installation`：安装期量化准备的低内存分片路径与跨档位共用的 prepared 缓存。
- `windows-local-generation`：8 GB 档的执行策略、资源预算与降级可见性。
- `generation-worker-protocol`：compact 配方身份、参数绑定与探针精度。

## 成功标准

- 五个生成模型在 8 GB 机器上各自完成：安装（空缓存下载）→ prepare → Gate 0 真实生成 → 取消 → 强杀恢复 → 界面发送/查看/导出，证据落在 `verification/`。
- compact 档每次生成实测 CUDA reserved ≤ 6.0 GiB，且不发生 WDDM 共享内存回退；加载 ≤ 600 s、单次生成 ≤ 900 s。
- 本机 `nvidia-smi` 报 8192 MiB 时，目录把这五个模型判为"可使用（紧凑档）"，并清楚展示画幅与预计速度；不再一律禁用。
- 12 GB 及以上设备行为不变：相同配方、相同参数、相同身份；现有 Rust/前端测试与 openspec strict 全绿。
- prepare 在本机可用内存 19.6 GiB 的条件下通过，并把实测预检门槛登记进文档与代码。
- 现有 Real-ESRGAN 与 BiRefNet 的 8 GB 行为不变。

## 不做 / 超出范围

- 不支持 8 GB 以下显存，不承诺核显/共享显存回退。
- 不新增模型、快照、能力数量或运行时版本；不引入 GGUF、INT8、SDNQ 等新量化格式。
- 不做多卡、CPU 推算或云端回退。
- 不为 8 GB 档降低采样步数（步数属模型身份，保持与 standard 一致）。
- 不重写 macOS 身份，不修改历史证据文件。
- 不在本 change 内做性能优化或发布流水线变更。
