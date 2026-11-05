# introduce-ci-release-pipeline

**状态：2026-11-05 审查后修订，等待用户确认新增修复范围；尚未归档。** 原始 workflow 已提交，但首次真实 Actions run 仍为 0 次。2026-11-05 的 Mac 端全面审查发现 macOS runner 已退役且架构标注错误、macOS Clippy 必然失败、并行测试存在偶发失败、公开仓库 Actions 供应链与双平台发布元数据不足；本轮将这些修复并入现有 change。

边界：`macos-14` 已于 2026-11-02 退役且为 Intel x64；当前把其产物命名为 `macos-arm64` 不成立。用户已授权在文档完成后将仓库切换为 public；源码实现必须在本修订获确认后开始。首次 quality/build/release run、产物下载校验与 draft Release 人工审阅仍以 GitHub 真实结果为准，不得用本地模拟冒充上线通过。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 背景、用户决定、范围、成功标准、不做项 |
| [design.md](design.md) | 三类 workflow、版本/缓存、步骤、未签名策略、风险 |
| [specs/ci-release/spec.md](specs/ci-release/spec.md) | 新能力 `ci-release` 的 delta |
| [tasks.md](tasks.md) | 实现、本地验证、用户启用与归档任务 |
| [test-plan.md](test-plan.md) | 本机可验证项 + user-gated 项 |
