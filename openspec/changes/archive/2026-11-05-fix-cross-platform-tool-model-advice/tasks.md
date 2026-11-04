# Tasks: fix-cross-platform-tool-model-advice

## 1. 实现

- [x] 1.1 `model-recipes.ts` 新增 `crossPlatformToolModelAdvice` 与组合入口 `crossPlatformModelAdvice`（文本优先、工具兜底）。
- [x] 1.2 `Workspace.tsx` 改用组合入口；无 text recipe 的跨平台工具草稿标签显示 `${modelDisplayName} · 当前平台未接入`。
- [x] 1.3 Vitest：工具模型双向提示、本平台工具不提示、组合入口与未知 id、文本矩阵不回归。

## 2. 安装版冒烟

- [x] 2.1 重建安装包并安装（记录 installer/installed exe SHA-256）。
- [x] 2.2 在 `C:\ist-a` 临时把草稿改为 macOS BiRefNet id，重启安装版，确认 Windows 工具提示、`当前平台未接入` 标签与发送禁用；截图/results.json 落盘，随后恢复草稿备份。
- [x] 2.3 回归：Windows 原生工具模型（BiRefNet/Real-ESRGAN）不出现跨平台提示；文本模型矩阵不变。

## 3. 质量门与归档

- [x] 3.1 `npm test/check/build`、`cargo fmt/clippy/test --workspace`（安装包构建内）、`openspec validate --all --strict` 通过。
- [x] 3.2 归档 change，同步 `workspace-canvas` 主规格；关闭 bd `image-studio-5w0`；更新 review 报告 F8 状态。
