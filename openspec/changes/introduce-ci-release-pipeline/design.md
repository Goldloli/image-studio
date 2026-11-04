# Design: introduce-ci-release-pipeline

## 触发与权限

| workflow | 触发 | 权限 | 说明 |
|---|---|---|---|
| `quality.yml` | push main、PR、workflow_dispatch | `contents: read` | 双平台质量门，不产出发布物 |
| `build.yml` | workflow_dispatch、tag `v*` | `contents: read` | 构建 Windows NSIS 与 macOS `.app` zip，上传 artifact + SHA256SUMS |
| `release.yml` | tag `v*`、workflow_dispatch（需要显式输入 tag） | `contents: write` | 下载两平台 artifact，创建 draft Release |

- 所有 workflow 设置 `concurrency`（同 ref 取消旧 run）与 `timeout-minutes`（质量 45、构建 90）。
- Release job 仅在 `refs/tags/v*` 或手动输入时运行，避免 PR 分支拿到写权限。
- 默认零 secrets；签名/公证留空并在文档中说明。

## 版本与缓存

- Rust：仓库 `rust-toolchain.toml` 固定 1.99.1；CI 使用 `dtolnay/rust-toolchain` 读取或显式 `rustup toolchain install 1.99.1`，先 `rustc -vV` 打印版本。
- Node：`actions/setup-node`，`node-version-file` 或显式 `24.19.0`，`cache: npm`、`cache-dependency-path: apps/frontend/package-lock.json`。
- OpenSpec：`npm install -g @fission-ai/openspec@1.12.0`（与本地一致），跑 `openspec validate --all --strict`。
- Python（可选质量 job）：`actions/setup-python` 3.12 + `pip install pytest`；只跑 import 守卫允许的纯 Python 单测（`pytest.importorskip` 会跳过缺少 torch/diffusers 的用例），GPU/模型用例标记 skipped，不伪装通过。
- Cargo/npm 缓存使用 actions cache；不缓存模型权重、不缓存 `target/` 发布物（artifact 单独上传）。

## 质量 workflow 步骤（两平台一致）

1. checkout；
2. 安装工具链/Node/OpenSpec；
3. `npm ci`（apps/frontend）；
4. 构建 sidecar：`cargo build --locked -p inference-supervisor -p diagnostic-worker -p realesrgan-worker`，按平台复制为 `apps/desktop/binaries/<bin>-<triple>[.exe]`（Windows `x86_64-pc-windows-msvc`，macOS `aarch64-apple-darwin`）；
5. `cargo fmt --all -- --check`；`cargo clippy --workspace --all-targets --locked -- -D warnings`；`cargo test --workspace --locked`；
6. `npm run test`、`npm run check`、`npm run build`；
7. `openspec validate --all --strict`；
8. （可选）`pytest` 纯 Python 子集。

macOS runner（`macos-14`，arm64）本机无 GPU，模型/生成测试不在 CI 范围；Rust/前端/规格门与产物构建是 CI 的目标。

## 构建 workflow

### Windows

- 复用 `scripts/build-windows-internal.ps1`（内部会做前端 build、sidecar 构建、NSIS 打包与包校验，并生成 `verification/windows-installer/internal-package.json`）。
- CI 传 `-SkipQualityGates`（质量已由 quality workflow 跑过），产物：`target/release/bundle/nsis/Image Studio_0.1.0_x64-setup.exe`。
- 生成 `SHA256SUMS`（安装包 + internal-package.json），上传 artifact `windows-installer`。

### macOS

- `cargo build --release --locked --workspace` 构建桌面与 sidecar；
- 扩展现有 `scripts/prepare-macos-dev-app.py` 增加 `--profile {debug,release}`（默认 debug 保持现状），release 时从 `target/release` 复制；
- 打包 `target/release/Image Studio.app` 为 zip（`ditto -c -k --keepParent`），生成 `SHA256SUMS`，上传 artifact `macos-app`。
- 产物未签名/未公证：文档说明首次打开需右键打开或 `xattr -dr com.apple.quarantine`，并标注后续可另立签名 change。

## 发布 workflow

- tag `v*` → 下载 `windows-installer` 与 `macos-app` artifacts；
- 用 `softprops/action-gh-release` 或 `gh release create --draft` 创建 draft Release，附：
  - Windows NSIS 安装包 + SHA256SUMS；
  - macOS `.app` zip + SHA256SUMS；
  - `LICENSE`/第三方许可摘要与简版 release notes（从 `doc/14-ci-release.md` 模板生成）。
- 不自动发布；用户在 GitHub 上审阅 draft 后手动 publish。
- 无 secrets；若后续启用签名，新增 secrets 文档与独立 change。

## 本地验证（本机可做）

- workflow YAML 语法/结构校验（`yamllint`；如可用再跑 `actionlint` 的全量 check）；
- 把 workflow 中的命令逐条在本机执行：Windows 侧 `scripts/build-windows-internal.ps1` 已多次成功；macOS 侧命令在 Windows 上无法执行，标记为 user-gated（由用户在 Mac 启用时验证），但脚本参数变更可在本机做静态检查/单测。
- `openspec validate --all --strict` 与全量质量门。

## 风险与权衡

| 风险 | 处理 |
|---|---|
| CI 首次运行失败（缓存/工具链/NSIS 下载） | 文档给排查步骤；workflow 打印版本与下载日志；用户启用后按 run 日志迭代 |
| macOS 未签名产物体验差 | 文档明确；签名/公证另立 change |
| CI 无法覆盖 GPU/模型行为 | 明确写"CI 不做模型验收"，本机证据继续作为发布依据 |
| YAML 无法本地真跑 | actionlint/yamllint + 命令清单逐条本机验证；真实 run 标记 user-gated |
