# fix-prepare-memory-contract

**状态：2026-11-05 文档与规格修正完成，行为已由 `2026-11-05-support-8gb-vram-tier` 的实测实现覆盖。**

`support-8gb-vram-tier` 归档时把"安装期量化准备按 safetensors 分片流式量化、可用内存 ≥ 8 GiB"写进了主规格，但实现从未做到这一点：prepare 仍以整个组件为单位量化。本 change 只修正规格文本，让主规格与已交付、已实测的行为一致，并把分片流式量化明确登记为未交付的后续项。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 为什么规格与实现不一致、修正范围 |
| [design.md](design.md) | 实测数据、为什么流式量化未实现、后续路径 |
| [specs/model-installation/spec.md](specs/model-installation/spec.md) | 修正后的量化准备要求 |
| [tasks.md](tasks.md) | 修正与验证任务 |
