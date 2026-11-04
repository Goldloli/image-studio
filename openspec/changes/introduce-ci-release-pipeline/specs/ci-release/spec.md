## Purpose

为 Windows/macOS 双平台提供可审计的 CI 质量门、安装包构建与 GitHub Release 草稿发布流水线；本机不执行真实 Actions run，首次启用由用户在 Mac/GitHub 侧完成。

## ADDED Requirements

### Requirement: 双平台质量控制流水线

仓库 SHALL 提供 GitHub Actions quality workflow，在 Windows 与 macOS 上执行固定版本的 Rust 工具链、Node、OpenSpec CLI，并运行 sidecar 构建、`cargo fmt --check`、`cargo clippy -D warnings`、workspace 测试、前端 test/check/build 与 `openspec validate --all --strict`；workflow 不得下载模型权重或运行 GPU 推理。

#### Scenario: push 或 PR 触发质量门
- **WHEN** 向主分支 push 或创建 PR
- **THEN** Windows 与 macOS 的质量 job 都运行上述检查，任一失败则 workflow 失败

#### Scenario: 无 GPU runner
- **WHEN** CI 运行
- **THEN** 不下载模型、不执行真实推理，模型验收证据仍来自本机记录

### Requirement: 双平台安装包构建与校验和

仓库 SHALL 提供手动/tag 触发的 build workflow，在 Windows 产出 NSIS 安装包、在 macOS 产出 `.app` zip，并为每个产物生成 `SHA256SUMS` 作为 artifact 上传；macOS 产物未签名/未公证的限制 MUST 在文档与 release 说明中写明。

#### Scenario: 手动构建
- **WHEN** 用户手动触发 build workflow
- **THEN** 两个平台的构建 job 分别产出安装包/zip 与 SHA256SUMS，并可下载

#### Scenario: 产物完整性
- **WHEN** 下载产物
- **THEN** 文档说明的校验命令能用 SHA256SUMS 验证文件完整

### Requirement: GitHub Release 草稿

仓库 SHALL 提供 tag `v*` 触发的 release workflow，创建 **draft** GitHub Release，附 Windows 安装包、macOS zip、两平台 SHA256SUMS、许可证与简版 release notes；默认不得自动 publish，也不得依赖签名 secrets。

#### Scenario: tag 发布草稿
- **WHEN** 推送 `v*` tag
- **THEN** 创建 draft Release，附两平台产物与 checksums，等待用户人工审阅后发布

#### Scenario: 版本一致性
- **WHEN** tag 与应用版本不一致
- **THEN** workflow 失败并提示版本不匹配

### Requirement: CI 证据与用户启用边界

本 change 的本机验收 SHALL 覆盖 YAML 语法校验、workflow 命令逐条本机执行、Windows 安装包构建与 checksum 样例；首次真实 Actions run 与 draft Release SHALL 由用户授权 push 后在 GitHub 侧验证并记录，不得以本地模拟冒充通过。

#### Scenario: 用户启用
- **WHEN** 用户在 Mac/GitHub 启用 Actions 并推送
- **THEN** 首次 quality/build/release run 的结果记录到 `verification/ci-release/`，失败时按文档排查
