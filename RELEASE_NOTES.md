# Image Studio {{TAG}}

<!-- GitHub Release notes are bilingual; English appears first. -->

Image Studio `{{VERSION}}` redesigns the settings experience and adds a verified
stable-update flow. It keeps the seven stable local models unchanged and adds an
explicitly experimental macOS arm64 MiniMax-H3-Image text-to-image profile.

Image Studio `{{VERSION}}` 重构设置体验，并加入经过校验的稳定版更新流程。本版
保留七个稳定本地模型不变，并新增一个显式实验性的 macOS arm64
MiniMax-H3-Image 文生图 profile。

## What's New / 更新内容

### Experimental MiniMax-H3-Image / 实验性 MiniMax-H3-Image

- Adds a dedicated `stable-diffusion.cpp` native worker and Metal runtime for the
  pinned Q3_K_S + Q2_K_M profile, with 1024² and 2048² text-to-image.
- The complete download is about 23.05 GB; the commonly cited 6.88 GB is only a
  Q2 DiT file, not a runnable model closure.
- macOS native UI acceptance covers install, license acknowledgement, generation,
  cancel, retry, reopen, and PNG export. Windows and 8 GB devices remain
  unverified and disabled; 4096² is not exposed.
- 为固定的 Q3_K_S + Q2_K_M profile 新增专用 `stable-diffusion.cpp` 原生 worker 与
  Metal 运行时，只开放 1024²/2048² 文生图。
- 完整下载约 23.05 GB；社区常见的 6.88 GB 只是 Q2 DiT 单文件，不是可运行闭包。
- macOS 原生 UI 验收覆盖安装、许可证确认、生成、取消、重试、重开与 PNG 导出；
  Windows、8 GB 设备仍未验证并保持禁用，4096² 不开放。

### Redesigned settings center / 重构设置中心

- **Six preference groups:** General & Appearance, Creation Defaults, Storage,
  Runtime, Updates, and About use a bilidown-style two-panel layout with
  row-based controls.
- **Persistent accent color:** six non-green presets and validated custom
  `#RRGGBB` values apply immediately and survive restart.
- **Clearer facts and actions:** storage availability is summarized once,
  runtime combinations remain independently scannable, and duplicate labels or
  project rows have been removed.
- **六类偏好：** 通用与外观、创作默认值、存储、运行环境、更新与关于采用
  bilidown 风格双面板与行式控件。
- **持久强调色：** 六个非绿色预设与合法的自定义 `#RRGGBB` 值即时生效，并在重启
  后恢复。
- **事实与动作更清晰：** 可用空间只汇总一次，模型运行组合保持独立可扫描；
  已移除重复的辅助标签和项目行。

### Verified stable updates / 稳定版更新

- **Official stable channel:** update checks read only public stable Releases from
  `Goldloli/image-studio`; draft and prerelease entries are ignored.
- **Bounded automatic checks:** the app checks about 10 seconds after startup,
  at most once every 24 hours, and never downloads or installs automatically.
- **Real transfer state:** downloads use a managed `.part` file, display actual
  bytes and speed, support cancellation, and require the matching SHA-256 before
  the package becomes installable.
- **Platform boundaries:** verified Windows NSIS packages can install and restart
  when no generation, model install, or cleanup is active. macOS opens the
  verified DMG for a manual drag into Applications and never bypasses
  Gatekeeper.
- **官方稳定通道：** 仅检查 `Goldloli/image-studio` 的公开稳定版 Release，忽略
  draft 与 prerelease。
- **有界自动检查：** 启动约 10 秒后检查，每 24 小时最多一次，且不会自动下载或
  安装。
- **真实传输状态：** 下载使用受管 `.part` 文件，显示真实字节与速度，支持取消，
  并必须在 SHA-256 一致后才可安装。
- **平台边界：** 已校验的 Windows NSIS 包可在没有生成、模型安装或清理任务时
  安装并重启；macOS 只打开已校验 DMG，由用户手动拖入“应用程序”，不绕过
  Gatekeeper。

### About and GitHub / 关于与 GitHub

- About now shows version, build, and Apache-2.0 once, with a single GitHub project
  entry, compact release/issue/license links, and GitHub-only author support.
- 关于页只展示一次版本、构建与 Apache-2.0，并提供唯一 GitHub 项目入口、紧凑的
  Releases/Issues/许可链接，以及仅 GitHub 的作者入口。

## Compatibility / 兼容性

- **Preferences:** schema v1 is read with field-level fallbacks and upgrades to v2
  on the next valid save.
- **Stable model identities unchanged:** the seven stable model revisions,
  runtimes, worker protocols, and recipes are unchanged.
- **Local-data migration:** SQLite schema v7 adds an explicit per-turn output
  resolution; historical turns default to 1024 without rewriting stored plans or
  assets.
- **No stable inference regression expected:** stable generation paths are
  unchanged; MiniMax-H3-Image is isolated in a dedicated profile.
- **偏好：** schema v1 可逐字段回退读取，并在下一次合法保存时升级到 v2。
- **稳定模型身份不变：** 七个稳定模型的 revision、运行时、worker 协议与配方均未
  改变。
- **本地数据结构迁移：** SQLite schema v7 为轮次增加显式输出分辨率；历史轮次
  默认 1024，不重写既有计划或资产。
- **预期稳定型号无推理回归：** 稳定生成链路未改；MiniMax-H3-Image 使用独立
  profile。

## Validation / 验证

- Frontend tests, TypeScript checks, production build, Rust formatting, Clippy,
  workspace tests, OpenSpec validation, workflow checks, and actionlint pass.
- The six settings groups were checked at 1280x800 and 960x640 in light and dark
  themes. Evidence is stored under
  `verification/operations/redesign-settings-center/`.
- A real request to the public GitHub Releases API returned the current stable
  release and the UI correctly reported that no newer version was available at
  test time.
- MiniMax-H3-Image passed isolated macOS installation, Metal probe, native UI
  1024²/2048² generation, cancel/retry, reopen, and export. Evidence is stored
  under `verification/minimax-h3-image-20261124/`. Windows remains `not_run`.
- Actual package download, SHA-256 verification over release bytes, DMG opening,
  Windows NSIS installation/restart, and active-task install refusal are
  `not_run` for this version. They are covered by unit/state tests and will be
  exercised against the next stable release on target machines.
- 前端测试、TypeScript、生产构建、Rust 格式、Clippy、workspace 测试、OpenSpec、
  workflow 检查与 actionlint 均通过。
- 六类设置已在 1280×800 与 960×640 的亮暗主题逐页检查，证据位于
  `verification/operations/redesign-settings-center/`。
- 真实 GitHub Releases API 请求成功返回当前稳定版；测试时 UI 正确显示没有更高
  版本。
- MiniMax-H3-Image 已完成隔离 macOS 安装、Metal 探针、原生 UI 1024²/2048²
  生成、取消/重试、重开与导出，证据位于
  `verification/minimax-h3-image-20261124/`；Windows 保持 `not_run`。
- 本版本的发布包真实下载、Release 字节 SHA-256 校验、打开 DMG、Windows NSIS
  安装/重启与活动任务拒绝安装为 `not_run`；状态边界已有单元测试，将在下一稳定版
  的对应设备上执行。

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

The installer is a current-user NSIS package with the WebView2 offline installer.
The build is unsigned, so SmartScreen may appear. Verify the SHA-256 before using
**More info → Run anyway**. The in-app update action also preserves SmartScreen
and UAC prompts and refuses installation while generation, model installation, or
cleanup is active.

安装包为 current-user NSIS，并内置 WebView2 离线安装器。构建未签名，可能出现
SmartScreen；请先核对 SHA-256，再使用 **更多信息 → 仍要运行**。应用内更新同样
保留 SmartScreen 与 UAC 提示，并在生成、模型安装或清理活动期间拒绝安装。

### macOS

1. Verify the DMG against `SHA256SUMS-macos.txt`.
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

1. 使用 `SHA256SUMS-macos.txt` 校验 DMG。
2. 打开 DMG。
3. 把 `Image Studio` 拖入“应用程序”。
4. 启动已安装的应用。

构建未签名、未公证。如果 macOS 提示
**“Image Studio.app 已损坏，无法打开”**，请先确认 DMG checksum 一致，再移除
quarantine：

```bash
sudo xattr -rd com.apple.quarantine "/Applications/Image Studio.app"
open "/Applications/Image Studio.app"
```

checksum 缺失或与下载文件不一致时不要执行。

## Known Limitations / 已知限制

- The builds are unsigned and not notarized.
- The update channel exposes stable releases only; this version does not provide
  a prerelease channel.
- Z-Image Turbo requires a 12 GB card.
- The 8 GB compact tier is certified on Turing sm75 (RTX 2060 SUPER).
- Model weights and private Python runtimes remain separate downloads.
- MiniMax-H3-Image is experimental, macOS arm64 only for this profile, and
  requires explicit license/territory acknowledgement. It does not certify
  Windows, 8 GB devices, or 4096².
- 产物未签名、未公证。
- 更新通道仅提供稳定版，本版本不提供 prerelease 通道。
- Z-Image Turbo 仍要求 12 GB 显卡。
- 8 GB 紧凑档仅在 Turing sm75（RTX 2060 SUPER）上完成认证。
- 模型权重与私有 Python 运行时仍需单独下载。
- MiniMax-H3-Image 为实验功能，本轮只认证该 macOS arm64 profile，且必须显式
  确认许可证/适用地域；不认证 Windows、8 GB 显存设备或 4096²。

## License / 许可证

Image Studio is licensed under Apache-2.0. Third-party libraries, fonts,
example prompts/images, model weights, and runtimes retain their upstream
licenses. See `LICENSE`, `NOTICE`, and `THIRD_PARTY_NOTICES.md`.

Image Studio 采用 Apache-2.0；第三方库、字体、示例提示词/图片、模型权重与运行时
继续遵循各自上游许可证，见 `LICENSE`、`NOTICE` 与 `THIRD_PARTY_NOTICES.md`。
