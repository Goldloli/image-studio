## ADDED Requirements

### Requirement: 公开发布包使用稳定应用身份

macOS 公开发布包 SHALL 使用正式 bundle identifier `io.github.goldloli.image-studio`，不得继续把开发值 `local.image-studio.foundation.dev` 作为对外发布身份。`Info.plist` SHALL 包含一致的显示名、短版本、构建版本、图标、可执行文件名与最低系统版本；正式身份变化 MUST NOT 静默迁移、覆盖或删除现有用户数据。

#### Scenario: 检查公开 `.app`

- **WHEN** 构建或下载 macOS 发布包
- **THEN** `plutil` 能解析 `Info.plist`，identifier 为 `io.github.goldloli.image-studio`，版本与应用版本一致，主程序与 sidecar 都属于受支持的 macOS arm64 架构

#### Scenario: 从开发包升级

- **WHEN** 用户从使用开发 identifier 的本地构建切换到正式发布包
- **THEN** 应用数据仍位于原登记路径，不因 bundle identifier 变化而删除、重置或假装迁移成功
