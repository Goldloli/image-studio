# Tasks: fix-cross-platform-model-selection

## 1. 平台支持集合与判定

- [ ] 1.1 `model-recipes.ts` 新增 `platformTextModelIds(platform)` 与 `isTextModelSupportedOn(platform, modelId)` 纯函数；`Object.values(MODEL_IDS[platform])` 统一提供 7 个模型 id。
- [ ] 1.2 `Workspace.tsx` 用 `crossPlatformTextModel = Boolean(activeTextRecipe) && !isTextModelSupportedOn(MODEL_PLATFORM, modelId)` 替换 `legacyWindowsTextModel` 的恒真条件。
- [ ] 1.3 警告文案按平台分支：Windows 提示选择"Z-Image / Klein / Qwen / MOSS"对应 Windows 型号并核对画幅与参数；macOS 提示该会话来自 Windows、选择 macOS 对应型号。
- [ ] 1.4 未就绪标签改为"当前平台未接入"（仅当确实属于另一平台 id 时），其余显示"未就绪"。

## 2. 测试

- [ ] 2.1 `model-recipes.test.ts` 增加 platform × modelId 矩阵：Windows 原生 7 id 全部 supported；旧 macOS 7 id 全部 unsupported；macOS 反向。
- [ ] 2.2 增加纯函数组合断言：Windows 上 Klein/Qwen/MOSS/Z-Image Windows id 不触发跨平台提示；旧 macOS id 触发并包含目标型号族文案。
- [ ] 2.3 `npm --prefix apps/frontend test`、`npm run check`、`npm run build` 全绿。

## 3. 安装版冒烟与文档

- [ ] 3.1 重建安装包并安装到目标机（或直接用开发构建），Windows 选 Klein / Qwen Turbo / MOSS 确认无跨平台警告；选择旧 macOS id 会话确认新警告文案。
- [ ] 3.2 记录证据 `verification/cross-platform-model-selection/`（截图 + results.json）；macOS 提示由用户在 Mac 上验证（user-gated，不冒充通过）。
- [ ] 3.3 更新 README/相关文档中的跨平台会话说明；全量质量门（fmt/clippy/workspace tests/前端/openspec strict）通过。

## 4. 归档

- [ ] 4.1 `openspec validate fix-cross-platform-model-selection --strict` 后归档，同步 `workspace-canvas` 主规格。
- [ ] 4.2 关闭 bd `image-studio-4vp`。
