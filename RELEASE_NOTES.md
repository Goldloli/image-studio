# Image Studio {{TAG}}

Image Studio is a local-first image creation desktop app built with Rust,
Tauri, and React. This release contains unsigned builds for Windows x64 and
Apple Silicon macOS. `{{VERSION}}` is the first update after the public
`v0.1.0`: it turns 8 GB VRAM support, the low-memory install path, and the
Windows desktop reliability work into downloadable builds for both platforms.

Image Studio 是使用 Rust、Tauri 与 React 构建的本地优先桌面图片创作工作台。
本次发布包含 Windows x64 与 Apple Silicon macOS 构建，当前产物均未签名。
`{{VERSION}}` 是 `v0.1.0` 公开后的第一个更新：把 8 GB 显存支持、安装期低内存
量化路径与 Windows 桌面可靠性修复变成两平台可直接下载的构建。

## What's New in {{VERSION}} / {{VERSION}} 更新内容

### 8 GB VRAM compact tier / 8 GB 显存紧凑档

- **Five more ways to generate on an 8 GB card.** Qwen-Image 2.1,
  Qwen-Image 2.1 Turbo, MOSS-Image-SFT, and FLUX.2 Klein 4B (text-to-image and
  edit) now run on 8 GB Turing cards through a frozen compact recipe:
  768×768, `float16`, measured CUDA reserved peaks **5.44–5.96 GiB** on an
  RTX 2060 SUPER 8 GB (sm75, Windows 11). BiRefNet and Real-ESRGAN were never
  VRAM-bound.
- **Automatic tier selection.** The host reads the device compute capability;
  the model page and composer state the compact tier, and a device below both
  tiers keeps the model disabled with the unmet requirement as its reason.
- **Z-Image Turbo stays at 12 GB.** The model overflows in fp16 on BF16-less
  Turing architectures; the measurement trail is in
  [`doc/16-zimage-quantization-investigation.md`](doc/16-zimage-quantization-investigation.md).
- **8 GB 显卡可用的生成能力增至五个。** Qwen-Image 2.1、Qwen-Image 2.1 Turbo、
  MOSS-Image-SFT 与 FLUX.2 Klein 4B（文生图与编辑）通过冻结的紧凑档配方
  768×768、`float16` 在 8 GB Turing 卡上运行；RTX 2060 SUPER 8 GB（sm75，
  Windows 11）实测 CUDA reserved 峰值 **5.44–5.96 GiB**。BiRefNet 与
  Real-ESRGAN 本来就不受显存限制。
- **自动选择档位。** 宿主读取设备计算能力，模型页与输入区标注紧凑档；低于
  两档门槛的设备保持禁用，并显示未满足的最底要求。
- **Z-Image Turbo 仍要求 12 GB。** 该模型在无 BF16 的 Turing 架构上存在 fp16
  溢出，完整测量过程见
  [`doc/16-zimage-quantization-investigation.md`](doc/16-zimage-quantization-investigation.md)。

### Low-memory install-time quantization / 安装期低内存量化

- **Install-time NF4 preparation now runs in a shard-streaming child process.**
  Each ~1 GiB tensor group is prepared by a short-lived child, so the worker
  commit peak is **1.95–3.6 GiB** instead of 16.4–23.1 GiB. The installer gate
  drops from *available physical RAM ≥ 28 GiB or commit headroom ≥ 26 GiB* to
  **available physical RAM ≥ 8 GiB and commit headroom ≥ 6 GiB**: a 16 GiB
  Windows machine can install Qwen-Image 2.1 and MOSS-Image-SFT, and the
  prepared caches stay byte-identical.
- **Standard-tier generation fixed after the new install path.** The Qwen text
  encoder is now loaded with `device_map="cpu", low_cpu_mem_usage=True` in the
  standard tier as well, so the staged CUDA cleanup actually releases reserved
  memory.
- **安装期 NF4 量化改为分片流式。** 每个约 1 GiB 的 tensor 组由短生命周期子进程
  量化，worker commit 峰值从 16.4–23.1 GiB 降到 **1.95–3.6 GiB**。安装门槛从
  *可用物理内存 ≥ 28 GiB 或 commit 余量 ≥ 26 GiB* 降到
  **可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB**：16 GiB Windows 机器也能安装
  Qwen-Image 2.1 与 MOSS-Image-SFT，且准备好的缓存逐字节一致。
- **新安装路径下的标准档生成已修复。** 标准档 Qwen 文本编码器同样改为
  `device_map="cpu", low_cpu_mem_usage=True` 装载，分段 CUDA 清理能够真正
  释放 reserved 显存。

### Windows desktop reliability / Windows 桌面可靠性

- **No more re-hashing the install on every send.** The 25–35 GB install is
  verified once; send preflight dropped from 60 s+ stalls to ~46 ms.
- **Stranded turns are bounded.** A 60 s turn watchdog fails a wedged send with
  a retry entry instead of leaving the composer spinning.
- **Cancellation is fast.** The process group is force-reaped, so cancel settles
  in 3–7 s instead of ~120 s, with no orphan processes.
- **Honest install state.** Model rows project the real installer phase
  (including `preparing_model`) and no longer claim "not installed".
- **Downloads recover.** Every artifact retries up to five times with backoff,
  and a part that fails checksum validation is deleted and re-downloaded instead
  of blocking the install forever.
- **No console windows.** Supervisor, worker and installer children are created
  with `CREATE_NO_WINDOW`, and the flag now survives suspended job spawns.
- **A real log file.** `<data>/logs/app.log` (8 MiB rotation) records panics,
  generation-time memory, and structured install/generation events.
- **Clearer failures.** Failure copy was clarified and localized, the
  hardware-blocked message is scoped to the model it applies to, the ncnn probe
  log decodes non-ASCII paths lossily, and native save dialogs are populated
  with the current file name.
- **不再重复哈希整个安装。** 25–35 GB 安装内容只在安装时校验一次；发送预检从
  60 s+ 卡顿降到约 46 ms。
- **滞留回合有界。** 60 s 回合看门狗会把卡死的发送判为失败并提供重试入口，
  输入区不再无限转圈。
- **取消更快。** 超期时强制回收整棵进程组，取消从约 120 s 降到 3–7 s，
  且没有残留进程。
- **安装状态可信。** 模型行投影真实安装阶段（含 `preparing_model`），
  不再错误显示"尚未安装"。
- **下载可恢复。** 每个文件最多 5 次退避重试；校验失败的分片会被删除并重新
  下载，而不是让安装永远失败。
- **不再弹终端窗口。** supervisor、worker 与安装器子进程统一使用
  `CREATE_NO_WINDOW`，并且该标志在挂起作业 spawn 后仍然生效。
- **有真正的日志。** `<data>/logs/app.log`（8 MiB 轮转）记录 panic、生成期
  内存，以及结构化的安装/生成事件。
- **错误文案更清楚。** 失败提示已清理并中文化，硬件门槛提示只针对对应型号，
  ncnn 探测日志对非 ASCII 路径做有损解码，原生保存对话框会填入当前文件名。

### Verification / 验证

- **CI quality gate passed on both platforms.** Rust formatting, Clippy with
  warnings denied, workspace tests (199 desktop + integration, 46 protocol),
  60 frontend tests, frontend typecheck/build, and `openspec validate --all
  --strict` ran on Windows x64 and macOS arm64.
- **Real-machine acceptance.** Windows 11 / RTX 2060 SUPER 8 GB covers the
  compact tier, installs, generation, cancellation, watchdog and console
  behavior; Windows 10 22H2 / RTX 3080 Ti 12 GB covers the standard tier
  (Klein 50 s, Z-Image 55 s, BiRefNet 26 s, Real-ESRGAN 9 s, Klein edit 47 s,
  MOSS install 364 s / generation 143 s, Qwen install 415 s / generation 84 s),
  native export, restart persistence, non-ASCII data root, download retry and
  bad-checksum recovery.
- **双平台 CI 质量门通过。** Rust 格式、Clippy `-D warnings`、workspace 测试
  （桌面 199 + 集成、协议 46）、前端 60 测试与类型检查/构建、
  `openspec validate --all --strict` 均在 Windows x64 与 macOS arm64 上运行。
- **真机验收。** Windows 11 / RTX 2060 SUPER 8 GB 覆盖紧凑档、安装、生成、
  取消、看门狗与控制台窗口；Windows 10 22H2 / RTX 3080 Ti 12 GB 覆盖标准档
  （Klein 50 s、Z-Image 55 s、BiRefNet 26 s、Real-ESRGAN 9 s、Klein 编辑 47 s、
  MOSS 安装 364 s / 生成 143 s、Qwen 安装 415 s / 生成 84 s）、原生导出、
  重启持久化、非 ASCII 数据根、下载重试与坏校验恢复。

## Known Limitations / 已知限制

- **Z-Image Turbo requires a 12 GB card.** The compact recipe identity exists in
  the protocol, but the catalog deliberately does not offer it on 8 GB.
- **The 8 GB compact tier is certified on Turing sm75** (RTX 2060 SUPER). Other
  8 GB architectures are expected to work through the same recipes but are not
  certified in this release.
- **Windows virtual memory still matters.** The new install gate is RAM/commit
  based; a machine whose auto-managed pagefile leaves very little commit
  headroom still needs a larger virtual-memory setting before installing
  Qwen-Image 2.1 or MOSS-Image-SFT.
- **Unsigned builds.** Windows SmartScreen and macOS Gatekeeper can block the
  first launch; verify the SHA-256 first (see Installation).
- **模型与运行时单独下载。** 每个生成模型首次安装需要下载约 25–33 GB，并准备
  NF4 缓存；这些内容不随安装包分发。
- **Z-Image Turbo 要求 12 GB 显卡；** 紧凑档配方身份保留在协议中，但目录在
  8 GB 设备上刻意不提供该型号。
- **8 GB 紧凑档仅在 Turing sm75（RTX 2060 SUPER）上认证；** 其他 8 GB 架构
  预期可通过同一配方运行，但本版本未认证。
- **16 GiB 级机器的 Windows 虚拟内存设置会影响安装。** 新的安装门槛基于
  可用内存/commit；自动管理页面文件导致 commit 余量过小的机器仍需调大
  虚拟内存。
- **产物未签名。** Windows SmartScreen 与 macOS Gatekeeper 可能拦截首次
  启动；请先核对 SHA-256（见安装说明）。

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
  background removal with RGBA/mask output, and exact 4× upscaling, with the
  compact 8 GB tier on top of the standard 12 GB tier.
- A conversation-based creation workspace, works library, task center, model
  manager, and persistent settings.
- Verified Rust/Tauri desktop hosts for Windows x64 and macOS arm64.
- 七个本地图片模型，覆盖文生图、单图编辑、去背景 RGBA/mask 与精确 4× 放大；
  在标准 12 GB 档之外新增 8 GB 紧凑档。
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
