# Test Plan: replace-macos-zip-with-dmg

## 本机与 CI 可验证项

| ID | 场景 | 期望 | 验证方式 |
|---|---|---|---|
| D01 | DMG 生成 | 从已验证 `.app` 生成一个 UDZO DMG，退出码为 0 | 在 macOS 运行打包脚本并保留日志 |
| D02 | DMG 内部结构 | 映像根目录包含 `Image Studio.app` 和 `/Applications` 快捷方式 | 挂载后 `find`、`readlink` 与截图/JSON 证据 |
| D03 | DMG 结构校验 | `hdiutil verify` 成功 | 本地与 macOS CI 日志 |
| D04 | 包身份 | identifier、短版本、构建版本、最低系统版本正确 | `plutil`/`PlistBuddy` |
| D05 | 架构 | 主程序与三个 sidecar 均为 arm64 | `file` 输出 |
| D06 | 许可资源 | LICENSE、NOTICE、第三方声明、字体和提示词库许可存在 | 挂载后文件存在检查 |
| D07 | Checksum | `SHA256SUMS-macos.txt` 能验证 DMG | `shasum -a 256 -c` |
| D08 | 唯一格式 | 未来 artifact 与 Release 中恰好一个 macOS DMG、零个 macOS zip | workflow 文件数断言与下载检查 |
| D09 | 损坏 DMG | 截断或修改后的 DMG 无法通过 checksum/`hdiutil verify`，release 不继续 | fixture 或故障注入 |
| D10 | 卸载清理 | 正常检查和失败路径都尝试 detach，后续验证不被残留挂载阻塞 | 日志与挂载表 |
| D11 | macOS release verification | release workflow 重新下载 DMG 并完成挂载验证后才进入 draft job | GitHub Actions run 图与日志 |
| D12 | 版本与 tag | Cargo、Tauri、前端与 tag 一致，tag 位于 main | release verify-version |
| D13 | 历史版本不变 | `v0.2.1` 及更早 Release 的附件和 checksum 不变 | GitHub API asset digest 对比 |
| D14 | 未签名说明 | README 与 Release Notes 明确打开 DMG、拖入应用、SHA-256 与 Gatekeeper quarantine 限制 | 文档审查与 Release body 检查 |
| D15 | 全量质量门 | fmt、Clippy、workspace tests、前端 test/check/build、OpenSpec strict、workflow 校验与 actionlint 通过 | 本地日志与双平台 CI |

## 发布后验收

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| R01 | 公开 Release | latest 返回包含本 change 的新版本，draft=false，单个 macOS DMG | GitHub API |
| R02 | 下载复验 | Checksum 通过，DMG 可挂载，应用身份和四个 arm64 可执行文件正确 | 本机记录 |
| R03 | 安装体验 | 打开 DMG 后可把应用拖入 Applications | 人工或 macOS UI smoke |
| R04 | 启动 | 已校验但未公证的应用仍可按文档绕过 quarantine 后启动 | 隔离数据根启动 smoke |
| R05 | 无 zip 回归 | 新 Release 不包含 macOS `.zip` | Release asset 列表 |

## 未运行口径

尚未执行的真实 GitHub Release 与 Apple Gatekeeper UI 流程记为 `not_run`；本地
DMG 生成或 CI 编译通过不得替代真实发布和启动验收。
