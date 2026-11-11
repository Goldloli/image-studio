# Design

## Context

见 [proposal.md](proposal.md)。当前 `asset_reference_conflicts`（`sources.rs`）把草稿扫描结果
（`params.sourceAssetId` 或任一 `capabilityDrafts[*].sourceAssetId` 命中）统一记为
`kind="draft"`、固定 detail；活动请求记为 `kind="active"`、固定 detail。前端只做字符串拼接。

## Goals / Non-Goals

**目标**：拒绝删除时用户能知道“谁在引用、属于哪类引用、下一步做什么”，特别是不可见的
`capabilityDrafts` 槽位引用；响应契约可被前端结构化渲染。

**非目标**：改变拒绝规则本身、自动清理引用、修改删除范围/文件清理、改变 activity 判断、
把能力显示名放进宿主（宿主只给稳定 capabilityId）。

## Decisions

### 1. 三类引用与契约扩展

`AssetReferenceConflict` 增加 `capability_id: Option<String>`（camelCase `capabilityId`），
`kind` 扩展为 `draft | retained | active`：

| kind | 来源 | 指引（宿主 detail 中的可执行句子） |
| --- | --- | --- |
| `draft` | 当前 draft 的 `sourceAssetId` 命中 | “在对应会话移除源图或更换源图后重试” |
| `retained` | `capabilityDrafts[capability]` 槽位命中，携带该 capabilityId | “切回该能力后移除源图（或更换源图）后重试” |
| `active` | pending/活动请求的源图快照命中 | “等待该请求完成或取消后再试” |

同一会话同一来源只返回一次；一个会话可能同时产生 `draft` 与 `retained`（不同能力），分别列出。

### 2. 槽位引用与当前源图的区分

扫描顺序：先判当前 `sourceAssetId`（`draft`），再遍历 `capabilityDrafts` 找命中槽位（每个命中
能力一条 `retained`）。与 c2e 修复语义一致：用户可通过“切回该能力 → 移除源图”清除槽位；若用户
在该能力下选择新源图，槽位来源会被新值覆盖，同样解除旧引用。

### 3. 前端结构化渲染

前端工具函数 `referenceConflictsMessage(conflicts)` 生成：

```text
仍被以下位置用作源图，请先解除后重试：
·「<会话>」<detail>
   [retained] 对应能力：<能力显示名>
```

- 能力显示名由新纯函数 `capabilityDisplayName(capabilityId)` 解析（文本 recipe 的
  `capabilityId → displayName`；BiRefNet/Real-ESRGAN 使用固定中文名；未知回退原始 id，避免
  编造能力名）。
- 首次确认与 `retryDelete` 两个分支共用同一格式化，避免两处文案漂移。
- 保留原有错误前缀语义，消息仍是纯文本（不引入富文本/HTML），便于复制与自动化断言。

### 4. 不新增 IPC 命令

`asset_delete` 的返回结构已含 `conflicts`，只扩展字段与 `kind` 枚举值；前端仅追加类型与格式化，
不新增命令或参数。

## Risks / Trade-offs

- `kind` 新值与新增字段是响应契约变化：仓库内唯一消费者是 `AssetLibrary.tsx` 与类型定义；无持久化
  依赖，无迁移风险。
- 能力名映射是 UI 展示信息；未知 id 回退原文，不阻塞删除或误导。
- 文案更长，但只在错误分支出现，且按行展示关联与指引。

## Migration Plan

1. 确认后先改宿主结构/扫描与单测（三类 kind 与 detail 引导语、capabilityId、重复去重）。
2. 再改前端类型、显示名与消息格式化，两个入口共用；补 Vitest。
3. 浏览器/原生复验：制造 `draft`/`retained`/`active` 三类拒绝，核对消息与解除后成功删除；
   质量门通过后归档并关闭 `image-studio-huq`。
