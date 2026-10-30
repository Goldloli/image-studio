# 现有模型测试与日常验收方案

状态：**已批准实施，验收未完成（35/37 P0）**。编写日期：2026-10-30。文档任务：`image-studio-ego`；执行任务：`image-studio-2px`。本轮结果见 [验收报告](../../../verification/model-usability-20261031/results.md) 与 [机器可读记录](../../../verification/model-usability-20261031/acceptance.json)。

建议阅读顺序：[提案](proposal.md) → [验收用例](test-plan.md) → [规格](specs/model-validation/spec.md) → [技术设计](design.md) → [实施任务](tasks.md)。

本 change 先验证当前唯一已接入的 Z-Image Turbo，覆盖隔离自动测试和默认用户目录下的原生日常流程。模型页中的 Klein、BiRefNet、Real-ESRGAN 作为计划项核对，不安装、不执行，也不计入已支持数。

后续 [Qwen/MOSS 接入方案](../introduce-qwen21-and-moss-image/README.md) 的实施以本 change 验收通过为前置条件。两份方案分别确认；本方案通过不代表新模型方案获批。CLI 的 artifacts complete / apply ready 仅表示文档齐备。
