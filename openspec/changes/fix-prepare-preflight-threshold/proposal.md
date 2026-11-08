## Why

当前构建在 Windows 10 / RTX 3080 Ti 12 GB / 32 GiB RAM 目标机上，**新用户全新安装** Qwen-Image 2.1 Turbo、Qwen-Image 2.1 或 MOSS-Image-SFT 会被安装期量化预检拒绝：

- 实测可用物理内存 22.6 GiB（门槛 28 GiB）、commit 余量 24.8 GiB（门槛 26 GiB）；
- 但同一台机器 2026-11-04 成功安装过同一批模型：prepare 进程 commit 峰值实测 **23.003 GiB**、可用物理内存最低 1.147 GiB，靠分页完成；当时 commit 余量 27.06 GiB，仅比 26 GiB 门槛高 1.06 GiB。
- 今天重启后自动管理的页面文件回到 4.8 GiB，commit 余量 24.4–24.8 GiB，**高于实测需要的 23.0 GiB**，却被 26 GiB 门槛误拒。

这是软件侧回归：预检把"能完成的机器"判成"不能完成"，并迫使最终用户去调虚拟内存。新用户在默认 Windows 设置下无法安装这三个模型，而这正是它要避免的用户体验。

## What Changes

- **预检阈值改为实测口径**：commit 余量门槛 26 GiB → **24 GiB**（23.0 GiB 实测最坏值 + 1 GiB 安全余量）；保留"可用物理内存 ≥28 GiB"通过条件。
- **可复用缓存短路**：当 prepared 缓存可复用（模型清单摘要 + inventory 摘要匹配）时跳过内存预检与重新量化——升级后重装不应因内存状态被拒。
- **失败文案**更新为新门槛并提供可行动建议。
- 主规格 `model-installation` 的量化准备要求与场景同步更新；不改量化实现本身（不做分片流式，所有者已暂缓）。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `model-installation`：修改"安装期量化准备使用低内存分片路径"要求中的预检条件（26→24 GiB commit 余量），并新增"可复用 prepared 缓存跳过预检"与"默认虚拟内存 32 GiB 机器完成首次安装"场景。

## Impact

- 代码：`apps/desktop/src/adapters/installation.rs`（`PREPARE_MIN_COMMIT_HEADROOM_BYTES`、预检与复用判断顺序、错误文案、单元测试）；无新增依赖。
- 规格：`openspec/specs/model-installation/spec.md`。
- 证据：`verification/windows-12gb-standard-tier/` 增补修复后安装/生成记录，更新 S3 blocked 记录。
- 行为边界：16 GiB 级机器仍会在量化前被拒（commit 余量达不到 24 GiB），不会在低内存机器上打爆页面文件；Windows 自动扩展页面文件仍可用于 24 GiB 余量机器的量化峰值。

## 批准状态

**待项目所有者确认后实现。** 确认后按 tasks 顺序修改预检、重跑本机全新安装（Qwen Turbo / Qwen 2.1 / MOSS）与标准档生成，再归档。
