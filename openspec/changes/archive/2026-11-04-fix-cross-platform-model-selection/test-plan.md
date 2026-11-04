# Test plan: fix-cross-platform-model-selection

## 纯函数矩阵（Vitest，本机可跑）

| platform | modelId | `isTextModelSupportedOn` | 预期 composer 提示 |
|---|---|---|---|
| windows | `MODEL_IDS.windows.zImage/klein/qwen/qwenTurbo/moss` | true | 无跨平台提示 |
| windows | `MODEL_IDS.macos.zImage/klein/qwen/moss` | false | 有提示，含"Windows"与"Z-Image / Klein / Qwen / MOSS" |
| macos | `MODEL_IDS.macos.*` | true | 无跨平台提示 |
| macos | `MODEL_IDS.windows.*` | false | 有提示，含"Windows 端"与"macOS 对应型号" |

## 安装版冒烟（Windows 目标机）

| ID | 场景 | 期望 |
|---|---|---|
| S1 | 选 Windows Klein / Qwen Turbo / MOSS | 输入框下方无"旧的跨平台模型选择"警告 |
| S2 | 选 Windows Z-Image（若安装）/未安装时 | 无该警告；未安装显示"未就绪" |
| S3 | 打开绑定旧 macOS Z-Image id 的会话/草稿 | 出现 Windows 版新警告，提示选择对应 Windows 型号 |
| S4 | 打开未安装文本模型 | 标签显示"未就绪"，不显示"当前 Windows 未接入" |

## user-gated（Mac 端）

| ID | 场景 | 期望 |
|---|---|---|
| M1 | macOS 打开含 Windows id 的会话 | 出现"来自 Windows 端，请选择 macOS 对应型号"提示，而非只有"未就绪" |
| M2 | macOS 选择本平台模型 | 无提示，发送正常 |

## 质量门

`npm test`（含新增矩阵）、`npm run check`、`npm run build`、`cargo fmt/clippy/test --workspace`、`openspec validate --all --strict`。
