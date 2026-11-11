# Tasks

状态：**已实现并在 macOS 原生验收通过（2026-11-11），准备归档。**
证据：`verification/cleared-source-slot-fix-20261111/RESULTS.md`。

## 1. 实现

- [x] 1.1 在 `workspace-state.ts` 构造旧能力槽位时传播显式清空：`action.patch.sourceAssetId === null` 时槽位来源置空，否则保留 `d.params.sourceAssetId`。（commit `4ea6ab4`）
- [x] 1.2 回归测试：显式移除源图清空旧槽位（A 不再是引用）；仅切换模型保留槽位来源；设置新源图只作用于目标能力。（`workspace-state.test.ts` 新增 3 项）

## 2. 验收与归档

- [x] 2.1 前端 `test/check/build`（76 tests）与 change strict 通过；原生 computer use 复验：Klein 编辑选源 A → 移除源图 → 作品页删除 A 不再被该会话阻止（实际删除的是本轮测试生成的作品）；仅切模型仍保留槽位来源；测试会话已清理。
- [x] 2.2 归档本 change、合并 delta、关闭 `image-studio-c2e`；小步本地提交；不 push。

## Workflow follow-up

- 历史残留的不可见槽位引用不自动迁移；用户可切回该能力后点击“移除源图”按新规则清空。
