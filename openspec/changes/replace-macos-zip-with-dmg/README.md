# replace-macos-zip-with-dmg

Use a DMG instead of a zip for future macOS release artifacts

## Status

**Implementation complete; local DMG acceptance passed; GitHub release pending.**
用户已于 2026-11-13 确认 DMG-only 方案并授权发布 `v0.2.2`。workflow、打包/验证
脚本、README、Release Notes 与版本号已经更新；已发布 Release 保持不可变。

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | DMG-only 的动机、范围、breaking artifact 变化与历史版本边界。 |
| [specs/ci-release/spec.md](specs/ci-release/spec.md) | macOS 构建、DMG 内容、Release 附件和挂载验证的新契约。 |
| [design.md](design.md) | `hdiutil`、Applications symlink、macOS verification job、旧版本不可变和备选方案。 |
| [tasks.md](tasks.md) | 确认后的打包、workflow、文档、验证、下一版本发布与归档步骤。 |
| [test-plan.md](test-plan.md) | 结构、身份、架构、checksum、损坏、cleanup、release 与历史版本测试。 |

本地 DMG 证据见
[`verification/ci-release/v0.2.2/local-dmg-validation.json`](../../../verification/ci-release/v0.2.2/local-dmg-validation.json)。
归档前必须完成 [tasks.md](tasks.md) 中的 GitHub release 验收。
