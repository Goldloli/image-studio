# Image Studio {{TAG}}

<!-- GitHub Release notes are bilingual; English appears first. -->

Image Studio `{{VERSION}}` is a local-first image studio release for Apple
Silicon macOS and Windows x64. It focuses the workspace around conversations
and works, adds real conversation/work management, and closes the remaining
desktop reliability gaps found by macOS and Windows acceptance.

Image Studio `{{VERSION}}` 是面向 Apple Silicon macOS 与 Windows x64 的本地优先
图片工作台版本。本次更新把界面重新聚焦到会话与作品，补齐真正的会话/作品管理，
并修复了 macOS 与 Windows 验收中剩余的桌面可靠性问题。

## Highlights / 更新内容

### A focused workspace / 更专注的创作界面

- **One navigation rail.** Remove duplicated page tabs, standalone feature and
  task pages, and the development diagnostics drawer. Create, inspiration,
  models, works, theme, about, and settings now form one clear hierarchy.
- **Production records in context.** Expand model, seed, dimensions, attempt,
  stage, and execution details directly from a conversation result or a work.
  There is no detour through a separate task page.
- **Licensing stays discoverable.** Prompt inspiration keeps source attribution
  while licence details are consolidated in Settings → About.
- **单层导航。** 删除重复的页面页签、独立功能页/任务页与开发诊断抽屉；创作、
  灵感、模型、作品、主题、关于与设置形成单一层级。
- **制作记录留在上下文。** 可在会话结果或作品中直接展开型号、种子、尺寸、
  Attempt、阶段与执行信息，不再跳到独立任务页。
- **许可信息可追溯。** 灵感库保留来源归属，通用许可说明集中到设置 → 关于。

### Conversation and work management / 会话与作品管理

- **Conversation rows have both actions.** Rename or delete a conversation from
  its row. A destructive action requires confirmation and is blocked while that
  conversation is still receiving, generating, cancelling, or cleaning up.
- **Deleting a conversation keeps its works.** Generated works remain available
  for preview, export, favourites, and deletion, with provenance marked as
  “conversation deleted”.
- **Favourite works across pages.** Work favourites are persisted separately
  from prompt inspiration, survive restart, and can drive a favourites-only
  filter with stable totals and pagination.
- **Reference-aware deletion.** A work used by an active draft, a retained
  capability slot, or an in-flight request is not deleted. The app names the
  conversation, reference type, and action required to release it.
- **Controlled cleanup.** Confirmed deletion removes application-managed
  originals, the companion mask, and dedicated preview caches as one recoverable
  operation. External exports and immutable execution history remain.
- **Reliable migration.** The local SQLite library upgrades transactionally from
  schema v5 to v6 without changing existing asset identities or model readiness.
- **会话行同时提供重命名与删除。** 删除需确认；会话仍在接收、生成、取消或清理
  时禁止删除。删除会话后，作品继续用于预览、导出、收藏与删除，并注明
  “会话已删除”。
- **作品收藏独立持久。** 作品收藏与灵感收藏隔离，支持跨重启保存和“仅收藏”
  筛选，并保持完整总数与稳定分页。
- **删除前检查引用。** 被当前草稿、未激活能力槽位或活动请求引用的作品不会被
  删除；应用会指出会话、引用类型和解除方式。
- **受控清理。** 确认删除后，应用内原图、同次蒙版与专用预览缓存作为一组可恢复
  操作处理；外部导出副本和不可变执行历史保留。
- **可靠升级。** 本地 SQLite 通过事务从 v5 升级到 v6，不改变既有资产身份和
  模型就绪状态。

### Desktop reliability / 桌面可靠性

- **Same-root second launch activates the running window.** On Windows, a second
  launch using the same data root restores and focuses the existing window in
  about 186 ms and exits successfully. Different `--data-dir` roots remain
  independent.
- **Unresponsive instances are explained.** If the existing window cannot be
  confirmed within five seconds, the second launch reports that condition
  instead of opening a conflicting WebView2 profile or touching the database.
- **Startup failures stay visible.** Panic logging now forwards to the startup
  failure hook, preserving both the native error dialog and
  `logs/startup-failure.log`.
- **Follow-up interaction fixes.** Draft capability slots remain valid across
  model switches, explicit source removal clears the outgoing slot, empty-prompt
  deletions have usable titles, and blocked work deletion explains the exact
  action needed.
- **同数据根二次启动会激活已有窗口。** Windows 上再次启动同一数据根时，已有窗口
  会在约 186 ms 内还原并聚焦，第二个进程成功退出；不同 `--data-dir` 数据根仍可
  独立运行。
- **无响应实例有明确说明。** 已有窗口在 5 秒内无法确认时，第二次启动会直接说明
  原因，不会创建冲突的 WebView2 profile，也不会改写数据库。
- **启动失败不再静默。** panic 日志钩子会继续调用启动失败钩子，原生错误对话框与
  `logs/startup-failure.log` 同时保留。
- **补齐交互问题。** 切换模型后能力槽位保持有效；显式移除源图会清空旧槽位；
  空提示词作品有可读的删除标题；受引用阻止的删除会给出具体解除动作。

## Validation / 验证

The release keeps build, UI, and real inference evidence separate:

- **macOS arm64:** the current native validation covers the simplified workspace,
  seven ready models, real text-to-image, BiRefNet, Klein reference editing,
  cancellation, export, v5→v6 upgrade, conversation/work deletion, and
  favourites.
- **Windows 10 / RTX 3080 Ti 12 GB:** the current-build acceptance completed
  9/9 real generation cases across seven models and eight capabilities. It also
  covered seeded and real-network installation with resume, work/conversation
  management, v5→v6 upgrade, cancellation, forced-kill recovery, startup
  diagnostics, and second-launch activation.
- **Windows 11 / RTX 2060 SUPER 8 GB:** the compact tier supports Qwen-Image 2.1,
  Qwen-Image 2.1 Turbo, MOSS-Image-SFT, and FLUX.2 Klein 4B (text-to-image and
  edit) at 768² with `float16`. The 0.2.0 functional regression found no
  execution-surface change and passed the current UI, storage, and lifecycle
  checks.
- **GitHub Actions:** the release workflow checks Rust formatting, Clippy,
  workspace tests, frontend tests/typecheck/build, OpenSpec, package identity,
  architecture, checksums, and tag/version provenance. CI does not download
  model weights or execute GPU inference.

本次发布继续把构建、界面与真实推理证据分开记录：

- **macOS arm64：** 当前原生验收覆盖精简工作区、七个模型就绪、真实文生图、
  BiRefNet、Klein 参考图编辑、取消、导出、v5→v6 升级、会话/作品删除与收藏。
- **Windows 10 / RTX 3080 Ti 12 GB：** 当前构建完成七个模型、八项能力共 9/9 个
  真实生成用例，并覆盖种子缓存与真实网络安装/续传、作品与会话管理、v5→v6
  升级、取消、强杀恢复、启动诊断与二次启动激活。
- **Windows 11 / RTX 2060 SUPER 8 GB：** 紧凑档支持 Qwen-Image 2.1 / Turbo、
  MOSS-Image-SFT 与 FLUX.2 Klein 4B（文生图与编辑），使用 768²、`float16`；
  0.2.0 功能回归确认执行面无变化，并通过当前界面、存储与生命周期检查。
- **GitHub Actions：** 发布流程检查 Rust 格式、Clippy、workspace 测试、前端
  测试/类型检查/构建、OpenSpec、包身份、架构、checksum 和 tag/版本来源。
  CI 不下载模型权重，也不执行 GPU 推理。

## Supported Models / 支持模型

| Model / 型号 | Capability / 能力 | Fixed recipe / 固定配方 |
|---|---|---|
| Z-Image Turbo | Text to image / 文生图 | 4-bit, 9 steps, 1:1 / 4:3 / 3:4 |
| FLUX.2 Klein 4B distilled | Text to image, single-image edit / 文生图、单图编辑 | 4-bit, 4 steps, guidance 1.0 |
| BiRefNet general epoch_244 | Background removal, RGBA + mask / 去背景、RGBA + mask | ONNX Runtime CPU, float32 |
| Real-ESRGAN x4plus | Exact 4× upscale / 精确 4× 放大 | ncnn Vulkan, tile 128 |
| Qwen-Image 2.1 | Text to image / 文生图 | NF4, 40 steps, guidance 1.0 |
| Qwen-Image 2.1 Turbo | Text to image / 文生图 | NF4, 8 steps, official Turbo sampling |
| MOSS-Image-SFT | Text to image / 文生图 | NF4, 50 steps, guidance 3.0 |

The standard tier targets at least 12 GB VRAM at 1024² with `bfloat16`; the
compact tier targets at least 8 GB VRAM at 768² with `float16`. Z-Image Turbo
requires 12 GB because it overflows in `float16` on BF16-less Turing cards.

标准档面向 ≥12 GB 显存、1024² 与 `bfloat16`；紧凑档面向 ≥8 GB 显存、768² 与
`float16`。Z-Image Turbo 在无 BF16 的 Turing 架构上存在 fp16 溢出，因此仍要求
12 GB 显存。

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

Do not run this command when the checksum is missing or does not match. If the
app is installed elsewhere, replace the quoted path.

macOS 构建未签名、未公证。如果已核对 checksum 的应用显示
**“Image Studio.app 已损坏，无法打开”**，请把应用放入“应用程序”，然后只移除
该应用包的 quarantine 属性：

```bash
sudo xattr -rd com.apple.quarantine "/Applications/Image Studio.app"
open "/Applications/Image Studio.app"
```

checksum 缺失或与下载文件不一致时不要执行；应用安装在其他位置时请替换引号中的
路径。

## Known Limitations / 已知限制

- The builds are unsigned and not notarized. Windows SmartScreen and macOS
  Gatekeeper can block first launch; verify the checksum before proceeding.
- Z-Image Turbo requires a 12 GB card.
- The 8 GB compact tier is certified on Turing sm75
  (RTX 2060 SUPER). Other 8 GB architectures are not certified by this release.
- Windows install-time NF4 preparation is RAM/commit based. Machines with very
  little commit headroom may need a larger virtual-memory setting before
  installing Qwen-Image 2.1 or MOSS-Image-SFT.
- Model weights and private Python runtimes are not bundled. Generative models
  download 25–33 GB each and retain prepared caches.
- Hardware results are scoped to the recorded machines and architectures. CI
  does not replace real GPU generation acceptance.
- 产物未签名、未公证；Windows SmartScreen 与 macOS Gatekeeper 可能拦截首次启动，
  请先核对 checksum。
- Z-Image Turbo 仍要求 12 GB 显卡。
- 8 GB 紧凑档仅在 Turing sm75（RTX 2060 SUPER）上完成认证，不外推到其他
  8 GB 架构。
- Windows 安装期 NF4 准备取决于可用内存与 commit 余量；余量很小的机器可能需要在
  安装 Qwen-Image 2.1 或 MOSS-Image-SFT 前调大虚拟内存。
- 模型权重与私有 Python 运行时不随安装包分发；每个生成模型约需下载 25–33 GB
  并保留准备好的缓存。
- 硬件结果仅限已记录机器与架构；CI 不能替代真实 GPU 生成验收。

## License / 许可证

Image Studio is licensed under Apache-2.0. Third-party libraries, fonts,
example prompts/images, model weights, and runtimes retain their upstream
licenses. See `LICENSE`, `NOTICE`, and `THIRD_PARTY_NOTICES.md`.

Image Studio 采用 Apache-2.0；第三方库、字体、示例提示词/图片、模型权重与运行时
继续遵循各自上游许可证，见 `LICENSE`、`NOTICE` 与 `THIRD_PARTY_NOTICES.md`。
