# 修复能力切换草稿槽位与作品删除确认标题

Change：`fix-capability-drafts-and-delete-title`

**状态：已实现并通过 macOS 原生验收（2026-11-11）；证据见 `verification/capability-drafts-fix-20261111/RESULTS.md`。**

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | 两个验收发现的问题、按能力规范化草稿、删除标题降级与范围约束。 |
| [design.md](design.md) | 能力→画幅映射、reducer 三处规范化、加载修复、标题降级及风险。 |
| [specs/](specs/) | `feature-catalog`、`asset-library` 两份 delta。 |
| [tasks.md](tasks.md) | 确认后的实现与验收范围。 |

问题来源：`verification/simplify-workspace-20261111/RESULTS.md`；草稿槽位问题由
`image-studio-31x` 跟踪。本阶段只提交文档，不修改实现。
