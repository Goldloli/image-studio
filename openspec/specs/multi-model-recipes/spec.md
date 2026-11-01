# multi-model-recipes Specification

## Purpose
使 Image Studio 能将用户选择的精确图像型号解析为可验证的本地文生图配方，并在请求、执行、组件、参数和历史记录之间保持稳定身份，允许有限扩展新家族而不把不同模型的采样、文本预算或就绪状态混为一体。

## Requirements

### Requirement: 型号和配方具有独立稳定身份

应用 SHALL 为 Z-Image Turbo、Qwen-Image 2.1 基础版、官方 Turbo 和 MOSS-Image-SFT 分别登记稳定型号与版本化配方，明确绑定能力、模型快照、完整组件、运行时锁和协议路线。相近名称、相同文件扩展名或共享后端不得产生隐式替代；前置Z-Image、Klein两个能力、BiRefNet与ncnn型号/能力映射SHALL保持可读且原义不变。

#### Scenario: 选择基础版和 Turbo
- **WHEN** 用户在不同会话选择 Qwen-Image 2.1 和 Qwen-Image 2.1-Turbo
- **THEN** 两个请求分别冻结各自型号、revision、配方及组件身份，Turbo 不会沿用基础版采样表或混用 transformer

#### Scenario: 读取历史 Z-Image
- **WHEN** 当前应用读取接入新模型前保存的 Z-Image 请求与有效计划
- **THEN** 保持原模型、q4/9 步/实际种子/尺寸和运行时身份，不将其改写为新默认模型或新版配方

### Requirement: 每个配方显式限定输入输出和有效参数

本change新增的三个纯文本配方 SHALL 只接受纯文本、1024×1024、单张 RGB PNG、u32 或空种子；Qwen 基础版 SHALL 使用 40 步/q8/guidance 1.0，Qwen Turbo SHALL 使用 8 步/q8/guidance 1.0 和该快照的官方 Turbo 采样曲线，MOSS-SFT SHALL 使用 50 步/q8/guidance 3.0、空负提示词及负分支跳层 [22] 的固定配方。有效参数 SHALL 显式记录实际组件量化/精度与采样身份，不以 q8 标签暗示所有组件同精度。用户不得传任意后端参数或自动启用 LoRA、缓存跳步和低内存替代。

#### Scenario: 旧会话的非方形参数切到新型号
- **WHEN** 用户将原先 4:3 的会话选择为本轮只支持 1:1 的新模型
- **THEN** 界面解释当前画幅不适用并阻止发送，保留原提示词与选择，用户明确改为 1:1 后才可提交，不能静默改尺寸或换回 Z-Image

#### Scenario: 请求未认证的能力
- **WHEN** 调用者对新型号请求参考图、编辑、RGBA、批量或未知采样字段
- **THEN** 宿主和执行边界明确拒绝，不丢弃字段后继续执行，也不因上游支持而宣称产品已接通

### Requirement: 文本预检跟随对应模板和预算

所有文本请求 SHALL 保持 16 KiB UTF-8 产品上限并保留原始输入。固定配方的有效 token 预算 SHALL 分别为 Z-Image及Klein各自模板512、所选 Qwen txt2img 2048、MOSS 4096，计入其自身模板与特殊 token。执行 SHALL 在采样前以本地真实 tokenizer 检查且禁用截断；超限原因和恢复编辑入口 SHALL 可见。MOSS 的标签或 JSON 布局可作为原始文本输入保留，不视为可执行代码或由产品保证几何约束。

#### Scenario: 新模型输入超过旧预算
- **WHEN** Qwen 输入含模板超过 512 token 但未超过 2048 且在字节上限内
- **THEN** 不因 Z-Image 的预算而拒绝，按 Qwen 模板实际验证并记录有效数量

#### Scenario: 模板使输入超限
- **WHEN** MOSS 原始文本在字节上限内但其聊天模板和特殊 token 使总量超过 4096
- **THEN** 明确拒绝且完整保留原输入，不截断到 4096，也不替换为其他型号

### Requirement: 按冻结选择执行且跨模型互斥

宿主 SHALL 按发送时冻结的配方解析实际模型、运行时、worker 和参数，执行前核对完整性；整个应用在生成及回收期间仍只允许一个活动执行，切换型号或会话不得隐式排队或加载另一个模型。执行中修改下一次选择 SHALL 不影响当前任务。未知配方、缺失组件、错误版本和未实现型号 SHALL 有明确拒绝原因。

#### Scenario: 运行中选择另一个模型
- **WHEN** Qwen 正在执行，用户到另一会话选择 MOSS 并编辑提示词
- **THEN** Qwen 原请求继续执行，MOSS 输入可保存但不启动、不排队，前一个执行回收后用户须明确发送

#### Scenario: 缺少所选模型
- **WHEN** MOSS 已安装而所选 Qwen Turbo 组件缺失
- **THEN** Turbo 请求被阻止并提示修复，不能借用 MOSS、基础版或 Z-Image 的安装/就绪状态

### Requirement: 历史重试和制作记录保持精确事实

重试 SHALL 使用原 Job 的型号、模型快照、配方版本、运行时和有效种子，新建 Attempt/令牌并保留旧记录；原组合缺失时 SHALL 阻止并解释恢复条件，不静默升级。任务、作品、会话及导出制作记录 SHALL 显示实际型号、版本化配方和参数，按实际模型筛选，历史图片不因缺少权重而不可查看。

#### Scenario: 运行时升级后重试
- **WHEN** 旧 Z-Image Job 使用原版运行时，新安装了供 Qwen/MOSS 使用的新版 worker 运行时
- **THEN** 旧 Job 仍走原版组合，若原环境缺失则要求按原身份恢复，不用新版 worker 重解释旧请求

#### Scenario: 跨模型作品检索
- **WHEN** 七型号/八能力配置产生结果后，用户按 Qwen Turbo 筛选作品和任务
- **THEN** 仅返回该精确变体的真实结果，其余型号和历史来源保持各自身份，不将所有 MFLUX 结果标成 Z-Image

### Requirement: 各配方独立取得执行认证

每个新配方 SHALL 独立提供精确设备上的上游基线、正式产品链路和原生日常证据，包含安装/禁网生成、质量对照、取消/恢复、资产和性能口径。共享运行时或家族实现不得自动继承另一配方认证；上游功能和本产品已验证能力 SHALL 分开展示。整批交付 SHALL 保留 Z-Image 受影响回归结果。

#### Scenario: Turbo 通过但基础版尚未运行
- **WHEN** Turbo 实机验收通过，而基础版只有源码或探针记录
- **THEN** 只声明 Turbo 对应配置的证据，基础版不能填入已验证设备或通过结果，整批任务不算完成

#### Scenario: 新模型引入回归
- **WHEN** Qwen 与 MOSS 出图成功但旧 Z-Image 重试、尺寸或原生导出失败
- **THEN** 本批交付被阻止，先修复并重跑对应原基线，不以新模型成功抵消旧模型失败

### Requirement: 后续实施不得越过三型号整批认证

本change实施 SHALL 以前置Klein生成/编辑、BiRefNet去背景、Real-ESRGAN放大全部适用必需项及Z-Image当前回归通过并归档为门槛，另须用户明确批准本方案。前置公共安装/v2/输入/输出合同 SHALL 复用且保持，不能创建另一套互相覆盖的状态或运行时；后续Qwen两配方通过后才推进MOSS。

#### Scenario: 只有历史Z-Image认证通过
- **WHEN** Z-Image历史37/37已通过但三个计划型号尚未接通或测试未全通过
- **THEN** Qwen/MOSS保持方案阶段，不因历史认证通过提前实施

#### Scenario: 已完成前置后增加Qwen
- **WHEN** 前置整批已认证/归档且本方案已获批
- **THEN** 在已交付公共边界内新增三配方，保留Klein/ONNX/ncnn原包身份、历史重试及处理能力，不覆盖已有运行时
