# Tasks: introduce-ci-release-pipeline

## 1. 文档与用户确认

- [x] 1.1 proposal/design/tasks/test-plan 与 spec delta 完成；用户确认"两个都按顺序做"后开始实现（2026-11-04）。
- [x] 1.2 关联 bd issue `image-studio-q2y`。

## 2. 质量 workflow

- [x] 2.1 `.github/workflows/quality.yml`：Windows/macOS 矩阵（checkout、Rust 1.99.1、Node 24.19.0、OpenSpec 1.12.0、fmt/clippy/workspace tests、前端 test/check/build、openspec strict）。
- [x] 2.2 Python worker 子集 job 未纳入 v1：测试依赖 torch/diffusers（GPU/模型许可），文档明确为本地运行；workflow 只覆盖 Rust/前端/规格门。
- [x] 2.3 本地校验：YAML 结构（PyYAML）、workflow 命令逐条本机执行记录、`openspec validate --all --strict` 24/24。证据 `verification/ci-release/local-validation.json`。

## 3. 构建 workflow

- [x] 3.1 `.github/workflows/build.yml`：workflow_call + workflow_dispatch；Windows job 复用 `scripts/build-windows-internal.ps1 -SkipQualityGates`，上传安装包 + `SHA256SUMS-windows.txt` + internal-package.json。
- [x] 3.2 macOS：`prepare-macos-dev-app.py` 新增 `--profile {debug,release}`（默认 debug 不回归，release 从 `target/release` 复制并写真实版本号）；CI 构建 sidecar + `.app`，`ditto` 打 zip，上传 + `SHA256SUMS-macos.txt`。
- [x] 3.3 本机验证：Windows 同参数构建成功（installer `fb37d774…`, 245,021,486 B，checksums 与 internal-package 一致）；macOS 脚本 py_compile/--help 通过并标记 user-gated。

## 4. 发布 workflow

- [x] 4.1 `.github/workflows/release.yml`：tag `v*` 触发，复用 build workflow，下载 artifacts，`gh release create --draft --generate-notes` 附两平台产物与 checksums。
- [x] 4.2 tag 与 `tauri.conf.json` version 一致性校验（失败即 fail）。
- [x] 4.3 本地验证 checksum 生成逻辑（`verification/ci-release/windows-SHA256SUMS.sample`）与 release notes 文档。

## 5. 文档与质量门

- [x] 5.1 `doc/14-ci-release.md`：workflow 说明、缓存、不可在 CI 跑的内容、用户启用步骤、未签名提示、故障排查。
- [x] 5.2 README 增加 CI/发布章节与 quality badge。
- [x] 5.3 质量门：fmt/clippy/workspace tests/前端/openspec strict 全绿（本机验证记录）。

## 6. 用户启用与收口

- [ ] 6.1 用户 push 仓库并启用 Actions（需明确授权 push/首跑）。
- [ ] 6.2 首次 quality/build run 通过；tag 出 draft Release；把 run URL 与 artifact checksum 落到 `verification/ci-release/`。
- [ ] 6.3 归档 change，同步 `ci-release` 主规格，关闭 bd `image-studio-q2y`。
