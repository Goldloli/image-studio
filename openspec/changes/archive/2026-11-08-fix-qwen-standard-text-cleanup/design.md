# Design: fix-qwen-standard-text-cleanup

## 1. 证据

| 观测 | 结果 |
|---|---|
| 首次标准档生成的 tried | `cleanup failed after text: CUDA reserved 5.83 GiB > 2.5 GiB`，attempt failed，无资产 |
| reference prepared 缓存（2026-11-04） | 同样失败；权重 safetensors 与低内存重建缓存**逐字节一致** |
| 标准档装载分支：`from_pretrained(prepared, torch_dtype)` 后 `.to("cuda:0")`，跑一次 `encode_prompt`，`del`+`gc`+`empty_cache` | reserved **5.830 GiB**，allocated 0.008 GiB（无法通过 2.5 GiB 检查） |
| 紧凑档装载分支：`from_pretrained(..., device_map="cpu", low_cpu_mem_usage=True)` 后 `.to("cuda:0")`，同样流程 | reserved **0.02 GiB**（通过） |

结论：不是 prepared 缓存差异，也不是显存预算问题，而是标准档文本编码器的装载方式让缓存分配器无法在阶段结束后归还显存。

## 2. 修复

`backends/qwen.py` 的 `load_text_encoder` 非 sequential 分支改为与 sequential 分支相同的装载参数：

```python
encoder = Qwen3VLForConditionalGeneration.from_pretrained(
    str(prepared_dir), torch_dtype=dtype, device_map="cpu", low_cpu_mem_usage=True
)
encoder = encoder.to("cuda:0")
```

- 保留 `.to("cuda:0")`：文本阶段仍是"整组件驻留 GPU→前向→释放"，不改变分阶段语义与显存峰值（实测 reserved 5.961 GiB 与原来相同）。
- 不放宽 `CLEANUP_RESERVED_LIMIT_GIB`；不放宽任何预算检查。
- 代码注释说明两种档位共用该装载方式的原因。

## 3. 验证

1. 复现脚本：修复后 reference 与重建缓存在 encode 后 `del`+`empty_cache` 的 reserved 均 ≤2.5 GiB。
2. 产品内：12 GB 标准档 Qwen Turbo 1024²、8 步生成成功；记录 PNG/耗时/CUDA 峰值。
3. 紧凑档 8 GB 分支未改动，既有 `windows-8gb-tier` 证据不受影响。
4. `cargo fmt/clippy/test`、`openspec validate --all --strict` 通过。

## 4. 风险与回滚

| 风险 | 缓解 |
|---|---|
| 标准档文本阶段用 device_map 装载后显存峰值变化 | 复现脚本实测 reserved 5.961 GiB，与旧分支相同；生成阶段仍有 10.5 GiB 预算检查 |
| 其他模型（MOSS）也用相同时序，是否同样问题 | 本次只改 Qwen 文本分支；MOSS 生成后续按同样方式验证，若发现问题另行登记 |
| worker wheel 身份变化导致重装 | 与低内存 prepare 同一次重装，证据同批记录 |

回滚：恢复原分支代码并把 wheel 重建即可，无数据迁移。
