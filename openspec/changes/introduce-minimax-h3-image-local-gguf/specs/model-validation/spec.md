# Spec Delta

## ADDED Requirements

### Requirement: MiniMax-H3-Image 按平台和 profile 独立认证

MiniMax-H3-Image 验收 SHALL 分别记录 macOS arm64 与 Windows x64 的模型、量化组合、原生二进制、许可证、设备、应用构建和画幅。每个平台 SHALL 独立提供空缓存或复用缓存安装、组件摘要、设备探针、禁网生成、取消/重试、原生 UI 发送、重开与导出证据；一个平台或一个画幅通过 MUST NOT 认证另一配置。

#### Scenario: 仅 macOS 完成生成

- **WHEN** macOS M3 Max profile 完成真实生成，而 Windows 目标机未运行
- **THEN** 仅 macOS 对应配置可记录为通过，Windows 保持 `not_run`

#### Scenario: 仅有上游示例

- **WHEN** 官方模型卡、社区量化页或推理框架提供示例图，但产品未执行安装和原生生成
- **THEN** 所有产品必需用例保持未运行，不以外部示例增加支持数量

### Requirement: 完整下载和运行资源必须有测量口径

MiniMax-H3-Image 认证 SHALL 分开记录完整权重下载字节、运行时和暂存空间、GPU 峰值、系统内存或 commit 峰值、生成耗时及测量工具。6.88 GB DiT、模型参数推算或任务管理器单点读数 MUST NOT 代替完整闭包和端到端资源证据。

#### Scenario: 12 GB Windows 实验 profile

- **WHEN** Windows RTX 3080 Ti 12 GB/32 GB 执行 2048²
- **THEN** 报告证明原生卸载路径未触发不可接受的共享回退，并记录实际峰值与完成时间

#### Scenario: 仅有 6.88 GB 文件测量

- **WHEN** 报告只说明 DiT 文件大小或下载完成，没有完整组件、内存和生成测量
- **THEN** 资源与设备门槛保持未通过，不宣称 8 GB 或 12 GB 可用

### Requirement: 量化候选必须通过质量对照

每个 MiniMax-H3-Image 量化候选 SHALL 使用固定提示词、种子、画幅、运行时和上游对照完成人工质量审阅与可复现数值记录。Q2 候选 MUST NOT 因能够运行或文件最小而自动成为正式默认；Q3、Q4 或其他候选的质量、速度和内存取舍 SHALL 分别记录。

#### Scenario: Q2 跑通但画质不达标

- **WHEN** Q2 能生成图片，但与更高精度基线相比出现不可接受的细节、文字或结构退化
- **THEN** Q2 只保留为实验探针，不登记为默认配方或稳定质量结论

#### Scenario: 更高精度候选超预算

- **WHEN** Q3 或 Q4 在当前设备超过显存、内存或时限门槛
- **THEN** 该候选不激活且不回退到 Q2，用户需选择已验证的原生 profile
