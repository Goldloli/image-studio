## Purpose

在 Windows 上补齐 macOS 已认证的其余六个型号与能力，并以分阶段量化装载或明确硬件门槛保证 12 GB 目标机可安全执行。

## ADDED Requirements

### Requirement: Windows 在 Z-Image 之外补齐 macOS 型号

Windows 产品 SHALL 提供 FLUX.2 Klein 4B distilled、BiRefNet epoch_244、Real-ESRGAN x4plus、Qwen-Image 2.1、Qwen-Image 2.1-Turbo 与 MOSS-Image-SFT 的安装与执行闭环，并为每个型号登记独立的模型变体、配方、安装、运行时与 worker 身份；不得用近似名称、共享后端或改写历史 ID 冒充接入。

#### Scenario: 七个型号同店展示
- **WHEN** 用户在 Windows 模型页浏览目录
- **THEN** 七个已接入型号分别显示接入、安装、运行时、设备与可执行状态，计划中型号数量为 0，不把计划中型号显示为可安装

#### Scenario: 选择新模型生成
- **WHEN** 用户安装并选择 Klein、Qwen 或 MOSS 生成，或选择 BiRefNet/Real-ESRGAN 处理
- **THEN** 请求冻结该型号自己的 revision、配方、组件、运行时与 worker 身份，不继承 Z-Image 或 macOS 的参数与 Ready 状态

### Requirement: 大模型分阶段量化装载与资源门槛

对于在 12 GB 目标机上无法整载的模型，Windows 执行 SHALL 采用分阶段执行：文本编码 → 释放并清空缓存 → DiT 采样 → VAE 分块解码；同一时刻只允许一个大组件驻留 GPU，记录独占显存、进程提交与耗时；SHALL 禁止以 WDDM 共享内存回退或页面文件超卖作为通过。

#### Scenario: Qwen Turbo 在 12 GB 上生成
- **WHEN** 设备可用显存不足以同时容纳文本编码器与 DiT
- **THEN** 应用先编码提示词并释放编码器，再加载 DiT 采样，再分块解码；资源报告显示独占显存不触发共享回退

#### Scenario: 阶段超预算
- **WHEN** 任一阶段超过显存或提交预算
- **THEN** 执行以可诊断错误失败并清理整树，不提交候选、不伪造资产、不切换到更高硬件冒充通过

### Requirement: 硬件门槛与许可对用户可见

当某型号在目标设备上无法满足资源预算时，应用 SHALL 在模型页写明所需显存/内存与量化方式并禁用安装/生成入口；同时 SHALL 展示每个型号的许可证，Qwen-Image 2.1 系列必须标明仅非商用研究/评估使用。

#### Scenario: 设备不满足门槛
- **WHEN** 当前设备显存或内存低于模型登记门槛
- **THEN** 模型页显示门槛原因，安装与生成入口不可用，不出现静默失败或自动降级到其他模型

#### Scenario: 非商用许可
- **WHEN** 用户查看 Qwen-Image 2.1 或其 Turbo 型号
- **THEN** 页面显示 Qwen Research License 的非商用限制，制作记录保留该型号与许可身份

### Requirement: 新能力的 Windows 产品闭环

每个新型号 SHALL 在安装目录的原生应用中完成真实执行、阶段进度、取消/强杀恢复、查看、任务与作品查询、PNG（或 RGBA/mask）导出，并保留独立可复核证据；不得以 helper、示例或 macOS 结果替代。

#### Scenario: Klein 单图编辑
- **WHEN** 用户发送单图编辑请求
- **THEN** 使用 Klein 参考图条件编辑，保留源图与取消状态，导出结果与制作记录一致

#### Scenario: BiRefNet 双输出
- **WHEN** 用户对受控源图执行去背景
- **THEN** 原子提交 RGBA primary 与 8-bit mask，作品计数只统计 primary，两者均可查看与导出
