## Why

低内存 prepare 落地后，12 GB 标准档 Qwen-Image 2.1 Turbo 首次实际生成失败：

- 错误：`cleanup failed after text: CUDA reserved 5.83 GiB > 2.5 GiB`；
- 与缓存无关：reference（2026-11-04）与低内存重建缓存的 weights 逐字节一致，两者都会触发；
- 根因：标准档文本编码器装载分支（`from_pretrained(prepared, torch_dtype=…)` + `.to("cuda:0")`）在前向后无法通过 `empty_cache` 释放 CUDA reserved（allocated 已为 0）；紧凑档分支（`device_map="cpu", low_cpu_mem_usage=True` + `.to("cuda:0")`）可以释放到 0.02 GiB。

这是生成路径的既有缺陷，被新的安装能力暴露出来；修复必须让标准档生成可用，否则"16 GiB 机器可安装"没有产品意义。

## What Changes

- `backends/qwen.py` 的标准档文本编码器装载改用 `device_map="cpu", low_cpu_mem_usage=True` 的装载方式（与紧凑档一致），其余编码/采样逻辑不变。
- 保持分阶段释放检查（text ≤2.5 GiB reserved）不变，不通过放宽检查掩盖问题。
- 更新 `windows-local-generation` 规格：增加"阶段释放后 reserved 必须低于阶段阈值"的场景，明确标准档与紧凑档共用该合同。

## Capabilities

### Modified Capabilities

- `windows-local-generation`：在"大模型分阶段量化装载与资源门槛"要求中补充阶段释放检查场景（标准档文本阶段结束后 released reserved ≤2.5 GiB 才进入采样）。

## Impact

- 代码：`tools/diffusers-worker-windows/studio_diffusers_worker/backends/qwen.py`。
- worker wheel 重建（identity 变化）→ 需要重新安装运行时；与低内存 prepare 验收同一次重装。
- 验证：本机 12 GB Qwen Turbo 标准档生成 + 既有 8 GB 紧凑档证据保留。
