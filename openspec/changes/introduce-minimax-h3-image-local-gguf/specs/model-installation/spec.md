# Spec Delta

## MODIFIED Requirements

### Requirement: 安装范围与来源明确

应用 SHALL 为本阶段正式登记的 Z-Image Turbo、Klein distilled 4B、BiRefNet general epoch_244、Real-ESRGAN x4plus、Qwen-Image 2.1、Qwen-Image 2.1-Turbo、MOSS-Image-SFT 以及实验性 MiniMax-H3-Image 的固定 revision、版本化配方及锁定运行时提供安装入口；安装清单 SHALL 固定各文件来源、版本、字节数、SHA-256、平台、组件角色及必要许可记录。前端 SHALL 仅提交登记的安装标识，不得传入任意下载 URL、脚本或执行路径。安装前 SHALL 展示该型号和运行时各自的原始下载量、可验证缓存命中和预计临时空间，并以用户主动安装动作开始，普通启动不得自动下载。候选来源元数据未完成组件闭包和内容摘要前 SHALL 不成为正式安装清单。

Qwen与MOSS清单 SHALL 默认使用ModelScope固定文件revision作为传输来源，同时保留Hugging Face原始revision、路径和SHA-256作为内容身份；下载URL不得使用`master`。正式启用国内源前 SHALL 证明每个文件路径、字节数和SHA-256与冻结上游快照一致。安装器 SHALL 只允许登记的ModelScope HTTPS域名与LFS重定向，不得因国内源而放宽任意域名、Range身份或最终摘要校验。MiniMax-H3-Image清单 SHALL 同时固定 DiT、Qwen3-VL-32B 文本编码器、24 通道 VAE、原生运行时可执行文件及许可证；用户未通过许可证与地域确认时 SHALL 不开始下载。

#### Scenario: 查看首次安装计划

- **WHEN** 用户打开尚未安装的 Z-Image Turbo
- **THEN** 模型页说明原始模型组件约 32.85 GB，另列运行时与临时空间，明确 q4 不代表同等缩小的下载量；未点击安装前不开始下载

#### Scenario: 安装标识不受支持

- **WHEN** 调用者传入未登记的型号、revision 或安装标识
- **THEN** 宿主拒绝请求，不解析前端提供的任意远程配置或运行命令

#### Scenario: 查看新型号计划

- **WHEN** 用户打开 Qwen 基础版、Turbo、MOSS-SFT 或 MiniMax-H3-Image 的安装计划
- **THEN** 显示所选精确变体及其完整清单所需下载/缓存/临时空间，不能将 q8、q4 或 6.88 GB DiT 当作完整组件缩小

#### Scenario: 三个计划型号的正式安装

- **WHEN** 用户查看前置已接入的 Klein、BiRefNet 或 Real-ESRGAN 安装计划
- **THEN** 继续展示它们各自的完整程序/模型/runtime 与空间预算，只接受固定来源主动安装，不因增加 Qwen/MOSS 或 MiniMax-H3-Image 删除前置安装能力

#### Scenario: Qwen或MOSS默认国内下载

- **WHEN** 用户主动安装 Qwen 基础版、Turbo 或 MOSS-SFT
- **THEN** 宿主从清单登记的 ModelScope 固定文件 revision 下载，不请求 Hugging Face 或 ModelScope 的 `master`，并在完成全部 Range 和 SHA-256 校验后才激活

#### Scenario: 国内源内容漂移或伪域名

- **WHEN** ModelScope 文件的大小、SHA-256、路径或范围响应与冻结清单不符，或重定向到未登记域名
- **THEN** 安装失败并保留已核验缓存和可恢复分段，不激活模型、不回退到未经确认的其他来源

#### Scenario: 实验型号未确认许可证

- **WHEN** 用户尝试安装 MiniMax-H3-Image 但未接受许可证或无法确认适用地域
- **THEN** 安装保持未开始，已有下载缓存和其他模型不受影响

## ADDED Requirements

### Requirement: MiniMax-H3-Image 使用完整闭包和独立原生运行时

MiniMax-H3-Image 安装 SHALL 只接受该型号专用的原生运行时、worker、DiT、文本编码器、VAE、许可证与 NOTICE 清单，并在激活前校验所有文件。运行时应位于应用自有版本目录，不依赖系统 Python、用户包目录或运行时联网补件；任一组件缺失或来源变化 MUST NOT 激活实验 profile。

#### Scenario: 仅 DiT 校验成功

- **WHEN** DiT 已完成下载和摘要校验，但文本编码器、VAE 或原生运行时缺失
- **THEN** 安装保持未就绪并列出缺件，不以 6.88 GB 文件或可执行探针替代完整安装

#### Scenario: 原生运行时来源变化

- **WHEN** 安装器发现原生二进制、依赖或许可证与其固定清单不符
- **THEN** 停止激活、清理本次暂存并保留其他型号的运行时和 Ready 记录

### Requirement: 实验型号空间预算展示完整权重与暂存

MiniMax-H3-Image 安装前 SHALL 分别展示量化权重总字节、原生运行时、已命中缓存、临时校验空间和最终占用，不得只显示 DiT 大小。空间不足 SHALL 在下载前失败；Q2、Q3 或 Q4 变体不得在用户不知情时互相替代。

#### Scenario: 最小 Q2 组合

- **WHEN** 用户查看 Q2 DiT 与已登记文本编码器、VAE 组成的安装计划
- **THEN** 页面显示完整约 20 GB 级权重预算及额外运行时和暂存空间，不把 6.88 GB 标为总下载量

#### Scenario: 更高精度候选

- **WHEN** 产品登记 Q3 或 Q4 候选用于质量验证
- **THEN** 各候选拥有独立字节、摘要、显示名称和安装身份，质量失败不会自动回退到 Q2
