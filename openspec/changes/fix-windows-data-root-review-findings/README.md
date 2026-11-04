# fix-windows-data-root-review-findings

**状态：等待用户确认（2026-11-04）。** 对已归档的数据根加固做独立 review 后发现 6 个真实问题（macOS 编译 blocker、路径预算 unsafe、canonical 绕过、旧格式残留不回收、清理晚于空间预检、默认根日志写 CWD）。完整 review 见 `verification/full-review-2026-11-04/README.md`；本目录目前只有 change 文档，未改实现代码。

来源：bd `image-studio-kf5`（P0）。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | review 结论、变更范围、成功标准、不做项 |
| [design.md](design.md) | cfg 隔离、最终布局预算（92）、canonical 校验、新旧暂存名匹配、清理时机、日志路径、验收设计 |
| [specs/](specs/) | `windows-desktop`（2 个 MODIFIED requirement）、`model-installation`（1 个 MODIFIED requirement） |
| [tasks.md](tasks.md) | 实现、单测、安装版复验、归档 |
| [test-plan.md](test-plan.md) | 单元矩阵 + Windows 安装版 + macOS user-gated |
