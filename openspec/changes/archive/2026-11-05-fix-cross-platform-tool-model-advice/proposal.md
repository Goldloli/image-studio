# Proposal: fix-cross-platform-tool-model-advice

## Why

`fix-cross-platform-model-selection` 修好了文本模型的跨平台提示，但**源图工具模型**
（BiRefNet / Real-ESRGAN）仍是盲区：这类草稿没有 `activeTextRecipe`，所以
`crossPlatformTextModelAdvice` 返回 null，Composer 既不显示跨平台提示，标签还会落到
`Z-Image Turbo · 未就绪`。Windows 用户打开 macOS 保存的 BiRefNet/Real-ESRGAN 会话时
只能看到错误的标签和禁用的发送按钮，不知道要重新选择对应 Windows 工具模型。

这是 `fix-cross-platform-model-selection` 验收时记录的 F8（bd `image-studio-5w0`），
用户 2026-11-04 确认"顺手修"。

## What Changes

- `model-recipes.ts` 新增 `crossPlatformToolModelAdvice(platform, modelId)`：识别两端
  冻结的 BiRefNet/Real-ESRGAN 模型 id，在跨平台时给出带目标工具名称的提示
  （Windows: `请选择对应的 Windows BiRefNet 去背景 / Real-ESRGAN 放大 型号并核对源图`；
  macOS: 对应反向提示）。
- 新增组合入口 `crossPlatformModelAdvice`（文本优先、工具兜底），Composer 改用它；
  工具/未知模型不再显示错误的 Z-Image 标签：跨平台工具草稿显示
  `<模型名> · 当前平台未接入`。
- 前端 Vitest 增加工具模型双向矩阵与组合入口断言。

## 成功标准

- Windows 上打开 macOS BiRefNet/Real-ESRGAN 草稿：显示新的 Windows 工具提示，标签为
  `BiRefNet epoch_244 · 去背景 · 当前平台未接入`（或 Real-ESRGAN 对应），发送仍禁用；
- Windows 原生工具模型、文本模型与 macOS 反向场景不回归；
- `npm test/check/build`、workspace 质量门、安装版冒烟通过；证据落盘后归档。

## 不做 / 超出范围

- 不自动迁移/改写跨平台草稿模型；工具 Composer 的源图导入交互保持现状（用户重选后即可用）。
- 不改文本模型的既有文案与行为。
