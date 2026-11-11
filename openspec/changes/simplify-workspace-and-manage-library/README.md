# 界面精简与会话、作品管理

Change：`simplify-workspace-and-manage-library`

**状态：实现与 macOS 原生验收完成（2026-11-11，见 `verification/simplify-workspace-20261111/RESULTS.md`）；Windows 目标机项保持未验收；准备归档。**

实现摘要：持久层 v5→v6 迁移、会话 tombstone、作品收藏与删除状态机及清理恢复、受限宿主命令；前端单层导航、去页签/功能页/任务页/开发抽屉/示例入口、原位制作记录、会话删除确认、作品收藏/删除/收藏筛选。自动化门禁：`cargo fmt/clippy/test`、前端 `test/check/build` 全部通过；正式规格尚未合并，change 未 archive。

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | 六项移除、两项新增、用户已确认的删除范围及待整体批准的交互选择。 |
| [design.md](design.md) | 导航与原位记录、schema 迁移、受限命令、收藏查询、文件清理及恢复设计。 |
| [specs/](specs/) | 九份现有 capability 的 delta；当前正式规格保持。 |
| [tasks.md](tasks.md) | 确认后实施与验收范围，当前所有任务未开始。 |
| [test-plan.md](test-plan.md) | 界面、删除/收藏、分页、引用、文件清理、中断与两平台验收。 |

2026-11-11 用户补充确认：删除会话保留已生成作品；删除作品同时清理应用内原图，已导出副本保持。整体 change 仍需用户确认。

提案编写跟踪：`image-studio-x16`。本阶段只编写和验证本文档集，不修改实现、更新正式规格或执行归档。

## 文档验证

- `openspec validate simplify-workspace-and-manage-library --strict`：通过。
- 9 份 capability 与 delta 一一对应，19 项实施任务均未开始；test-plan.md 包含 31 个编号验收场景，功能验收均未运行。
- `openspec validate --all --strict`：21 项通过，4 份既有正式规格因需求正文超过 500 字符触发 strict 警告而未通过，分别是 generation-worker-protocol、model-installation、windows-desktop、windows-local-generation。本次未修改这些文件；已记录为 `image-studio-6g2` 单独跟踪。

## 归档计划（原生验收通过后执行）

1. 先完成 `verification/simplify-workspace-20261111/CHECKLIST.md` 的原生项，写 `RESULTS.md` 并保存截图/文件证据；Windows 未测项保持“未验收”。
2. 在真实工作区执行 `openspec archive simplify-workspace-and-manage-library --yes`。本机已用临时副本演练：9 份 delta 合并为 `+15 / ~16 / -13`，`openspec list` 归零，除既有 4 份规格长度警告外无新增校验失败；演练不修改工作区、不代表已归档。
3. 归档后修正两份正式规格的 Purpose，使其描述保留职责而非退役页面：
   - `feature-catalog`：维护宿主真实能力登记与当前可执行性，让创作页按已接入能力提供统一、可追溯的提交链路；不提供独立功能目录或功能页。
   - `task-center`：让用户在原会话卡与作品预览中查看可读制作记录、状态、失败原因与下一步，并保持执行身份一致地停止或显式重试；不提供独立任务列表或任务详情页。
4. 提交归档与 Purpose 修正，更新 tasks.md/README 状态，关闭 bd `image-studio-i6`。
