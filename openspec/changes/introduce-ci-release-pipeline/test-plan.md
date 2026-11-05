# Test plan: introduce-ci-release-pipeline

## 原始本机可验证项

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| C1 | workflow YAML 语法 | yamllint 通过（如可用再跑 actionlint） | 本地命令输出 |
| C2 | 质量命令清单 | workflow 中的 fmt/clippy/test/前端/openspec 命令在本机逐条通过 | 日志 |
| C3 | Windows 构建脚本同参数 | `scripts/build-windows-internal.ps1 -SkipQualityGates` 产出安装包 + SHA256SUMS 样例 | 产物 + checksum 文件 |
| C4 | macOS 打包脚本静态检查 | `--profile release` 参数解析与路径常量单测/静态检查通过；标注 Windows 上不可执行 | 单元测试 |
| C5 | release 模板/checksum 脚本 | 本机生成一次 SHA256SUMS 样例并通过校验 | 文件 |
| C6 | 质量门 | fmt/clippy/tests/前端/openspec strict 全绿 | 日志 |

## 用户启用后验证（Mac / GitHub）

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| C7 | push 后 quality workflow | Windows/macOS 两个 job 全绿 | Actions run URL |
| C8 | workflow_dispatch build | 两平台 artifact 与 SHA256SUMS 可下载 | run artifacts |
| C9 | tag `v*` release | draft Release 含两平台产物 + checksums，未自动发布 | release URL |
| C10 | 未签名产物说明 | 文档与 release notes 明确 SmartScreen/Gatekeeper 提示 | 文档 |

## 2026-11-05 Mac 审查基线

| ID | 场景 | 当前结果 | 证据/说明 |
|---|---|---|---|
| R1 | macOS runner 可调度性与架构 | fail | 官方 runner 清单：`macos-14` 为 x64，已于 2026-11-02 退役；当前 workflow 使用 `macos-14` 且错误命名为 arm64 |
| R2 | macOS Clippy `-D warnings` | fail | 13 项错误：Windows-only unused/dead code、`drop(())` 等 |
| R3 | 默认并行 workspace tests | flaky | 首次运行 `existing_lease_blocks_launch_and_does_not_claim_resource_release` 失败；单测和 `--test-threads=1` 通过 |
| R4 | workspace release build | pass | `cargo build --release --locked --workspace` 成功，但产生 13 项 warning |
| R5 | macOS 手工 `.app` 打包 | pass | `prepare-macos-dev-app.py --profile release` 成功；四个 Mach-O 为 arm64，`Info.plist` lint 通过 |
| R6 | 当前 bundle identity | fail/decision | 当前 identifier 为开发值 `local.image-studio.foundation.dev` |
| R7 | workflow 语法 | pass | `actionlint -no-color` 0 错误 |
| R8 | workflow 公开仓库安全 | fail | zizmor 31 findings；含 unpinned actions、checkout credential persistence、workflow-level release write、cache policy |
| R9 | 密钥扫描 | pass with disclosure | Git 历史未命中常见 GitHub/AWS/private-key 模式；当前树有 225 个 macOS 本机路径与 50 个 Windows 用户名路径 |
| R10 | 前端/API/规格门 | pass | frontend 58 tests、typecheck、build；`npm audit` 0 vulnerabilities；OpenSpec strict 24/24 |

R1–R3、R6 与 R8 已在本地修复：workflow 使用 `macos-15` arm64、macOS Clippy 全绿、默认并行测试连续 5 次通过、正式 bundle identity 已写入并验证、Actions 无 high/medium 安全发现。对应真实 GitHub runner 行仍待 C11–C19 验证。

2026-11-05 真实运行已完成：最终 `main` quality（41979600295）、手动双平台 build（41976972819）和 `v0.1.0` draft Release（41979625386）全部成功；两平台下载产物 checksum、Windows metadata 与 macOS arm64/包身份已独立复核。详见 [GitHub Actions acceptance](../../../verification/ci-release/github-actions-acceptance-2026-11-05.json)。

## 修复后新增验收

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| C11 | macOS runner | `macos-15` 上 `uname -m` 为 arm64，quality/build 均可调度 | Actions run 日志 |
| C12 | macOS Clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` 通过 | run 日志 |
| C13 | 并行测试稳定性 | 默认并行 workspace suite 连续至少 5 次通过 | 本地日志 + CI run |
| C14 | Action 供应链 | 所有 `uses:` 固定完整 SHA；checkout 不持久化凭据 | zizmor/actionlint 与 workflow 静态检查 |
| C15 | 最小权限 | quality/build 无写权限；仅 release job `contents: write` | workflow 静态检查 |
| C16 | 版本与来源 | tag、Cargo、Tauri、package version 一致；tag commit 属于 main | 失败与通过分支测试 |
| C17 | macOS artifact identity | zip 解包后 `Info.plist` identifier、version、最低系统版本、arm64 sidecar 均正确 | `plutil`/`file`/checksum 记录 |
| C18 | Release 内容 | 两平台产物、checksums、README、第三方许可摘要、未签名说明、notes 均存在；仍为 draft | release URL |
| C19 | 公开仓库设置 | secret scanning/vulnerability alerts 状态、分支保护与 Actions 权限已记录 | GitHub 设置/API 输出 |

## 不做

- 不在 CI 跑模型下载/GPU 推理/真实生成。
- 不验证代码签名与公证（后续 change）。
