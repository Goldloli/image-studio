# fix-daily-generation-interactions

状态：**已实施、验收通过并归档**。执行任务：`image-studio-lgf`。验收证据见 [修复报告](../../../../verification/daily-generation-interaction-fix-20261031/results.md)。

本 change 修复当前模型 P0 日常验收发现的三个交互回归：

- `image-studio-bp9`：快速双击发送时，第二击命中已经切换的停止按钮并取消提交。
- `image-studio-1yi`：正式资产已提交后，首次进入作品页仍显示旧计数，需要手动刷新。
- `image-studio-d67`：关闭作品预览或返回来源后，切回创作会重开旧图片查看器。

阅读顺序：[提案](proposal.md) → [生成会话规格](specs/generation-conversation/spec.md) → [作品库规格](specs/asset-library/spec.md) → [技术设计](design.md) → [实施任务](tasks.md)。

边界：不改变 Job/Attempt/AssetRef、IPC、SQLite schema，也不把冷网络安装 `I01` 或真实 IME 自动化 `D03` 混入本次产品修复。用户确认本 change 后，才可进入测试与实现。
