# Proposal

## Why

当前主分支相对 `v0.1.1` 主要删除旧页面和导航，并加入会话删除、作品收藏/删除与 SQLite v6 等本地管理功能；模型、运行时、worker、配方和 GPU 执行链未调整。此时再次执行 Windows 七模型、八能力全量生图与资源认证会重复既有证据，却也遗漏 CI 无法覆盖的真实安装、WebView2、Windows 文件占用和原生交互。

因此需要一份独立的 Windows **功能实机验收**：验证当前安装包和管理界面真实可用，不下载模型、不运行推理、不生成新图片；既有完整生图认证仅在执行面发生变化或证据不可追溯时重新执行。

## What Changes

- 为 Windows 增加“功能回归”验收档：使用当前构建和既有测试数据，验证安装、升级、窗口、导航、草稿、会话、作品、导出、删除及设置等功能。
- 功能回归 MUST NOT 下载模型、安装模型运行时、启动推理 worker、创建新生成任务或要求新的模型输出图片；界面截图、日志和现有作品文件可正常作为证据。
- 将完整生图认证改为按变更影响面触发：模型、运行时、worker、协议、配方、设备门槛、安装量化或执行提交链变化时才重新执行。
- 当差异分类不明确、既有认证缺失或不适用于当前产品时，默认回退到完整生图认证，不把功能通过冒充模型认证。
- 增加 Windows 功能回归清单和证据格式，明确当前构建、安装包哈希、旧生成基线、数据根、逐项状态与未运行项。

## Capabilities

### New Capabilities

无。本 change 复用既有 Windows 桌面能力，不引入新的产品能力。

### Modified Capabilities

- `windows-desktop`：把 Windows 验收拆成当前构建功能回归与按执行面变化触发的生图重认证，并明确功能回归不产生模型图片。

## Impact

- 文档：新增 proposal、spec delta、design、tasks 与 Windows 实机测试计划。
- 验收：需要 Windows 10/11 x64 实机和当前 CI 安装包；可在既有 RTX 3080 Ti 12 GB 目标机执行。
- 产品代码、IPC、数据库、模型清单、运行时、worker、安装器内容与发布流程均不修改。
- 既有 `verification/windows-12gb-standard-tier/` 与相关 Windows 证据继续作为生图基线，不在本 change 中改写为新的实测结果。
- Beads：`image-studio-7ed` 跟踪文档与后续验收。
