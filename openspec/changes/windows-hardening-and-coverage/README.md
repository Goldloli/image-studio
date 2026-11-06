# windows-hardening-and-coverage

**状态：2026-11-06 提案与清单完成，等待项目所有者确认后按优先级执行。**

本轮 8 GB 验收与桌面缺陷修复暴露出两类剩余工作：**验收覆盖不全**（几个能力从未在产品里跑过）与**可观测性不足**（GUI 没有日志文件，排查靠重定向 stderr）。本 change 把这些建议、证据与执行顺序写成可跟踪的文档，避免散落在会话记录里。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 为什么需要、范围、成功标准、不做项 |
| [design.md](design.md) | 逐条建议的依据、方案与风险 |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | 运行日志与失败文案的 delta |
| [tasks.md](tasks.md) | 按优先级的执行清单 |
| [test-plan.md](test-plan.md) | 验收覆盖矩阵与判定规则 |
