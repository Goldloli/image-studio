# Spec Delta

## Purpose

为 Image Studio 定义一个来源固定、许可证边界清晰且必须经过真机验证的 MiniMax-H3-Image 本地文生图型号，使高分辨率能力不会因单一量化文件较小而被误报为完整可用。

## ADDED Requirements

### Requirement: 模型身份包含完整可运行闭包

MiniMax-H3-Image 型号 SHALL 同时绑定官方上游快照、固定社区量化提交、DiT、Qwen3-VL-32B 文本编码器、专用 24 通道图像 VAE、原生运行时和许可证身份。只有 DiT 文件、相似模型、P-H3 派生模型或不匹配文本编码器 MUST NOT 登记为就绪。

#### Scenario: 只下载 6.88 GB DiT

- **WHEN** 本机只存在 MiniMax-H3-Image 的 Q2_K DiT 文件
- **THEN** 模型保持缺件且安装与生成不可用，不把 6.88 GB 文件计作完整模型

#### Scenario: 组件来源发生替换

- **WHEN** DiT、文本编码器、VAE、运行时或许可证任一 revision 与登记清单不符
- **THEN** 安装拒绝激活并指出具体组件，不借用另一量化版本或另一模型家族继续

### Requirement: 首期固定纯文生图配方

MiniMax-H3-Image SHALL 只接受非空文本、u32 或空种子和无源图请求，使用 6 步、guidance 1.0、Euler 与 `minimax-h3-image` sigma 预设，输出单张 RGB PNG。产品 SHALL 在采样前用该快照真实 tokenizer 校验 4096 token，并保持现有 16 KiB 原文上限且禁止截断；负提示词、任意 CFG 和任意采样步数 MUST 被拒绝。

#### Scenario: 合法文生图

- **WHEN** 用户提交未超过文本预算的纯文本请求
- **THEN** 有效计划冻结实际种子、尺寸、6 步、guidance、sigma、组件精度、量化和运行时身份

#### Scenario: 请求上游支持但本产品未接入的能力

- **WHEN** 请求包含负提示词、参考图、图像编辑、透明背景或批量输出
- **THEN** 宿主在推理前拒绝并保留输入，不向上游透传或静默移除字段

### Requirement: 初始画幅与高分辨率分别授权

MiniMax-H3-Image 首期 SHALL 支持经对应设备 profile 实测的 1024² 和 2048²；4096² 或其他画幅 MUST NOT 仅因上游支持而可选。每个画幅 SHALL 绑定设备、显存、内存、运行时和实际资源证据，画幅不可用时保留提示词并提示明确选择有效值。

#### Scenario: 12 GB 实验设备选择 2048²

- **WHEN** Windows RTX 3080 Ti 12 GB/32 GB profile 通过 2048² 真机门槛
- **THEN** 该 profile 可选择 2048²，并使用登记的原生卸载策略

#### Scenario: 未验证 4096²

- **WHEN** 当前设备没有 MiniMax-H3-Image 4096² 的通过证据
- **THEN** 界面禁用该画幅并说明未验证，不静默缩成 2048² 后继续执行

### Requirement: 实验支持级别独立于正式型号

MiniMax-H3-Image SHALL 使用 `experimental` 支持级别，并在模型页、关于页和制作记录中与现有稳定型号分开表达。实验型号完成各自安装、离线生成、质量和原生路径验收前 MUST NOT 增加正式支持数、默认模型选项或稳定设备认证。

#### Scenario: 仅有发布资料

- **WHEN** 只有官方或社区资料、没有本机真实生成证据
- **THEN** 型号最多显示为计划中或实验性，不显示为可执行或稳定支持

#### Scenario: 实验型号通过自有验收

- **WHEN** MiniMax-H3-Image 的必需证据全部通过
- **THEN** 产品支持数只增加该已验证配置，现有七个稳定型号的事实和认证保持不变

### Requirement: 许可证、地域和商业边界在执行前确认

安装与生成 SHALL 要求用户明确接受 MiniMax-H3-Image Community License，并展示美国、英国、欧盟和韩国不在适用地域、年收入超过 100 万美元及托管服务需单独授权的边界。应用 SHALL 归档固定许可证与 NOTICE，并在相关模型和商业界面显示 MiniMax-H3-Image 名称。

#### Scenario: 不适用地域

- **WHEN** 用户声明处于许可证排除地域或无法确认适用地域
- **THEN** 产品不提供权重下载、安装或本地执行入口，并保留许可证说明

#### Scenario: 超出普通商业许可范围

- **WHEN** 用户声明拟提供托管服务或达到许可证单列授权门槛
- **THEN** 产品阻止以普通本地许可继续，并提供取得 MiniMax 单独授权的说明

### Requirement: 设备门槛不继承 8 GB 紧凑档

MiniMax-H3-Image MUST 使用独立设备 profile，不得继承其他生成模型的 8 GB compact 档。Windows 12 GB/32 GB 只能在专用卸载路径、完整闭包和资源报告通过后登记为实验性 profile；显存或内存未满足、设备事实未知或未完成真实探针时 SHALL 阻止安装与生成。

#### Scenario: RTX 2060 SUPER 8 GB

- **WHEN** 设备为 Windows 11 RTX 2060 SUPER 8 GB/32 GB 且没有 MiniMax-H3-Image 专用通过证据
- **THEN** 该型号显示不支持或未验证，不回退到 768² compact 配方

#### Scenario: 设备事实不可读

- **WHEN** 应用无法确认显存、内存或计算设备
- **THEN** 安装与生成入口保持禁用并给出可行动检查原因

### Requirement: 启用前完成真机证据门

MiniMax-H3-Image SHALL 在用户主动安装、完整摘要校验、设备探针、禁网生成、取消回收、显式重试、原生 UI 发送和导出全部通过后才能从计划中变为实验性可执行。编译、fixture、上游示例、社区截图、单一探针或 macOS 结果 MUST NOT 替代 Windows 目标机证据，反之亦然。

#### Scenario: 上游示例成功

- **WHEN** 上游仓库或社区模型卡展示了可运行示例，但本机尚未安装和生成
- **THEN** 证据状态仍为 `not_run`，不增加可执行型号或设备认证

#### Scenario: 单平台通过

- **WHEN** macOS profile 通过而 Windows 目标机未运行
- **THEN** 只登记 macOS 对应配置，Windows 保持未验证，不复制认证结论
