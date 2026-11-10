# CI 与发布（GitHub Actions）

仓库 `Goldloli/image-studio` 为 public。CI/release 由三个 workflow 组成，所有第三方 action 固定到完整 commit SHA；quality/build 仅有 `contents: read`，只有最终 draft Release job 获得 `contents: write`。所有 checkout 设置 `persist-credentials: false`。

## Workflow 一览

| 文件 | 触发 | 作用 |
|---|---|---|
| `.github/workflows/quality.yml` | push `main`、PR、手动 | Windows x64 + macOS 15 arm64 的 fmt、Clippy `-D warnings`、workspace tests、前端 test/check/build、OpenSpec strict |
| `.github/workflows/build.yml` | 手动、被 release 调用 | Windows NSIS 与 macOS `.app` zip、架构/包身份检查、SHA256SUMS artifact |
| `.github/workflows/release.yml` | tag `v*` | 版本/tag/main 来源检查，复用双平台构建，创建 draft GitHub Release |

固定工具版本为 Rust **1.99.1**、Node **24.19.0**、npm **11.17.0**、OpenSpec **1.12.0**。CI 不下载模型权重、不运行 GPU 推理；模型和真实生成验收继续以本机证据为准。

## macOS runner 与产物

- `macos-14` 是 Intel x64，并已于 2026-11-02 退役；workflow 使用官方 arm64 `macos-15`，并在 job 中执行 `uname -m=arm64`。
- Windows 产物为目标 triple `x86_64-pc-windows-msvc`；macOS 产物为 Apple Silicon arm64，最低系统版本 macOS 11。
- macOS bundle identifier 为 `io.github.goldloli.image-studio`。`.app` 包含桌面、诊断、监督与 Real-ESRGAN sidecar，以及项目许可证、字体/提示词库/第三方声明。
- `.app` 先完整验证再 zip；发布 job 再次解包检查 `Info.plist`、四个 Mach-O、许可证资源和版本。

## 版本与来源

以下四项必须完全一致：

1. Git tag 去掉前缀 `v`；
2. 根 `Cargo.toml` 的 `[workspace.package].version`；
3. `apps/desktop/tauri.conf.json` 的 `version`；
4. `apps/frontend/package.json` 的 `version`。

tag commit 还必须属于 `main`。任一条件不满足时，release 在 Windows/macOS 长构建前失败。

## Artifact 与 draft Release

- Windows artifact：NSIS current-user installer、LF `SHA256SUMS-windows.txt`、`internal-package.json`。
- macOS artifact：`Image-Studio-<version>-macos-arm64.zip`、LF `SHA256SUMS-macos.txt`。
- 发布 job 先执行两平台 checksum，再解包 macOS 包做身份/架构/资源检查。
- draft Release 附两平台产物、checksums、README、`LICENSE`、`NOTICE`、`THIRD_PARTY_NOTICES.md` 与 `RELEASE-NOTES.md`。
- 不自动 publish。同名 Release 已存在时，重跑会明确失败，避免生成部分 release。

`v0.1.0` 已于 2026-11-05 人工审阅并公开：
<https://github.com/Goldloli/image-studio/releases/tag/v0.1.0>。公开页包含 Windows x64
NSIS 安装包、macOS arm64 zip、两平台 checksum、双语 `RELEASE-NOTES.md` 与许可文件。
workflow 后续仍默认创建 draft，由维护者检查后发布。

`v0.1.1` 已于 2026-11-10 人工审阅并公开：
<https://github.com/Goldloli/image-studio/releases/tag/v0.1.1>。tag 提交 `069c6e2`，
同一提交上的 quality run 37 双平台通过，release run 3（版本来源检查、双平台构建、
draft 发布）全部成功。公开页包含 Windows x64 NSIS 安装包
`Image.Studio_0.1.1_x64-setup.exe`（245,148,864 B）、macOS arm64 zip
`Image-Studio-0.1.1-macos-arm64.zip`（32,516,275 B）、两平台 checksum、双语
`RELEASE-NOTES.md` 与许可文件。下载复验通过：两个 checksum 文件均 OK，Windows
安装包 SHA-256 与 CI 生成的 `internal-package.json` 一致，macOS `.app` 的
identifier、版本、最低系统版本与 arm64 架构均正确。验收记录见
[`verification/ci-release/v0.1.1/`](../verification/ci-release/v0.1.1/README.md)。

下载后应在对应页签执行：

```bash
sha256sum -c SHA256SUMS-windows.txt
sha256sum -c SHA256SUMS-macos.txt
```

## 未签名产物

当前 release 不包含代码签名与公证：

- Windows SmartScreen 可能提示“Windows 已保护你的电脑”；核对 checksum 后使用“更多信息 → 仍要运行”。
- macOS Gatekeeper 可能阻止首次启动；核对 checksum 后右键打开，或运行
  `xattr -dr com.apple.quarantine "Image Studio.app"`。

签名/公证需要证书与 secrets，应另立 change。

## 公开仓库安全

- Secret scanning、push protection、依赖漏洞告警与 Dependabot security updates 已启用；目标是把 Security 告警保持为 0。
- quality/build 对 fork PR 只有只读 token，不接触 secrets。
- Actions 缓存只用于 quality/build；发布 artifact 来自当前 tag run，并由 SHA-256 再次验证。
- main 分支保护、真实 runs、Dependabot 合并结果和公开 Release 验收记录在 `verification/ci-release/`。

## 本地验证

```bash
python3 scripts/ci/validate-workflows.py
actionlint -no-color
uvx zizmor .github/workflows
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm --prefix apps/frontend test
npm --prefix apps/frontend run check
npm --prefix apps/frontend run build
openspec validate --all --strict
```

Zizmor 的固定版本提示应保持 0 high/medium；当前仅允许已有固定版本全局 npm/OpenSpec 安装与同仓库 workflow 引用产生 low/help。真实 GitHub run 是最终验收，不以本地 YAML 结构校验替代。

## 故障排查

| 现象 | 处理 |
|---|---|
| macOS job 未调度 | 确认使用 `macos-15`；检查 GitHub runner 标签是否仍受支持 |
| Clippy 只在 macOS 失败 | 重新跑两平台 `--all-targets`，检查 Windows-only cfg/dead code |
| 版本校验失败 | 同步 tag、Cargo、Tauri 与前端 package version |
| tag 不属于 main | 将 tag 移到已合入 main 的发布提交 |
| macOS 产物非 arm64 | 检查 runner host、Cargo target 与四个 sidecar 的 `file` 输出 |
| `gh release create` 提示已存在 | 审阅或删除同名 draft 后重跑；不要覆盖已发布版本 |
| 下载包被系统拦截 | 校验 SHA-256；未签名限制见上文 |

## 不在流水线范围

- 模型权重下载、GPU 推理、真实图片质量验收。
- 代码签名、公证、自动更新、商店上架。
- 自动 publish 或绕过人工 draft 审阅。
