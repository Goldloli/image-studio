# certify-windows-10-target-machine

**状态：2026-11-08 提案与清单完成，等待项目所有者确认后执行。未获确认前不开始构建、不开始验收执行。**

当前主分支（`b861ae5`，领先 `origin/main` 0 提交）相比本机上一次验收（2026-11-04 的 v0.1.0 安装版）多了 86 个提交：8 GB 紧凑档、Windows 桌面缺陷修复、运行日志、CI/发布流水线。这些工作主要是在 RTX 2060 SUPER 8 GB 参考机与 CI 上取得证据；本机是**另一台目标机**——Windows 10 22H2 / RTX 3080 Ti 12 GB（标准档），它的产品级回归已被明确交给所有者执行（`windows-hardening-and-coverage/tasks.md` 3.3、`support-8gb-vram-tier/test-plan.md` U1/U2）。本 change 把这条主线连同本机可执行的其余验收项、证据缺口与文档收口写成可跟踪的执行清单。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 为什么需要、范围、成功标准、不做项 |
| [design.md](design.md) | 本机事实、逐项方法与决策点（内存预检、种子缓存、手工项） |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | 12 GB 标准档真机回归证据要求（ADDED） |
| [tasks.md](tasks.md) | 按优先级的执行清单（未确认前全部保持未完成） |
| [test-plan.md](test-plan.md) | 用例矩阵、判定规则与证据位置 |

证据目录（执行时创建）：`verification/windows-12gb-standard-tier/`。
