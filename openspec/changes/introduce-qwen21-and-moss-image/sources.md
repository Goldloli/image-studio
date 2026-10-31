# 来源、精确变体与研究边界

初次核对：2026-10-30；固定revision/许可元数据复核：2026-10-31，Asia/Shanghai。这里只查询了官方元数据与现有固定运行时源码，没有下载或运行新权重。

## 1. 型号身份

| 拟登记 ID | 官方仓库 | 本轮核对 revision | 首轮配方 |
|---|---|---|---|
| qwen-image-2.1-mflux-v1 | Qwen/Qwen-Image-2.1 | d26bb61231c349cf6b7896fa83353113880e1ba3 | txt2img / q8 / 40 步 / guidance 1.0 |
| qwen-image-2.1-turbo-mflux-v1 | Qwen/Qwen-Image-2.1-Turbo | fc13ba1986fcb905854dc8de2f52eee395f49b5b | txt2img / q8 / 8 步 / guidance 1.0 |
| moss-image-sft-mflux-v1 | OpenMOSS-Team/MOSS-Image-SFT | f972bd4b4b9e3852133cc5da954d87bc32cd8fb9 | txt2img / q8 / 50 步 / guidance 3.0 / skip layers [22] |

正式安装 manifest 需逐文件大小、内容 SHA-256、来源、组件角色和许可记录。以上 revision 是本次提案的固定目标；发现上游缺陷而需要更换 revision 时，记录新旧差异并补充本方案，不能实施时跟随 main 漂移。

## 2. 元数据证据与体积

[model-source-metadata.json](references/model-source-metadata.json) 保存 HF API 的固定 revision、文件路径、大小、可取得的 LFS SHA-256、普通文件 Git blob ID、候选组件选择与原始许可声明。

| 仓库 | 全仓库文件元数据总量 | 候选推理组件总量 | 候选文件数 |
|---|---:|---:|---:|
| Qwen 基础版 | 33,134,949,561 bytes | 33,131,601,593 bytes | 24 |
| Qwen Turbo | 32,493,327,348 bytes | 32,451,599,267 bytes | 15 |
| MOSS-SFT | 27,229,060,886 bytes | 27,219,772,520 bytes | 18 |

候选按固定 MFLUX 组件布局筛选，包含权重、配置、tokenizer/processor，以及 Qwen Turbo 所需根配置/采样信息。它们**不是可执行 manifest**：普通非 LFS 文件尚未取得内容 SHA-256，组件闭包仍需通过无网络加载审计，来源 README/许可证和必要运行配置还须纳入正式记录。全仓库体积不能直接用作最终下载计划。

q8 代表运行加载选择，不代表下载已是 q8；组件是否可以共享必须按内容散列证明。此处没有把潜在共享扣成确定节省，也没有推导峰值内存。安装时按缺失缓存、完整目标、运行时展开和保留空间分别预算。

## 3. 运行路线

现有固定 MFLUX 0.24.0 来源 revision 为 `8a7fd0a486635f670ef90e02efe1c051ed3f133c`，本地已安装包包含 `qwen21` 和 `moss_image`。本提案优先复用该固定版本，不整体升级后端。

- [Qwen 2.1 文生图说明](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/qwen21/README.md)：基础版与官方 Turbo 的默认采样不同。本方案选原始 txt2img 路线，输出 RGB。
- [Qwen txt2img 组件定义](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/qwen21/weights/qwen21_weight_definition.py)：独立 VAE、transformer、processor 与文本编码器；该路线的文本编码器跳过量化并以 bf16 加载，tokenizer 长度 2048。不得把另一编辑路线的量化范围套过来。
- [Qwen 编辑路线说明](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/qwen21/reference/README.md)：与纯文生图存在视觉塔、归一化、条件编码及 RGBA 语义差异；本次不选该路线，也不将其能力显示为产品已支持。
- [MOSS 说明](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/moss_image/README.md)和[组件定义](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/moss_image/weights/moss_image_weight_definition.py)：使用 MOSS 自己的文本编码器/模板、VAE 和采样规则；文本预算 4096，带聊天模板，不能继承 Z-Image 的 512。默认参考采样为 50 步、CFG 3 和负分支跳层 [22]。

2048/4096 是本提案选定固定实现的含模板预算目标，仍需以真实本地 tokenizer 验证特殊 token 与格式化一致，禁止依靠库默认截断。上游称支持不代表本产品可用；所有新配置均须实机基线与产品链路验证。

## 4. 来源与许可记录

- [Qwen 基础版固定模型卡](https://huggingface.co/Qwen/Qwen-Image-2.1/tree/d26bb61231c349cf6b7896fa83353113880e1ba3)、[Turbo 固定模型卡](https://huggingface.co/Qwen/Qwen-Image-2.1-Turbo/tree/fc13ba1986fcb905854dc8de2f52eee395f49b5b)：API cardData 的 license 为 `other`，license_name 为 `qwen-research`，license_link 为 `LICENSE`；界面和来源记录应展示精确名称及原文入口，不简写为 Apache-2.0。
- [MOSS-SFT 固定模型卡](https://huggingface.co/OpenMOSS-Team/MOSS-Image-SFT/tree/f972bd4b4b9e3852133cc5da954d87bc32cd8fb9)：API 标签 Apache-2.0，仍须分别保留所有下载组件和后端来源。
- [Qwen 官方仓库](https://github.com/QwenLM/Qwen-Image-2.1)、[MOSS 官方仓库](https://github.com/OpenMOSS/MOSS-Image)供模型语义和基线核对；本机产品不直接执行它们示例中的远程自定义 pipeline。

以上是原始声明记录，不是对任意用途的授权推断。主程序不内嵌新权重；运行由用户明确安装固定来源内容。正式 manifest 在接入准备阶段保存原文和组件来源。

## 5. 明确排除和仍待执行的事实

Qwen 旧版、其他 Turbo/LoRA、编辑/控制分支不因名称相近而纳入；MOSS Base 和社区 4/8 步蒸馏 LoRA 不纳入。新模型的下载校验、Mac 加载、峰值资源、单图速度、字形质量、JSON 布局效果、取消和恢复均尚未执行；没有把上游其他芯片的数字转换为这台 M3 Max 的性能承诺。

## 6. 接入顺序修订

2026-10-31用户明确要求先接通并测试三个计划型号，再到Qwen/MOSS。最新固定revision及许可元数据见[本次复核](../archive/2026-11-01-integrate-planned-models/references/hf-and-runtime-metadata.json)。这次没有重新拉取三个完整文件清单，原组件大小仍是2026-10-30候选统计，不声称已重新核验权重内容。公共边界由[前置方案](../archive/2026-11-01-integrate-planned-models/README.md)交付，前置已于2026-11-01认证并归档；本方案虽已获条件确认，但用户要求此后暂停，等待明确恢复。
