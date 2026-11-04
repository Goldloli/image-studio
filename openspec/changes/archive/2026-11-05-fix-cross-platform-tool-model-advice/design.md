# Design: fix-cross-platform-tool-model-advice

## 现状

`Workspace.tsx` 的跨平台提示与标签分支只覆盖 `textRecipeForModel` 命中的模型。
BiRefNet/Real-ESRGAN 是源图工具，没有 text recipe；当草稿保存的是**另一平台**的
工具模型 id 时：

- `crossPlatformAdvice` 为 null → 无提示；
- `activeTextRecipe` 为 null → 标签落到最后的 `Z-Image Turbo · 未就绪`；
- `supportedDraftParams` 返回 false、发送禁用，但用户不知道该换哪个工具模型。

## 方案

### D1 工具模型镜像提示（`model-recipes.ts`）

```ts
const TOOL_ROLE_LABELS = { birefnet: 'BiRefNet 去背景', realesrgan: 'Real-ESRGAN 放大' };

crossPlatformToolModelAdvice(platform, modelId): string | null
```

- 仅匹配 `MODEL_IDS.{windows,macos}.{birefnet,realesrgan}` 的冻结 id；当前平台支持则
  返回 null；未知 id 返回 null。
- 文案按平台给出目标工具名与"核对源图"要求，不再混入文本型号族。

```ts
crossPlatformModelAdvice(platform, modelId) =
  crossPlatformTextModelAdvice(...) ?? crossPlatformToolModelAdvice(...)
```

### D2 Composer 接入（`Workspace.tsx`）

- 变量改为 `crossPlatformAdvice = crossPlatformModelAdvice(...)`；
- 文本模型分支保持原逻辑；
- 新增无 text recipe 的跨平台分支：
  `${modelDisplayName(modelId)} · 当前平台未接入`
  （`modelDisplayName` 已包含 mac/Windows 工具 id 的显示名）；
- 警告文案继续使用同一个 `<p class="composer-warning" role="alert">`。

### D3 测试

`model-recipes.test.ts` 增加：
- windows/macos 双向工具提示存在且包含目标工具名与"核对源图"；
- 本平台工具模型返回 null；
- 组合入口：文本仍返回型号族文案、工具返回工具文案、未知 id 为 null；
- 文本矩阵与原有 58 项测试不回归。

### D4 安装版冒烟

在隔离根 `C:\ist-a` 中把草稿参数临时改为 mac `birefnet-general-epoch-244` /
`image.remove-background.birefnet-general-epoch-244.v1`，重启安装版读取该草稿，确认：

- `.composer-warning` 显示 `请选择对应的 Windows BiRefNet 去背景 型号…`；
- 模型标签显示 `BiRefNet epoch_244 · 去背景 · 当前平台未接入`；
- 发送禁用；随后从备份恢复草稿。

## 风险

| 风险 | 处理 |
|---|---|
| 文案与 `fix-cross-platform-model-selection` 的文本提示冲突 | 文本入口原样保留，组合入口只做兜底；两套测试都保留 |
| 跨平台工具草稿 Composer 布局（无工具控件） | 只加提示/标签，不改变布局；用户重选本平台工具后恢复原交互 |
| macOS 真机不可验证 | 与文本修复一致，平台矩阵单测覆盖，Mac 冒烟 user-gated |
