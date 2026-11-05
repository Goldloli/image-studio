## ADDED Requirements

### Requirement: 当前档位与降级原因对用户可见

模型页与发送区 SHALL 展示当前设备将使用的执行档位、画幅与计算精度；当设备不满足 `standard` 档而满足 `compact` 档时，SHALL 明确说明降级原因（例如"显存 8 GB < 12 GB，自动使用紧凑档 768²"），不得只显示"可用"而不说明实际画幅。参数面板显示的画幅 MUST 与实际请求契约一致。

#### Scenario: 8 GB 设备显示紧凑档

- **WHEN** 设备显存 8 GB，用户打开模型页或发送区
- **THEN** 五个生成模型显示"紧凑档 768²"及降级原因，安装与生成入口可用，不显示 1024² 参数

#### Scenario: 12 GB 设备保持标准档

- **WHEN** 设备显存 12 GB 及以上
- **THEN** 显示"标准档 1024²"，不出现任何降级提示

#### Scenario: 档位与请求一致

- **WHEN** 用户以紧凑档发送请求
- **THEN** 请求契约中的画幅为 768（或实测确定的 512），界面不退化为显示 1024²

## MODIFIED Requirements

### Requirement: Windows 目录登记新模型与硬件/许可元数据

模型目录 SHALL 为七个已接入型号登记平台可达性、Windows 安装与运行时 ID、能力、许可证，以及**按执行档位划分**的硬件门槛 `tiers`：每档注册 `id`、`min_vram_gb`、`min_ram_gb`、`resolution`、`compute_dtype`。`standard` 档为 12 GB / 32 GB / 1024² / `bfloat16`，`compact` 档为 8 GB / 32 GB / 768² / `float16`（单模型实测允许登记 512²）。条目 SHALL 保留可执行的最低门槛 `min_vram_gb`（compact 值）与 `standard_min_vram_gb` 供展示。`hardware_eligible` SHALL 在任一档满足时为真，`can_install` 与 `can_use` MUST 使用同一判定；设备事实未知时两档都不得视为满足。Windows 页面只投影与当前平台实际组合一致的状态。

#### Scenario: 目录新增档位元数据

- **WHEN** 目录为 Klein、BiRefNet、Real-ESRGAN、Qwen 基础/Turbo、MOSS 与 Z-Image 登记 Windows 条目
- **THEN** 每个条目包含固定 source/revision、能力、许可、`tiers` 与既有型号 ID，且不改变既有型号 ID 或能力 ID

#### Scenario: 8 GB 设备的门槛判断

- **WHEN** 设备显存 8 GB、内存 32 GB
- **THEN** 五个生成模型的 `hardwareEligible` 为真，原因文字说明使用紧凑档；Real-ESRGAN 与 BiRefNet 的既有判定不变

#### Scenario: 12 GB 设备的门槛判断

- **WHEN** 设备显存 12 GB、内存 32 GB
- **THEN** `hardwareEligible` 为真且选择 `standard` 档，`resolution` 为 1024，行为与改动前一致

#### Scenario: 目录新增型号

- **WHEN** 目录新增 Klein、BiRefNet、Real-ESRGAN、Qwen 基础/Turbo 或 MOSS 的 Windows 条目
- **THEN** 条目包含固定 source/revision、能力、许可、`tiers` 硬件门槛与既有型号 ID，且不改变既有型号 ID 或能力 ID

#### Scenario: 门槛判断

- **WHEN** 当前设备不满足某型号登记的全部档位门槛
- **THEN** 目录与模型页标记该型号在当前设备不可执行，安装/生成入口禁用并显示原因；满足紧凑档时不走此分支

#### Scenario: 设备事实不可读

- **WHEN** `nvidia-smi` 不可用或显存无法读取
- **THEN** 条目不可执行，原因文字提示无法确认并给出检查驱动的建议，不得默认按 compact 放行
