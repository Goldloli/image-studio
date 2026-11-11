# Design

## Context

见 [proposal.md](proposal.md)。根因位于前端草稿状态：`workspace-state.ts` 的 `params` reducer 在
切换能力时把旧能力槽位按“切换前参数”写入 `capabilityDrafts`，却不检查目标能力支持的画幅；当
切换前画幅（如 4:3）不被目标能力支持（Klein/Qwen/MOSS/工具均只支持 1:1）时，槽位立即非法。
宿主 `validate_registered_draft_params` 对每个槽位按能力校验画幅，任一非法即拒绝整份草稿。

## Goals / Non-Goals

**目标**：让任何一次能力切换都产生对宿主合法的参数；修复历史异常草稿；空指令删除确认可辨识。

**非目标**：改宿主校验、放宽宿主规则、新增回收“非法槽位”的迁移、改变 Klein/Qwen/MOSS 的
画幅能力、改变删除范围或确认文案主体。

## Decisions

### 1. 能力 → 受支持画幅的单一映射

在前端增加纯函数 `aspectsForCapability(capabilityId)`（`model-recipes.ts`）：

- macOS `Z_IMAGE_CAPABILITY` → `['1:1','4:3','3:4']`；
- Windows Z-Image、Klein 文本/编辑、Qwen、Qwen Turbo、MOSS、BiRefNet、Real-ESRGAN → `['1:1']`；
- 未知能力 → `['1:1']`（保守，避免把未验证画幅写入槽位）。

与 `textRecipeForModel(...).aspects` 的数据保持一致；工具能力本就在 UI 中固定 1:1。不新增网络
或宿主查询，纯前端映射。备选“从宿主读取能力画幅表”需要新增 IPC 且与现有 recipe 重复，不采用。

### 2. reducer 在三个位置规范化

- **切换前槽位**：`slots[current]` 的 `aspect` 按 `current` 能力的受支持画幅规范化；
- **切换后参数**：应用 patch 后按 `next` 能力规范化当前 `params.aspect`；
- **兜底**：每次 `params` 动作返回前，仍按最终 `capabilityId` 规范化一次，防止未来新增 patch
  路径绕过。

规范化规则：当前画幅受支持则保留；否则取该能力受支持画幅的首项（现有能力均为 1:1，唯一多画幅
能力保留 1:1 首项；Z-Image 现有 4:3/3:4 不受影响）。

### 3. 加载时修复历史异常参数

增加 `normalizeDraftParams(params)` 并接入 `Workspace.tsx` 的 `storedDraft()`：把每个
`capabilityDrafts` 槽位与当前参数按能力规范化。这样旧版本留下的异常草稿在重启后进入内存即为
合法状态，下一次保存会持久化修复结果；不需要 schema/迁移，也不触碰执行历史。

### 4. 删除确认标题降级

`library-state.ts` 增加 `workDeleteTitle(item)`：提示词非空 → 截断至 24 字并加省略号；否则来源
会话标题非空 → 使用会话标题；否则 `该作品`。`AssetLibrary.tsx` 只替换标题，范围说明与确认行为
不变。

## Risks / Trade-offs

- 规范化会“修正”用户未提交但非法的旧画幅（例如历史 Klein 草稿的 4:3）→ 只在该画幅本就不被
  宿主接受时发生，属于恢复可保存状态；提示词、种子、源图等不受影响。
- 未知能力按 1:1 保守处理 → 若未来接入非 1:1 能力需同步更新映射与测试（在 tasks 中记录）。
- 标题降级不改变删除范围，不影响文件清理与引用检查。

## Migration Plan

1. 用户确认后建立实现 issue，先写纯函数与 reducer 测试（复现 4:3→Klein→切走场景）。
2. 接入 `storedDraft()` 修复路径与 `AssetLibrary` 标题，跑前端 test/check/build。
3. 浏览器预览复现路径确认保存失败消失；原生应用（computer use）抽验 Klein 切换与空指令删除标题。
4. 全量质量门通过后归档、合并两份 delta，关闭 `image-studio-31x`。
