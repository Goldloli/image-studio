## Why

当前产品只有Z-Image真正接入；用户要求先完成Klein、BiRefNet、Real-ESRGAN的接入与全部测试，再推进Qwen/MOSS。本方案以该前置change交付的安装、输入/输出和v2协议边界为基础，新增三个明确文生图配方与中文文字/布局验证，不能越过前置验收。

**批准状态：用户已条件确认。** 用户于2026-10-31确认按顺序实施本方案；开始条件仍是`integrate-planned-models`的三型号全部适用P0、原生日常与当前Z-Image回归通过并归档。本 change 当前未开始实现，前置门槛不因批准而降低。原Z-Image历史认证已于2026-10-31归档，不再是唯一前置。来源初查2026-10-30，固定revision于2026-10-31复核，见 [sources.md](sources.md)。

## What Changes

- 增加三个精确变体：Qwen/Qwen-Image-2.1、Qwen/Qwen-Image-2.1-Turbo、OpenMOSS-Team/MOSS-Image-SFT；固定 revision、组件、许可来源和各自配方，不把家族名视为通配支持。
- 首轮固定 1024×1024、每次一张 RGB PNG；Qwen 基础版 40 步/q8/guidance 1.0、Turbo 8 步/q8/guidance 1.0、MOSS-SFT 50 步/q8/guidance 3.0。量化范围按组件实际规则记录。Z-Image 9 步/q4/三个尺寸保持现有语义。
- 复用按型号安装与就绪状态，新增模型manifest和Qwen/MOSS共享的新版worker/runtime；Z-Image、Klein、BiRefNet及ncnn现有安装、历史任务与重试继续绑定各自原版本。安装和推理保持有界串行，不加入队列或热池。
- 在前置generation v2的严格载荷内新增Qwen/MOSS配方分支，冻结型号、配方和有效参数；宿主继续支持历史 Z-Image v1 路线，拒绝跨版本/跨型号身份和隐式回退。
- 模型页、会话设置、默认值、任务、作品和制作记录使用真实型号/配方；不支持的画幅保持可解释的拒绝，切换型号不静默重写已有输入或历史计划。
- 复用前置验收报告与日常流程，每个新配方独立经过本机上游基线、正式安装/禁网生成、取消/恢复、原生 UI、质量和性能记录；最后执行七型号/八能力配置的跨后端切换和前置四型号回归。

初始候选组件元数据约为 33.13 GB、32.45 GB、27.22 GB，三项分别计算；这是下载前的候选集合，不是最终安装大小、独占占用或运行内存。q8 在加载时发生，不相应缩小原始权重下载。完整依赖/临时空间由正式清单和安装预检决定。Qwen 的原始许可名称为 qwen-research，MOSS 模型卡为 Apache-2.0，必须保留精确来源，不能沿用其他 Qwen 型号的许可描述。

批准实施将包含这三个固定来源的模型/私有运行时下载和本机验收。产品不随主程序捆绑这些权重，不运行仓库中的远程自定义代码，不调用在线 API，也不自动修改系统 Python。

本change新增的Qwen/MOSS配方不含编辑、参考图、RGBA、图层蒙版、任意尺寸、用户可调 CFG/steps、LoRA、小文本编码器替换、低内存配方、模型卸载/迁移、Windows 或其他硬件认证。Klein、BiRefNet、Real-ESRGAN由前置change接入认证，本方案不删除或降级其既有能力。新模型首轮只支持方图是本提案已确认的范围选择。

## Capabilities

### New Capabilities

- `multi-model-recipes`：型号与配方身份、专用参数/文本预算、按选择分发及历史兼容、完整执行追溯。

### Modified Capabilities

- `model-installation`：在前置四型号登记上增加Qwen/MOSS固定清单，复用独立状态和版本隔离。
- `local-generation`：从首个配方扩大至受限配方表，步骤进度按实际 total，保留原生命周期和单活动约束。
- `generation-worker-protocol`：在既有v2中增加严格配方载荷，保留v1、源图和双输出合同，不重解释历史。
- `model-catalog`：新增三个独立条目，目录详情/状态/操作按型号呈现，不借用其他型号就绪事实。
- `application-settings`：默认型号、运行环境与来源说明支持多模型组合，创建/切换时验证真实可接受参数。

`core-contracts` 的已有身份、请求/计划/运行记录结构和 `model-validation` 的证据含义继续复用，不仅因增加模型数量就升其 schema。领域合法性校验由固定配方决定。

## Impact

影响 Rust core 的配方校验、宿主安装与调度、generation 协议/监督配置、Python worker、本地 manifest 与目录、模型/会话/设置界面及验收工具。前端仍只传登记 ID 和有界产品参数，所有路径、下载源与执行选择由宿主解析。

复用前置generation v2，不另增协议族/版本；v1/v2载荷仍不能互换；同版本前后端一起交付，宿主保留 v1 以读取和重试历史 Z-Image。采用独立Qwen/MOSS worker构建与runtime锁，不覆盖原Z-Image、Klein、ONNX或ncnn包与历史配方。

按“三个计划型号接入/全部验收及归档 → 核对已交付公共边界 → Qwen两配方 → MOSS-SFT → 七型号/原生日常回归”推进。任一配方缺少证据时不声明整批完成；若需要缩小交付范围，先修改提案和 deltas 重新确认，不把遗漏模型偷偷视为已完成。全部通过后统一归档。
