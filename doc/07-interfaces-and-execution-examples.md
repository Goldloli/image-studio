# 07 · 接口草案与完整执行示例

研究日期：2026-10-28。本文把前面各卷的建议落实为可讨论的接口形状。所有示例均为说明性草案，不是可安装模型配置、正式 API 或应用实现；示例 ID 和指纹不指向真实对象。返回[总览](README.md)。

## 1. Rust 工程中各类库负责什么

| 能力 | 可研究的 Rust 基础 | 应避免的混用 |
|---|---|---|
| 桌面工作区 | egui / eframe | 不在 UI 帧中进行同步推理 |
| 异步 IO 与 worker 管理 | Tokio | 不把重型 CPU 推理当普通 async IO |
| 数据与协议 | Serde 与适合的序列化格式 | 不让无约束 JSON 替代全部领域类型 |
| 本地事务 | rusqlite 或经过比较的 SQLite 接入 | 不把长推理放进数据库事务 |
| 图片编解码 | image 与专用颜色/格式模块 | 不认为基础解码器自动解决全部 ICC/RAW/HDR |
| 下载与 HTTP | reqwest | 不把进度 UI 与下载状态机绑死 |
| 诊断 | tracing | 不把敏感输入与令牌写入普通日志 |
| 文件选择 | rfd 或平台接口 | 尊重原生对话框线程与平台规则 |
| 系统凭据 | keyring 或平台 adapter | 选择版本与 feature 后验证实际凭据后端 |
| 推理 | ort / Candle / 自定义 worker bridge | 不把库存在当作具体模型兼容证明 |

这些只是职责匹配的候选，未锁版本、未审计依赖或构建实际应用。[Tokio](https://docs.rs/tokio/latest/tokio/)、[Serde](https://docs.rs/serde/latest/serde/)、[rusqlite](https://docs.rs/rusqlite/latest/rusqlite/)、[image](https://docs.rs/image/latest/image/)、[reqwest](https://docs.rs/reqwest/latest/reqwest/)、[tracing](https://docs.rs/tracing/latest/tracing/)、[rfd](https://docs.rs/rfd/latest/rfd/)、[keyring](https://docs.rs/keyring/latest/keyring/)

## 2. 稳定标识与版本的形状

建议使用不同 ID 类型区分项目、资产、运行与组件，避免函数收到一个任意字符串后在运行时才发现传错对象。来源 revision、内容散列与用户对象 ID 分别保存。

以下 Rust 片段仅展示类型边界，未作为项目代码编译：

```rust
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct AssetId(pub String);

#[derive(Clone, Debug, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub struct ModelVariantId(pub String);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AssetInput {
    pub asset_id: AssetId,
    pub role: String,
    pub order: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskKind {
    TextToImage,
    InstructionEdit,
    Inpaint,
    Outpaint,
    Upscale,
    BackgroundRemoval,
    Segmentation,
    ImageUnderstanding,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageSize {
    pub width: u32,
    pub height: u32,
}
```

TaskKind 只是当前任务类别的候选。插件扩展需要稳定的能力命名空间，不能要求每次新增外部任务都修改核心枚举。可以在正式设计时采用内建枚举加扩展能力 ID；这里先展示需要区分的语义。

AssetInput 的 role 也应由能力 schema 校验，例如 source、identity、style、mask 或 control。排序是用户意图的一部分；没有对应能力的角色不能未经说明地丢弃。

## 3. 用户请求与有效执行参数分开

用户请求保留用户原话、选择与允许的策略。执行参数保存解析后的模型专用设置和预处理。

```json
{
  "schema_version": 1,
  "request_id": "request-example-001",
  "project_id": "project-example-001",
  "task": "instruction_edit",
  "instruction": "把背景换成木桌，保留杯子上的图案",
  "inputs": [
    {"asset_id": "asset-example-cup", "role": "source", "order": 0}
  ],
  "model_variant_id": "model-example-edit",
  "requested_output": {"width": 1024, "height": 1024, "count": 2},
  "policy": {
    "execution_location": "local",
    "allow_model_substitution": false,
    "allow_resolution_change": false,
    "allow_cpu_fallback": false
  },
  "parameters": {"seed": 314159},
  "idempotency_key": "user-action-example-001"
}
```

请求里不直接携带任意 worker 命令、Python 代码或可执行路径。模型 ID 由本地目录解析，用户导入模型也先形成受管理条目。输出尺寸是否有效由具体能力检查；示例中的 1024 只是说明，不是所有模型的固定推荐值。

当用户允许自动选择模型或 fallback，解析计划应列出实际选择和调整。不能因为应用能自动决定，就省去记录这些决定。

## 4. 能力 schema 的示例

以下是用于讨论的能力描述形状，不是一个真实模型的认证声明：

```json
{
  "capability_id": "example.image.instruction_edit.v1",
  "input_roles": [
    {"role": "source", "kind": "image", "minimum": 1, "maximum": 1},
    {"role": "reference", "kind": "image", "minimum": 0, "maximum": 3}
  ],
  "instruction_required": true,
  "output_kinds": ["image_rgb"],
  "size_rules": {
    "minimum_side": 256,
    "maximum_pixels": 1048576,
    "multiple_of": 16
  },
  "parameters": {
    "seed": {"type": "integer", "minimum": 0},
    "steps": {"type": "integer", "minimum": 1, "maximum": 100},
    "native_options": {"type": "object", "schema_id": "example.edit.options.v1"}
  },
  "features": {
    "explicit_mask": false,
    "negative_prompt": false,
    "native_alpha": false,
    "progress_steps": true,
    "cooperative_cancel": true
  }
}
```

正式 schema 还需要处理条件依赖、单位、默认值、互斥项、固定参数、UI 提示、缓存键和版本兼容。上面的字段展示了一个关键点：能力通过明确契约表达，不能仅靠自然语言简介或 UI 是否显示按钮判断。

## 5. 模型安装解析结果

解析模型时应返回一个依赖图，而不是单个路径。图中每个节点描述组件角色、来源 revision、散列、格式和安装状态；边描述版本约束与使用关系。

例如一个编辑配方可能需要主网络、文本/视觉编码器、tokenizer、processor 和 decoder。另一种像素空间模型可能不需要 decoder/VAE。共用组件只有在实际内容与兼容约束一致时才复用。

计划显示总下载字节、已经缓存的字节、临时空间需求和缺少的运行时。用户取消安装时保持未激活状态；下一次安装可复用已经完整校验的对象。

## 6. 执行计划示例

```json
{
  "plan_version": 1,
  "plan_id": "plan-example-001",
  "request_id": "request-example-001",
  "recipe_id": "recipe-example-edit",
  "recipe_revision": 3,
  "runtime_package_id": "runtime-example-macos-arm64",
  "runtime_revision": 2,
  "device_profile_id": "device-example-apple-gpu",
  "model_snapshot_id": "snapshot-example-001",
  "effective_output": {"width": 1024, "height": 1024, "count": 2},
  "effective_parameters": {"seed": 314159, "steps": 20},
  "stages": [
    {"id": "prepare", "kind": "resolve_inputs"},
    {"id": "load", "kind": "load_components"},
    {"id": "encode", "kind": "encode_conditioning"},
    {"id": "generate", "kind": "model_inference"},
    {"id": "decode", "kind": "decode_output"},
    {"id": "validate", "kind": "validate_output"},
    {"id": "commit", "kind": "commit_artifacts"}
  ],
  "adjustments": [],
  "compatibility_evidence_id": "evidence-example-only",
  "executable": false
}
```

示例特意标记不可执行，避免文档数据被误用为真实配置。正式计划中的组件、版本、精度、内存策略和验证证据必须来自解析结果；不能用这里的假 ID 执行任务。

Stages 由执行路线提供。上例包含 decode，但它不是所有模型的强制阶段。输出可以是多张图、mask、深度或结构化数据，完成事件应支持多个带类型 artifact。

## 7. 一次任务从点击到提交的完整链路

| 序号 | 参与方 | 行为 | 形成的持久化结果 |
|---|---|---|---|
| 1 | UI | 提交用户请求与幂等键 | 请求草稿或提交记录 |
| 2 | 应用核心 | 校验输入与项目状态 | 已接受请求 |
| 3 | Planner | 解析组件、能力、路线和参数 | 版本化执行计划 |
| 4 | Scheduler | 短事务认领并分配资源 | Attempt 与 fencing token |
| 5 | Supervisor | 启动或复用兼容 worker | Worker 身份与握手信息 |
| 6 | Worker | 验证计划、加载并执行 | 有界进度、预览与临时输出 |
| 7 | 核心 | 接收输出声明并校验 | 待提交 artifact 清单 |
| 8 | 核心 | 复核租约与取消状态 | 被接受的最终状态 |
| 9 | 存储 | 激活对象并提交引用 | 资产、来源关系与 manifest |
| 10 | UI | 展示结果与制作记录 | 用户采用/继续编辑动作 |

核心提交成功后，即使 UI 尚未收到事件，重启也能从数据库看到结果。UI 收到成功事件前数据库尚未提交则容易产生“显示成功但结果丢失”，因此顺序应明确。

如果某一步重复收到请求，按幂等键、attempt 和阶段语义处理。计算本身不必保证分布式意义上的恰好一次，但最终资产提交可以设计为一次被接受的结果。

## 8. Worker 握手与能力探针

worker 启动后先报告协议范围、引擎版本、构建标识、运行时包、可见设备与可探测能力。核心再发最小探针，检查动态库、设备初始化和一个具有判别力的实际计算。

普通版本命令成功不能证明 GPU 算子可用；设备枚举成功也不能证明指定模型可运行。探针分级：运行时可启动、设备可初始化、基础算子可运行、模型配方可加载、真实任务可完成。

探针结果有时间和环境指纹。驱动、OS、运行时或包内容改变后重新评估。探针在后台执行，UI 不因其卡住。

## 9. 事件顺序与重连

消息包含 attempt_id、sequence 和阶段。相同 attempt 内序号单调；时间戳用于观察，不能代替顺序。核心拒绝旧 attempt 的消息推进当前状态。

UI 可以通过事件快速更新，也应能够查询当前状态恢复。事件丢失或重连不会要求用户重新运行任务。预览允许合并或丢弃，最终状态保存在数据库。

远程或 ComfyUI 桥接需要记录外部任务 ID 和本地 attempt 的映射，避免把不同执行器的 ID 当作产品全局身份。

## 10. 取消竞争的一个具体例子

假设 worker 已写完图片，正在发出完成消息，同时用户点击取消。核心先以短事务记录取消请求；完成消息到达时检查当前 attempt 与 token，若取消在最终提交前已生效，就不提交普通成功状态。

临时图片可以按明确策略清理，或保留为“取消时已产生的临时结果”供高级诊断；不应偷偷进入正常成功资产。若成功提交已经先完成，取消返回“已完成”，并保持已提交资产。

这个行为需要在产品规格中明确选择。关键是以一个可测试边界决定结果，而不是由网络或线程恰好谁快决定界面说法。

## 11. 错误类型与恢复动作

| 错误码候选 | 含义 | 恢复方向 |
|---|---|---|
| MODEL_NOT_INSTALLED | 缺少模型快照 | 安装所需组件 |
| COMPONENT_MISMATCH | 文件或组件不匹配 | 选择正确快照或重新下载 |
| RUNTIME_UNAVAILABLE | 运行时缺失或损坏 | 安装/修复运行时 |
| DEVICE_UNSUPPORTED | 当前路线不支持设备 | 显式选择另一条兼容路线 |
| PRECISION_UNSUPPORTED | 指定精度不被支持 | 使用经过验证的精度配方 |
| TASK_UNSUPPORTED | 模型不支持任务 | 保留输入，选择支持此任务的模型 |
| INPUT_INVALID | 输入数量、角色或尺寸不符合 | 修正请求或确认预处理方案 |
| OUT_OF_MEMORY | 资源不足 | 释放 worker 或生成新的低内存计划 |
| OUTPUT_INVALID | 输出文件或语义契约不符 | 保留诊断，重试或更换受验证配方 |
| STORAGE_UNAVAILABLE | 目标卷离线或不可写 | 重新连接或迁移受管理存储 |
| WORKER_LOST | worker 非正常退出 | 新 attempt 重跑 |
| EXECUTION_INTERRUPTED | 系统或应用中断 | 查看记录并重新执行 |
| UPSTREAM_AUTH_REQUIRED | 源站需要用户授权 | 使用原始来源要求的授权流程 |
| CANCELLED_BY_USER | 用户取消 | 保留已完成项与请求记录 |

最终错误结构同时包含稳定代码、用户摘要、开发诊断、可重试性和建议动作。只有确实可以重试的错误才自动进入重试策略，并设置次数与退避上限。

## 12. RunManifest 应保存的事实

最少包含：原始请求、有版本的有效计划、输入资产及其散列、全部组件来源与 revision、运行时与引擎、设备与精度、有效参数、随机性设置、预处理变换、阶段耗时、警告、输出资产和最终状态。

不要把当前模型目录再查一遍的结果当作历史事实。目录可能升级，模型名称也可能更改；manifest 应固化执行时的真实信息。

提示词增强若由 VLM/LLM 完成，保留用户原文、增强后的文本、所用模型与用户采用行为。重跑时可以选择复用最终文本或重新增强，两种行为要分开。

## 13. 从一个模型扩展到下一个模型

第二个模型接入时，按以下顺序检查差异：输入角色、输出类型、组件图、参数语义、预处理、推理阶段、取消点、预览方式、设备约束和质量证据。

只有差异确实属于产品共用语义时，才扩展核心契约。例如透明输出与多参考是产品能力；某层权重的具体名字应留在 adapter。某个模型的特殊 Python 调用也不应变成整个产品公共 API。

每次接入都应形成一个小的“差异与证据”文档，使后续家族逐渐复用稳定模式。最终生态能力来自这些可累积的契约与验证，不靠一个无所不能的加载函数。

## 14. 未来编写正式规格时的切口

可以把首个 change 定义为“macOS 本地图片生成闭环”，明确具体硬件、一个 runtime、一个模型变体、输入输出、安装、取消、资产提交与恢复行为。接下来用第二后端或第二任务验证可扩展性。

本卷给出了可讨论的接口形状，但没有替用户决定最终版本、依赖、首批模型或 UI 框架。确认这些选择后再进入正式实现，更容易让代码与已接受的行为保持一致。
