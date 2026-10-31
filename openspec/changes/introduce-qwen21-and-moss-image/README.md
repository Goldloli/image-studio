# Qwen-Image 2.1/Turbo 与 MOSS-Image-SFT 接入方案

状态：**前置已完成；按用户2026-11-01要求暂停，尚未实施**。编写：2026-10-30；顺序与实施确认：2026-10-31。文档任务：`image-studio-ego`；执行跟踪：Qwen `image-studio-u3q`、MOSS `image-studio-g37`。

前置：[三个计划模型接入与验收](../archive/2026-11-01-integrate-planned-models/README.md)已完成并归档；[整批118/118认证](../../../verification/planned-models-20261031/batch-acceptance.json)通过。Qwen/MOSS不会自动开始，等待用户明确恢复；历史Z-Image认证已通过，见[归档记录](../archive/2026-10-31-certify-current-model-usability/README.md)。用户确认本方案与三个计划型号整批认证通过是独立条件，不能互相替代。

建议阅读顺序：[提案](proposal.md) → [来源和型号](sources.md) → [技术设计](design.md) → [验收计划](test-plan.md) → [规格目录](specs/) → [实施任务](tasks.md)。

完整顺序为Klein生成/编辑 → BiRefNet → Real-ESRGAN → 三型号整批验收 → 本方案。进入本方案后顺序为 Qwen 基础版/官方 Turbo 两个精确配方，再到 MOSS-SFT。首轮均为 **1024×1024、单张 RGB PNG、纯文本生成**；保留 Z-Image 现有三个尺寸和原配方。Qwen 编辑/RGBA/多参考、MOSS Base/社区加速 LoRA 与可视化布局编辑不在本次范围。

本轮只查询官方模型元数据和固定 MFLUX 源码，没有下载新模型权重或执行这些模型。源码中存在上游实现不等于 Image Studio 已经接入或当前设备已经验证。

本方案依赖前置change归档后的规格；复用它已交付的按型号安装和generation v2，不再次建立公共框架。Qwen/MOSS需新的worker/runtime身份，不能原地覆盖已认证Klein包。两份方案可以一并确认；其后按阶段门槛推进，不逐步重复询问批准。
