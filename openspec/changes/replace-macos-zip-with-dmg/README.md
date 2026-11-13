# replace-macos-zip-with-dmg

Use a DMG instead of a zip for future macOS release artifacts

## Status

**Proposal only; waiting for user confirmation.** 用户已于 2026-11-13 明确要求后续
macOS 发布改为 DMG 且不要 zip。本阶段只更新 OpenSpec 文档，不修改 workflow、
打包脚本、README、版本号或已发布 Release。

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | DMG-only 的动机、范围、breaking artifact 变化与历史版本边界。 |
| [specs/ci-release/spec.md](specs/ci-release/spec.md) | macOS 构建、DMG 内容、Release 附件和挂载验证的新契约。 |
| [design.md](design.md) | `hdiutil`、Applications symlink、macOS verification job、旧版本不可变和备选方案。 |
| [tasks.md](tasks.md) | 确认后的打包、workflow、文档、验证、下一版本发布与归档步骤。 |
| [test-plan.md](test-plan.md) | 结构、身份、架构、checksum、损坏、cleanup、release 与历史版本测试。 |

归档前必须完成 [tasks.md](tasks.md)；真实发布前不得声称新格式已经交付。
