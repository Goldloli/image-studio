# windows-local-generation Specification

## Purpose
为 Windows 10 x64 的 RTX 3080 Ti 12 GB、32 GB 主存设备建立首个可追溯的 Z-Image Turbo 本地文生图闭环，先用同机完整加载和连续执行证明低内存配方可行，再将它用于正式安装、任务和资产流程。

## Requirements

### Requirement: Windows 配方先通过目标设备可行性门槛

Windows Z-Image 组合 SHALL 在产品安装/发送入口激活前完成独立 Gate 0，使用固定来源和同机上游调用完成首次加载与后续三个独立进程的 1024×1024 真实生成。每次 SHALL 无 OOM/崩溃，在 600 秒加载及 900 秒执行时限内完成，CUDA peak reserved ≤10.5 GiB、受管生成进程树 private commit 峰值 ≤24 GiB、系统可用物理内存最低 ≥2 GiB。报告 SHALL 记录加载/编码/采样/解码、系统提交量、WDDM dedicated/shared、测量方法与限制。未通过 SHALL 不激活、不用缩图、换模型、增物理内存、纯 CPU 回退或未验证组合替代目标。

#### Scenario: 首次加载成功但连续运行失败
- **WHEN** 首张可生成而后续独立进程 OOM、超时或超出资源预算
- **THEN** Gate 0 保持未通过并保存失败证据，产品不声明本机 Z-Image 已验证

#### Scenario: 官方小算子或其他 GPU 通过
- **WHEN** 仅 CUDA/NF4 探针通过，或模型只在更大显存设备运行
- **THEN** 不能替代目标机完整模型与资源门槛，缺失项保持未运行

### Requirement: Windows Z-Image 使用独立固定执行组合

Windows 文生图 SHALL 只使用已登记的独立 Windows recipe、runtime、worker 与完整快照，原始模型 revision 固定为 `f332072aa78be7aecdf3ee76d5c247082da564a6`。输入 SHALL 为非空文本、空/u32 seed，无源图，原文至多 16 KiB UTF-8、含模板和特殊 token 至多 512 token；输出固定单张 1024×1024 RGB PNG、9 步参数、guidance 0.0。实际 seed、scheduler、回调 total、组件级精度/量化/卸载与完整环境身份 SHALL 在执行前冻结并留在制作记录；NF4 不得等同于旧 MFLUX q4。未知字段、超限文本或不支持尺寸 SHALL 明确拒绝并保留输入，不截断或静默调整。

#### Scenario: 用户请求非方图或多张
- **WHEN** Windows 请求包含 1024×768、768×1024、多张或额外源图
- **THEN** 返回 Windows 当前配方不支持，不自动缩成方图，不借用 Mac 配方约束或执行身份

#### Scenario: tokenizer 模板导致超限
- **WHEN** 原文未超 16 KiB 但加模板后的实际 token 超过 512
- **THEN** 在采样前拒绝并保留完整输入，不截断、不使用估算字符数代替真实 tokenizer

#### Scenario: CPU 卸载已经登记
- **WHEN** 当前冻结配方包含 CPU 编码或组件卸载
- **THEN** 制作记录准确表达混合 CPU/CUDA 策略，GPU 失败后仍不得自动切成纯 CPU 生成

### Requirement: Windows 故障不改变请求或伪造成功

Windows 组合 SHALL 复用持久化接受、提交幂等、单活动执行、取消终态裁决、候选验证及正式资产事务。显存/主存不足、驱动/DLL、协议、超时和文件故障 SHALL 保留可区分原因与恢复动作，仅明确识别的内存错误标为内存不足。失败或取消 SHALL 不自动重试、换精度/后端/模型/尺寸；用户重试 SHALL 使用原冻结计划、新 Attempt 和新令牌。最终进度或候选文件 MUST 不替代成功事务。

#### Scenario: 真实 CUDA 内存不足
- **WHEN** 生成或解码遇到明确的 CUDA OOM
- **THEN** 当前执行失败并回收，完整输入和计划保留，用户可编辑或明确重试，作品库不新增伪结果

#### Scenario: 取消与最后候选竞争
- **WHEN** 取消先被持久接受而 worker 随后产生合法 PNG
- **THEN** 不提交成功资产，确认清理后为 Cancelled，旧候选不能污染新 Attempt

### Requirement: Windows 生成形成可离线使用的产品闭环

安装完成后，Windows SHALL 使用完整本地组件在网络被禁止时完成中英文文生图；结果在原会话卡片、任务与作品中使用同一正式 AssetRef 和制作记录。重启 SHALL 恢复已提交草稿、任务和作品；导出 PNG SHALL 与正式资产内容一致。模型缺失或设备不可用 SHALL 不阻断已有作品查看/导出。实机认证 SHALL 独立于安装/探针并记录质量、耗时、资源、取消/强杀恢复和原生 UI 证据。

#### Scenario: 安装后整进程树禁网
- **WHEN** 所有组件已核验且宿主、监督、worker 及后代网络被禁止
- **THEN** 新任务仍生成真实 PNG，不下载补件、不调用在线 API，导出摘要与正式资产一致

#### Scenario: 重启后查看结果
- **WHEN** 用户完成生成、正常退出后从安装目录重启
- **THEN** 原会话和作品可重开并导出，未加载模型也能查看已有图片与制作记录

### Requirement: Windows 在 Z-Image 之外补齐 macOS 型号

Windows 产品 SHALL 提供 FLUX.2 Klein 4B distilled、BiRefNet epoch_244、Real-ESRGAN x4plus、Qwen-Image 2.1、Qwen-Image 2.1-Turbo 与 MOSS-Image-SFT 的安装与执行闭环，并为每个型号登记独立的模型变体、配方、安装、运行时与 worker 身份；不得用近似名称、共享后端或改写历史 ID 冒充接入。

#### Scenario: 七个型号同店展示
- **WHEN** 用户在 Windows 模型页浏览目录
- **THEN** 七个已接入型号分别显示接入、安装、运行时、设备与可执行状态，计划中型号数量为 0，不把计划中型号显示为可安装

#### Scenario: 选择新模型生成
- **WHEN** 用户安装并选择 Klein、Qwen 或 MOSS 生成，或选择 BiRefNet/Real-ESRGAN 处理
- **THEN** 请求冻结该型号自己的 revision、配方、组件、运行时与 worker 身份，不继承 Z-Image 或 macOS 的参数与 Ready 状态

### Requirement: 大模型分阶段量化装载与资源门槛

对于在目标机上无法整载的模型，Windows 执行 SHALL 采用分阶段执行：文本编码 → 释放并清空缓存 → DiT 采样 → VAE 分块解码；同一时刻只允许一个大组件驻留 GPU，记录独占显存、进程提交与耗时；SHALL 禁止以 WDDM 共享内存回退或页面文件超卖作为通过。分阶段语义 SHALL 同时适用于标准档（12 GB，1024²，`bfloat16`，CUDA reserved ≤ 10.5 GiB）与紧凑档（8 GB，768²，`float16`，CUDA reserved ≤ 6.0 GiB）：紧凑档 SHALL 保持同一"单大组件驻留"语义：文本编码器、DiT 与 VAE 依次独占显存并在阶段间释放；实测表明 bitsandbytes NF4 检查点无法逐层搬移（quant_state 与 meta 张量限制），因此逐层 offload 不作为 compact 实现。每个阶段在进入下一阶段前 SHALL 完成组件释放与 `empty_cache`，并校验 CUDA reserved 已降到该阶段的释放阈值（文本阶段 ≤ 2.5 GiB）；未通过 SHALL 以可诊断错误失败，MUST NOT 通过放宽阈值或跳过检查来继续执行。

#### Scenario: Qwen Turbo 在 12 GB 上生成

- **WHEN** 设备可用显存不足以同时容纳文本编码器与 DiT
- **THEN** 应用先编码提示词并释放编码器，再加载 DiT 采样，再分块解码；资源报告显示独占显存不触发共享回退

#### Scenario: Qwen Turbo 在 8 GB 上生成

- **WHEN** 设备显存 8 GB
- **THEN** 文本编码器按层上卡并在编码后释放，DiT 以 768² 采样，VAE 分块解码；两档使用同一模型快照与同一份 prepared 缓存

#### Scenario: 文本阶段释放后显存归零

- **WHEN** 文本编码与提示词编码在标准档或紧凑档完成
- **THEN** 删除文本编码器、执行 `empty_cache` 后，CUDA reserved 降到 ≤ 2.5 GiB 才进入 DiT 采样
- **AND** 若释放后仍高于阈值，执行以可诊断错误失败并清理整树，不放宽阈值、不跳过检查

#### Scenario: 阶段超预算

- **WHEN** 任一阶段超过本档的显存或提交预算
- **THEN** 执行以可诊断错误失败并清理整树，不提交候选、不伪造资产、不切换到更高硬件冒充通过

### Requirement: 硬件门槛与许可对用户可见

当某型号在目标设备上无法满足资源预算时，应用 SHALL 在模型页写明所需显存/内存与量化方式并禁用安装/生成入口；同时 SHALL 展示每个型号的许可证，Qwen-Image 2.1 系列必须标明仅非商用研究/评估使用。

#### Scenario: 设备不满足门槛
- **WHEN** 当前设备显存或内存低于模型登记门槛
- **THEN** 模型页显示门槛原因，安装与生成入口不可用，不出现静默失败或自动降级到其他模型

#### Scenario: 非商用许可
- **WHEN** 用户查看 Qwen-Image 2.1 或其 Turbo 型号
- **THEN** 页面显示 Qwen Research License 的非商用限制，制作记录保留该型号与许可身份

### Requirement: 新能力的 Windows 产品闭环

每个新型号 SHALL 在安装目录的原生应用中完成真实执行、阶段进度、取消/强杀恢复、查看、任务与作品查询、PNG（或 RGBA/mask）导出，并保留独立可复核证据；不得以 helper、示例或 macOS 结果替代。

#### Scenario: Klein 单图编辑
- **WHEN** 用户发送单图编辑请求
- **THEN** 使用 Klein 参考图条件编辑，保留源图与取消状态，导出结果与制作记录一致

#### Scenario: BiRefNet 双输出
- **WHEN** 用户对受控源图执行去背景
- **THEN** 原子提交 RGBA primary 与 8-bit mask，作品计数只统计 primary，两者均可查看与导出

### Requirement: 8 GB 显存设备使用紧凑档执行

当设备显存满足紧凑档门槛（≥ 8 GB）但不满足标准档门槛时，Windows 生成 SHALL 使用该模型的 compact 配方：画幅 768²（单模型实测超预算时允许登记 512²）、`compute_dtype="float16"`，并保持与标准档相同的"同一时刻只有一个 NF4 大组件驻留"的分阶段策略（实测表明 bitsandbytes 检查点不支持逐层 offload）。compact 执行 SHALL 满足：CUDA reserved ≤ 6.0 GiB、加载 ≤ 600 s、单次生成 ≤ 900 s，且 MUST NOT 触发 WDDM 共享内存回退。若某模型在 512² 下仍无法满足预算，该模型在 8 GB 设备上 SHALL 标记为不可执行并如实说明，不得用降低步数或虚构通过。

#### Scenario: 8 GB 设备生成 Qwen Turbo

- **WHEN** 设备显存 8 GB，用户以紧凑档发送一次 Qwen-Image 2.1-Turbo 文生图
- **THEN** 文本编码器作为单个组件上卡、编码后释放，DiT 以 768² 采样，VAE 分块解码；资源报告显示 CUDA reserved ≤ 6.0 GiB 且无共享内存回退

#### Scenario: 紧凑档超预算

- **WHEN** 某模型 768² 实测超过 6.0 GiB
- **THEN** 该模型登记 512² 重测；仍超预算时 8 GB 设备上的执行入口禁用并显示实测原因，不静默降级到更小画幅或更少步数

#### Scenario: 8 GB 下不可行的型号不登记紧凑档\n\n- **WHEN** 某型号在 8 GB 设备上实测无法满足预算或输出无效（如 Z-Image Turbo 的 fp16 采样产出 NaN、加载峰值约 7.8 GiB）\n- **THEN** 该型号不登记 compact 档，继续按 12 GB 门槛禁用，并在证据中记录失败的实测数据，不得把"已实现 compact 配方"当作可用\n\n#### Scenario: 计算精度不匹配

- **WHEN** compact 配方声明 `compute_dtype="float16"`，而设备或 worker 只能提供其他精度
- **THEN** 请求在推理前被拒绝并给出可行动错误，不用 bf16 软件模拟冒充通过
