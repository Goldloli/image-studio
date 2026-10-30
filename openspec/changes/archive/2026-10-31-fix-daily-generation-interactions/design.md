## Context

See [proposal.md](proposal.md) for motivation and [generation-conversation](specs/generation-conversation/spec.md) / [asset-library](specs/asset-library/spec.md) for the required behavior.

当前前端把三个页面常驻在同一 Tauri 窗口中，通过 `hidden` 切换可见性。`Workspace` 的发送流程会先插入乐观 Turn，再等待 `conversation_send`；composer 在乐观 Turn 出现后立即显示为停止按钮。`AssetLibrary` 只在挂载和手动刷新时查询，因此可能保留应用启动时的空结果。`requestedAsset` 是 App 级持久状态，Workspace 的打开效果又依赖每秒变化的 `studio` 快照，导致查看器关闭后仍被旧请求和后续快照重复打开。

## Goals / Non-Goals

**Goals:**

- 用一个可测试的提交状态边界阻止同一发送位置的二次点击，同时保留占位卡上的独立主动停止能力。
- 保持作品页常驻以保留筛选、布局和阅读状态，在实际激活时重新查询当前视图。
- 把跨页面资产/会话导航改为一次性意图，关闭查看器或返回来源后不再重放。
- 不改变 Job、Attempt、AssetRef、SQLite schema 或 IPC payload。

**Non-Goals:**

- 不修复冷网络安装 `I01` 或自动化 IME 环境 `D03`；两者继续沿用原验收阻塞记录。
- 不引入隐式任务队列、后台并行生成或新的资产缓存。
- 不重构整个页面导航框架，也不改变任务页已有的轮询刷新策略。
- 不删除或迁移用户数据；修复后的验收只新增可辨识测试内容。

## Decisions

### 1. 提交状态优先于乐观 Turn 的按钮语义

在现有 `submittingDraftId` 之外增加同步提交锁 ref，使同一事件循环内的第二次发送/Enter 在读取 React state 前就被拒绝。composer 的行动状态改为单一派生值：

- 宿主确认尚未返回：显示不可点击的提交中状态，即使乐观 Turn 已出现也不在 composer 暴露“停止”；
- 已确认活动 Turn：composer 显示停止，行为保持现状；
- 空闲且可发送：显示发送。

占位卡上的停止按钮不受该锁影响，继续走现有 `cancellationKeys` 和 TurnId 核对流程。这样双击不可能由第二击取消，而用户仍能在第一击后从明确位置主动停止。备选方案是时间窗口去抖或移动停止按钮，两者都不如显式状态边界可验证。

### 2. 作品页激活查询而非重新挂载

保留 `AssetLibrary` 的常驻实例，增加 `active` 输入。当页面从非当前变为当前时，以当前 `filterKey`、`refreshNonce` 和激活序号发起新查询；不活跃时使旧请求失效并停止轮询式状态更新。既有筛选、网格/列表和可保留的滚动位置留在组件内。

激活刷新期间继续显示加载反馈，不把旧 `page.total` 当作当前有效计数。响应使用现有 `requestSeq` 防迟到覆盖。备选方案是每次导航卸载并重建作品页，简单但会丢失用户筛选与阅读位置，因此不采用。

### 3. 跨页面资产导航按 nonce 一次性消费

Workspace 保存最后处理的 `requestedAsset.nonce`；相同 nonce 即使 `studio` 每秒刷新也不再执行打开效果。打开效果只在请求 nonce、数据加载状态和非 native 环境变化时重算，不再把整个 `studio` 对象作为依赖。

返回来源时 App 在导航前清除 `requestedAsset`；如果用户从任务页打开，则保留一次性 `returnPage` 供本次返回使用。关闭查看器、按 Escape 或显式返回来源都走同一清理路径。显式点击会话内图片只创建本地 viewer state，不重新生成跨页请求。

### 4. 回归测试与原生验收分层

将提交按钮派生状态、一次性 nonce 消费和作品页激活条件提取为纯状态函数，在现有 Vitest `conversation-state` / `library-state` 测试中覆盖双击、确认前后、nonce 重复、页面重复激活和旧响应失效。这样不新增 DOM 测试运行时依赖。

最终集成验证使用现有原生 CUA：真实模型快速双击发送、完成后首次进入作品页、关闭作品预览后多次切换创作标签。Rust 幂等测试继续证明同键重放返回同一 Job、不同请求被拒绝。

## Risks / Trade-offs

- 提交锁拖得过久会让用户以为发送按钮失效 → 保留“正在接收”文案和 spinner，并在 finally/明确拒绝后释放；占位卡始终提供停止。
- 页面激活查询产生重复请求 → 复用请求序号并使不活跃实例的响应失效；筛选变化仍走原有防抖。
- 返回页与一次性请求清理顺序错误会回到错误页面 → 在打开 viewer 时保存本次 `returnPage`，返回回调使用已捕获值，随后再清 App 请求。
- 修复只覆盖原生桌面行为 → 验收明确使用原生 app；浏览器预览继续显示其既有“作品库仅桌面可用”状态。
- 现有每秒 studio 快照可能触发其他页面的无谓重渲染 → 本 change 只收窄 requestedAsset 效果依赖，不扩大全局轮询重构。

## Migration Plan

无需数据迁移。实现只修改前端状态与组件输入，先在开发构建中跑纯状态测试、完整前端/Rust 门禁，再用现有默认数据目录和隔离模型执行原生回归。若回归失败，回滚本 change 的前端文件即可；现有 Job、资产、草稿和安装记录不受影响。

## Open Questions

无。三个行为边界均已在 spec 中明确，不需要新增产品选择。
