# Image Studio {{TAG}}

Image Studio is a local-first image creation desktop app built with Rust,
Tauri, and React. This release contains unsigned builds for Windows x64 and
Apple Silicon macOS.

Image Studio 是使用 Rust、Tauri 与 React 构建的本地优先桌面图片创作工作台。
本次发布包含 Windows x64 与 Apple Silicon macOS 构建，当前产物均未签名。

## Downloads / 下载

- **Windows x64**
  `Image.Studio_{{VERSION}}_x64-setup.exe`
- **macOS 11+ Apple Silicon**
  `Image-Studio-{{VERSION}}-macos-arm64.zip`
- Verify with `SHA256SUMS-windows.txt` or `SHA256SUMS-macos.txt`.
  请使用对应平台的 `SHA256SUMS-*.txt` 校验文件。

```bash
sha256sum -c SHA256SUMS-windows.txt
sha256sum -c SHA256SUMS-macos.txt
```

## Installation / 安装

### Windows

The installer uses a current-user NSIS package. SmartScreen may show
“Windows protected your PC” because the build is unsigned. Verify the SHA-256
first, then use **More info → Run anyway**.

安装包采用 current-user NSIS。由于未签名，SmartScreen 可能提示
“Windows 已保护你的电脑”；请先核对 SHA-256，再选择“更多信息 → 仍要运行”。

### macOS

The app is unsigned and not notarized. Gatekeeper may block the first launch.
Verify the SHA-256, then Control-click the app and choose **Open**, or run:

应用未签名、未公证，Gatekeeper 可能阻止首次启动。请先核对 SHA-256，然后
Control-click 应用并选择“打开”，或运行：

```bash
xattr -dr com.apple.quarantine "Image Studio.app"
```

## What Is Included / 包含内容

- Seven local image models covering text-to-image, single-image editing,
  background removal with RGBA/mask output, and 4× upscaling.
- A conversation-based creation workspace, works library, task center, model
  manager, and persistent settings.
- Verified Rust/Tauri desktop hosts for Windows x64 and macOS arm64.
- 七个本地图片模型，覆盖文生图、单图编辑、去背景 RGBA/mask 与 4× 放大。
- 包含会话式创作、作品库、任务中心、模型管理与持久设置。

Model weights and private Python runtimes are downloaded separately from fixed
upstream sources and are **not bundled** in these archives.

模型权重和私有 Python 运行时不随安装包分发，需要在应用内从固定上游源安装。

## Validation / 验证

The release was built and checked by GitHub Actions on Windows x64 and macOS
arm64. Workflow checks cover Rust formatting, Clippy with warnings denied,
workspace tests, frontend tests/build, OpenSpec strict validation, package
architecture, checksums, version consistency, and tag provenance.

本版本由 GitHub Actions 在 Windows x64 与 macOS arm64 runner 上构建和检查；
覆盖 Rust 格式、Clippy、workspace 测试、前端测试/构建、OpenSpec strict、
包架构、checksums、版本一致性及 tag 来源。CI 不下载模型权重，也不执行 GPU
推理；真实模型验收证据保存在仓库 `verification/`。

## License / 许可证

Image Studio is licensed under Apache-2.0. See `LICENSE`, `NOTICE`, and
`THIRD_PARTY_NOTICES.md`. Model and runtime materials remain subject to their
upstream licenses.

Image Studio 采用 Apache-2.0；模型与运行时材料继续遵循各自上游许可证。
