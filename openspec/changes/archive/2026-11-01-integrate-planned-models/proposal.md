## Why

模型中心的四个首批目标只有 Z-Image Turbo 已接入；Klein、BiRefNet 和 Real-ESRGAN 仍只有目录说明。用户要求先接通并测试这三个计划型号，全部验收后再推进 Qwen 和 MOSS，因此需要补齐实际安装、图片输入、不同执行器及真实产品验收。

**状态：已完成并于2026-11-01归档。** 用户按 Klein → BiRefNet → Real-ESRGAN → 整批验收顺序确认实施，最终四配置114个适用P0及4项整批回归全部通过（118/118），见[整批认证](../../../../verification/planned-models-20261031/batch-acceptance.json)和[阶段结果](../../../../verification/planned-models-20261031/results.md)。Z-Image本轮中英/三尺寸、取消恢复及历史v1重试已重新验证。用户明确要求归档之后暂停，Qwen/MOSS尚未开始。

## What Changes

- 保留现有三个目录 ID，逐项接入 FLUX.2 Klein 4B distilled 的文生图与单图指令编辑、BiRefNet general epoch_244 的去背景与蒙版、Real-ESRGAN x4plus 的固定 4× 放大。
- 安装器支持按型号独立状态、明确目标的安装/停止/继续与三类固定运行时；完整 manifest、离线依赖和真实设备检查通过后才激活，不通过修改 planned 字段冒充接入。
- 增加受控本地图片导入与已有作品选图；提交冻结能力、源图及处理配方。编辑、去背景和放大均复用统一 Turn/Job/Attempt、取消、重试、任务和作品链路。
- 支持 BiRefNet 一次事务提交透明 PNG 和蒙版，并按配方验证输出类型、尺寸和上限；Real-ESRGAN 输出可达 4096×4096，不全局放宽旧 Z-Image 的限制。
- 先在隔离根完成后端基线与正式产品测试，再通过默认目录的原生操作验收安装、发送、重开和导出。三型号各自全部必需项及 Z-Image 回归通过后，才允许进入后续 Qwen/MOSS 实施。
- 修订现有未批准的 `introduce-qwen21-and-moss-image` 方案，使其复用本批公共边界，顺序为 **Klein → BiRefNet → Real-ESRGAN → 三者整批验收 → Qwen 2.1 / Turbo → MOSS-SFT**。后续方案可以一并审阅批准，阶段间不重复要求批准；实质范围变化才重新确认。

## Capabilities

### New Capabilities

- `image-processing`：单源图导入、受控输入、编辑/去背景/放大配方、输出语义及来源追溯。

### Modified Capabilities

- `model-installation`：逐型号 manifest、隔离运行时与独立激活。
- `model-catalog`：三个新型号的真实接入、能力与可执行状态。
- `feature-catalog`：启用三项图片处理功能并按真实输入提供开始入口。
- `local-generation`：Klein 文生图与四步配方，不替换旧 Z-Image。
- `generation-conversation`：在统一会话提交源图功能，保存并恢复操作上下文。
- `generation-worker-protocol`：严格 v2 输入与多候选输出，支持 Python 和原生执行器。
- `project-storage`：导入来源、多输出原子提交与历史数据库迁移。
- `application-settings`：默认生成模型排除工具型号，环境与关于信息反映实际能力。
- `model-validation`：三个型号分别认证并作为 Qwen/MOSS 的前置门槛。

## Impact

涉及 core 配方/契约、desktop 安装/调度/存储/IPC、worker-protocol、原生监督器、前端功能/模型/会话/任务/作品页；新增 Klein worker v2、隔离 ONNX worker 和原生 ncnn 包装执行进程。SQLite 需迁移现有单输出约束；旧 v1、运行时锁和请求 JSON 保持原义。正式资源锁与实现均在批准后编写。

本轮不加入多参考、LoRA、任意模型插件、Core ML 加速、其他放大倍率、动漫模型、人脸修复、批处理或其他平台认证。Qwen/MOSS 继续由另一份方案定义，不能因本提案验收完成而自动视为已实现。
