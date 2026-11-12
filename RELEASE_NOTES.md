# Image Studio {{TAG}}

<!-- GitHub Release notes are bilingual; English appears first. -->

Image Studio `{{VERSION}}` is a maintenance release for Apple Silicon macOS
and Windows x64. It packages the latest dependency updates without changing the
model catalog, inference runtimes, worker identities, recipes, local schema, or
user workflow.

Image Studio `{{VERSION}}` 是面向 Apple Silicon macOS 与 Windows x64 的维护版本。
本版把最新依赖更新纳入安装包，不改变模型目录、推理运行时、worker 身份、配方、
本地数据结构或用户工作流。

## Maintenance Updates / 维护更新

### Desktop and archive dependencies / 桌面与压缩包依赖

- **Native file dialogs:** `rfd` is updated from 0.17.2 to 0.18.0, including
  upstream dialog and macOS file-name/directory handling fixes.
- **Archive handling:** `zip` is updated from 9.2.2 to 9.5.0 and `flate2` from
  1.1.10 to 1.1.11 for the current compatible archive stack.
- **Identifiers:** `uuid` is updated from 1.29.0 to 1.29.2.
- **原生文件对话框：** `rfd` 从 0.17.2 更新到 0.18.0，包含上游对话框与 macOS
  文件名/目录处理修复。
- **压缩包处理：** `zip` 从 9.2.2 更新到 9.5.0，`flate2` 从 1.1.10 更新到
  1.1.11。
- **标识符：** `uuid` 从 1.29.0 更新到 1.29.2。

### Frontend toolchain / 前端工具链

- **Runtime and build dependencies:** Vite 8.3.7, `@vitejs/plugin-react` 6.3.0,
  Lucide React 1.70.0, and Vitest 5.1.0 are now used by the bundled UI and its
  quality gates.
- **前端运行时与构建依赖：** 内嵌界面与质量门现在使用 Vite 8.3.7、
  `@vitejs/plugin-react` 6.3.0、Lucide React 1.70.0 与 Vitest 5.1.0。

## Compatibility / 兼容性

- **No model migration.** All seven model identities, pinned source revisions,
  prepared caches, worker protocols, and execution recipes are unchanged.
- **No local-data migration.** The SQLite schema remains v6; existing
  conversations, works, favourites, deletion state, and installation records
  remain compatible.
- **No new product feature.** The single-rail workspace, work management,
  cancellation, retry, export, single-instance activation, and startup
  diagnostics behave as in 0.2.0.
- **无需迁移模型。** 七个模型身份、固定来源 revision、准备好的缓存、worker 协议
  与执行配方均未改变。
- **无需迁移本地数据。** SQLite schema 仍为 v6；现有会话、作品、收藏、删除状态
  与安装记录继续兼容。
- **没有新增产品功能。** 单层工作区、作品管理、取消、重试、导出、单实例激活与
  启动诊断行为与 0.2.0 一致。

## Validation / 验证

- The real-model and native UI evidence recorded for 0.2.0 remains the behavior
  baseline because model and worker execution code did not change.
- The 0.2.1 release rebuilds both desktop packages on the updated dependency
  set and reruns Rust formatting, Clippy, workspace tests, frontend tests,
  typecheck, production build, OpenSpec validation, package identity checks,
  architecture checks, and checksums.
- GitHub Actions does not download model weights or run GPU inference.
- 由于模型与 worker 执行代码未变，0.2.0 已记录的真实模型与原生界面证据继续作为
  行为基线。
- 0.2.1 在更新后的依赖组合上重新构建双平台桌面包，并重跑 Rust 格式、Clippy、
  workspace 测试、前端测试、类型检查、生产构建、OpenSpec、包身份、架构与
  checksum 检查。
- GitHub Actions 不下载模型权重，也不执行 GPU 推理。

## Downloads / 下载

- **Windows x64:** `Image.Studio_{{VERSION}}_x64-setup.exe`
- **macOS 11+ Apple Silicon:** `Image-Studio-{{VERSION}}-macos-arm64.zip`
- Verify with `SHA256SUMS-windows.txt` or `SHA256SUMS-macos.txt`.
- 请使用对应的 `SHA256SUMS-*.txt` 校验下载文件。

## Installation / 安装

### Windows

The installer uses a current-user NSIS package and embeds the WebView2 offline
installer. The build is unsigned, so SmartScreen may show “Windows protected
your PC”. Verify the SHA-256 first, then use **More info → Run anyway** only if
the digest matches.

安装包使用 current-user NSIS，并内置 WebView2 离线安装器。由于产物未签名，
SmartScreen 可能显示“Windows 已保护你的电脑”。请先核对 SHA-256，只有完全一致时
才选择“更多信息 → 仍要运行”。

### macOS

The macOS build is unsigned and not notarized. If opening a checksum-verified
app shows **“Image Studio.app is damaged and can’t be opened”**, move it to
Applications and remove only the quarantine attribute:

```bash
sudo xattr -rd com.apple.quarantine "/Applications/Image Studio.app"
open "/Applications/Image Studio.app"
```

Do not run this command when the checksum is missing or does not match.

macOS 构建未签名、未公证。如果已核对 checksum 的应用显示
**“Image Studio.app 已损坏，无法打开”**，请把应用放入“应用程序”，然后只移除
该应用包的 quarantine 属性：

```bash
sudo xattr -rd com.apple.quarantine "/Applications/Image Studio.app"
open "/Applications/Image Studio.app"
```

checksum 缺失或与下载文件不一致时不要执行。

## Known Limitations / 已知限制

- The builds are unsigned and not notarized.
- Z-Image Turbo requires a 12 GB card.
- The 8 GB compact tier is certified on Turing sm75 (RTX 2060 SUPER).
- Windows install-time NF4 preparation is RAM/commit based and may require a
  larger virtual-memory setting on low-headroom machines.
- Model weights and private Python runtimes remain separate downloads.
- 产物未签名、未公证。
- Z-Image Turbo 仍要求 12 GB 显卡。
- 8 GB 紧凑档仅在 Turing sm75（RTX 2060 SUPER）上完成认证。
- Windows 安装期 NF4 准备取决于可用内存与 commit 余量，余量很小的机器可能需要
  调大虚拟内存。
- 模型权重与私有 Python 运行时仍需单独下载。

## License / 许可证

Image Studio is licensed under Apache-2.0. Third-party libraries, fonts,
example prompts/images, model weights, and runtimes retain their upstream
licenses. See `LICENSE`, `NOTICE`, and `THIRD_PARTY_NOTICES.md`.

Image Studio 采用 Apache-2.0；第三方库、字体、示例提示词/图片、模型权重与运行时
继续遵循各自上游许可证，见 `LICENSE`、`NOTICE` 与 `THIRD_PARTY_NOTICES.md`。
