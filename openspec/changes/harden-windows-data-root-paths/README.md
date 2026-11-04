# harden-windows-data-root-paths

**状态：等待用户确认（2026-11-04）。** 用户已确认顺序：先做本 change，再做 `introduce-ci-release-pipeline`；本目录目前只有 change 文档，未写实现代码。

来源：`achieve-windows-model-parity` 验收发现的深数据根长路径失败（`verification/windows-parity-app/install-failure-long-data-root.json`）与 4.23 GB 暂存目录残留；bd `image-studio-slk`。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 背景、用户决定、变更范围、成功标准、不做项 |
| [design.md](design.md) | 实测数据、缩短暂存名、启动前校验、残留清理、风险 |
| [specs/](specs/) | `windows-desktop`、`model-installation` 的 delta |
| [tasks.md](tasks.md) | 实现、测试、安装包复验与归档任务 |
| [test-plan.md](test-plan.md) | 深根/超限/回收/回归验收矩阵 |
