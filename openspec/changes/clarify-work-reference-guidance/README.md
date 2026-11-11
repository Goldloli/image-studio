# 完善作品删除被引用时的解除指引

Change：`clarify-work-reference-guidance`

**状态：已实现并通过 macOS 原生验收（2026-11-12）；证据见 `verification/reference-guidance-20261112/RESULTS.md`。**

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | 验证到的规格符合性缺陷、三类引用与契约扩展。 |
| [design.md](design.md) | 宿主扫描与 detail 指引、前端结构化渲染、能力名回退与风险。 |
| [specs/asset-library/spec.md](specs/asset-library/spec.md) | 修改“作品删除检查引用并如实表达未完成”要求，新增三类解除场景。 |
| [tasks.md](tasks.md) | 确认后的实现与验收范围。 |

验证证据：`image-studio-huq`；规格 `openspec/specs/asset-library/spec.md`；
实现位置 `apps/desktop/src/adapters/library/sources.rs`、`apps/frontend/src/AssetLibrary.tsx`；
原生验收实际消息见 `verification/simplify-workspace-20261111/RESULTS.md`（A08）。
