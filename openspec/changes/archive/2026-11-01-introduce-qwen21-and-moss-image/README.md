# Qwen-Image 2.1/Turbo 与 MOSS-Image-SFT 接入方案

状态：**实现与验收完成，已于2026-11-01归档**。编写：2026-10-30；顺序与实施确认：2026-10-31；恢复实施与完成验收：2026-11-01。文档任务：`image-studio-ego`；执行跟踪：Qwen `image-studio-u3q`、MOSS `image-studio-g37`。

前置：[三个计划模型接入与验收](../2026-11-01-integrate-planned-models/README.md)已完成并归档；[整批118/118认证](../../../../verification/planned-models-20261031/batch-acceptance.json)通过。用户已明确恢复Qwen/MOSS实施，按Qwen基础版/Turbo通过后再推进MOSS；历史Z-Image认证已通过，见[归档记录](../2026-10-31-certify-current-model-usability/README.md)。

建议阅读顺序：[提案](proposal.md) → [来源和型号](sources.md) → [技术设计](design.md) → [验收计划](test-plan.md) → [规格目录](specs/) → [实施任务](tasks.md)。

完整顺序为Klein生成/编辑 → BiRefNet → Real-ESRGAN → 三型号整批验收 → 本方案。进入本方案后顺序为 Qwen 基础版/官方 Turbo 两个精确配方，再到 MOSS-SFT。首轮均为 **1024×1024、单张 RGB PNG、纯文本生成**；保留 Z-Image 现有三个尺寸和原配方。Qwen 编辑/RGBA/多参考、MOSS Base/社区加速 LoRA 与可视化布局编辑不在本次范围。

本轮已完成固定来源下载、本机基线和正式产品验收。最终结论、性能口径和原始证据入口见[MOSS/整批阶段报告](../../../../verification/qwen-moss-20261101/moss-stage-report.md)与[Qwen 阶段报告](../../../../verification/qwen-moss-20261101/qwen-stage-report.md)。

本方案依赖前置change归档后的规格；复用它已交付的按型号安装和generation v2，不再次建立公共框架。Qwen/MOSS需新的worker/runtime身份，不能原地覆盖已认证Klein包。两份方案可以一并确认；其后按阶段门槛推进，不逐步重复询问批准。
