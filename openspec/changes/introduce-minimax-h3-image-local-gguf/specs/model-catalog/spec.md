# Spec Delta

## ADDED Requirements

### Requirement: MiniMax-H3-Image 作为实验性第八型号登记

模型目录 SHALL 将 MiniMax-H3-Image 登记为第八个逻辑型号和第九项能力，并增加 `supportLevel` 语义区分 `stable` 与 `experimental`。目录统计 SHALL 同时区分总条目、已接入、稳定、实验、已安装和当前可执行，实验型号不得被静默计入七个稳定型号的认证结论。

#### Scenario: 实验型号已安装

- **WHEN** MiniMax-H3-Image 完整安装并在当前设备通过实验 profile
- **THEN** 总条目为八、能力为九、稳定型号仍为七，实验型号有一个独立可执行事实

#### Scenario: 仅登记尚未接入

- **WHEN** 目录只包含 MiniMax-H3-Image 的固定来源、许可和计划中参数
- **THEN** 它显示为计划中或实验边界，不增加已接入、已安装、可执行或稳定计数

### Requirement: 实验型号的尺寸和设备事实独立展示

模型详情 SHALL 明确 MiniMax-H3-Image 首期只接入文生图、支持已登记画幅和独立设备 profile，并显示 6.88 GB 只是 DiT 文件而非完整下载量。实验型号 MUST 不继承其他模型的标准档、紧凑档、Mac 或 Windows 认证。

#### Scenario: 查看实验型号详情

- **WHEN** 用户展开 MiniMax-H3-Image
- **THEN** 页面分别显示 DiT、文本编码器、VAE、运行时、完整下载量、支持画幅、许可证和当前平台证据

#### Scenario: 8 GB 设备浏览目录

- **WHEN** Windows RTX 2060 SUPER 8 GB/32 GB 打开模型页
- **THEN** MiniMax-H3-Image 显示无已登记 compact 档且不可执行，不复用其他型号的 8 GB 可用状态

### Requirement: 实验支持级别不改变稳定筛选语义

模型页筛选 SHALL 保持现有“已安装、可执行、计划中、设备满足”行为，并增加可核对的实验支持标识。稳定筛选、关于页支持范围和默认创作模型 MUST 只在 MiniMax-H3-Image 通过对应验收且用户明确启用后变化。

#### Scenario: 实验型号可执行但未提升为稳定

- **WHEN** 当前机器可以执行 MiniMax-H3-Image，但实验证据仅覆盖该机器
- **THEN** 它出现在全部与可执行结果中，并保持实验标识，不作为其他硬件或新会话默认值

#### Scenario: 实验型号失败

- **WHEN** MiniMax-H3-Image 安装失败或运行探针失败
- **THEN** 其他七个稳定型号的筛选、安装和可执行状态不受影响
