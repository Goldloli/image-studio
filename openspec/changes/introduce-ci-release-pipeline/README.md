# introduce-ci-release-pipeline

**状态：等待用户确认（2026-11-04）。** 按用户决定：Windows 全模型对齐归档后，第二个 change 做双平台 CI/release；本目录目前只有 change 文档，未写 workflow/脚本实现。

边界：workflow 文件由代理编写并本地校验；首次 push/启用 Actions、打 tag 发布 draft Release 需要用户在 Mac/GitHub 侧操作并授权。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 背景、用户决定、范围、成功标准、不做项 |
| [design.md](design.md) | 三类 workflow、版本/缓存、步骤、未签名策略、风险 |
| [specs/ci-release/spec.md](specs/ci-release/spec.md) | 新能力 `ci-release` 的 delta |
| [tasks.md](tasks.md) | 实现、本地验证、用户启用与归档任务 |
| [test-plan.md](test-plan.md) | 本机可验证项 + user-gated 项 |
