# fix-cross-platform-model-selection

**状态：等待用户确认（2026-11-04）。** 用户报告 Windows 端在已接入的 Klein/Qwen/MOSS 上出现"旧的跨平台模型选择，请选择 Windows Z-Image"警告（本机已复现），并反馈 macOS 有类似问题；本目录目前只有 change 文档，未写实现代码。

来源：bd `image-studio-4vp`。根因、方案与测试矩阵见 [design.md](design.md)。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 现象、根因摘要、变更范围、成功标准、不做项 |
| [design.md](design.md) | 恒真条件根因、平台支持集合方案、文案、测试矩阵、风险 |
| [specs/workspace-canvas/spec.md](specs/workspace-canvas/spec.md) | `workspace-canvas` 的 delta |
| [tasks.md](tasks.md) | 实现、测试、冒烟、归档任务 |
| [test-plan.md](test-plan.md) | 纯函数矩阵 + Windows 冒烟 + Mac user-gated |
