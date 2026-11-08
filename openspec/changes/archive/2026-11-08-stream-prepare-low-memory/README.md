# stream-prepare-low-memory

**状态（2026-11-08）：低内存 prepare 已实现并完成产品内验收（模拟 16 GiB 环境），Qwen Turbo / Qwen base / MOSS 均已安装并完成 1024² 标准档生成；剩余真实 16 GiB 实机证据与规格归档。**

安装期 NF4 量化准备目前以整组件为单位在 CPU 上构造模型，实测 commit 峰值 16.4–23.1 GiB、可用物理内存最低接近 0，安装预检因此要求 28 GiB 物理内存或 26 GiB commit 余量——32 GiB 内存 + 默认页面文件的 Windows 机器全新安装会被误拒，16 GiB 内存机器更无法安装 Qwen/MOSS。

本 change 交付真正的低内存量化路径：

- 量化工作按分片/分块拆到**独立子进程**（进程退出即归还内存），任一进程不持有完整 bf16 组件；
- 目标：可用物理内存 ≥ 8 GiB、commit 峰值 ≤ 10 GiB 能完成安装；
- 产物与现有 `models/<snapshot>.prepared` 格式兼容（可被现有 worker 直接加载），优先做到与现有缓存逐字节一致，否则登记新缓存身份并给出数值等价证据；
- 预检阈值按实测冻结为"可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB"；
- 在没有 16 GiB 实机的情况下，用"预留内存到只剩 ~8 GiB 可用 + Job Object commit 限制"模拟验收并记录限制。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 为什么做、范围、成功标准、不做项 |
| [design.md](design.md) | 现状证据、子进程分片方案、格式兼容、验收与风险 |
| [specs/model-installation/spec.md](specs/model-installation/spec.md) | 修正后的量化准备要求（真正交付低内存门槛） |
| [tasks.md](tasks.md) | 实现与验收任务 |
