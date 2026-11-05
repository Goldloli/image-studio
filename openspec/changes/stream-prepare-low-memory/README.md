# stream-prepare-low-memory

**状态：2026-11-06 提案文档完成，等待项目所有者确认后开始实现。**

安装期 NF4 量化准备目前以整组件为单位在 CPU 上构造模型，实测 commit 峰值 16.4–23.1 GiB，可用物理内存可降到接近 0。`2026-11-05-support-8gb-vram-tier` 归档时曾把"按 safetensors 分片流式量化、可用内存 ≥ 8 GiB"写成要求；`2026-11-05-fix-prepare-memory-contract` 已把该要求在规格中纠正为现状，并把流式量化登记为独立后续项。本 change 就是那个后续项：让可用物理内存 ≥ 8 GiB 的机器（典型为 16 GiB 内存 + 8 GB 显存）也能完成安装期的 Qwen/MOSS 量化准备。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 为什么做、范围、成功标准、不做项 |
| [design.md](design.md) | 分片流式量化的技术路线、逐架构适配、预算与证据 |
| [specs/model-installation/spec.md](specs/model-installation/spec.md) | 修正后的量化准备要求（把 8 GiB 门槛真正交付） |
| [tasks.md](tasks.md) | 实现与验收任务 |
