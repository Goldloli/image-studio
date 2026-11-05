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

## 6. 2026-11-05 Mac 审查阻断

- [x] 6.1 记录本机审查证据：`macos-14` 已退役且为 x64；macOS Clippy 13 项告警失败；并行 workspace 测试偶发 execution-lease 失败；顺序执行通过。
- [ ] 6.2 将 quality/build macOS runner 改为当前受支持的 arm64 `macos-15`，并在 job 中断言 `uname -m=arm64`。
- [ ] 6.3 修复 macOS `-D warnings` 的平台条件编译、dead code 与无效 drop，确保 `cargo clippy --workspace --all-targets --locked -- -D warnings` 通过。
- [ ] 6.4 修复 execution-lease 并行测试不稳定；默认并行 suite 连续至少 5 次通过。

## 7. 公开仓库与 Actions 供应链

- [x] 7.1 在 OpenSpec 修订展示后按用户授权把 `Goldloli/image-studio` 切为 public，并记录公开时间与确认无硬编码凭据。
- [ ] 7.2 所有 actions 固定到完整 commit SHA；checkout `persist-credentials: false`；quality/build 只读。
- [ ] 7.3 release workflow 默认只读，只有最终 draft Release job 获得 `contents: write`。
- [x] 7.4 在公开后检查 secret scanning、vulnerability alerts、main 分支保护与 Actions 权限；记录仍需人工配置的项。
- [ ] 7.5 更新 README/CI 文档，移除“私有”和错误的 `macos-14 arm64` 描述；披露历史证据中的本机路径与无根 LICENSE 现状。

## 8. 首次 CI 与双平台 Release 生成

- [ ] 8.1 在任何构建前校验 tag、Cargo workspace、Tauri 与前端 package version 一致，并确认 tag commit 属于 `main`。
- [ ] 8.2 macOS build 验证四个 sidecar 的 Mach-O 架构、`Info.plist`、正式 bundle identifier `io.github.goldloli.image-studio`、zip 结构与 SHA-256。
- [ ] 8.3 Windows build 验证 installer target、字节数和 internal-package SHA-256；NSIS/Tauri utils 下载改用固定 SHA-256。
- [ ] 8.4 draft Release 附两平台产物、checksums、README、第三方许可摘要、未签名说明和简版 notes；不得自动 publish。
- [ ] 8.5 运行首次 quality 与手动 build；下载两平台 artifact、复算 checksum、确认 macOS arm64，并把 run/artifact 记录落到 `verification/ci-release/`。
- [ ] 8.6 推送与 `tauri.conf.json` 一致的 `v*` tag，确认 draft Release 内容；发布动作仍由用户人工完成。

## 9. 收口与归档

- [ ] 9.1 全量本地质量门、actionlint、workflow 安全扫描、OpenSpec strict 全部通过。
- [ ] 9.2 用 GitHub 真实 run URL、artifact checksum、draft Release URL 补齐验收证据。
- [ ] 9.3 归档本 change，同步 `ci-release` 与 `product-identity` 主规格。
- [ ] 9.4 关闭 Beads 实现 issue，并报告所有 user-gated 或仍需人工配置的项。
