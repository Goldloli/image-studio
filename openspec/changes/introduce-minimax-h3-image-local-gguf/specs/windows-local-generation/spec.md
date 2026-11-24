# Spec Delta

## ADDED Requirements

### Requirement: MiniMax-H3-Image Windows 使用独立 12 GB 实验 profile

Windows 产品 MAY 为 MiniMax-H3-Image 登记一个独立的实验性 12 GB/32 GB profile，使用固定原生运行时、GGUF 完整闭包、CPU 卸载和分块 VAE 解码。该 profile SHALL 只开放通过真机验证的 1024² 与 2048²，并在模型页显示完整下载、实验标识、许可证和实测资源；MUST NOT 增加、改写或继承现有七个 Windows 稳定型号的 profile。

#### Scenario: RTX 3080 Ti 12 GB 通过

- **WHEN** 目标 Windows 10 设备在隔离安装和禁网条件下完成 MiniMax-H3-Image 生成
- **THEN** 该机器可显示对应画幅的实验性可执行事实，其他设备不自动继承

#### Scenario: 8 GB 补充机

- **WHEN** Windows 11 RTX 2060 SUPER 8 GB/32 GB 打开 MiniMax-H3-Image
- **THEN** 型号显示无已登记 compact 档且不可安装或生成，不把 6.88 GB 文件等同于 8 GB 可用

### Requirement: Windows 原生实验体不得伪造稳定支持

MiniMax-H3-Image Windows 执行 SHALL 复用单活动生成、持久化接受、取消终态、候选验证和整树回收合同，但不改变当前稳定支持数或默认模型。未通过 Windows 真机、原生 UI、导出和恢复证据时 MUST 保持计划中或不可用。

#### Scenario: 探针通过但未真实生成

- **WHEN** 原生 CUDA 或 GGUF 小算子探针成功，但没有完整模型生成
- **THEN** Windows profile 仍为未运行，不增加实验性可执行型号或稳定数量

#### Scenario: 实验失败影响其他型号

- **WHEN** MiniMax-H3-Image 下载、安装、加载或生成失败
- **THEN** 当前七个稳定型号的安装、Ready、执行和作品保持可用，不删除其运行时或缓存
