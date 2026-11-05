# Design: introduce-ci-release-pipeline

## 触发与权限

| workflow | 触发 | 权限 | 说明 |
|---|---|---|---|
| `quality.yml` | push main、PR、workflow_dispatch | `contents: read` | 双平台质量门，不产出发布物 |
| `build.yml` | workflow_dispatch、被 release 调用 | `contents: read` | 构建 Windows NSIS 与 macOS `.app` zip，上传 artifact + SHA256SUMS |
| `release.yml` | tag `v*` | 默认 `contents: read`；仅最终 job `contents: write` | 验证版本/来源，下载两平台 artifact，创建 draft Release |

- 所有 workflow 设置 `concurrency`；quality 可取消旧 run，release 不取消正在进行的构建。
- timeout 与当前实现一致：quality 60 分钟，构建 90 分钟，release 元数据 job 5–20 分钟。
- 所有 `actions/*` 与第三方 action 固定到完整 commit SHA；checkout 设置 `persist-credentials: false`。
- Release 写权限只授予最终发布 job，verify/build 继续持有只读 token；PR 或 fork 不接触写权限与 secrets。
- 默认零 secrets；签名/公证留空并在文档中说明。

## 版本与缓存

- Rust：仓库 `rust-toolchain.toml` 固定 1.99.1；CI 使用 `dtolnay/rust-toolchain` 读取或显式 `rustup toolchain install 1.99.1`，先 `rustc -vV` 打印版本。
- Node：`actions/setup-node`，`node-version-file` 或显式 `24.19.0`，`cache: npm`、`cache-dependency-path: apps/frontend/package-lock.json`。
- OpenSpec：安装固定 `@fission-ai/openspec@1.12.0`（与本地一致），跑 `openspec validate --all --strict`；CI 记录包版本与安装来源。
- Python（可选质量 job）：`actions/setup-python` 3.12 + `pip install pytest`；只跑 import 守卫允许的纯 Python 单测（`pytest.importorskip` 会跳过缺少 torch/diffusers 的用例），GPU/模型用例标记 skipped，不伪装通过。
- Cargo/npm 缓存使用 actions cache；不缓存模型权重、不缓存 `target/` 发布物（artifact 单独上传）。

## 质量 workflow 步骤（两平台一致）

1. checkout；
2. 安装工具链/Node/OpenSpec；
3. `npm ci`（apps/frontend）；
4. 在任何 Cargo 命令前执行 `npm run build`，因为 `tauri::generate_context!` 编译期嵌入 `apps/frontend/dist`；
5. `cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；`cargo test --workspace --locked`；
6. `npm run test`、`npm run check`；
7. `openspec validate --all --strict`；
8. 输出 rustc/cargo/node/npm/OpenSpec 与 runner 架构。

macOS quality/build 使用官方 arm64 label `macos-15`，并断言 `uname -m=arm64`。CI 无 GPU，模型安装与真实生成仍不在 CI 范围。

### macOS 代码门修复

- NVIDIA timeout、MiB 换算、`nvidia-smi` 解析与 Windows memory probe 只在 Windows 编译；共享测试通过平台无关测试模块保留。
- Windows-only manifest/wheel 常量使用 `#[cfg(windows)]`，macOS 不产生 dead-code 告警。
- `drop(outer_job)` 只在 Windows job object 类型上执行；非 Windows 分支不伪造拥有者。
- `save_gate_draft` 等测试 helper 仅在对应测试平台编译，避免 `lib --all-targets` 的 dead-code。
- execution-lease 测试采用有界等待观察 flock 释放；产品路径仍保持 fail-closed，不通过放宽锁语义掩盖问题。

## 构建 workflow

### Windows

- 复用 `scripts/build-windows-internal.ps1`（内部会做前端 build、sidecar 构建、NSIS 打包与包校验，并生成 `verification/windows-installer/internal-package.json`）。
- CI 传 `-SkipQualityGates`（质量已由 quality workflow 跑过），产物名从 Tauri version 推导，不硬编码 `0.1.0`。
- workflow 校验实际 installer、`internal-package.json` 中的 SHA-256/字节数和 x64 target triple 一致，再生成 LF `SHA256SUMS-windows.txt`。
- NSIS 与 Tauri utils 下载使用固定 SHA-256；官方源优先，镜像仍须命同一下载内容身份。

### macOS

- `cargo build --release --locked --workspace` 构建桌面与 sidecar；
- `macos-15` job 在构建前断言 arm64，`file` 校验四个 Mach-O 均为 arm64；
- `prepare-macos-dev-app.py` 先清理目标 bundle，再复制固定 sidecar 集合与图标；`Info.plist` 使用正式 identifier、显示名、version 与最低系统版本；
- 打包 `Image-Studio-${VERSION}-macos-arm64.zip`，用 `ditto` 时去除无关 extended attributes，生成 LF `SHA256SUMS-macos.txt`；
- 检查 zip 解包后的 bundle 结构、`plutil -lint`、version 与 release tag 一致，再上传 `macos-app`。
- 产物未签名/未公证：文档说明首次打开需右键打开或 `xattr -dr com.apple.quarantine`，并标注后续可另立签名 change。

## 发布 workflow

- `verify-version` 在任何构建前检查 tag 去掉 `v` 后与 Cargo workspace、Tauri、前端 package version 完全一致，并确认 tag commit 位于 `main`；
- build 复用当前 tag commit，上传 `windows-installer` 与 `macos-app` artifacts；
- 用 `gh release create --draft` 创建 draft Release，附：
  - Windows NSIS 安装包 + SHA256SUMS；
  - macOS `.app` zip + SHA256SUMS；
  - 项目 README、第三方许可摘要、未签名使用说明与简版 release notes。
- 不自动发布；用户在 GitHub 上审阅 draft 后手动 publish。
- 发布 job 先验证两平台 SHA256SUMS；若同名 draft 已存在，重跑必须明确复用或失败，不能生成部分 release。
- 无 secrets；若后续启用签名，新增 secrets 文档与独立 change。

## 公开仓库边界

- 仓库切换 public 后，GitHub 托管 runner 的 workflow 仍只使用只读 token；所有第三方 action 以 commit SHA 固定。
- secret scanning、Dependabot/安全更新与分支保护在公开后检查；要求 main 变更通过 quality，release tag 只能由维护者创建。
- 当前历史包含本地 macOS/Windows 用户名与工作目录路径，但未发现 GitHub token、云凭据或私钥。公开表示接受这些非凭据证据随 Git 历史可见。
- 当前仓库没有根 `LICENSE`；本 change 不替用户选择许可证。Release 只承诺附项目 README、第三方许可摘要和未签名说明，不声称项目已采用开源许可证。

## 本地验证（本机可做）

- workflow YAML 语法/结构校验（`yamllint`；如可用再跑 `actionlint` 的全量 check）；
- 把 workflow 中的命令逐条在本机执行：Windows 侧 `scripts/build-windows-internal.ps1` 已多次成功；macOS 侧命令在 Windows 上无法执行，标记为 user-gated（由用户在 Mac 启用时验证），但脚本参数变更可在本机做静态检查/单测。
- `openspec validate --all --strict` 与全量质量门。

## 风险与权衡

| 风险 | 处理 |
|---|---|
| CI 首次运行失败（缓存/工具链/NSIS 下载） | 文档给排查步骤；workflow 打印版本与下载日志；用户启用后按 run 日志迭代 |
| 退役 runner 或架构漂移 | 固定当前支持的 `macos-15` arm64；运行时断言 host 与 Mach-O；产物名由实际架构生成 |
| macOS `-D warnings` 仍受平台条件影响 | 两平台分别以 `--all-targets` 运行 Clippy；平台专属测试也纳入 |
| 并行测试偶发失败 | 连续默认并行执行；对文件锁释放使用有界观察，不降低生产锁安全性 |
| 公开仓库 action 供应链风险 | action SHA 固定、checkout 不持久化凭据、最小权限、release 写入隔离 |
| 发布产物来源错误 | 四项版本一致性 + tag 位于 main + checksums + 下载后架构检查 |
| macOS 未签名产物体验差 | 文档明确；签名/公证另立 change |
| CI 无法覆盖 GPU/模型行为 | 明确写"CI 不做模型验收"，本机证据继续作为发布依据 |
| YAML 无法本地真跑 | actionlint/yamllint + 命令清单逐条本机验证；真实 run 标记 user-gated |
