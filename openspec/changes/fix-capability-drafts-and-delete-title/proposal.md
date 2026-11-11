# 修复能力切换草稿槽位与作品删除确认标题

## Why

macOS 原生验收 `simplify-workspace-and-manage-library` 发现两处问题（均已记录在
`verification/simplify-workspace-20261111/RESULTS.md`）：

1. **能力切换使草稿永久保存失败（image-studio-31x，既有缺陷）**：4:3 会话切到只支持 1:1 的
   Klein 时，画幅不会被修正；随后切换能力会把 Klein 文本能力以 `aspect=4:3` 写入
   `capabilityDrafts` 槽位，宿主对 1:1-only 能力的校验拒绝后续每一次 `draft_save`。新建会话、
   发送前 flush 全部失败，只能重启（DB 中仍是旧值）才能恢复。
2. **空指令作品的删除确认标题为「」**：BiRefNet 作品 instruction 为空时，确认框标题显示
   “删除作品「」？”，范围说明完整但无法从标题辨认目标。

## What Changes

### 用户已明确要求的修复

| 问题 | 提案行为 |
| --- | --- |
| 草稿槽位中毒 | 前端在能力/模型切换时按目标能力规范化画幅；保存槽位前按槽位能力规范化；加载既有草稿时同样修复，使历史异常参数在下一次保存被纠正。宿主协议与数据格式不变。 |
| 删除确认标题为空 | 删除确认标题优先用提示词；提示词为空时改用来源会话标题；两者都为空时用“该作品”。范围说明文字不变。 |

### 范围与约束

- 仅前端（`workspace-state.ts`、`model-recipes.ts` 或新增纯函数、`library-state.ts`、
  `AssetLibrary.tsx`）与测试；不新增依赖，不改宿主校验、数据库或 schema。
- 不改变正常路径的参数语义：调用方已提交的 4:3/3:4（Z-Image）保持不变；仅当当前画幅不被
  目标能力支持时收敛到该能力的首个受支持画幅（工具与 Klein/Qwen/MOSS 均为 1:1）。
- 既有 host 校验继续作为最后防线；本提案不让非法槽位进入持久化。

## Capabilities

### Modified Capabilities

- `feature-catalog`：能力切换 SHALL 保持草稿槽位与当前参数对宿主能力合法。
- `asset-library`：作品删除确认标题 SHALL 在提示词为空时仍能标识目标作品。

## Impact

- 前端纯函数与 reducer、`AssetLibrary` 标题、对应 Vitest；无 IPC/协议/存储变更。
- 验收：复现路径在浏览器预览或原生应用中不再出现保存失败；删除空指令作品的标题可读；
  质量门（前端 test/check/build、Rust 全量、OpenSpec strict）通过。
- 本提案阶段只提交文档；未获用户确认前不写实现代码。
