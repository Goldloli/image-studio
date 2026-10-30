# 黑色应用图标接入与验收

日期：2026-10-30，Asia/Shanghai。用户确认 `introduce-app-icon-branding` 后实施。Beads：`image-studio-d07`；图标实现与本轮 macOS 验收完成。

## 实现结果

用户批准的 1254×1254 RGBA 原图保存在 `resources/brand/icon-reference.png`，SHA-256 为 `f67ff9721c85c360d4ba4f287403cc3d865e1da74b92c632d32ebd3e54d79aac`。原图带 alpha 透明外围。生成器先校验源文件身份和 PNG 头，输入不符立即失败；不重绘、不改色。

`scripts/generate-app-icons.mjs` 使用 Node 标准库及 macOS 自带 sips/iconutil，生成保留安全区的 1024 母版、含 10 个 macOS 尺寸/倍率组合的 ICNS、7 帧 Windows ICO、32 px favicon 和 512 px UI 品牌图。UI 图从原图中心裁切 1040×1040 后缩放。清单记录各输出、各源 PNG 帧和 ICNS 提取 PNG 帧的 SHA-256；iconutil 会重新编码 PNG，因此两类帧哈希分别记录。

Tauri 配置已使用 ICNS/PNG/ICO；macOS 开发包 helper 将图标复制到 Contents/Resources/AppIcon.icns，并设置 CFBundleIconFile/CFBundleIconName。左栏和关于页使用同一品牌派生图；左栏按钮名称仍为“Image Studio · 返回创作”，关于页图形随可见标题作为装饰。加载失败回退保留对应名称或装饰语义。

## 已通过

| 检查 | 证据与范围 |
|---|---|
| 资源一致性 | 9 份文件哈希、10 份 ICNS 提取 PNG 的尺寸/哈希、7 份 ICO 帧尺寸/位深/哈希通过；重复生成的最终资源哈希一致 |
| Finder 图标 | [Finder 简介](brand-ui/native-finder-info.jpg)中的小图标与大预览均为批准的黑色图标 |
| Dock / 应用切换器 | 用户在本会话人工核对并明确回复“两处都是新黑色图标”；证据类型为用户观察，不宣称工具截图通过。身份记录见 [验收确认](app-icon-branding-acceptance.json) |
| 小尺寸 | [16 px](brand-ui/icon-16x16.png)、[32 px](brand-ui/icon-32x32.png)保留黑底、奶油前景和橙色图片语义，无拉伸；16 px 的阴影和层叠细节自然减少 |
| 正常窗口 | 浏览器 1× [浅色](brand-ui/browser-light-1280.png)/[深色](brand-ui/browser-dark-1280.png)，原生 2× [浅色](brand-ui/native-1280-light.jpg)/[深色](brand-ui/native-1280-dark.jpg) |
| 960×640 原生 | [浅色](brand-ui/native-960-light.jpg)、[深色](brand-ui/native-960-dark.jpg)、[关于页](brand-ui/native-about-960-dark.jpg)；截图实际为 1920×1280，未将诊断 scale 当成系统 DPR |
| 导航与名称 | 原生 AX 显示“Image Studio · 返回创作”；由关于页点击品牌入口可返回创作，仍为同一个新会话、没有新生成任务；关于图形不重复朗读产品名 |
| 资源加载失败 | 生产预览服务器只对品牌 PNG 返回 404。仍可进入模型页、由品牌入口返回创作、输入“图标回退验收：清晨山湖”（11 字符），打开并返回示例图片查看器；[截图](brand-ui/browser-fallback-viewer.png)。不宣称浏览器支持原生生成 |
| 前端 | TypeScript/生产 build 通过；Vitest 3 文件、18 项通过 |
| Rust | fmt、Clippy 全 workspace/all-targets 且 warnings 为错误通过；workspace tests 143 项通过、2 项显式忽略；workspace build 通过 |
| 应用包 | prepare helper 成功，图标元数据及 Resources/AppIcon.icns 身份匹配；未签名、未安装、未公证 |
| 进程禁网启动 | [机器记录](app-icon-offline-startup.json)：sandbox-exec 网络拒绝探针失败且内嵌 React 主 WebView Finished 后进程存活；不修改系统网络、不下载模型、不启动推理 |

截图左上角紫色系统控制指示器来自验收工具，不属于应用界面。

## 平台范围与证据来源

Dock 与应用切换器最初因桌面工具超时而未判通过；随后用户完成本机人工核对并明确回复“两处都是新黑色图标”，补足该验收项。没有将 Finder 截图或文件配置推导为系统入口实测。

manifest 的 verifiedOn.macOS 已标记 true，Windows 保持 false。Windows ICO 结构和各帧已核对，但尚未运行 Windows GUI。OpenSpec 10 项任务已完成，归档为 [2026-10-30-introduce-app-icon-branding](../openspec/changes/archive/2026-10-30-introduce-app-icon-branding/README.md)，并更新 [product-identity 主规格](../openspec/specs/product-identity/spec.md)。其他模型/设置和功能页面 change 保持原确认边界。

本轮没有 commit、push 或远程同步，没有修改模型、生成参数、业务协议或存量生成数据。
