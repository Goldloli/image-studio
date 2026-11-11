# Design

## Context

见 [proposal.md](proposal.md)。根因在 `workspace-state.ts` 的 `params` reducer 能力切换分支：
旧能力槽位由 `d.params`（切换前状态）构造，而“移除源图”与能力切换在同一个 patch 中发生：

```ts
// 移除源图按钮
setDraftParams({ sourceAssetId: null, capabilityId: textCapability })
```

因此 `slots[edit]` 记录的是 `d.params.sourceAssetId`（仍为 A），而当前参数已清空；引用检查
（`asset_reference_conflicts` 扫描所有槽位的 `sourceAssetId`）继续把 A 视为被引用。

## Goals / Non-Goals

**目标**：显式“移除源图”后，任何能力槽位都不再保留该来源；用户无需删除会话即可解除作品删除的
引用阻止；不改变“切换模型后切回可恢复原编辑状态”的既有行为。

**非目标**：自动清理历史数据库中已有的不可见槽位引用（无法区分保留状态与残留）；改变引用检查
规则；在错误对话框中新增“如何解除”的文案（当前 detail 已指出会话与草稿）。

## Decisions

### 1. 显式清空传播到旧槽位，新来源只作用于目标能力

在构造 `slots[current]` 时：

```ts
sourceAssetId:
  action.patch.sourceAssetId === null
    ? null                                  // 用户显式移除源图 -> 旧槽位也清空
    : d.params.sourceAssetId ?? null,       // 切换模型/设置新源图 -> 保留旧槽位来源
```

- “移除源图”patch 恰好携带 `sourceAssetId: null`，因此清除会同时落在被切走的能力上；
- “选择源图”patch 携带具体 AssetId，只属于目标能力，不覆盖旧槽位（旧槽位的来源属于该能力自己的
  编辑状态）；
- `seed`/`aspect`/`prompt` 的既有处理不变（工具的 seed 清空只作用于新的工具参数）。

### 2. 加载不迁移，提供可用的解除路径

无法从持久化数据判断某个槽位来源是“用户刻意保留的编辑状态”还是“旧 bug 留下的不可见引用”，
因此不做启动迁移。修复后，用户切回该能力时来源会被恢复（可见），点击“移除源图”即可按新规则
同时清空槽位；这满足 `asset-library` “解释需解除的引用”的要求，且不会误删用户刻意保留的槽位。

### 3. 回归测试

- 显式移除：Klein 编辑（源图 A）→ `setDraftParams({sourceAssetId:null, capabilityId:text})` →
  `capabilityDrafts[edit].sourceAssetId === null`，随后 `asset_reference_conflicts` 不再命中 A；
- 保留语义：Klein 编辑（源图 A）→ 仅切换模型到 Z-Image（patch 无 sourceAssetId）→
  `capabilityDrafts[edit].sourceAssetId === 'A'`，切回可恢复；
- 新来源不覆盖：从文本能力选择源图 B（patch.sourceAssetId=B）→ 旧文本/其他槽位来源不被改成 B。

## Risks / Trade-offs

- 用户先选 A 再选 B 时，A 只在目标能力槽位被 B 覆盖；其他能力槽位各自的来源保持，符合“每个能力
  独立编辑状态”的既有语义。
- 若未来新增“清空源图但不切换能力”的 UI，同样走 `sourceAssetId:null` patch，规则自然生效。

## Migration Plan

1. 确认后实现 reducer 变更与三个回归测试。
2. 原生 computer use 复验：Klein 编辑选源 A → 移除源图 → 作品页删除 A 不再被该会话阻止；
   取消删除以免实际删除用户作品；另验证直接切模型仍保留槽位来源。
3. 全量质量门后归档、合并 delta，关闭 `image-studio-c2e`。
