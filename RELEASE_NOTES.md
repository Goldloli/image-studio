# Image Studio {{TAG}}

<!-- GitHub Release notes are bilingual; English appears first. -->

Image Studio `{{VERSION}}` changes macOS delivery to a native disk image. The
release publishes one arm64 DMG containing the application and an Applications
shortcut, so installation is a drag instead of a zip extraction followed by a
manual move.

Image Studio `{{VERSION}}` 将 macOS 下载方式改为原生磁盘映像。本版发布一个
arm64 DMG，包含应用与“应用程序”快捷方式，安装时直接拖拽即可，无需先解压 zip
再手动复制应用。

## What's New / 更新内容

### macOS DMG delivery / macOS DMG 交付

- **One macOS download:** the release artifact is
  `Image-Studio-{{VERSION}}-macos-arm64.dmg`. Future releases publish no macOS
  zip.
- **Drag-to-install:** the mounted volume contains `Image Studio.app` and an
  `Applications` link.
- **Verified before release:** CI runs `hdiutil verify`, mounts the image read-only,
  checks the Applications link, bundle identifier, version, minimum macOS
  version, arm64 main executable and sidecars, and legal resources.
- **Historical releases remain unchanged:** `v0.2.1` and earlier keep their
  original zip attachments.
- **单一 macOS 下载包：** 发布产物为
  `Image-Studio-{{VERSION}}-macos-arm64.dmg`；后续版本不再发布 macOS zip。
- **拖拽安装：** 打开的磁盘映像包含 `Image Studio.app` 与 `Applications`
  快捷方式。
- **发布前验证：** CI 执行 `hdiutil verify`，以只读方式挂载映像，并检查
  应用程序快捷方式、bundle identifier、版本、最低 macOS 版本、arm64 主程序与
  sidecar，以及许可资源。
- **历史版本保持不变：** `v0.2.1` 及更早版本继续保留原始 zip 附件。

### Compatibility / 兼容性

- **No model migration:** all seven model identities, source revisions, prepared
  caches, runtimes, worker protocols, and execution recipes are unchanged.
- **No local-data migration:** SQLite schema v6 and existing conversations,
  works, favourites, deletion state, and installation records remain compatible.
- **Windows unchanged:** the current-user NSIS installer and application behavior
  are unchanged.
- **无需迁移模型：** 七个模型身份、来源 revision、准备好的缓存、运行时、worker
  协议与执行配方均未改变。
- **无需迁移本地数据：** SQLite schema v6 与现有会话、作品、收藏、删除状态及
  安装记录继续兼容。
- **Windows 不变：** current-user NSIS 安装包与应用行为保持原样。

## Validation / 验证

- The current macOS application bundle is generated from the existing verified
  release build path, then packaged as a UDZO DMG.
- The DMG contains one `Image Studio.app`, one `Applications` link, and no zip.
- The application identifier is `io.github.goldloli.image-studio`, version
  `{{VERSION}}`, minimum macOS 11.0, and all four executables are arm64.
- GitHub Actions reruns Rust formatting, Clippy, workspace tests, frontend
  tests/typecheck/build, OpenSpec validation, workflow checks, package identity,
  architecture, DMG mount checks, and checksums. It does not download model
  weights or run GPU inference.
- 当前 macOS 应用包继续沿用以真实发布构建生成 `.app` 的流程，再封装为 UDZO DMG。
- DMG 只包含一个 `Image Studio.app`、一个 `Applications` 快捷方式，不包含 zip。
- 应用 identifier 为 `io.github.goldloli.image-studio`，版本为 `{{VERSION}}`，
  最低 macOS 11.0，四个可执行文件均为 arm64。
- GitHub Actions 重新执行 Rust 格式、Clippy、workspace 测试、前端测试/类型检查/
  构建、OpenSpec、workflow 检查、包身份、架构、DMG 挂载与 checksum；不下载模型
  权重，也不执行 GPU 推理。

## Downloads / 下载

- **Windows x64:** `Image.Studio_{{VERSION}}_x64-setup.exe`
- **macOS 11+ Apple Silicon:** `Image-Studio-{{VERSION}}-macos-arm64.dmg`
- Verify with `SHA256SUMS-windows.txt` or `SHA256SUMS-macos.txt`.
- 请使用对应的 `SHA256SUMS-*.txt` 校验下载文件。

```bash
shasum -a 256 -c SHA256SUMS-macos.txt
```

```powershell
Get-FileHash .\Image.Studio_{{VERSION}}_x64-setup.exe -Algorithm SHA256
```

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

1. Verify `Image-Studio-{{VERSION}}-macos-arm64.dmg` against
   `SHA256SUMS-macos.txt`.
2. Open the DMG.
3. Drag `Image Studio` into Applications.
4. Launch the installed application.

The build is unsigned and not notarized. If macOS reports
**“Image Studio.app is damaged and can’t be opened”**, remove quarantine only
after confirming the DMG checksum:

```bash
sudo xattr -rd com.apple.quarantine "/Applications/Image Studio.app"
open "/Applications/Image Studio.app"
```

Do not run the command when the checksum is missing or does not match.

1. 使用 `SHA256SUMS-macos.txt` 校验
   `Image-Studio-{{VERSION}}-macos-arm64.dmg`。
2. 打开 DMG。
3. 把 `Image Studio` 拖入“应用程序”。
4. 启动已安装的应用。

构建未签名、未公证。如果 macOS 提示
**“Image Studio.app 已损坏，无法打开”**，请先确认 DMG checksum 一致，再移除该
应用包的 quarantine 属性：

```bash
sudo xattr -rd com.apple.quarantine "/Applications/Image Studio.app"
open "/Applications/Image Studio.app"
```

checksum 缺失或与下载文件不一致时不要执行。

## Known Limitations / 已知限制

- The builds are unsigned and not notarized.
- Z-Image Turbo requires a 12 GB card.
- The 8 GB compact tier is certified on Turing sm75 (RTX 2060 SUPER).
- Windows install-time NF4 preparation is RAM/commit based.
- Model weights and private Python runtimes remain separate downloads.
- 产物未签名、未公证。
- Z-Image Turbo 仍要求 12 GB 显卡。
- 8 GB 紧凑档仅在 Turing sm75（RTX 2060 SUPER）上完成认证。
- Windows 安装期 NF4 准备取决于可用内存与 commit 余量。
- 模型权重与私有 Python 运行时仍需单独下载。

## License / 许可证

Image Studio is licensed under Apache-2.0. Third-party libraries, fonts,
example prompts/images, model weights, and runtimes retain their upstream
licenses. See `LICENSE`, `NOTICE`, and `THIRD_PARTY_NOTICES.md`.

Image Studio 采用 Apache-2.0；第三方库、字体、示例提示词/图片、模型权重与运行时
继续遵循各自上游许可证，见 `LICENSE`、`NOTICE` 与 `THIRD_PARTY_NOTICES.md`。
