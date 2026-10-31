# 接入并验收三个计划模型

日期：2026-10-31，Asia/Shanghai。状态：**用户已确认实施**。调研/文档 Beads：`image-studio-70s`；实施 Beads：`image-studio-d6w`。

推荐阅读：[提案](proposal.md) → [调研与来源](research.md) → [设计](design.md) → [测试计划](test-plan.md) → [规格](specs/) → [实施任务](tasks.md)。

| 顺序 | 型号 | 本批交付范围 |
|---|---|---|
| 1 | FLUX.2 Klein 4B distilled | 文生图与单图指令编辑，q4、4 步、1024×1024 |
| 2 | BiRefNet general epoch_244 ONNX | CPU 去背景，原尺寸透明 PNG 和蒙版 |
| 3 | Real-ESRGAN x4plus ncnn | 固定 4×，分块放大，最长边不超过 4096 |
| 4 | 整批验收 | 三型号逐项真实测试、原生日常、跨模型和旧 Z-Image 回归 |
| 5 | Qwen 2.1 / 官方 Turbo | 按[既有方案](../introduce-qwen21-and-moss-image/README.md)执行 |
| 6 | MOSS-Image-SFT | 在 Qwen 阶段通过后执行 |

Klein 已在 Apple M3 Max / 128 GiB 完成安装、文生图、单图编辑、质量、生命周期、禁网、恢复和原生日常验收，结构化结果为 `50 passed / 0 failed / 0 not_run / 0 blocked`。BiRefNet epoch_244 也已完成空缓存安装、CPU 图探针、RGBA/mask、质量、取消/崩溃恢复、双导出和原生日常验收，结构化结果为 `31 passed / 0 failed / 0 not_run / 0 blocked`。两阶段证据见[阶段报告](../../../verification/planned-models-20261031/results.md)；Real-ESRGAN 的实现与真实执行仍为 **not_run**，三型号整批门禁保持关闭。

用户明确确认后才能开始实现；OpenSpec artifacts complete / apply ready 仅表示文档齐全。本 change 获批后可按既定阶段连续推进，不要求每个步骤重复审批。Qwen/MOSS 实施还须其自身方案获批且本批认证通过。
