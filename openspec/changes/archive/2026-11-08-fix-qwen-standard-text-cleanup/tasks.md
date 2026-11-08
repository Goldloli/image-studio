# Tasks: fix-qwen-standard-text-cleanup

- [x] 1.1 复现失败并定位到标准档文本编码器装载分支（reference/新建缓存均复现；紧凑分支可释放）。
- [x] 1.2 记录证据：`target/probe-qwen-text.log`、`target/probe-qwen-seq.log`、attempt error `cleanup failed after text: CUDA reserved 5.83 GiB > 2.5 GiB`。
- [x] 2.1 `backends/qwen.py` 标准档文本分支改为 `device_map="cpu", low_cpu_mem_usage=True` 装载，保留 `.to("cuda:0")` 与所有预算检查。
- [x] 2.2 复现脚本验证：修复后 reference/重建缓存的文本阶段 reserved 均为 0.020 GiB（≤2.5 GiB）。
- [x] 2.3 重建 worker wheel（`543966ce…`）与桌面应用并重装运行时（prepared 缓存复用，安装 179 s）。
- [x] 2.4 产品内 12 GB 标准档 Qwen Turbo 1024² 8 步生成通过（40.7 s，PNG 2,010,413 B，SHA-256 `92a9d4f8…`），证据 `verification/windows-12gb-standard-tier/S3c-qwen-generation-after-cleanup-fix.json`。
- [x] 3.1 `openspec/specs/windows-local-generation/spec.md` 的阶段释放场景已通过归档同步。
- [x] 3.2 回归：紧凑档 8 GB 分支未改动；`cargo test -p image-studio-desktop` 通过；`openspec validate --all --strict` 28/28。
- [x] 3.3 归档本 change；关闭 bd 缺陷 `image-studio-2uj`。
