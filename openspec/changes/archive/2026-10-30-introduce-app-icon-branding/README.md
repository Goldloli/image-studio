# introduce-app-icon-branding

将用户确认的黑色橙米色层叠图片图标登记为 Image Studio 的产品品牌图标，并定义桌面启动图标与工作区品牌标记的落地、适配和验收要求。

状态：用户已确认实施，资源接入与本轮 macOS 验收完成；Dock/应用切换器由用户人工核对确认。10 项任务完成并归档；Windows ICO 仅生成资源，未做 Windows GUI 实机验收。

参考 PNG 的外围为 alpha 透明区。应用图标保留其透明留白；界面版本从 1254×1254 原始参考中心裁切 1040×1040，再缩放为 512×512。各 macOS 尺寸 PNG、Windows ICO 各尺寸 PNG 帧及最终产物的 SHA-256 记录到 `resources/brand/manifest.json`，用于复核派生关系。

参考图：[approved-black-icon-reference.png](references/approved-black-icon-reference.png)，文件身份见 [icon-reference.json](references/icon-reference.json)。
