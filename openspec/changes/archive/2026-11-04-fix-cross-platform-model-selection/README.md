# fix-cross-platform-model-selection

**状态：已实现、安装版验收并归档（2026-11-04）。** 用户确认修复后：Windows 原生 Klein/Qwen/MOSS 不再误报跨平台警告；旧 macOS id 显示新的 Windows 型号族提示与“当前平台未接入”标签；macOS 反向提示由同一前端矩阵覆盖，真机验证 user-gated。证据 verification/cross-platform-model-selection/，commit 959d95c。

来源：bd `image-studio-4vp`。根因、方案与测试矩阵见 [design.md](design.md)。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 现象、根因摘要、变更范围、成功标准、不做项 |
| [design.md](design.md) | 恒真条件根因、平台支持集合方案、文案、测试矩阵、风险 |
| [specs/workspace-canvas/spec.md](specs/workspace-canvas/spec.md) | `workspace-canvas` 的 delta |
| [tasks.md](tasks.md) | 实现、测试、冒烟、归档任务 |
| [test-plan.md](test-plan.md) | 纯函数矩阵 + Windows 冒烟 + Mac user-gated |
