# 修复“移除源图”后旧能力槽位仍保留来源引用

## Why

`fix-capability-drafts-and-delete-title` 归档后发现的相邻缺陷（`image-studio-c2e`）：
在 Klein 编辑能力中选择作品 A 作为源图后点击“移除源图”，UI 会把草稿切换回文本能力并清空当前
`sourceAssetId`；但 `workspace-state.ts` 在保存被切走的编辑能力槽位时使用的是**切换前**参数，
槽位里仍保留 `sourceAssetId=A`。结果：

- 作品 A 被该会话视为“草稿引用”，作品页删除被拒绝并指向该会话；
- 当前界面已看不到源图（显示“导入源图”），用户无法从 UI 直接清空这份不可见引用；
- 唯一解除方式目前是删除整个会话，代价过大。

## What Changes

- 当同一次参数变更**显式清空源图**（`sourceAssetId` 明确设为 null，即“移除源图”）并切换能力时，
  被切走的旧能力槽位 SHALL 同时清空其来源引用。
- 设置新源图（`sourceAssetId` 为新 AssetId）时 MUST 只作用于目标能力，不得覆盖旧能力槽位的来源，
  保持“切走再切回可恢复原编辑状态”的现有语义。
- 其余字段与行为不变：普通模型切换仍保留旧槽位来源；槽位画幅规范化、加载修复、宿主校验不变。

## Capabilities

### Modified Capabilities

- `feature-catalog`：能力切换与草稿源图引用解除保持一致，来源被用户显式移除后不再构成引用。

## Impact

- 前端 `workspace-state.ts` 的 reducer 与 Vitest；无 IPC、协议、数据库或 schema 变更。
- 既有已残留的不可见槽位引用不做自动迁移（无法区分“保留的编辑状态”与“残留引用”，后者由旧 bug
  产生）；修复后用户可切回该能力 → 移除源图来清除，使规格中的“解释需解除的引用”有了实际路径。
- 本提案阶段只提交文档；未获用户确认前不写实现代码。
