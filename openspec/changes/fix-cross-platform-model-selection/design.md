# Design: fix-cross-platform-model-selection

## 复现与根因

`apps/frontend/src/Workspace.tsx`（e025057 引入）：

```ts
const legacyWindowsTextModel = Z_IMAGE_MODEL_ID === Z_IMAGE_WINDOWS_MODEL_ID
  && Boolean(activeTextRecipe)
  && active.params.modelId !== Z_IMAGE_WINDOWS_MODEL_ID;
```

`Z_IMAGE_MODEL_ID` 是按平台解析的常量：Windows 上等于 `Z_IMAGE_WINDOWS_MODEL_ID`，因此第一个条件在 Windows 上**恒真**。e025057 时 Windows 只有 Z-Image 可执行，因此"非 Windows Z-Image 的文本模型一律提示重选"是合理的；`achieve-windows-model-parity` 接入 Klein/Qwen/MOSS 后该条件没有同步更新，导致：

- Windows 原生 id（`flux2-klein-4b-distilled-windows-v1`、`qwen-image-2.1-windows-v1`、`qwen-image-2.1-turbo-windows-v1`、`moss-image-sft-windows-v1`）全部误报；
- 未就绪标签固定显示"当前 Windows 未接入"（即使模型只是因为未安装而未就绪）；
- macOS 上条件恒假，因此 Windows 端保存的会话只有"未就绪"，没有跨平台说明。

实测（安装版）：选中 `FLUX.2 Klein 4B` / `Qwen 2.1 Turbo`（均 ready）→ 警告出现。

## 方案

### D1 平台支持集合成为唯一事实源（`model-recipes.ts`）

```ts
export const platformTextModelIds = (platform: ModelPlatform): ReadonlySet<string> =>
  new Set(Object.values(MODEL_IDS[platform]).map((entry) => entry.model));

export const isTextModelSupportedOn = (platform: ModelPlatform, modelId: string): boolean =>
  platformTextModelIds(platform).has(modelId);
```

纯函数、无 DOM 依赖，测试矩阵可直接用 `detectModelPlatform` 之外的显式 platform 参数。

### D2 Composer 判定（`Workspace.tsx`）

```ts
const crossPlatformTextModel =
  Boolean(activeTextRecipe) && !isTextModelSupportedOn(MODEL_PLATFORM, active.params.modelId);
```

- 警告文案按当前平台给出目标：
  - Windows：`此会话保留来自其他平台的模型选择。请从模型菜单明确选择对应的 Windows 型号（Z-Image / Klein / Qwen / MOSS），并核对画幅与参数后再发送。`
  - macOS：`此会话保留 Windows 端的模型选择。请从模型菜单明确选择对应的 macOS 型号，并核对画幅后再发送。`
- 标签：`crossPlatformTextModel && !modelReady` 时显示 `· 当前平台未接入`，否则沿用 `· 未就绪`。
- 其余行为（发送禁用、草稿保留、源图/能力判定）不变；`newTextOnlyModel` 等仍使用模型 id 集合，不受影响。

### D3 测试矩阵（`model-recipes.test.ts`）

| platform | modelId | 期望 |
|---|---|---|
| windows | `MODEL_IDS.windows.klein.model` | supported → 无警告 |
| windows | `MODEL_IDS.windows.qwen/qwenTurbo/moss/zImage` | supported → 无警告 |
| windows | `MODEL_IDS.macos.zImage/klein/qwen/moss` | unsupported → 警告 |
| macos | `MODEL_IDS.macos.*` | supported → 无警告 |
| macos | `MODEL_IDS.windows.*` | unsupported → 警告（新增 macOS 提示） |

另加一条组件层断言：在 Windows 平台选择 Klein/Qwen Turbo 时 `.composer-warning` 不包含"旧的跨平台模型选择"（可用现有 Vitest + 纯函数组合验证，避免引入 DOM 渲染测试成本）。

### D4 冒烟（安装版）

- Windows：打开安装版 → 选 Klein / Qwen Turbo / MOSS → 无跨平台警告；选择旧 macOS id（可通过旧会话或手工写入草稿模拟）→ 出现新警告与目标型号提示；未就绪文本模型标签显示"未就绪"而非"当前 Windows 未接入"。
- macOS：由用户在 Mac 上验证 Windows 会话提示（本机无法运行 macOS 安装版，标记 user-gated）。

## 风险与权衡

| 风险 | 处理 |
|---|---|
| 平台 UA 检测在浏览器预览下得到 macos，测试/预览出现 macOS 提示 | 与现有 `MODEL_PLATFORM` 行为一致；纯函数矩阵覆盖两种平台 |
| 旧会话在 Windows 上不再提示"选 Windows Z-Image"，而选 Klein 等 | 这正是修复目标；提示改为型号族选择 |
| 文案变化影响快照测试 | 现无该文案快照；新增断言按平台匹配 |
