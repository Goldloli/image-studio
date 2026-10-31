# Qwen-Image 2.1 / Turbo 阶段报告

日期：2026-11-01
状态：终端侧全部核心验收通过；原生 UI 点击验收待解锁 macOS 后继续。

## 结论

Qwen-Image 2.1 基础版与官方 Turbo 已从 ModelScope 固定文件 revision 完成隔离安装，并在 Apple M3 Max / 128 GiB 上通过上游基线、正式产品生成、取消重试、强杀恢复、tokenizer 边界和完整断网生成。

原生 dev app 已启动在隔离数据根，但 macOS 当前锁屏，CUA 无法自动解锁和点击。因此 U01/U02 不标记通过，Qwen Beads `image-studio-u3q` 保持进行中，MOSS `image-studio-g37` 尚不开始。

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

## 待完成

macOS 解锁后需在已启动的 [Image Studio Qwen Models.app](../../../target/dev/Image%20Studio%20Qwen%20Models.app) 和隔离数据根 `target/qwen-moss-validation-20261101` 上完成：

1. 模型页确认基础版、Turbo 分别显示真实就绪与设备状态。
2. 会话切换两个型号，确认 1:1 默认、4:3 显式拒绝。
3. 原生发送、重开与 PNG 导出。
4. 保存 UI 证据后关闭 `image-studio-u3q`，再领取 `image-studio-g37` 开始 MOSS。

未完成点击验收前不得将 `verifiedDevices` 或 U01/U02 标记为通过。
