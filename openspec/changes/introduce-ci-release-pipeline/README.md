# introduce-ci-release-pipeline

**状态：2026-11-05 本地实现完成，等待真实 GitHub Actions、artifact 下载和 draft Release 验收；尚未归档。** macOS Clippy、并行测试、退役 runner、公开仓库 Actions 供应链、正式 bundle identity 与双平台 Release 结构已修复；首次真实 quality/build/release run 仍未发生。

边界：`macos-14` 已于 2026-11-02 退役且为 Intel x64；当前把其产物命名为 `macos-arm64` 不成立。用户已授权在文档完成后将仓库切换为 public；源码实现必须在本修订获确认后开始。首次 quality/build/release run、产物下载校验与 draft Release 人工审阅仍以 GitHub 真实结果为准，不得用本地模拟冒充上线通过。

2026-11-05 已确认 `Goldloli/image-studio` 为 public；漏洞告警、Dependabot security updates、secret scanning 与 push protection 已启用，当前 secret-scanning alerts 为 0。main 分支保护与首次 Actions run 尚未完成。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 背景、用户决定、范围、成功标准、不做项 |
| [design.md](design.md) | 三类 workflow、版本/缓存、步骤、未签名策略、风险 |
| [specs/ci-release/spec.md](specs/ci-release/spec.md) | 新能力 `ci-release` 的 delta |
| [tasks.md](tasks.md) | 实现、本地验证、用户启用与归档任务 |
| [test-plan.md](test-plan.md) | 本机可验证项 + user-gated 项 |
