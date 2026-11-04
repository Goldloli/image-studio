# Test plan: harden-windows-data-root-paths

## 环境

- Windows 10 22H2 / RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K（目标机）。
- 安装版 `Image Studio_0.1.0_x64-setup.exe`（本 change 重建的 commit）。
- 数据根：短根 `C:\ist-a`（回归）、复现原失败的 ~83 字符深根（加固验证）、>117 字符超限根（拒绝验证）。

## 用例

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| D1 | 暂存目录名长度 | `.staging-<pid>-<8hex>` ≤ 26 字符，格式唯一 | 单元测试 |
| D2 | 深数据根（~83 字符）安装共享 v2 型号 | 运行时安装 + probe 成功，无 `文件名或扩展名太长` | 安装日志 + ready record + JSON |
| D3 | 超限数据根（>117 字符）启动 | 退出码 3，`startup-failure.log` 含中文可操作建议，无下载/staging | 日志 + 截图 + JSON |
| D4 | 残留 `.staging-*` 清理 | 正式运行时目录不动；staging 回收；清理失败不阻塞 | 单元测试 + 目标机证据 |
| D5 | 短根回归 | 安装/激活/生成不变 | 现有回归 + 冒烟 |
| D6 | 质量门 | fmt/clippy/test/前端/openspec 全绿 | 日志 |

## 不做

- 不验证 registry LongPathsEnabled 打开后的任意长路径（超出范围）。
- 不在 macOS/Linux 上验证 Windows 专属路径行为。
