# Qwen-Image 2.1/Turbo 与 MOSS-Image-SFT 接入方案

状态：**待用户审阅，未批准实施**。日期：2026-10-30。文档任务：`image-studio-ego`；批准后的执行跟踪：Qwen `image-studio-u3q`、MOSS `image-studio-g37`。

前置：[现有模型测试与日常验收](../certify-current-model-usability/README.md) 必须先完成。用户确认本提案和前置认证通过是两个独立条件，不能互相替代。

建议阅读顺序：[提案](proposal.md) → [来源和型号](sources.md) → [技术设计](design.md) → [验收计划](test-plan.md) → [规格目录](specs/) → [实施任务](tasks.md)。

接入顺序为 Qwen 基础版/官方 Turbo 两个精确配方，再到 MOSS-SFT。首轮均为 **1024×1024、单张 RGB PNG、纯文本生成**；保留 Z-Image 现有三个尺寸和原配方。Qwen 编辑/RGBA/多参考、MOSS Base/社区加速 LoRA 与可视化布局编辑不在本次范围。

本轮只查询官方模型元数据和固定 MFLUX 源码，没有下载新模型权重或执行这些模型。源码中存在上游实现不等于 Image Studio 已经接入或当前设备已经验证。
