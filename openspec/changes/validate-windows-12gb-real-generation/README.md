# validate-windows-12gb-real-generation

在 Windows 10 / RTX 3080 Ti 12 GB 目标机上对**当前构建**执行一轮包含真实出图的完整验收：种子缓存安装七型号、八项能力真实生成、用本轮真实作品验证 v6 作品/会话管理、真实 v5 库升级，以及取消/强杀/单实例/启动诊断。

**状态：待用户确认。** 用户确认本 change 后才可执行 [test-plan.md](test-plan.md)；确认前不下载模型、不推理、不生成图片。

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | 为什么在目标机补做真实出图验收，以及本轮范围。 |
| [design.md](design.md) | 验收档位、构建/数据根/种子缓存/资源口径/证据格式的决策与风险。 |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | 所有者要求触发的真实出图验收、作品管理与证据口径。 |
| [test-plan.md](test-plan.md) | 可在本机逐项执行的完整测试流程（含出图矩阵）。 |
| [tasks.md](tasks.md) | 确认后的执行、证据、人工复核与归档顺序。 |

本 change 不修改产品行为；发现的缺陷按影响面另立 Beads 与 OpenSpec change。相关证据：

- `verification/windows-12gb-standard-tier/`：2026-11-08 本机 12 GB 标准档基线（旧构建，执行面未变部分可被引用）。
- `verification/windows-functional-regression-20261112/`：2026-11-12 Windows 11 / RTX 2060 SUPER 8 GB 补充机功能回归（无推理、无新图）。
- `verification/windows-8gb-tier/`：2026-11-05/06 8 GB 紧凑档基线。
