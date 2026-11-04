# fix-cross-platform-tool-model-advice

**状态：已实现、安装版验收并归档（2026-11-05）。** 用户确认"顺手修"后补齐源图工具模型的跨平台提示：Windows 打开 macOS BiRefNet/Real-ESRGAN 草稿时显示对应的 Windows 工具型号提示与 `当前平台未接入` 标签，不再落到错误的 `Z-Image Turbo · 未就绪`。证据 `verification/cross-platform-model-selection/f8/`。

来源：`fix-cross-platform-model-selection` 验收记录的 F8；bd `image-studio-5w0`。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 背景、范围、成功标准、不做项 |
| [design.md](design.md) | 现状、工具镜像提示、Composer 接入、测试与冒烟、风险 |
| [specs/workspace-canvas/spec.md](specs/workspace-canvas/spec.md) | MODIFIED requirement |
| [tasks.md](tasks.md) | 实现、安装版冒烟、质量门、归档 |
| [test-plan.md](test-plan.md) | 单元矩阵 + Windows 冒烟 + Mac user-gated |
