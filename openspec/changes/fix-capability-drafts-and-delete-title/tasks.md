# Tasks

状态：**已实现并在 macOS 原生验收通过（2026-11-11），准备归档。**

## 1. 草稿能力切换修复

- [x] 1.1 在 `model-recipes.ts` 增加 `aspectsForCapability` 纯函数；在 `workspace-state.ts` 增加 `normalizeDraftParams`。单测覆盖：macOS Z-Image 支持 1:1/4:3/3:4，Klein/Qwen/MOSS/工具为 1:1，未知能力回退 1:1。
- [x] 1.2 在 `params` reducer 的“切换前槽位、切换后参数、返回前兜底”三处规范化，并在 `Workspace.tsx` 的 `storedDraft()` 接入加载修复。单测复现：4:3 Z-Image → Klein 文本（画幅变 1:1）→ 切到编辑并保存槽位 → 再切走，全部槽位 1:1；同时确认 Z-Image 4:3/3:4 不被改写。
- [x] 1.3 浏览器预览复现原失败路径，确认不再出现“能力草稿结构或范围无效”且草稿保存成功。

## 2. 删除确认标题修复

- [x] 2.1 在 `library-state.ts` 增加 `workDeleteTitle(item)` 并接入 `AssetLibrary.tsx`；单测覆盖提示词非空截断、提示词空用会话标题、两者都空显示“该作品”。

## 3. 集成验收与归档

- [x] 3.1 前端 `test/check/build`、Rust `fmt/clippy/test`、`openspec validate --all --strict`（仅允许既有 4 条长度警告）通过；原生应用 computer use 抽验：4:3 会话切 Klein 保存成功、空指令作品删除标题可读。
- [x] 3.2 归档本 change、合并 delta，关闭 `image-studio-31x`，按项目约定小步本地提交；不 push。

## Workflow follow-up

- 用户确认后才开始实现；实现完成后归档并关闭 Beads issue。
- 若未来接入非 1:1 能力，需同步更新 `aspectsForCapability` 与测试。
