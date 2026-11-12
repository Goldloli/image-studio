# validate-windows-12gb-real-generation

在 Windows 10 / RTX 3080 Ti 12 GB 目标机上对**当前构建**执行一轮包含真实出图的完整验收：种子缓存安装七型号、八项能力真实生成、用本轮真实作品验证 v6 作品/会话管理、真实 v5 库升级，以及取消/强杀/单实例/启动诊断。

**状态：2026-11-12 已执行并归档。** 结果：真实出图 9/9 通过；安装/管理/v5 升级/取消/强杀/启动诊断通过；整树禁网、真实 IME、显示缩放切换、视觉接受/拒绝按用户决定 waived；看门狗、迁移失败保护、磁盘不足与 worker 内部 CUDA reserved 为 not_run。证据：[verification/windows-12gb-real-generation-20261112/](../../../../verification/windows-12gb-real-generation-20261112/README.md)；逐项状态见其 [results.json](../../../../verification/windows-12gb-real-generation-20261112/results.json)。

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | 为什么在目标机补做真实出图验收，以及本轮范围与执行状态。 |
| [design.md](design.md) | 验收档位、构建/数据根/种子缓存/资源口径/证据格式的决策与风险。 |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | 所有者要求触发的真实出图验收、作品管理与证据口径。 |
| [test-plan.md](test-plan.md) | 可在本机逐项执行的完整测试流程（含出图矩阵）。 |
| [tasks.md](tasks.md) | 执行结果与证据索引；归档待用户确认。 |

本 change 不修改产品行为；执行中发现的观察项登记为 Beads `image-studio-10a`（模型页安装后刷新停住）与 `image-studio-5vb`（Klein 私有提交预算边界），其余工具/安装观察记录在环境证据中。相关证据：

- `verification/windows-12gb-real-generation-20261112/`：本轮当前构建真实出图与完整验收。
- `verification/windows-12gb-standard-tier/`：2026-11-08 本机 12 GB 标准档基线（旧构建，执行面已变化，仅作对照）。
- `verification/windows-functional-regression-20261112/`：2026-11-12 Windows 11 / RTX 2060 SUPER 8 GB 补充机功能回归（无推理、无新图）。
