# ci-release Specification

## Purpose
为 Windows/macOS 双平台提供可审计的 CI 质量门、安装包构建与 GitHub Release 草稿发布流水线；本机不执行真实 Actions run，首次启用由用户在 Mac/GitHub 侧完成。

## Requirements

### Requirement: 双平台质量控制流水线

仓库 SHALL 提供 GitHub Actions quality workflow，在 Windows 与 macOS arm64 上执行固定版本的 Rust 工具链、Node、OpenSpec CLI，并运行 `cargo fmt --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、workspace 测试、前端 test/check/build 与 `openspec validate --all --strict`；workflow 不得下载模型权重或运行 GPU 推理。macOS runner SHALL 使用当前受支持的 arm64 label，并在 job 内断言 `uname -m=arm64`。

#### Scenario: push 或 PR 触发质量门
- **WHEN** 向主分支 push 或创建 PR
- **THEN** Windows 与 macOS 的质量 job 都运行上述检查，任一失败则 workflow 失败

#### Scenario: 无 GPU runner
- **WHEN** CI 运行
- **THEN** 不下载模型、不执行真实推理，模型验收证据仍来自本机记录

#### Scenario: macOS 平台条件编译
- **WHEN** macOS quality job 运行 `clippy --all-targets -- -D warnings`
- **THEN** Windows-only import、常量、探测函数与测试 helper 不产生 unused/dead-code 错误，默认并行 workspace 测试稳定通过

### Requirement: 双平台安装包构建与校验和

仓库 SHALL 提供手动/被 release 调用的 build workflow，在 Windows x64 产出 NSIS 安装包、在 macOS arm64 产出 `.app` zip，并为每个产物生成 `SHA256SUMS` 作为 artifact 上传；macOS 产物未签名/未公证的限制 MUST 在文档与 release 说明中写明。

#### Scenario: 手动构建
- **WHEN** 用户手动触发 build workflow
- **THEN** 两个平台的构建 job 分别产出安装包/zip 与 SHA256SUMS，并可下载

#### Scenario: 产物完整性
- **WHEN** 下载产物
- **THEN** 文档说明的校验命令能用 SHA256SUMS 验证文件完整

#### Scenario: macOS 架构与包身份
- **WHEN** macOS build 完成
- **THEN** 构建 job 与下载后的 `.app` 都证明主程序和全部 sidecar 为 arm64，`Info.plist` 有效且使用正式 bundle identifier `io.github.goldloli.image-studio`

### Requirement: GitHub Release 草稿

仓库 SHALL 提供 tag `v*` 触发的 release workflow，创建 **draft** GitHub Release，附 Windows 安装包、macOS zip、两平台 SHA256SUMS、项目 README、第三方许可摘要、未签名说明与简版 release notes；默认不得自动 publish，也不得依赖签名 secrets。

#### Scenario: tag 发布草稿
- **WHEN** 推送 `v*` tag
- **THEN** 创建 draft Release，附两平台产物与 checksums，等待用户人工审阅后发布

#### Scenario: 版本一致性
- **WHEN** tag 与根 Cargo workspace、Tauri 或前端 package version 不一致
- **THEN** workflow 失败并提示版本不匹配

#### Scenario: 来源不在主分支
- **WHEN** `v*` tag 指向的提交不属于 `main`
- **THEN** 在任何长构建开始前失败，不生成 Release

### Requirement: 公开仓库 Actions 供应链与最小权限

公开仓库 SHALL 将所有第三方 action 固定到完整 commit SHA，checkout SHALL 设置 `persist-credentials: false`；quality 与 build MUST 只有 `contents: read`，只有最终 draft Release job MAY 获得 `contents: write`。首次公开后 SHALL 检查 secret scanning、vulnerability alerts、分支保护与 Actions 权限，并记录仍需人工配置的项。

#### Scenario: fork pull request
- **WHEN** 外部 fork 提交 pull request
- **THEN** quality 仅以只读 token 运行，不接触 release 写权限或 secrets

#### Scenario: workflow 供应链变更
- **WHEN** 新增或升级 action
- **THEN** 使用经过审阅的完整 commit SHA，并由静态安全检查验证

### Requirement: CI 证据与用户启用边界

本 change 的本机验收 SHALL 覆盖 actionlint、workflow 静态安全扫描、命令执行、macOS 默认并行测试稳定性、release build 与 `.app` 包检查；首次真实 Actions run 与 draft Release SHALL 在 GitHub 侧验证并记录，不得以本地模拟冒充通过。

#### Scenario: 用户启用
- **WHEN** 用户在 Mac/GitHub 启用 Actions 并推送
- **THEN** 首次 quality、手动 build 与 tag release run 的 URL、架构、checksum 和 draft Release 结果记录到 `verification/ci-release/`，失败时按文档排查
