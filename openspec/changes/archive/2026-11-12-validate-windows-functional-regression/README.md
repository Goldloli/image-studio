# validate-windows-functional-regression

在 Windows 实机安装当前构建，验证界面删改、v6 升级、会话与作品管理、文件删除及安装包行为；不执行模型推理或生成新图片。

**状态：2026-11-12 已执行并归档。** 验收在 Windows 11 / RTX 2060 SUPER 8 GB 补充机完成（`verification/windows-functional-regression-20261112/`）；目标机复验见 Beads `image-studio-b65`。

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | 为什么采用 Windows 功能回归，以及何时仍需完整生图认证。 |
| [design.md](design.md) | 功能验收边界、执行面判断规则、证据口径与风险。 |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | 将 Windows 验收拆分为功能回归与按影响面触发的生图认证。 |
| [test-plan.md](test-plan.md) | 可直接在 Windows 目标机执行的逐项清单；明确禁止模型推理。 |
| [tasks.md](tasks.md) | 用户确认后的执行、证据与归档顺序。 |

本 change 只调整验收策略与证据解释，不改变应用运行行为、接口、数据库或模型配方。Beads 跟踪：`image-studio-7ed`。

