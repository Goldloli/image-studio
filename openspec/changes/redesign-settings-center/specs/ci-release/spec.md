# Spec Delta

## ADDED Requirements

### Requirement: 公开稳定版 Release 可被应用更新器消费

面向用户的稳定版 Release SHALL 使用 `vX.Y.Z` 标签，并同时提供当前平台固定命名的 NSIS 安装包、macOS arm64 DMG、对应 `SHA256SUMS-windows.txt` / `SHA256SUMS-macos.txt` 与可供应用显示的发布说明。应用更新器 MUST 不把 draft 或 prerelease 作为稳定更新；发布产物命名或 checksum 缺失时 MUST 不能成为可安装更新。

#### Scenario: 稳定版可被检查
- **WHEN** 用户发布一个指向 `main` 的稳定版 `vX.Y.Z` Release
- **THEN** API 中可读取该版本、发布说明、Windows 安装包、macOS DMG 与两份 checksum，应用能够完成稳定版版本比较

#### Scenario: 更新资产缺失
- **WHEN** Release 缺少当前平台安装包或对应 checksum
- **THEN** 应用更新器拒绝该候选版本并提示发布资产不完整，不开始下载或安装

#### Scenario: 预发布不进入稳定通道
- **WHEN** Release 被标记为 prerelease 或 draft
- **THEN** 稳定通道检查忽略它，自动更新不会向普通用户提示
