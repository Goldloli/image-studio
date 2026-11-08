# certify-windows-10-target-machine

**状态：2026-11-08 提案与清单完成，等待项目所有者确认后执行。未获确认前不开始构建、不开始验收执行。**

历史手工项（W05 100%/200% 缩放、W06 真实 IME、parity 4.4 样图复核）已在 2026-11-03 / parity 归档时由所有者接受，本 change 不重测、不再询问；只保留廉价可选检查（默认跳过）。

本机安装版构建于 2026-11-04 23:57；`v0.1.0` 标签（2026-11-05 14:18）之后的 **76 个提交**全部晚于该构建，从未在本机运行过，内容包括：8 GB 紧凑档、Windows 桌面缺陷修复、运行日志、CI/发布流水线。这些工作主要是在 RTX 2060 SUPER 8 GB 参考机与 CI 上取得证据；本机是**另一台目标机**——Windows 10 22H2 / RTX 3080 Ti 12 GB（标准档），它的产品级回归已被明确交给所有者执行（`windows-hardening-and-coverage/tasks.md` 3.3、`support-8gb-vram-tier/test-plan.md` U1/U2）。本 change 把这条主线连同本机可执行的其余验收项、证据缺口与文档收口写成可跟踪的执行清单。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 为什么需要、范围、成功标准、不做项 |
| [design.md](design.md) | 本机事实、逐项方法与决策点（内存预检、种子缓存、手工项） |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | 12 GB 标准档真机回归证据要求（ADDED） |
| [tasks.md](tasks.md) | 按优先级的执行清单（未确认前全部保持未完成） |
| [test-plan.md](test-plan.md) | 用例矩阵、判定规则与证据位置 |

证据目录（执行时创建）：`verification/windows-12gb-standard-tier/`。

文档校验：`openspec validate --all --strict` 27/27 通过（2026-11-08，含本 change）。
