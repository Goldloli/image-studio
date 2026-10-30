# Image Studio 品牌图标

正式视觉参考为用户确认的黑色橙米色层叠图片图标。原始文件为 `icon-reference.png`，SHA-256：

```text
f67ff9721c85c360d4ba4f287403cc3d865e1da74b92c632d32ebd3e54d79aac
```

用户已通过“下一步吧”“继续”确认实施，资源接入与本轮 macOS 验收已完成。原图外围为 alpha 透明区；派生过程保留透明度，不能填充为白底。Windows ICO 已生成但未做 Windows GUI 实机验收。

重新生成所有派生资源：

```sh
node scripts/generate-app-icons.mjs
```

只有完成 macOS 原生 Dock/Finder、浅暗主题和小尺寸验收后，才可用以下命令将清单中的 macOS 状态标记为已验证：

```sh
node scripts/generate-app-icons.mjs --mac-verified
```

| 文件 | 用途 |
|---|---|
| `icon-reference.png` | 用户确认的 1254×1254 RGBA 原始参考 |
| `app-icon-1024.png` | 保留外围透明留白的 1024×1024 应用母版 |
| `app-icon-ui-512.png` | 从 1254×1254 原始参考中心裁切 1040×1040，再缩放为 512×512，用于工作区和关于页 |
| `app-icon-32.png` | 浏览器 favicon 与小尺寸检查 |
| `../../apps/desktop/icons/icon.icns` | macOS 打包图标 |
| `../../apps/desktop/icons/icon.ico` | Windows 多尺寸图标，当前未做 Windows GUI 实机验收 |
| `manifest.json` | 源文件、尺寸、平台状态、各 macOS 尺寸 PNG 与 Windows ICO 各尺寸 PNG 帧及最终输出的 SHA-256 |

生成脚本只使用 macOS 自带的 `sips`、`iconutil` 和 Node.js 标准库，不依赖额外图像包。Windows ICO 含 16、24、32、48、64、128 和 256 像素 PNG 帧。

生成器校验批准源图的哈希、1254×1254 尺寸、8-bit RGBA PNG 格式，不符即停止。清单同时记录生成 PNG 帧和 ICNS 提取 PNG 帧的哈希：iconutil 会重新编码 PNG，两者不能直接按文件字节比较。清单包含生成工具与系统版本；跨系统版本重新导出后需重新核对。

当前验收结果见 [图标接入记录](../../verification/app-icon-branding-results.md)。Finder、浅暗主题、小窗口、回退及禁网启动由工具验证；Dock/应用切换器由用户人工核对并确认。macOS 总体验证标记为 true，Windows 保持 false；[确认记录](../../verification/app-icon-branding-acceptance.json)关联已验收的 ICNS 哈希。

macOS iconset 各尺寸 PNG 和 Windows ICO 各尺寸 PNG 帧的 SHA-256 已记录在清单中；应逐份核对哈希及母版来源，不能仅以 ICNS 或 ICO 容器存在作为尺寸资源验收证据。普通重新生成命令会重置清单中的验收标记，历史确认记录保留；只有输出哈希仍匹配且本轮检查通过时，才可重新标记。
