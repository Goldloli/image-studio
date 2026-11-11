# “移除源图”后旧槽位来源残留修复 —— 验收结果

日期：2026-11-11（Asia/Shanghai）。Change：`fix-cleared-source-slot-reference`（Beads `image-studio-c2e`）。
执行方式：Vitest 回归测试 + 原生 computer use（`target/dev/Image Studio.app`，v6 真实数据根，
构建于修复提交 `4ea6ab4` 之后）。

## 自动化

`apps/frontend/src/workspace-state.test.ts` 新增 3 项：

1. **显式移除**：Klein 编辑（源图 `asset-A`）→ `setDraftParams({sourceAssetId:null, capabilityId:text})`：
   当前参数与 `capabilityDrafts[edit].sourceAssetId` 均为 null，序列化后的参数不再包含 `asset-A`；
2. **仅切换模型保留**：Klein 编辑（源图 A）→ 仅切换 Z-Image：`capabilityDrafts[edit].sourceAssetId`
   仍为 `asset-A`，当前参数来源为 null（切回该能力可恢复）；
3. **新源图不覆盖**：选择 `asset-B` 作为编辑源图后切到 BiRefNet：编辑槽位保留 B，Z-Image 槽位仍为
   null，当前参数来源为新工具的状态。

前端 `npm test`：**76 passed**；`check`、`build` 通过。

## 原生复验

1. 新建会话，真实生成作品 A（`asset-e4f11299d23a48bb944ce82feabe756f`，Z-Image Turbo，4:3）；
2. 选 Klein 模型并把 A 设为编辑源图：DB 当前草稿 `sourceAssetId=A`，保存成功；
3. **仅切模型到 Z-Image**：DB `capabilityDrafts[edit].sourceAssetId=A` 保留、当前参数来源 null——
   符合“切回可恢复”的既有语义；
4. 切回 Klein 编辑并再次选择 A，点击 **移除源图**：DB 当前 `sourceAssetId=null`，
   `capabilityDrafts[edit].sourceAssetId=null`，**不再有任何可见或不可见槽位引用 A**；
5. 作品页删除 A：确认框正常打开（标题“删除作品「验收-清除源图引用」？”），确认后
   `deletion_state=deleted`、登记文件消失，**没有**出现“仍被以下位置用作源图”的拒绝——
   修复前该场景必然被隐藏槽位阻止；
6. 清理：删除本次测试会话（墓碑），确认无悬挂引用后退出应用。
   `stale_refs=0`，应用已退出。

截图：`native-delete-after-source-removal.png`（可正常进入删除确认）、
`native-work-deleted.png`（删除完成后的作品页）。

## 质量门

- 前端 test/check/build 通过；Rust 未改动，全量 `cargo fmt/clippy/test --workspace` 327 passed；
- `openspec validate fix-cleared-source-slot-reference --strict` 通过；全量 strict 仅剩既有 4 条长度
  警告（`image-studio-6g2`）。

## 说明

历史版本在数据库中留下的“不可见槽位来源”不做自动迁移（无法区分用户刻意保留的编辑状态与残留
引用）。修复后用户可切回该能力 → 点击“移除源图”即可按新规则清空槽位，解除作品删除的引用阻止。
