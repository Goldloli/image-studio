# 草稿能力切换与删除确认标题修复 —— 验收结果

日期：2026-11-11（Asia/Shanghai）。Change：`fix-capability-drafts-and-delete-title`。
执行方式：Vitest 回归测试 + 无头构建检查 + 原生 computer use（`target/dev/Image Studio.app`，
真实数据根，v6）。

## 修复 1：能力切换不再让草稿保存失败（image-studio-31x）

自动化（`apps/frontend/src/workspace-state.test.ts`）：

- 4:3 Z-Image → Klein 文本：当前画幅收敛为 1:1；
- 再切 Klein 编辑并保存槽位后切回 Z-Image：所有 `capabilityDrafts` 槽位画幅均通过
  `aspectsForCapability`，Klein 编辑槽位为 1:1，Z-Image 恢复 4:3；
- 同能力画幅 patch 不受影响（3:4 保留）；
- 载入历史异常参数（Klein 文本 4:3 槽位）被修复为 1:1，Z-Image 3:4 槽位保持。

原生复现（旧版必现“保存失败：能力草稿结构或范围无效”）：

1. 新建会话，默认 **4:3**（`4:3 · 1 张`）；
2. 模型菜单选 **FLUX.2 Klein 4B**：画幅自动变为 **1:1 · 1 张**，页脚“草稿已保存到本机”，
   **没有**出现保存失败；
3. 选择一张作品作为 Klein 编辑源图：保存成功；DB 草稿：
   `capabilityDrafts.{klein-edit,klein-text}.aspect=1:1`、`z-image.aspect=4:3`，
   `capabilityId=image.edit.flux2-klein-4b.v1`，`sourceAssetId` 正确；
4. 切回 **Z-Image Turbo**：画幅恢复 **4:3 · 1 张**，页脚保存成功，槽位保持合法。
5. 清理：删除本次测试会话（已生成墓碑，不残留引用）。

## 修复 2：空指令作品的删除确认标题

自动化（`apps/frontend/src/library-state.test.ts`）：提示词非空截断到 24 字加省略号；提示词为空
使用来源会话标题；两者都空显示“该作品”。

原生：作品页按会话筛选 `去背景`（该作品 instruction 为空），打开删除确认，标题显示
**“删除作品「去背景」？”**（截图 `native-fix-delete-title-empty-prompt.png`），范围说明不变；
随后取消，未删除作品。

## 质量门

- `npm --prefix apps/frontend test`：73 passed（新增 3 项 reducer 回归 + 3 项标题单测）；
- `npm --prefix apps/frontend run check`、`run build`：通过；
- Rust 侧未改动，`cargo fmt --check`/`clippy -D warnings`/`test --workspace`：327 passed；
- `openspec validate fix-capability-drafts-and-delete-title --strict`：通过；
  全量 strict 仅剩既有 4 条长度警告（`image-studio-6g2`）。

## 本轮发现但未修复（另行跟踪）

清理测试会话时发现：点击“移除源图”会切换能力，但保存旧能力槽位时使用的是切换前参数，槽位里仍
保留被移除的 `sourceAssetId`；该作品之后会在“作品删除引用检查”里被这个不可见槽位阻止，且 UI
没有直接清空该槽位来源的路径。已建 `image-studio-c2e` 跟踪，未纳入本 change。
