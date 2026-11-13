# replace-macos-zip-with-dmg

Use a DMG instead of a zip for future macOS release artifacts

## Status

**Implemented, accepted and publicly released as `v0.2.2`.** 用户已于 2026-11-13
确认 DMG-only 方案并授权直接发布。workflow、打包/验证脚本、README、Release Notes
与版本号已经更新；历史 Release 保持不可变。

| 文档 | 内容 |
| --- | --- |
| [proposal.md](proposal.md) | DMG-only 的动机、范围、breaking artifact 变化与历史版本边界。 |
| [specs/ci-release/spec.md](specs/ci-release/spec.md) | macOS 构建、DMG 内容、Release 附件和挂载验证的新契约。 |
| [design.md](design.md) | `hdiutil`、Applications symlink、macOS verification job、旧版本不可变和备选方案。 |
| [tasks.md](tasks.md) | 确认后的打包、workflow、文档、验证、下一版本发布与归档步骤。 |
| [test-plan.md](test-plan.md) | 结构、身份、架构、checksum、损坏、cleanup、release 与历史版本测试。 |

完整证据见
[`verification/ci-release/v0.2.2/`](../../../verification/ci-release/v0.2.2/README.md)：
GitHub run、10 个公开附件、单一 DMG、checksum、挂载身份、arm64、启动 smoke 与
历史版本不变均已复核。
