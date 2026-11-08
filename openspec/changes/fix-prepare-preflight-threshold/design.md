# Design: fix-prepare-preflight-threshold

## 1. 证据与结论

| 观测 | 值 | 来源 |
|---|---|---|
| prepare 进程 commit 峰值（MOSS text encoder） | **23.003 GiB** | `C:\ist-a\logs\installation-process.log`（2026-11-04 成功安装） |
| prepare 期间系统可用物理内存最低值 | 1.147 GiB（靠分页完成） | 同上 |
| 2026-11-04 预检 commit 余量 | **27.06 GiB**，以 26 GiB 门槛通过（余量的余量 1.06 GiB） | `C:\ist-a\logs\prepare-preflight.json` |
| 2026-11-08 新用户全新根预检 | 可用物理 22.57 GiB（<28）、commit 余量 **24.83 GiB**（<26）→ blocked，无半成品 | `verification/windows-12gb-standard-tier/S3-qwen-prepare-preflight.json` |
| 今天（应用运行中）commit 余量 | 24.36–24.83 GiB | 实时 `Get-Counter` |

结论：本机 commit 余量已大于实测需要的 23.0 GiB，26 GiB 门槛是**过度保守**；机器能完成量化，却被判为不能，且要求最终用户改虚拟内存——这是软件回归。8 GB 参考机（commit 余量 60+ GiB）不受影响。

## 2. 修复设计

### 2.1 预检阈值

- `PREPARE_MIN_COMMIT_HEADROOM_BYTES`: 26 GiB → **24 GiB**。
  - 依据：实测最坏 prepare commit 23.003 GiB + 1 GiB 安全余量。
  - 保留 `PREPARE_MIN_AVAILABLE_PHYSICAL_BYTES = 28 GiB` 作为另一条通过条件（物理内存充裕的机器无需依赖分页）。
- 错误文案中的门槛数字同步更新（28 GiB / 24 GiB），其余建议不变。

### 2.2 可复用缓存短路

当前顺序是"先预检、后判断 prepared 缓存能否复用"，导致升级重装即使不需要重新量化也会被内存状态拦住。改为：

1. 先判断 `prepared_cache_reuse_allowed(&prepared)`（模型清单摘要 + 完整 inventory 摘要匹配）；
2. 可复用 → 跳过内存预检与 prepare，直接走复用路径并记录 `reused`；
3. 不可复用 → 执行内存预检；不通过则在量化前失败并保留可行动错误。

### 2.3 明确不做的替代方案

- **直接取消预检、失败再报错**：会把 16 GiB 级机器打到页面文件耗尽/长时间 thrash，失败边界不可界定；本轮不做。
- **分片流式量化**：`stream-prepare-low-memory` 已由所有者决定暂缓，本轮不改量化实现。
- **让用户调虚拟内存**：不属于产品修复，用户不应为软件门槛改系统设置。

## 3. 行为边界

- 24 GiB 余量 + 实测 23.0 GiB 峰值：prepare 期间可能触发 Windows 自动扩展页面文件（正常行为，2026-11-04 已观察到），仍有 1800 s 准备超时兜底。
- 低于 24 GiB 余量且低于 28 GiB 可用物理内存的机器：仍在量化前失败，错误文案给出关闭程序/换机器等建议。
- 复用缓存路径不进行任何量化，不需要内存余量。

## 4. 验证

- 单元：`prepare_memory_preflight` 边界（24 GiB-1 拒绝、24 GiB 通过、28 GiB 物理通过）；`prepared_cache_reuse_allowed` 为真时不再调用预检。
- 本机实测：全新根 `C:\ist-accept` 安装 Qwen-Image 2.1 Turbo / Qwen-Image 2.1 / MOSS-Image-SFT（修复前 blocked，修复后应通过），各跑一次标准档生成，记录 stages/耗时/PNG/GPU 采样。
- 回归：现有 8 GB 参考机证据不受影响；`openspec validate --all --strict`、`cargo fmt/clippy/test`、跨平台测试通过。

## 5. 风险与回滚

| 风险 | 缓解 |
|---|---|
| 24 GiB 余量机器上 prepare 分页导致机器卡顿 | 已有 1800 s 超时与失败可行动错误；prepare 完成后 release 内存；日志记录峰值 |
| 页面文件磁盘增长 | prepare 完成后 Windows 按策略管理；C: 当前 1 TB 空闲 |
| 修改主规格导致与历史证据不一致 | 证据文件保留原 26 GiB 口径的 blocked 记录，新增修复后记录；spec 明确旧门槛误拒证据 |

回滚：恢复 `PREPARE_MIN_COMMIT_HEADROOM_BYTES = 26 GiB` 与旧的预检顺序即可，无数据迁移。
