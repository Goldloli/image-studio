# 修复“移除源图”后旧槽位来源残留

Change：`fix-cleared-source-slot-reference`

**状态：已实现并通过 macOS 原生验收（2026-11-11）；证据见 `verification/cleared-source-slot-fix-20261111/RESULTS.md`。**

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | `image-studio-c2e` 的问题、显式清空传播规则与范围。 |
| [design.md](design.md) | reducer 构造槽位的修改、保留语义、迁移取舍与回归测试。 |
| [specs/feature-catalog/spec.md](specs/feature-catalog/spec.md) | 修改“能力切换保持草稿参数对宿主合法”要求并新增两个场景。 |
| [tasks.md](tasks.md) | 实现与验收结果。 |

问题来源：`verification/capability-drafts-fix-20261111/RESULTS.md` 的“本轮发现但未修复”一节；
Beads `image-studio-c2e`。
