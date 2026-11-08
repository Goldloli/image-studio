# Tasks: fix-prepare-preflight-threshold

> 未获项目所有者确认前，不得开始实现。修复后按顺序补测并归档。

## 1. 实现

- [ ] 1.1 `apps/desktop/src/adapters/installation.rs`：`PREPARE_MIN_COMMIT_HEADROOM_BYTES` 26 GiB → 24 GiB，更新 `prepare_memory_error` 文案的阈值数字。
- [ ] 1.2 调整量化准备流程顺序：先判断 prepared 缓存可复用；可复用时跳过量化的内存预检与 prepare，记录 `reused`；不可复用时再预检。
- [ ] 1.3 更新单元测试：预检边界（23.99 GiB 拒绝 / 24.00 GiB 通过 / 28 GiB 物理通过）、可复用缓存跳过预检、错误文案含新阈值与建议。
- [ ] 1.4 若 `prepared_cache_reuse_allowed` 需要暴露给测试，补最小可见性改动，不改变语义。

## 2. 构建与质量门禁

- [ ] 2.1 `scripts/build-windows-internal.ps1 -SkipBundle`（含 fmt/clippy/test）与 `cargo build --release -p image-studio-desktop` 通过。
- [ ] 2.2 `openspec validate --all --strict` 通过；保存构建/测试日志到证据目录。

## 3. 本机实测（全新根）

- [ ] 3.1 用修复后构建在 `C:\ist-accept` 全新根安装 Qwen-Image 2.1 Turbo：记录预检实测值、prepare 峰值/耗时、ready 结果（应通过）。
- [ ] 3.2 安装 Qwen-Image 2.1 与 MOSS-Image-SFT（可选串行），记录 prepare 耗时与资源。
- [ ] 3.3 三个模型各完成一次标准档生成（1024²），记录 stages、墙钟、PNG 尺寸/字节/SHA-256、host GPU 采样。
- [ ] 3.4 验证已安装型号重启后仍就绪；导出 PNG 与资产逐字节一致。
- [ ] 3.5 证据写入 `verification/windows-12gb-standard-tier/`：新增 `S3b-prepare-after-fix.json`、S8b 生成记录，并更新 S3 的 blocked 记录与 README。

## 4. 规格与收口

- [ ] 4.1 `openspec/specs/model-installation/spec.md` 同步修改后的量化预检要求与场景（阈值 24 GiB、复用短路、默认虚拟内存 32 GiB 机器场景）。
- [ ] 4.2 更新 `certify-windows-10-target-machine` 的 tasks/test-plan：S9 blocked 项改为通过并引用新证据；补齐 Qwen/MOSS 标准档结论。
- [ ] 4.3 `openspec archive fix-prepare-preflight-threshold`，`openspec validate --all --strict` 通过。
- [ ] 4.4 在 bd 登记该缺陷并在修复验证后关闭（引用证据）。
