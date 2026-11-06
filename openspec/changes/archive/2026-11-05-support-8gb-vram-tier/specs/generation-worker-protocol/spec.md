## ADDED Requirements

### Requirement: 紧凑档配方与探针精度按档位绑定

生成 v2 SHALL 为五个 Windows 生成模型登记 compact 配方身份，并与 standard 配方并列成为闭集：`qwen21-windows-txt2img-nf4-40-compact-v1`、`qwen21-turbo-windows-txt2img-nf4-8-compact-v1`、`moss-image-sft-windows-txt2img-nf4-50-compact-v1`、`klein4b-text-windows-compact-v1`、`klein4b-edit-windows-compact-v1` 与 `zimage-turbo-windows-nf4-compact-v1`。compact 参数映射 SHALL 在既有键之外固定包含 `resolution`（768 或 512）、`compute_dtype`（`float16`）与 `load_strategy`（`staged-nf4-sequential-v1` 或 `nf4-sequential-offload-v1`）；协议 SHALL 继续拒绝多一个键或少一个键的载荷。worker 的 `--probe` NF4 自检 SHALL 按档位的 `compute_dtype` 构造张量，MUST NOT 固定使用 `bfloat16`。

#### Scenario: 8 GB 设备拿到 compact 配方

- **WHEN** 宿主在 8 GB 设备上为 Qwen Turbo 选择 compact 档
- **THEN** 握手与 execute 携带 compact 配方身份与完整参数映射，worker 校验通过并执行

#### Scenario: 档位与参数串用

- **WHEN** 载荷使用 standard 配方身份却携带 `resolution=768`，或使用 compact 身份却缺少 `compute_dtype`
- **THEN** 在推理前拒绝并回收本次执行，不按字段猜测补齐

#### Scenario: sm75 探针

- **WHEN** 设备 compute capability 为 7.5、compact 档要求 `float16`
- **THEN** 探针以 fp16 构造 NF4 自检并通过；若设备不支持所需精度则探针失败并给出可行动错误

## MODIFIED Requirements

### Requirement: 新版执行载荷严格绑定配方

本change新增三个纯文本分支的v2载荷 SHALL 显式携带登记型号、配方/版本、快照身份、提示词、输出尺寸/数量语义、实际种子、配方允许的有效参数以及宿主解析的本地输入/输出目录，并与握手及冻结计划一致；输入列表为空，候选仅primary。配方白名单 SHALL 同时覆盖 standard 与 compact 两档身份，且两档的参数映射分别冻结、互不替代。双方 SHALL 拒绝未知字段、非法参数、非有限数值、未登记配方和跨型号快照，保留既有帧/日志上限、单Attempt单逻辑结果及前置配方限定多输出、候选输出及取消/回收语义；前置工具能力不因此被要求虚构prompt/seed。不能把有界JSON参数解释为任意代码或命令。

#### Scenario: 载荷与握手型号不一致

- **WHEN** 握手绑定 Qwen Turbo，但 execute 声明基础版配方或另一模型快照
- **THEN** 在模型推理前拒绝，当前请求不会运行或产生正式资产

#### Scenario: 合法但未允许的后端字段

- **WHEN** 载荷携带上游可识别但本配方未开放的 LoRA、参考图、缓存跳步或任意 scheduler
- **THEN** 明确拒绝该字段，不忽略后继续执行，也不直接向后端透传

#### Scenario: 两个档位的映射互不替代

- **WHEN** 同一模型分别以 standard 与 compact 档发送
- **THEN** 两次请求分别匹配各自的冻结参数映射；用其中一档的参数去匹配另一档必须被拒绝

#### Scenario: 版本载荷串用

- **WHEN** 在协商 v1 后发送 v2 execute，或在 v2 后发送旧的无配方 execute
- **THEN** 严格解码失败且回收本次执行，不能根据字段猜测并切换协议
