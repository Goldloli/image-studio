## Purpose

让模型目录登记 Windows 可达性、量化/硬件门槛与许可，使用户在安装前能判断型号是否可执行。

## ADDED Requirements

### Requirement: Windows 目录登记新模型与硬件/许可元数据

模型目录 SHALL 为七个已接入型号登记平台可达性、Windows 安装与运行时 ID、能力、许可证，以及 `min_vram_gb`、`min_ram_gb`、量化方式等硬件门槛元数据；Windows 页面只投影与当前平台实际组合一致的状态。

#### Scenario: 目录新增型号
- **WHEN** 目录新增 Klein、BiRefNet、Real-ESRGAN、Qwen 基础/Turbo 或 MOSS 的 Windows 条目
- **THEN** 条目包含固定 source/revision、能力、许可与硬件门槛，且不改变既有型号 ID

#### Scenario: 门槛判断
- **WHEN** 当前设备不满足某型号登记的显存或内存门槛
- **THEN** 目录与模型页标记该型号在当前设备不可执行，安装/生成入口禁用并显示原因
