# introduce-ci-release-pipeline

**状态：已实现并通过本机验证，等待仓库所有者 push/启用 Actions 后归档（2026-11-04）。** workflow 文件与 macOS 打包扩展已提交（ed29da7）；YAML 结构、全部 workflow 命令、Windows CI 同参数构建与 checksum 均在本机通过（verification/ci-release/）。用户需授权 push 并在 GitHub 启用 Actions、首次运行后归档。

边界：workflow 文件由代理编写并本地校验；首次 push/启用 Actions、打 tag 发布 draft Release 需要用户在 Mac/GitHub 侧操作并授权。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 背景、用户决定、范围、成功标准、不做项 |
| [design.md](design.md) | 三类 workflow、版本/缓存、步骤、未签名策略、风险 |
| [specs/ci-release/spec.md](specs/ci-release/spec.md) | 新能力 `ci-release` 的 delta |
| [tasks.md](tasks.md) | 实现、本地验证、用户启用与归档任务 |
| [test-plan.md](test-plan.md) | 本机可验证项 + user-gated 项 |
