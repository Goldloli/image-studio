# activate-running-instance

修复 Windows 端“再点一次图标就打不开”的启动体验：同一数据根的第二次启动改为激活已运行窗口，而不是撞上 WebView2 profile 锁后弹“Image Studio 启动失败”。

**状态：2026-11-12 已实现并完成本机安装包验收（提交 `5345d4c`，实测见 `verification/single-instance-20261112/`）；归档待在用户确认后执行。**

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 现象、证据、目标行为与范围 |
| [design.md](design.md) | 按数据根的单实例身份、激活协议、回退路径与风险 |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | 数据根所有权 delta：同根第二次启动激活已有窗口 |
| [tasks.md](tasks.md) | 实现、回归与本机验收任务 |
| [test-plan.md](test-plan.md) | 本机可执行的手工与自动化验收步骤 |

背景证据：`verification/windows-functional-regression-20261112/README.md` 的 I03/F1；
启动失败诊断已由 change `restore-startup-failure-diagnostics`（提交 `9600557`）修复，
本 change 处理它背后的用户可见体验。

Beads：`image-studio-an2`。

