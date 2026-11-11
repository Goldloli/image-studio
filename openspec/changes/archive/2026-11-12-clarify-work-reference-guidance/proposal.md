# 完善作品删除被引用时的解除指引

## Why

验证确认存在规格符合性缺陷（Beads `image-studio-huq`，`asset-library` 正式规格）：

- 规格要求删除被引用作品时“解释需解除的引用”，场景进一步要求“标明关联会话和**解除方式**”；
- 实现（`apps/desktop/src/adapters/library/sources.rs`）只返回
  `detail=“待发送草稿仍把该作品作为源图”` / `“活动中的请求仍把该作品作为源图”`；
  前端（`AssetLibrary.tsx`）显示“仍被以下位置用作源图：「会话」…”后结束，没有任何可执行指引；
- 草稿的两类引用被合并成同一种 `kind=draft`：**当前生效源图** 与 **`capabilityDrafts` 未激活能力
  槽位里的残留源图** 文案相同。后者在 composer 中不可见，用户必须切回对应能力才能看到并移除；
- 活动请求也没有提示“等待完成或取消后再试”。

原生验收 A08 实际显示：
`仍被以下位置用作源图：「验收-停止测试」待发送草稿仍把该作品作为源图`。

## What Changes

- 宿主区分三类引用并返回结构化信息：
  - `kind=draft`：当前草稿激活的源图；指引“在对应会话移除或更换源图后重试”；
  - `kind=retained`：未激活能力槽位保留的源图，附 `capabilityId`；指引“切回对应能力后移除
    源图，或更换源图后重试”（c2e 修复后该路径有效）；
  - `kind=active`：发送/生成/清理中的活动请求；指引“等待该请求完成或取消后再试”。
- 前端按结构化冲突渲染可读消息（含能力显示名，未知能力回退标识），两个删除入口（首次确认与
  重试）共用；响应契约新增 `capabilityId`，`kind` 允许新值 `retained`。
- 规格 delta：修改 `asset-library` 的“作品删除检查引用并如实表达未完成”要求并新增三类场景。

## Capabilities

### Modified Capabilities

- `asset-library`：拒绝删除时给出与引用类型匹配的解除方式，而非只列会话。

## Impact

- Rust：`adapters/library/sources.rs` 的冲突扫描与结构、相关测试；
- 前端：`studio-api.ts` 类型、能力显示名映射、`library-state.ts` 消息格式化、`AssetLibrary.tsx`
  两个错误分支、Vitest；
- 无数据库/schema/持久化变更；无新增依赖；仅行为与契约字段扩展。
- 本提案阶段只提交文档；未获用户确认前不写实现代码。
