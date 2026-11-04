# CI 与发布（GitHub Actions）

本仓库的 CI/release 由三个 workflow 组成，面向私有仓库 `Goldloli/image-studio`。workflow 文件由本仓库维护；**首次启用、push 与 tag 发布需要仓库所有者授权**，代理不在本地自动 push。

## Workflow 一览

| 文件 | 触发 | 作用 | 产物 |
|---|---|---|---|
| `.github/workflows/quality.yml` | push `main`、PR、手动 | Windows + macOS 矩阵跑 rustfmt、clippy `-D warnings`、workspace 测试、前端 `npm ci/test/check/build`、`openspec validate --all --strict` | 无（质量信号） |
| `.github/workflows/build.yml` | 手动、被 release 调用 | Windows：`scripts/build-windows-internal.ps1 -SkipQualityGates` 产出 NSIS；macOS：`cargo build --release` + `scripts/prepare-macos-dev-app.py --profile release` 打包 `.app` zip | `windows-installer`、`macos-app`（各含 `SHA256SUMS-*.txt`） |
| `.github/workflows/release.yml` | tag `v*` | 调用 build workflow，校验 tag 与 `apps/desktop/tauri.conf.json` 版本一致，创建 **draft** GitHub Release | 两平台安装包/zip + checksums + 自动 release notes |

所有 workflow 使用固定版本：Rust **1.99.1**、Node **24.19.0**、OpenSpec CLI **1.12.0**。质量 workflow 不下载模型权重、不跑 GPU 推理；模型与生成验收仍以 `verification/windows-parity-*`、`verification/windows-data-root-guard/` 等本机证据为准。

## 干净检出与产物布局（review 修正）

- **前端必须先构建**：`tauri::generate_context!` 在编译期嵌入 `apps/frontend/dist`，干净检出时该目录不存在会直接 panic。quality job 与 macOS build job 都在任何 cargo 步骤之前执行 `npm run build`；Windows build 由 `scripts/build-windows-internal.ps1` 内部先构建前端。
- **artifact 必须是平铺目录**：`actions/upload-artifact` 以搜索路径的最近公共祖先作为 artifact 根；把仓库根文件与 `target/release/...` 混在一起会让下载结果包含目录，`gh release create ... artifacts/*` 会把目录当文件而失败。build job 现在先复制到 `target/ci-artifacts/{windows,macos}/` 再上传，release 用 `find artifacts -type f` 只传文件。
- **校验和用 LF**：Windows job 用 `System.IO.File::WriteAllText` 写 `SHA256SUMS-windows.txt`（避免 CRLF 让 Ubuntu 的 `sha256sum -c` 找不到文件）；release job 在创建 draft 前执行 `sha256sum -c SHA256SUMS-{windows,macos}.txt`。
- **先校验 tag 再构建**：release.yml 有独立的 `verify-version` job，版本不匹配在几分钟内失败，不会白跑两个 90 分钟构建。
- **版本号不硬编码**：Windows 安装包文件名由 `apps/desktop/tauri.conf.json` 的 version 推导，版本升级不需要改 workflow。
- 第三方 action 目前使用主版本 tag（私有仓库、无 secrets 的 v1）；升级为 commit SHA 固定作为后续加固项。

## 未签名产物（v1 已知限制）

当前 release 不包含代码签名与公证：

- **Windows**：SmartScreen 会提示"Windows 已保护你的电脑"，选择"更多信息 → 仍要运行"。
- **macOS**：Gatekeeper 会阻止直接打开；右键 → 打开，或执行
  `xattr -dr com.apple.quarantine "Image Studio.app"`。
- 签名/公证需要证书与 secrets，另行立项（不阻塞本流水线）。

## 用户启用步骤（需要 push 授权）

1. 确认本地提交已就绪：`git status`、`git log --oneline origin/main..HEAD`。
2. 授权并 push 到 GitHub（本仓库保守策略：push 需明确授权）：
   `git push origin main`
3. 在 GitHub 仓库打开 **Actions**，允许 workflow 运行；先手动跑一次：
   - Actions → `quality` → Run workflow；
   - Actions → `build-artifacts` → Run workflow，下载两平台 artifact 并核对 `SHA256SUMS-*.txt`。
4. 检查 `apps/desktop/tauri.conf.json` 的 version 与计划 tag 一致，然后推送 tag：
   `git tag v0.1.0 && git push origin v0.1.0`
5. `release-draft` 会创建 **draft** Release；在 GitHub 上人工审阅 notes/产物后手动 Publish。
6. 把首次 run 的 URL 与 artifact 记录写入 `verification/ci-release/`。

## 本地可验证项

- workflow YAML 结构与语法：`python scripts/ci/validate-workflows.py`（PyYAML 解析 + 触发/jobs/steps 结构检查；随仓库提交）。
- 干净检出 blocker 已本机复现：移走 `apps/frontend/dist` 后 `cargo check -p image-studio-desktop` 报 `frontendDist … doesn't exist`；workflow 已把 `npm run build` 放到所有 cargo 步骤之前。
- workflow 中的命令逐条在本机执行过：rustfmt、clippy、workspace 测试、前端 test/check/build、openspec strict、Windows 内部安装包构建（含 CI 同参数 `-SkipQualityGates`）。
- Windows build job 的 staging/checksums 逻辑已在本机用相同 PowerShell 片段跑通（平铺目录 + LF checksums）。
- macOS 打包脚本的 `--profile release` 参数在 Windows 上只能做编译/`--help` 静态验证；真实 macOS 构建与首次 Actions run 标记为 user-gated。

## 故障排查

| 现象 | 处理 |
|---|---|
| Windows job 下载 NSIS/WebView2 失败 | `scripts/build-windows-internal.ps1` 内置重试；确认 runner 可访问 GitHub 与 Microsoft 下载源，重跑 job |
| Rust 缓存导致旧工具链 | 检查 `rustc -V` 步骤输出；必要时清除 Actions cache |
| clippy/测试在 runner 上失败但本机通过 | 检查 Windows/macOS 差异与路径大小写；保留 run 日志到 `verification/ci-release/` |
| release job 版本校验失败 | 修改 `tauri.conf.json` version 或重打 tag，二者必须一致 |
| macOS artifact 打不开 | 未签名预期行为；按上文右键打开/xattr 说明处理 |

## 不在本流水线范围

- 代码签名、公证、公证后的自动更新。
- 模型权重下载、GPU 推理、真实生成验收。
- 自动 push、自动发布（release 始终 draft）。
