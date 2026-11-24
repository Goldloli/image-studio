# Proposal

## Why

MiniMax-H3-Image 已发布开放权重，社区 GGUF 量化把单一 DiT 文件降到约 6.88 GB，为 Image Studio 增加高分辨率文生图能力提供了可行入口。但完整运行还依赖 32B 文本编码器、专用 VAE 和新的原生推理运行时，且许可证、显存、质量和真机证据均未验证；若直接把它并入当前七模型基线，会把“文件较小”误写成“低成本可用”。

## What Changes

- 新增一个独立、实验性的 MiniMax-H3-Image 本地文生图型号与能力；完成认证前不改变当前七模型/八能力的产品结论。
- 使用固定提交的 `stable-diffusion.cpp` 原生 worker，分别锁定 DiT、文本编码器、VAE、二进制和许可证，不复用现有 MFLUX 或 Diffusers 配方。
- 首期只接入纯文生图，支持 1024² 与 2048² 的可验证画幅；4096² 只在对应设备实测、登记后开放，图像编辑与多参考图不在本 change 范围。
- GGUF 组件以完整可运行闭包安装，不把 6.88 GB 的 Q2_K DiT 单独当作可运行模型；Q2 仅用于可行性探针，正式画质基线从更高精度候选开始验证。
- 为 MiniMax-H3-Image 增加独立支持级别、设备门槛和许可证确认；Windows RTX 3080 Ti 12 GB/32 GB 只能作为实验结果，不能继承现有 8 GB compact 档。
- 当前七个型号、八个能力、历史安装 ID、配方和运行时身份保持不变；用户明确确认本 change 前不写实现代码。

## Capabilities

### New Capabilities

- `minimax-h3-image`: 定义 MiniMax-H3-Image 的固定上游身份、完整组件闭包、许可证与地域边界、文生图配方、原生运行时以及不支持能力的边界。

### Modified Capabilities

- `model-catalog`: 增加实验性第八型号的展示、统计、平台和设备门槛，不把实验状态混入稳定认证。
- `model-installation`: 扩展安装范围，加入 MiniMax-H3-Image 的完整 GGUF 组件、原生运行时、许可证归档、空间预算和地域确认。
- `local-generation`: 增加 MiniMax-H3-Image 的纯文生图执行、进度、取消、重试与冻结参数行为。
- `generation-worker-protocol`: 增加专用原生 worker 的身份、请求、候选输出和进程回收要求，拒绝复用其他模型协议身份。
- `model-validation`: 增加 MiniMax-H3-Image 的安装、离线生成、质量、设备、取消和原生路径证据门槛。
- `windows-local-generation`: 增加 Windows 12 GB 实验 profile，并明确不登记 8 GB compact 档。

## Impact

- 宿主模型目录、安装 profile、Ready 记录、设备门禁和生成调度新增一个独立型号分支。
- 新增一份固定模型清单、一份原生运行时清单、一个 `stable-diffusion.cpp` sidecar 和一个实现 generation v2 的原生 worker。
- 前端模型目录、创作配方、能力筛选、许可提示和 4K 可用性需要增加实验型号语义。
- 发布产物、第三方声明、下载域名、许可证归档和校验规则需要同时更新；七个现有型号与八项现有能力的身份不得被重写。
- 本 proposal 完成后仍需用户确认；确认前 tasks 仅为审批后的实施与验收清单。
