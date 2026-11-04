# Tasks: introduce-ci-release-pipeline

## 1. 文档与用户确认

- [ ] 1.1 proposal/design/tasks/test-plan 与 spec delta 完成，等用户确认后再写实现（workflow/脚本）。
- [ ] 1.2 用户确认后创建 bd issue 关联（`image-studio-q2y`）。

## 2. 质量 workflow

- [ ] 2.1 `.github/workflows/quality.yml`：Windows/macOS 矩阵（checkout、工具链 1.99.1、Node 24.19.0、OpenSpec 1.12.0、sidecar 构建、fmt/clippy/workspace tests、前端 test/check/build、openspec strict）。
- [ ] 2.2 可选 Python 子集 job：setup-python 3.12 + pytest，import 守卫缺失依赖的用例跳过；明确不是模型验收。
- [ ] 2.3 本地校验：YAML 语法（yamllint）、workflow 内命令逐条在本机跑通/记录；`openspec validate --all --strict`。

## 3. 构建 workflow

- [ ] 3.1 `.github/workflows/build.yml`：workflow_dispatch + tag `v*`，Windows job 复用 `scripts/build-windows-internal.ps1 -SkipQualityGates`，上传安装包 + `SHA256SUMS`。
- [ ] 3.2 macOS：`prepare-macos-dev-app.py` 增加 `--profile {debug,release}`（默认 debug 不回归），release 从 `target/release` 复制；CI 构建 sidecar + `.app`，`ditto` 打 zip，上传 + `SHA256SUMS`。
- [ ] 3.3 本机验证：Windows 路径按 CI 参数重跑安装包构建；macOS 脚本参数/单测在 Windows 上静态验证并记录 user-gated。

## 4. 发布 workflow

- [ ] 4.1 `.github/workflows/release.yml`：tag `v*` 触发、`contents: write`、下载两平台 artifacts，创建 draft Release（安装包、zip、SHA256SUMS、许可证、release notes 模板）。
- [ ] 4.2 tag/版本一致性检查（`tauri.conf.json` version 与 tag 匹配），失败即 fail。
- [ ] 4.3 本地验证 release 描述模板与 checksum 脚本（在本机生成一次 SHA256SUMS 样例）。

## 5. 文档与质量门

- [ ] 5.1 `doc/14-ci-release.md`：workflow 说明、缓存、不可在 CI 跑的内容、用户启用步骤、未签名提示、故障排查。
- [ ] 5.2 README 增加 CI/发布章节与 badge 占位。
- [ ] 5.3 全量质量门：fmt/clippy/tests/前端/openspec strict 全绿；提交 workflow 与文档。

## 6. 用户启用与收口

- [ ] 6.1 用户在 Mac 侧 push 仓库并启用 Actions（需用户明确授权 push/首跑）。
- [ ] 6.2 首次 quality/build run 通过；tag 出 draft Release；把 run URL 与 evidence 落到 `verification/ci-release/`。
- [ ] 6.3 归档 change，同步 `ci-release` 主规格，关闭 bd `image-studio-q2y`。
