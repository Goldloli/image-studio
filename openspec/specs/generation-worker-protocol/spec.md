# generation-worker-protocol Specification

## Purpose

建立真实图片执行专用的版本化通信契约，将其与现有诊断 v1 的能力和状态隔离；通过身份核对、有界通道、候选输出及受管理进程生命周期，为 Rust 宿主提交可追溯结果和可靠取消提供可以独立验证的执行边界。

## Requirements

### Requirement: 生成握手具有独立身份

生成协议 SHALL 使用独立且必需的协议族标识、版本范围、构建身份和生成能力列表。双方 SHALL 在执行前核对最高共同版本、会话、运行时锁标识、模型快照与能力；首个握手消息每方向序号为零，后续不重置。前置源图/候选角色/双输出及原生后代监督合同SHALL保持，不因本批纯文本分支删除。宿主 SHALL 支持历史 Z-Image 的 v1 路线和前置Klein/ONNX/ncnn及本次Qwen/MOSS的v2路线，各次执行只接受被冻结配方允许的协议版本与载荷；新模型不得降级为 v1 或借用另一型号能力。诊断 v1 与生成协议 SHALL 明确拒绝交叉连接，不自动解释为另一协议或因诊断成功增加可用模型。

#### Scenario: 正确的生成 worker
- **WHEN** 宿主选择的生成 worker 回显正确会话、所选配方允许的版本、运行时及能力
- **THEN** 连接可接受本次执行，后续消息继续使用协商身份和递增序号

#### Scenario: 协议或运行时错误
- **WHEN** 诊断 worker、错误协议族、无交集版本或非预期运行时连接生成通道
- **THEN** 执行被拒绝并清理进程，不能继续生成或把通信成功显示为模型可用

#### Scenario: 旧 worker 连接新配方
- **WHEN** 只支持 Z-Image v1 的 worker 接到 Qwen 或 MOSS 的执行计划
- **THEN** 握手失败并报告版本/身份不匹配，不将新模型请求转成旧 Z-Image 请求

### Requirement: 生成控制与日志均有上限

生成控制通道 SHALL 使用严格 JSONL，普通输出和原生库日志 SHALL 进入独立日志通道。接收方 SHALL 在完整消息分配前执行帧上限，限制队列与日志存储；进度可合并，终态及错误不可因进度堆积永久不可见。图片 SHALL 通过受控候选文件传递，不编码进控制消息。

#### Scenario: 高频进度或普通库输出
- **WHEN** worker 高频产生步骤状态或推理库向普通输出打印信息
- **THEN** UI 不被阻塞，控制通道不将日志解释为结果，缓存保持上限

#### Scenario: 非法或超限帧
- **WHEN** worker 发送无换行超限数据、未知消息类型或错误 JSON
- **THEN** 宿主报告明确协议错误并回收执行，不能接受成功状态

### Requirement: 每次执行按 Attempt 归属且结果只是候选

执行及相关事件 SHALL 带有效的 Job、Attempt、执行令牌、会话、执行请求标识和单调序号；一个执行进程只接受一个Attempt的一次逻辑单图处理；v2的该结果可含配方规定的primary/mask附件。错误或旧身份、重复事件不得推进当前状态。候选输出 SHALL 指向当前attempt中约定的普通文件并包含角色、类型、尺寸、大小和散列；v1保留单result.png，v2最多两候选且角色集合必须匹配配方；收到候选或最后一步进度不得直接登记成功。

#### Scenario: 旧 Attempt 晚到
- **WHEN** 重试后收到旧 Attempt、令牌或会话的进度及候选输出
- **THEN** 当前 Attempt 不变，旧输出不能创建正式资产或成功记录

#### Scenario: 输出候选到达
- **WHEN** worker 报告完成计算并返回候选描述
- **THEN** 宿主仍须等待执行进程结束、验证文件并通过存储提交门禁，才可对外成功

### Requirement: 宿主消失与取消均回收受管理进程

生成进程及其后代 SHALL 由独立于 UI 和 Python 计算线程的原生生命周期监督。正常取消 SHALL 先请求合作退出，再在有限等待后强制回收；宿主意外消失 SHALL 触发监督端终止执行并清理后代。每个数据根的执行租约 SHALL 阻止旧执行尚存时启动新的生成；恢复不得仅凭历史 PID 杀进程。

#### Scenario: 取消发生在无步骤回调阶段
- **WHEN** 用户在模型加载、文本编码或解码阶段取消，worker 未及时合作响应
- **THEN** 原生监督在截止时间后回收受管理进程，宿主只在确认回收后标记取消终态

#### Scenario: 应用被强制结束
- **WHEN** 生成期间宿主被强制终止
- **THEN** 监督端通过父通道关闭清理执行，重启期间旧租约未释放前拒绝新生成，不复用旧暂存目录或盲目向历史 PID 发信号

### Requirement: 源图执行载荷与多输出契约有界

v2载荷 SHALL 严格绑定登记model_id、recipe/版本、snapshot/runtime、能力、宿主解析的只读输入和输出规格，拒绝未知字段、错角色、错hash、非有限参数及任意路径/命令/代码。文生图输入为空，源图能力仅一张；工具不需要虚构prompt/seed。每份候选 SHALL 有唯一允许角色且与冻结像素/字节/类型上限一致；缺mask、额外/重复角色或候选不合法不得提交部分成功。各后端及ncnn后代 SHALL 受同一监督树/执行租约管理，确认全部回收后才终结取消。

#### Scenario: 去背景缺少蒙版候选
- **WHEN** worker只返回RGBA或重复返回primary而没有所需mask
- **THEN** 宿主拒绝成功并回收，不能将已有候选注册成正式单输出结果

#### Scenario: 原生包装器退出但ncnn仍存活
- **WHEN** 取消或宿主退出后包装器先退出，ncnn子进程尚未回收
- **THEN** 执行仍保持清理/租约占用，监督继续回收，不能显示已释放或启动另一推理

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

### Requirement: Windows Z-Image 使用独立的生成 v2 配方身份

Windows Z-Image SHALL 使用 generation v2 中显式登记的新配方分支，握手核对本次 Windows 平台、worker build、runtime 锁、模型快照、recipe/capability 和严格载荷。它 SHALL 保留本次冻结的 seed、采样/精度/量化/卸载身份并只输出一个 primary RGB PNG 候选。未知字段、错版本/平台/组合或旧 Attempt 事件 SHALL 拒绝，不与旧 MFLUX Z-Image v1 或其他 v2 配方互换；既有 v1/v2 路线与帧/日志/候选上限保持。

#### Scenario: Windows worker 收到旧 v1 计划
- **WHEN** Windows worker 连接旧 MFLUX Z-Image v1 执行或载荷
- **THEN** 握手/载荷不匹配并回收，不把旧请求重新解释为 Windows NF4

#### Scenario: 错平台或错运行时回显
- **WHEN** worker 回显 Mac runtime、其他 recipe 或不匹配的 Windows 依赖身份
- **THEN** 在采样前拒绝，记录身份错误，不接受通信成功作为模型就绪

#### Scenario: 最后一步只有候选输出
- **WHEN** Windows worker 发完真实采样回调或 primary 候选
- **THEN** 宿主继续等待计算清理、候选验证和资产事务，不能仅凭回调或 worker 的成功声明显示正式完成

### Requirement: 新 Windows worker 沿用有界协议与执行身份

Klein/Qwen/MOSS 的 diffusers worker、BiRefNet 的 ONNX worker 与 Real-ESRGAN 的 ncnn worker SHALL 使用 generation v2 的握手、帧边界、有界日志、取消令牌、租约与整树回收合同；worker 身份 SHALL 包含型号、配方、运行时与构建 ID，未知/错平台/错版本请求必须拒绝。

#### Scenario: 错平台请求
- **WHEN** 请求携带 macOS 配方或未登记型号身份
- **THEN** worker 拒绝且不启动计算，不产生候选或资产

#### Scenario: 取消与强杀
- **WHEN** 采样或推理期间用户取消，或宿主/监督进程被强杀
- **THEN** worker 与后代被回收，租约释放，重启后标记 Interrupted 且不伪造成功

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
