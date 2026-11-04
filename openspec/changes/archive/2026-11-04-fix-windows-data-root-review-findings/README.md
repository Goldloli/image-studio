# fix-windows-data-root-review-findings

**状态：已实现、安装版验收并归档（2026-11-04）。** 独立 review 发现的 6 个数据根问题全部修复：macOS cfg 编译、短运行时目录 `rt-<12 hex>` + 94 units 预算（按最长 Python 可打开路径）、canonical data 复检、新旧暂存目录在空间预检前回收、默认根日志路径。安装版验证 data=88 / 94 真实生成成功、data=127 exit 3、真实 4.23 GB 旧残留被回收、旧运行时目录迁移后 6/6 ready。证据 `verification/windows-data-root-guard/review-fix/` 与 `verification/full-review-2026-11-04/README.md`。

来源：bd `image-studio-kf5`（P0）。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | review 结论、变更范围、成功标准、不做项 |
| [design.md](design.md) | cfg 隔离、最终布局预算（92）、canonical 校验、新旧暂存名匹配、清理时机、日志路径、验收设计 |
| [specs/](specs/) | `windows-desktop`（2 个 MODIFIED requirement）、`model-installation`（1 个 MODIFIED requirement） |
| [tasks.md](tasks.md) | 实现、单测、安装版复验、归档 |
| [test-plan.md](test-plan.md) | 单元矩阵 + Windows 安装版 + macOS user-gated |
