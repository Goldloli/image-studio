# Test plan: fix-cross-platform-tool-model-advice

## 单元（Vitest）

| ID | 场景 | 期望 |
|---|---|---|
| U1 | Windows 打开 macOS BiRefNet/Real-ESRGAN id | 提示包含 `Windows BiRefNet 去背景` / `Windows Real-ESRGAN 放大` 与"核对源图" |
| U2 | Windows 原生工具模型 | 返回 null，无提示 |
| U3 | macOS 打开 Windows 工具 id | 提示包含"Windows 端"与 macOS 目标工具名；macOS 原生工具为 null |
| U4 | 组合入口 | 文本 id 仍返回型号族文案；工具 id 返回工具文案；未知 id 为 null |
| U5 | 既有文本矩阵 | 58 项前端测试全部通过（新增后 59 项） |

## 安装版（Windows 目标机）

| ID | 场景 | 期望 |
|---|---|---|
| A1 | 草稿改为 mac BiRefNet id（隔离根，备份后恢复） | 警告含 `Windows BiRefNet 去背景`；标签 `BiRefNet epoch_244 · 去背景 · 当前平台未接入`；发送禁用 |
| A2 | Windows 原生 BiRefNet/Real-ESRGAN 草稿 | 无跨平台警告；标签正常 |
| A3 | 文本模型跨平台会话 | 保持修复后的文本型号族提示（不回归） |

## user-gated

| ID | 场景 | 期望 |
|---|---|---|
| M1 | macOS 打开 Windows 工具会话 | 显示"来自 Windows 端，请选择 macOS 对应工具型号"（本机无法运行 macOS，矩阵单测覆盖逻辑） |

## 质量门

`npm test/check/build`、安装包构建（含 workspace 测试/前端门）、`openspec validate --all --strict`。
