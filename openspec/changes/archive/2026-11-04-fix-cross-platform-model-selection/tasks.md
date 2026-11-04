# Tasks: fix-cross-platform-model-selection

## 1. 平台支持集合与判定

- [x] 1.1 `model-recipes.ts` 新增 `platformModelIds(platform)`（预计算 Set）与 `isModelSupportedOn(platform, modelId)`；覆盖每平台 7 个模型 id。
- [x] 1.2 `Workspace.tsx` 用 `crossPlatformTextModelAdvice(MODEL_PLATFORM, modelId)` 替换 `legacyWindowsTextModel` 恒真条件。
- [x] 1.3 警告文案按平台分支：Windows 提示选择对应 Windows 型号族（Z-Image / Klein / Qwen / MOSS）并核对画幅与参数；macOS 提示会话来自 Windows、选择 macOS 对应型号。
- [x] 1.4 未就绪标签改为"当前平台未接入"（仅另一平台 id），其余"未就绪"。

## 2. 测试

- [x] 2.1 `model-recipes.test.ts` 增加 platform × modelId 矩阵：Windows/macOS 各 7 id、工具/未知 id 边界。
- [x] 2.2 纯函数组合断言：Windows 原生 5 个文本 id 不触发提示；旧 macOS 5 个文本 id 触发且文案含目标型号族；macOS 反向。
- [x] 2.3 前端门：`npm test` 57 passed、`npm run check`、`npm run build` 全绿。

## 3. 安装版冒烟与文档

- [x] 3.1 重建安装包（installer `63bdbf24…`，245,017,051 B，installed exe `d50afea6…`）并安装；Windows 选 Klein / Qwen Turbo / MOSS 确认无跨平台警告。
- [x] 3.2 旧 macOS id 场景：临时改写隔离根 `C:\ist-a` 草稿为 `z-image-turbo-mflux-v1`，安装版显示新 Windows 版警告 + `当前平台未接入`，发送禁用；截图与 results.json 落盘，草稿已备份/恢复。
- [x] 3.3 证据 `verification/cross-platform-model-selection/`；macOS M1/M2 记录为 user-gated；全量质量门（最终安装包构建包含 fmt/clippy/workspace tests/frontend，且 `openspec validate`）通过。

## 4. 归档

- [x] 4.1 `openspec validate fix-cross-platform-model-selection --strict` 通过后归档，同步 `workspace-canvas` 主规格。
- [x] 4.2 关闭 bd `image-studio-92e`（实现任务）。
