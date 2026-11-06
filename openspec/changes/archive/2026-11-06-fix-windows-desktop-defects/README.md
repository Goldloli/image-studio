# fix-windows-desktop-defects

**状态：2026-11-06 提案文档完成，等待项目所有者确认后开始实现。**

本 change 修复 8 GB 验收过程中在本机暴露的四个 Windows 桌面缺陷。它们都不是"没做完"，而是**已交付行为在真实使用中出错**：连续发送会卡死、取消落定要两分钟、安装期界面显示"尚未安装"、应用内下载 wheel 偶发失败。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 四个缺陷的现象、证据、修复范围与成功标准 |
| [design.md](design.md) | 逐条根因假设、修复方案、回归方式与风险 |
| [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md) | 发送可靠性与取消时限的 delta |
| [specs/model-catalog/spec.md](specs/model-catalog/spec.md) | 安装期阶段投影的 delta |
| [specs/model-installation/spec.md](specs/model-installation/spec.md) | 下载重试与可行动失败的 delta |
| [tasks.md](tasks.md) | 实现与验收任务 |
| [test-plan.md](test-plan.md) | 本机可验证项（含连续发送与故障注入） |
