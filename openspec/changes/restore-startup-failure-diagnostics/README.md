# restore-startup-failure-diagnostics

修复 2026-11-12 Windows 11 本机功能回归暴露的缺陷 F1：同一数据根启动第二个实例时进程以退出码 101 静默结束，既没有 `logs/startup-failure.log`，也没有原生错误对话框。根因是日志 panic 钩子替换了启动失败钩子。

**状态：实现与复验完成（提交 `9600557`，复验安装包 `f54a434`）；等待用户确认后归档。**

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 现象、证据、根因、修复范围与成功标准 |
| [design.md](design.md) | 钩子链方案、备选方案与风险 |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | “启动失败可解释且不静默退出”的钩子链 delta |
| [tasks.md](tasks.md) | 实现、回归与本机复验任务 |
| [test-plan.md](test-plan.md) | 本机可复现的验证步骤 |

Beads：`image-studio-z1v`。证据：`verification/windows-functional-regression-20261112/`（I03、F1）。
