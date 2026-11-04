# Proposal: introduce-ci-release-pipeline

## Why

当前仓库没有任何 GitHub Actions workflow：质量门（fmt/clippy/workspace tests/前端 test+build/openspec strict）只能在本机手动跑；Windows NSIS 安装包与 macOS `.app` 也没有可复现的 CI 构建与发布产物。用户已决定把 CI/release 作为独立 change（`introduce-ci-release-pipeline`）：由代理出设计与 workflow 文件，用户在 Mac 端启用（GitHub 仓库 `Goldloli/image-studio`），本机只做本地可验证部分；push/发布仍由用户明确授权。

## 用户决定（2026-11-03，重申于 2026-11-04）

1. 本 change 只交付 CI/release 设计与 workflow 文件 + 本地可验证证据；首次 push/启用由用户在 Mac 侧操作。
2. 不做代码签名/公证（Windows SmartScreen、macOS Gatekeeper 的提示在首次未签名版本中接受并文档化）；后续如需签名，另立 change。
3. 发布形态：GitHub Release 草稿（draft），附 Windows NSIS 安装包、macOS `.app` zip、SHA256SUMS 与许可证/说明；不自动对公发布。
4. 不在 CI 下载模型权重或运行真实 GPU 推理（无 GPU runner）；模型/生成验收仍以本机证据为准。

## What Changes

- 新增 **quality workflow**（push/PR/手动）：Windows 与 macOS 矩阵跑 Rust fmt、clippy `-D warnings`、workspace tests、前端 `npm ci/test/check/build`、`openspec validate --all --strict`、以及不依赖 GPU/模型的 Python worker 单测；两个平台都构建 sidecar 后再跑 Rust 测试。
- 新增 **build workflows**：Windows 用现有 `scripts/build-windows-internal.ps1` 产出 NSIS 安装包；macOS 用 release 配置构建 sidecar + 桌面并打包 `.app` zip（扩展现有 `prepare-macos-dev-app.py` 支持 `--profile release`）；两者都生成 `SHA256SUMS` 并作为 workflow artifact 上传。
- 新增 **release workflow**：`v*` tag 触发，构建/下载两平台 artifact，创建 GitHub Release **draft**，附安装包、zip、`SHA256SUMS`、许可证与简版 release notes；默认无 secrets。
- 新增文档 `doc/14-ci-release.md`：workflow 说明、缓存策略、不可在 CI 跑的内容、用户启用步骤（推送仓库、在 GitHub 打开 Actions、tag 前检查）、未签名产物提示。
- README 增加 CI/发布章节与 badge 占位（badge 指向 workflow 文件，push 后生效）。

## 成功标准

- 3 类 workflow 的 YAML 在本地通过语法/结构校验（`yamllint` 或等效），workflow 内引用的脚本/命令在本机可执行（`--dry-run` 级验证：把 workflow 命令逐条在本机跑通/记录）。
- Windows/macOS 构建脚本在本地按 CI 相同参数成功产出安装包/`.app` zip 与 `SHA256SUMS`；openspec strict 通过；全量质量门绿。
- 文档给出用户在 Mac 启用与首次运行/发布的具体步骤；未签名/无 secrets 的限制明确。
- 首次真实 Actions run 与 Release 草稿作为用户启用后的证据（本机无法代跑，标记 user-gated，不冒充通过）。

## 不做 / 超出范围

- 不做代码签名、公证、自动更新、商店上架、私有 runner、GPU 推理 CI。
- 不在 CI 下载/缓存模型权重（体积与许可问题）；不做性能基准。
- 不自动 push 仓库或创建线上 release（需要用户明确授权与 GitHub 侧启用）。
