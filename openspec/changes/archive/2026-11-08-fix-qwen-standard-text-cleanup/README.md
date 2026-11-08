# fix-qwen-standard-text-cleanup

**状态：2026-11-08 缺陷已复现，提案与实施计划如下，等待所有者确认后归档（实现随之完成，见 tasks）。**

## 问题

在 12 GB 标准档上首次实际运行 Qwen-Image 2.1 Turbo 生成（低内存 prepare 安装完成后），文本阶段结束时报：

> cleanup failed after text: CUDA reserved 5.83 GiB > 2.5 GiB

复现结论：

- 与新的 prepared 缓存无关：使用 2026-11-04 的 reference 缓存同样失败；两者权重的 safetensors **逐字节一致**（`13c2f7cd…` / `010c2c3b…`）。
- 根因是 `backends/qwen.py` 的标准档文本编码器加载分支：`from_pretrained(prepared, torch_dtype=…)` 后再 `.to("cuda:0")`。该分支在运行一次前向后，即使 `del` + `gc.collect()` + `torch.cuda.empty_cache()`，CUDA reserved 仍保持 5.83 GiB（allocated 已降到 0.008 GiB），无法通过 2.5 GiB 的阶段性释放检查。
- 同一缓存的**紧凑档分支**（`device_map="cpu", low_cpu_mem_usage=True` 后 `.to("cuda:0")`）在相同前向后可把 reserved 释放到 0.02 GiB，检查通过。
- 因此这是标准档文本阶段装载方式的缺陷：`empty_cache` 无法回收该加载方式留下的缓存块，后续采样阶段被正确拒绝。旧的 Gate 0 脚本没有走这条 worker 路径，所以此前未被发现。

## 修复

标准档文本编码器改用与紧凑档相同的装载方式：

```python
encoder = Qwen3VLForConditionalGeneration.from_pretrained(
    str(prepared_dir), torch_dtype=dtype, device_map="cpu", low_cpu_mem_usage=True
)
encoder = encoder.to("cuda:0")
```

行为不变（NF4 组件整体上卡、前向与编码逻辑不变），只修正装载路径，使文本阶段结束后的显存释放满足既有分阶段合同。

## 验证

- 直接复现脚本：修复后 reference 与新建缓存的前向后 reserved 都应 ≤2.5 GiB；
- 产品内：12 GB 标准档 Qwen Turbo 1024² 8 步生成通过，记录 PNG/耗时/CUDA 峰值；
- 紧凑档 8 GB 路径不受影响（其分支未改动），既有证据保留；
- `cargo fmt/clippy/test`、`openspec validate --all --strict` 通过。

## 影响

- 仅 `tools/diffusers-worker-windows/studio_diffusers_worker/backends/qwen.py`（文本编码器加载分支与注释）。
- worker wheel 身份变化 → 已安装模型需重装运行时（与本轮低内存 prepare 验收一致）。
- 规格：`windows-local-generation` 的"大模型分阶段量化装载与资源门槛"增加一条"阶段释放检查"场景。
