# Tasks

状态：**已实现并在 macOS 原生验收通过（2026-11-12），准备归档。**证据：`verification/reference-guidance-20261112/RESULTS.md`。

## 1. 宿主冲突契约

- [x] 1.1 `AssetReferenceConflict` 增加 `capabilityId`，扫描区分 `draft`/`retained`/`active` 三类来源；`retained` 携带命中的能力 id；每类 detail 带可执行解除指引；同会话同来源去重。
- [x] 1.2 更新/新增 Rust 测试：当前源图拒绝（`draft`、无 capabilityId）、未激活槽位拒绝（`retained`、含 capabilityId 与解除指引）、活动请求拒绝（`active`、等待/取消指引）；按指引解除后冲突清空。

## 2. 前端渲染

- [x] 2.1 `studio-api.ts` 的 `kind` 联合类型加入 `retained`、增加可选 `capabilityId`；新增 `capabilityDisplayName`（文本 recipe + BiRefNet/Real-ESRGAN + 未知回退）与 `referenceConflictsMessage`（按行输出关联、指引与能力名）；`AssetLibrary` 首次确认与重试两个分支共用。
- [x] 2.2 Vitest：三类消息格式、能力显示名映射与未知回退、空/多冲突不崩溃。

## 3. 集成验收与归档

- [x] 3.1 浏览器/原生复验：制造 draft、retained、active 三类拒绝并核对消息；按提示解除后删除成功；质量门（前端 test/check/build、Rust fmt/clippy/test、OpenSpec strict，仅允许既有 4 条长度警告）通过。
- [x] 3.2 归档 change、合并 delta、关闭 `image-studio-huq`，验收证据写入 `verification/`。

## Workflow follow-up

- 用户确认后实施；不得改变拒绝规则或自动解除引用。
