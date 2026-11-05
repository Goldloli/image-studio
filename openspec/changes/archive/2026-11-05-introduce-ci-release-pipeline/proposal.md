# Proposal: introduce-ci-release-pipeline

## Why

原始 workflow 尚未真实运行。2026-11-05 的 Mac 审查发现它会因退役的 `macos-14` runner、macOS `-D warnings` 和并行测试不稳定而失败，当前 macOS 产物也被误标为 arm64。仓库同时获准转为 public，因此必须在首次 CI 与双平台 draft Release 前补齐可调度性、供应链、版本来源、产物身份和发布说明。

## 用户决定（2026-11-03，重申于 2026-11-04）

1. 本 change 只交付 CI/release 设计与 workflow 文件 + 本地可验证证据；首次 push/启用由用户在 Mac 侧操作。
2. 不做代码签名/公证（Windows SmartScreen、macOS Gatekeeper 的提示在首次未签名版本中接受并文档化）；后续如需签名，另立 change。
3. 发布形态：GitHub Release 草稿（draft），附 Windows NSIS 安装包、macOS `.app` zip、SHA256SUMS 与许可证/说明；不自动对公发布。
4. 不在 CI 下载模型权重或运行真实 GPU 推理（无 GPU runner）；模型/生成验收仍以本机证据为准。
5. 2026-11-05 用户授权按顺序完成 Mac 审查、OpenSpec 修订和仓库公开；公开后仍须等待本修订确认，才可修改实现代码。

## What Changes

- 将 macOS quality/build runner 从已退役的 Intel `macos-14` 迁移到官方支持的 arm64 `macos-15`，并在构建时断言 `uname -m=arm64`；产物名、Mach-O 架构和 workflow label 必须一致。
- 修复 macOS `cargo clippy --workspace --all-targets --locked -- -D warnings`：按平台 cfg 隔离 NVIDIA/Windows-only 代码，移除 `drop` 非拥有值等告警，并增加对应静态门禁。
- 稳定并行 workspace 测试中的 execution-lease 释放断言；至少连续多次默认并行运行通过，不能用永久单线程 CI 掩盖竞态。
- 公开仓库场景硬化：第三方 action 固定到 commit SHA，checkout 不持久化凭据，quality/build 仅有 `contents: read`，只有最终 release job 获 `contents: write`。
- 发布前校验 tag、Cargo workspace、Tauri 和前端包版本一致，并校验 tag 提交来自 `main`；release workflow 仅从当前运行下载并验证两平台 artifact。
- macOS `.app` 使用正式发布标识 `io.github.goldloli.image-studio`，打包目录先清理，产物校验 `Info.plist`、arm64 架构、sidecar 集合和 SHA-256。
- Release 附件补齐 README、第三方许可摘要、未签名使用说明与简版发布说明；仍创建 draft，不自动公开。
- 更新 README、CI 文档和验收证据；仓库公开后移除 README 中“私有”描述，并记录 secret scanning、首次 runs、artifact checksums 与 draft Release 链接。
- 新增 **quality workflow**（push/PR/手动）：Windows 与 macOS 矩阵跑 Rust fmt、clippy `-D warnings`、workspace tests、前端 `npm ci/test/check/build`、`openspec validate --all --strict`、以及不依赖 GPU/模型的 Python worker 单测；两个平台都构建 sidecar 后再跑 Rust 测试。
- 新增 **build workflows**：Windows 用现有 `scripts/build-windows-internal.ps1` 产出 NSIS 安装包；macOS 用 release 配置构建 sidecar + 桌面并打包 `.app` zip（扩展现有 `prepare-macos-dev-app.py` 支持 `--profile release`）；两者都生成 `SHA256SUMS` 并作为 workflow artifact 上传。
- 新增 **release workflow**：`v*` tag 触发，构建/下载两平台 artifact，创建 GitHub Release **draft**，附安装包、zip、`SHA256SUMS`、许可证与简版 release notes；默认无 secrets。
- 新增文档 `doc/14-ci-release.md`：workflow 说明、缓存策略、不可在 CI 跑的内容、用户启用步骤（推送仓库、在 GitHub 打开 Actions、tag 前检查）、未签名产物提示。
- README 增加 CI/发布章节与 badge 占位（badge 指向 workflow 文件，push 后生效）。

## Capabilities

### New Capabilities

- `ci-release`: GitHub Actions 双平台质量门、安装包构建、artifact/checksum、公开仓库供应链边界与 draft Release。

### Modified Capabilities

- `product-identity`: 增加正式公开发布包的稳定 bundle identifier 与可验证包身份要求；现有图标身份不变。

## 成功标准

- 3 类 workflow 的 YAML 在本地通过语法/结构校验（`yamllint` 或等效），workflow 内引用的脚本/命令在本机可执行（`--dry-run` 级验证：把 workflow 命令逐条在本机跑通/记录）。
- Windows/macOS 构建脚本在本地按 CI 相同参数成功产出安装包/`.app` zip 与 `SHA256SUMS`；openspec strict 通过；全量质量门绿。
- macOS 默认并行 workspace 测试连续多次通过；macOS Clippy 与 release build 均无阻断告警。
- workflow 通过 `actionlint` 和公开仓库安全扫描；所有第三方 action 使用 commit SHA，release 写权限只存在于最终 job。
- 首次 quality 与手动 build run 均为绿，macOS artifact 经下载后确认是 arm64；tag 生成 draft Release 并通过全部 checksum。
- 文档给出用户在 Mac 启用与首次运行/发布的具体步骤；未签名/无 secrets 的限制明确。
- 首次真实 Actions run 与 Release 草稿作为用户启用后的证据（本机无法代跑，标记 user-gated，不冒充通过）。

## 不做 / 超出范围

- 不做代码签名、公证、自动更新、商店上架、私有 runner、GPU 推理 CI。
- 不在 CI 下载/缓存模型权重（体积与许可问题）；不做性能基准。
- 不自动 push 仓库或创建线上 release（需要用户明确授权与 GitHub 侧启用）。
- 本 change 不替用户选择根源码许可证；若未提供正式 LICENSE，公开仓库按默认版权保留处理，发布附件只列第三方许可与项目 README。
- 不重写历史验收证据中的本机路径或 Windows 用户名；公开动作被视为用户已接受这些非凭据信息随历史公开。
