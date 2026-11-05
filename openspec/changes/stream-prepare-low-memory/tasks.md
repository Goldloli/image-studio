# Tasks: stream-prepare-low-memory

> 未获用户确认前，不得开始实现。

## 1. 文档与确认

- [ ] 1.1 proposal / design / tasks 与 spec delta 完成并展示，等待项目所有者确认。
- [ ] 1.2 确认后建立 bd issue 跟踪。

## 2. 试点：Qwen DiT

- [ ] 2.1 实现 meta 骨架 + 逐 shard 量化 + `save_pretrained` 的通用函数。
- [ ] 2.2 用 Qwen-Image 2.1-Turbo 的 transformer（2 shards）验证：产物可被现有 worker 加载并通过 `_component_ready`。
- [ ] 2.3 采集 commit 峰值、可用内存最低值、耗时，与现有 20.1 GiB 对比。

## 3. Qwen3-VL 文本编码器与 MOSS

- [ ] 3.1 Qwen3-VL 文本编码器（4 shards）：处理 tied weights、视觉塔与 meta 构造期计算。
- [ ] 3.2 MOSS transformers 文本编码器。
- [ ] 3.3 MOSS 自定义 DiT：自建 `nn.Linear → Linear4bit` 替换，保持 shim 语义。
- [ ] 3.4 四个组件逐一采集同口径证据，目标 commit ≤ 10 GiB、可用内存 ≥ 8 GiB。

## 4. 预检与失败路径

- [ ] 4.1 按实测冻结预检门槛（可用物理内存 ≥ 8 GiB、commit 余量 ≥ 6 GiB）。
- [ ] 4.2 失败即清理未完成缓存，保留原快照，错误可行动；补 Rust 侧测试。
- [ ] 4.3 旧路径保留为降级/回滚选项，行为与现有证据兼容。

## 5. 数值与端到端验收

- [ ] 5.1 抽样比较反量化权重（1%，bit-identical 或相对误差 < 1e-6）。
- [ ] 5.2 新缓存跑 Qwen Turbo compact Gate 0（768²、8 步、固定种子），与现有输出对比并记录 PSNR。
- [ ] 5.3 参考机器端到端：安装 → prepare → 生成，记录资源报告与截图。
- [ ] 5.4 更新 `doc/15-windows-8gb-tier.md`、`verification/windows-8gb-tier/` 与 model-installation 规格。

## 6. 质量门

- [ ] 6.1 `cargo fmt --check`、`clippy -D warnings`、workspace 测试、前端 test/build 全绿。
- [ ] 6.2 `openspec validate --all --strict` 通过；归档 change 并同步主规格。
