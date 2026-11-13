# ci-release Delta

## MODIFIED Requirements

### Requirement: 双平台安装包构建与校验和

仓库 SHALL 提供手动/被 release 调用的 build workflow，在 Windows x64 产出 NSIS
安装包、在 macOS arm64 产出只读 `.dmg`，并为每个产物生成 `SHA256SUMS` 作为
artifact 上传。未来版本的 macOS 发布附件不得同时提供 `.app` zip。DMG SHALL 包含
经过验证的 `Image Studio.app` 和指向 `/Applications` 的快捷方式；macOS 产物
未签名/未公证的限制 MUST 在文档与 release 说明中写明。

#### Scenario: 手动构建
- **WHEN** 用户手动触发 build workflow
- **THEN** Windows job 产出 NSIS 安装包，macOS job 产出
  `Image-Studio-<version>-macos-arm64.dmg` 与对应 SHA256SUMS，并可下载

#### Scenario: 产物完整性
- **WHEN** 下载产物
- **THEN** 文档说明的 SHA-256 命令能验证 DMG 或 Windows 安装包完整

#### Scenario: macOS 架构与包身份
- **WHEN** macOS build 完成并挂载 DMG
- **THEN** `Info.plist` 使用正式 identifier `io.github.goldloli.image-studio`、
  版本正确、最低系统版本为 macOS 11，主程序与全部 sidecar 均为 arm64

#### Scenario: DMG 安装内容
- **WHEN** 打开构建完成的 DMG
- **THEN** 映像根目录包含 `Image Studio.app` 与 `/Applications` 快捷方式，且
  不包含作为独立发布附件的 `.app` zip

### Requirement: GitHub Release 草稿

仓库 SHALL 提供 tag `v*` 触发的 release workflow，创建 **draft** GitHub Release，
附 Windows 安装包、macOS DMG、两平台 SHA256SUMS、项目 README、第三方许可摘要、
未签名说明与简版 release notes；默认不得自动 publish，也不得依赖签名 secrets。
未来版本的 macOS Release MUST 只附一个 `.dmg`，不得附 `.app` zip。

#### Scenario: tag 发布草稿
- **WHEN** 推送未来版本的 `v*` tag
- **THEN** 创建 draft Release，附两平台产物与 checksums，等待用户人工审阅后发布

#### Scenario: 版本一致性
- **WHEN** tag 与根 Cargo workspace、Tauri 或前端 package version 不一致
- **THEN** workflow 失败并提示版本不匹配

#### Scenario: 来源不在主分支
- **WHEN** `v*` tag 指向的提交不属于 `main`
- **THEN** 在任何长构建开始前失败，不生成 Release

#### Scenario: macOS 附件形态
- **WHEN** release job 汇总 Mac artifact
- **THEN** draft Release 恰好包含一个 macOS DMG、零个 macOS `.zip`，且 checksum
  文件名与 DMG 文件名一致

### Requirement: CI 证据与用户启用边界

本 change 的本机验收 SHALL 覆盖生成 DMG、只读挂载、挂载后应用身份与架构检查、
DMG checksum、actionlint、workflow 静态安全扫描、release build 与 OpenSpec
strict；首次真实 Actions run 与 draft Release SHALL 在 GitHub 侧验证并记录，
不得以本地模拟或旧的 zip 证据冒充通过。

#### Scenario: 用户启用
- **WHEN** 用户在 Mac/GitHub 推送实现并触发 quality、手动 build 与 tag release
- **THEN** DMG 的挂载验证、checksum、架构与 draft Release 结果记录到
  `verification/ci-release/`，失败时按文档排查

#### Scenario: 历史版本
- **WHEN** 查看 `v0.2.1` 及更早的公开 Release
- **THEN** 旧附件保持不可变，只有首个包含本 change 的新版本采用 DMG-only 契约
