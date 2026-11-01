# Qwen-Image 2.1 / Turbo 阶段报告

日期：2026-11-01
状态：Qwen 基础版与 Turbo 全部 P0 验收通过。

## 结论

Qwen-Image 2.1 基础版与官方 Turbo 已从 ModelScope 固定文件 revision 完成隔离安装，并在 Apple M3 Max / 128 GiB 上通过上游基线、正式产品生成、取消重试、强杀恢复、tokenizer 边界、完整断网生成和原生 UI 发送/重开/导出。

两个型号的 `verifiedDevices` 已更新为这台 M3 Max；Qwen Beads `image-studio-u3q` 可关闭，随后按依赖启动 MOSS `image-studio-g37`。

## 来源与安装

- ModelScope 三个仓库共 60 个正式组件全部与冻结 HF 快照的路径、字节数、SHA-256 一致。
- 普通文件额外通过 Git blob ID 核对。
- 每个下载 URL 使用 ModelScope 逐文件 revision，不使用 `master`。
- 安装器允许 `modelscope.cn` 及其 LFS 子域，拒绝伪域名，并显式发送 `ImageStudio/0.1.0` UA 以通过 CDN ACL。
- 基础版：25 个组件，33,131,609,424 bytes。
- Turbo：16 个组件，32,451,607,098 bytes。
- 共享 runtime：`mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3`。

## Qwen 基础版

- 上游基线：T01–T03 全部 40/40 步；总 1214.85 秒。
- 产品连续生成：T01–T03 全部成功，40/40 步，单任务 298.0、312.6、309.9 秒。
- 产品资产与导出 PNG SHA-256 一致，完成后无残留 worker。
- 取消：`sampling 1/40`、22.1 秒取消；同 Job 新 Attempt 成功。
- 强杀：`sampling 1/40`、22.2 秒退出 73；重开后 Attempt 为 `interrupted`，同 Job 重试成功。
- 断网：完整 `deny network*` 沙盒中 T01–T03 全部 40/40 步并导出成功。

证据：

- [上游基线](qwen-baseline/results.json)
- [产品生成](qwen-product/results.json)
- [取消重试](qwen-cancel-retry/results.json)
- [强杀恢复](qwen-kill-retry/results.json)
- [断网产品生成](qwen-offline/results.json)
- [tokenizer 边界](qwen-token-boundaries.json)

## Qwen Turbo

- 上游基线：T01–T03 全部 8/8 步；总 193.70 秒。
- 产品连续生成：T01–T03 全部成功，8/8 步，单任务 93.7、78.5、78.6 秒。
- 产品资产与导出 PNG SHA-256 一致，完成后无残留 worker。
- 取消：`sampling 1/8`、21.4 秒取消；同 Job 新 Attempt 成功。
- 强杀：`sampling 1/8`、21.9 秒退出 73；重开后 Attempt 为 `interrupted`，同 Job 重试成功。
- 断网：完整 `deny network*` 沙盒中 T01–T03 全部 8/8 步并导出成功。

证据：

- [上游基线](qwen-turbo-baseline/results.json)
- [产品生成](qwen-turbo-product/results.json)
- [取消重试](qwen-turbo-cancel-retry/results.json)
- [强杀恢复](qwen-turbo-kill-retry/results.json)
- [断网产品生成](qwen-offline/results.json)
- [tokenizer 边界](qwen-turbo-token-boundaries.json)

## 质量审阅

- 基础版中文海报正确生成“秋日咖啡 / 周末限定”，排版清晰。
- Turbo 中文海报同样完整清晰；8 步版本仍维持可读文字。
- 两版本的英文产品静物、水彩小猫、主体结构和 COLOR/RGB 1024×1024 输出均正常。

## 原生 UI

- 模型页显示 7 个正式接入型号、2 个已安装、2 个可执行；基础版和 Turbo 均显示组件齐备、运行时就绪、设备探针通过。
- 两个型号分别显示实机验证：`Apple M3 Max / 128 GiB / macOS arm64`。
- Turbo 在原生界面发送剪纸狐狸提示词，完成 8/8 步并成功生成；系统导出 PNG 与 Asset SHA-256 同为 `af520df8b257e998306a5d19435f2802c367e2252ae4e056c21d724b960e2b2c`。
- 基础版在原生界面发送灯塔提示词，完成 40/40 步并成功生成；系统导出 PNG 与 Asset SHA-256 同为 `bcd5592155fa63bf4e45cd1002cf9c2e5ec4a83c2a6801c6da974cdf831cda55`。
- 关闭并重开同一个数据根后，两个原生任务及 21 轮历史均保留。
- 原生创作参数只提供 1:1、单张；不支持的非方图画幅由严格契约测试保证不会被静默改写。

证据：[原生 UI 结果](qwen-native-ui/results.json)。

## 质量门

- `cargo test --workspace`：通过。
- `cargo clippy --workspace --all-targets -- -D warnings`：通过。
- `cargo fmt --all -- --check`：通过。
- 前端测试：37/37 通过。
- 前端生产构建：通过。
- worker v3：6/6 通过。
- Python acceptance tests：16/16 通过。
- OpenSpec strict：21/21 通过。
- 六锁/wheel 源码审计：通过，审计过程不联网。
- 原生 Tauri 渲染器断网启动：通过。

## 后续验收

Qwen 门禁满足后，`image-studio-g37` 已完成 MOSS-SFT 固定源安装、独立基线、T01–T04 文字/布局、取消恢复、断网、原生 UI 和整批回归。完整后续记录见[MOSS/整批阶段报告](moss-stage-report.md)。
