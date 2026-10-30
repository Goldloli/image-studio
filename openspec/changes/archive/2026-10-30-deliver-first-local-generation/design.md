## Context

本 change 接续已完成的 Tauri 2 + React 工作区与 [首批选型](../../../../doc/08-first-model-batch.md)。现有 core 已有 UserRequest / ExecutionPlan / RunManifest v1、类型化身份和 Attempt 令牌，但没有执行状态机或持久化；worker-protocol 当前严格限定诊断。不能把诊断成功、示例或会话内状态作为真实生成基础。

本轮为待审批文档。上游代码与文件元数据已查询，尚无运行时安装、权重下载或本机图片推理结论。

## Goals / Non-Goals

目标是一个默认项目、一个固定生成型号、单活动 Attempt、每次一张 PNG，完成安装、生成、取消、保存、重启查看、显式重试与导出。目标验收设备是现有 M3 Max / 128 GiB / macOS 环境。

不做其他首批模型、多参考/图片编辑、批量队列、模型常驻池、量化派生缓存、LoRA、节点编辑器、跨平台推理认证、自动断点续算、模型卸载/更新 UI 或公开签名分发。原界面的示例、中文编辑、主题与导航继续使用。

## Decisions

### 1. 代码与所有权边界

- studio-core 增加生成编排、就绪查询、Job/Attempt 生命周期及安装/存储端口；现有三份文档 schema v1 继续使用，不加入 UI 阶段和数据库字段。
- apps/desktop 负责 Tauri DTO、SQLite/文件、下载与平台适配；阻塞 I/O、散列和数据库操作均离开 UI 线程。
- worker-protocol 新增独立 generation 模块；诊断 Message、Envelope 和 HostSession 不变。两套 wire 协议不可自动互换。
- tools/mflux-worker 是受管理 Python 计算 worker，依赖固定 MFLUX；只读已验证的本地模型路径，输出自己的 attempt/result.png。
- tools/inference-supervisor 是独立原生进程入口，监督 Python 和父宿主通道，管理执行租约、转发有界消息并完成强制回收。该进程边界用于宿主被杀后仍能清理 GPU 计算，不依赖 Python 是否正持有 GIL。
- React 保留工作区挂载，使用有限业务 IPC 接入草稿、模型、任务与资产，不直接操作数据库、文件系统或解释器。

新增监督入口是实际生命周期边界，不为每个模型建立独立 Rust crate。模型实现仍由薄 adapter 承接；后续型号不因此自动支持。

### 2. 固定配方与真实参数

模型为 Tongyi-MAI/Z-Image-Turbo@f332072aa78be7aecdf3ee76d5c247082da564a6。MFLUX 固定 0.24.0 / commit 8a7fd0a486635f670ef90e02efe1c051ed3f133c，Python minor 固定 3.13；具体解释器补丁、MLX/torch/transformers 等精确版本及 wheel 哈希在实现准备阶段锁定并交付，不允许运行时解析浮动依赖。

首个配方只接受三种预设：1024×1024、1024×768、768×1024；数量 1、9 步、q4 加载、种子为空或 u32。空种子由宿主解析成具体值并冻结，重复 Attempt 使用原计划的具体种子。Z-Image Turbo 不支持 guidance 的路径会在固定 API 中归一为 0.0；有效精度、scheduler、量化参数和版本按固定实现解析后写入计划并核对，不能沿用 UI 通用演示预设。

请求 UTF-8 提示词上限 16 KiB，并使用同一固定 tokenizer 与 chat template 检查有效 token 预算；超过 512 token 必须报错，不能静默 truncation 或自动摘要。token 预检在 worker 的加载准备阶段、GPU 生成前执行，不依赖字符数估算。16 的倍数约束由宿主先校验，虽上游会舍入也不利用其静默改变尺寸。

调用 generate_image 必须显式传 num_inference_steps=9：固定 API 默认值为 4，而 Turbo CLI 示例为 9。生成过程中编辑草稿不会改变已经持久化的 UserRequest 或 ExecutionPlan。

上游依据见 [实际 API](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/z_image/variants/z_image.py#L55) 和 [Z-Image 说明](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/z_image/README.md#L90)。

### 3. 安装包与存储布局

按现有 AppPaths 数据根扩展，不把模型放进可能被系统清理的缓存目录：

    data/
      library.sqlite3
      host.lock
      execution.lock
      runtimes/<runtime-manifest-digest>/
      models/<model-snapshot-digest>/
      runs/<safe-attempt-storage-key>/
      commits/<safe-asset-storage-key>.tmp
      assets/<safe-asset-storage-key>.png
      quarantine/
    cache/
      downloads/
      thumbnails/

现有 data/diagnostics 保留给诊断。业务 ID 允许冒号，文件名不直接使用外部 ID；由宿主生成安全随机存储键，在数据库映射。

解释器使用固定的 python-build-standalone macOS arm64 install-only 发行文件；采用 3.13 补丁版本并记录其来源、大小、SHA-256 和许可。依赖清单仅允许已锁定 wheel，包含我们自己的 worker wheel；安装使用私有解释器/venv、无交互和 no-index/require-hashes/only-binary 语义，不接收任意包名或现场源码构建。

解释器/venv 安装在最终版本目录并标为 installing，而非创建后随意重命名 venv，以避免绝对 shebang/解释器路径失效。该目录在完整探针通过前不进入活动注册表；失败只清理本次管理目录，不替换既有可用版本。程序文件校验后才允许执行解释器。

模型采用原始固定 revision 快照：vae、transformer、text_encoder 的 safetensors/json 与 tokenizer 文件，另保存许可及必要来源说明。当前选中 17 个组件文件的元数据合计 32,848,304,893 字节；不含运行时和临时空间。见 [规划证据](references/z-image-source-metadata.json)，其中 Git blob ID 不能充当 SHA-256。正式清单需补齐所有小文件的 SHA-256；本证据不是下载 manifest。

q4 在原始组件加载时执行，首轮不导出或缓存量化派生目录。它不会缩小原始下载量，且可能有加载转换峰值；空间与内存口径分开展示。

### 4. 有界下载与真实就绪

Rust 下载器只接受内嵌登记的安装清单，使用固定 revision 的 HTTPS URL 和审核后的重定向域名列表；下载签名 URL 变化不改变字节身份。并发文件最多 2，使用部分文件、远端身份与偏移记录；Range/If-Range 身份不符或返回 200 时重建当前部分文件，完成后计算整文件 SHA-256。

安装前按缺失下载、目标拷贝/解压峰值和预留输出空间计算剩余空间。缓存与数据目录可能跨卷，不能假设可原子 rename；跨卷复制后再验完整性。磁盘不足、网络中断、哈希失败与用户取消分别记录，不把失败目录标 Ready。重复安装意图复用同一次安装，不开第二套下载。

运行时探针包括实际解释器与包版本、MLX 设备小算子、worker 身份以及本地组件完整性。它不生成图片，也不证明整模型已通过性能认证。组件 Ready、运行时 Ready、设备探针及产品兼容证据分别记录；UI 显示当前可执行组合及验证范围，不把目录中的一条型号当成本机已安装。

普通启动只做轻量状态恢复，不下载或加载大模型。执行前后台重新核对必要文件身份与完整性；若缺失或损坏，退回不可用并保留已有结果。生成传本地绝对目录，禁用 HF/Transformers 联网及遥测，不允许 repo-id 触发上游自动下载或 trust_remote_code。

依据：[依赖边界](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/pyproject.toml)、[组件定义](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/z_image/weights/z_image_weight_definition.py#L9)、[本地解析](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/common/resolution/path_resolution.py#L48)。

### 5. 独立生成协议与进程生命周期

协议族固定为 image-studio.generation，首版独立版本 1；这不是修改诊断 v1。握手包含最高共同版本、会话、构建身份、运行时锁与唯一能力 image.text-to-image.z-image-turbo.v1。运行时身份不符、未知消息、交叉协议均 fail closed。

控制消息为 Hello/HelloAccepted、Execute、Progress、Cancel、OutputCandidate、Cancelled、Error。Execute 和其 Cancel/进度/候选共用执行请求标识，携带 JobId、AttemptId、execution_token；每方向独立递增 seq，一个会话只接受一次 Execute。原生监督端是宿主 wire writer，计算子进程事件由其有界转发，避免多个 writer 各自抢序号。Python 内部协议用同一 golden fixtures 验证字段和错误语义。

帧上限 64 KiB，日志内存窗口 128 KiB、每 Attempt 磁盘日志 8 MiB，进度只保留最新快照，关键状态队列有上限并可查询；图片不走 JSON/base64。Python 在导入推理库前保留协议专用 stdout FD，把普通 Python/C 输出导向 stderr。

每个 Attempt 一个监督进程和一个 Python 计算进程，不复用热模型。监督端持有 execution.lock，Python/后代的租约继承与退出路径保证旧执行未释放前不能误判空闲；宿主同时持有整个数据根的 host.lock。

合作取消先写数据库 CancelRequested，再发 Cancel；Python 控制线程置标记，在回调等安全点退出。3 秒仍未退出，原生监督强制结束计算进程组并等待回收；5 秒回收截止仍无法确认时保持清理异常与忙碌状态，不能谎报 Cancelled/资源已释放。宿主正常超时/退出与监督端父管道 EOF 都执行原生清理。计算子进程也监测父通道关闭；宿主在本次仍持有的进程身份下可清理监督异常，不依据重启前 PID 盲目 kill。

模型加载超时初值 10 分钟，生成含解码超时 15 分钟，均独立于诊断 5 秒探针超时；未接受取消时超时进入失败与清理，已接受取消时保留取消意图并继续回收，不以超时改写为 Failed。精确值随首轮实测记录再调整，不能用 UI 心跳虚构步骤。

MFLUX in-loop 回调位于 scheduler.step 后、mx.eval(latents) 前，显示“采样步骤 n/9”，含义为后端到达该点；即使 9/9 也保持活动，等待输出解码、写文件及宿主提交。加载、编码、解码没有相同取消检查点，因此强制回收是必需兜底。

### 6. Job 状态与幂等

新增持久化 Attempt 调度记录，状态为 Submitted / Preparing / Running / Committing / CancelRequested / Succeeded / Failed / Cancelled / Interrupted；这些不是给现有 Job 身份对象增加含混的 UI 状态。每次 Attempt 的终态不可逆，Job 对外状态由当前 Attempt 派生；显式追加新 Attempt 后可以重新进入活动，但旧 Attempt 和 manifest 不变。用户请求与有效计划先写入，再启动执行；token 预检失败可以在准备阶段终结且不启动采样。

(project_id, submission_key) 唯一：同键同内容返回同一 Job，不同内容拒绝。一个事务保证至多一个活动 Attempt；旧 execution.lock 仍占用时拒绝新生成，不排隐式队列。

所有事件先校验会话、请求、序号、Job/Attempt/token，再做数据库条件更新。现有 validate_event 只验证身份，不能替代“当前 Attempt、非终态、未接受取消”的提交门禁。

取消和成功以同一 SQLite 写事务序列化，先提交者决定胜负。取消先提交后不得登记图片，随后 Error/超时/断连只保留诊断并继续回收；已有任何终态时取消返回已结束，成功时具体显示已完成。Cancelled 只在回收完成后写入终态，崩溃恢复仍按未终态 Attempt 标 Interrupted 的规则处理。

显式重试非成功任务沿用 Job 和冻结计划，追加新 Attempt/token；旧记录不可改写。用户修改提示词或对成功结果再次生成创建新 Job。

### 7. SQLite、不可变文件与崩溃窗口

选择 bundled SQLite / rusqlite，连接在专用后台边界串行使用，启用 foreign_keys、WAL 和 synchronous=FULL。schema version 首版使用 user_version；损坏或未知版本显示错误，不迁移为空库。

最小表为 projects、drafts、jobs、attempts、assets、installations；请求放 Job，计划及最终 RunManifest JSON 放 Attempt，另有索引字段供分页查询，不复制出第二套真相。RunManifest 使用现有 ExecutionRecord 与 Succeeded/Failed/Cancelled/Interrupted。

成功顺序固定：

1. 计算进程正常结束，原生监督确认计算/后代已经退出；OutputCandidate 只是候选。
2. 宿主只从对应 runs 目录读取约定 result.png，以不跟随链接且不被特殊文件阻塞的方式取得句柄，并核对它是普通文件；拒绝 FIFO/socket/device、链接与越界，复制到 commits 中新建的宿主临时文件。
3. 对该副本验证最大 32 MiB 和受限 PNG 头部，先确认尺寸与计划完全一致，再在不超过计划像素数、64 MiB 解码分配上限下完整解码（含元数据限制），核对实际字节数和 SHA-256；拒绝修改中的文件、解码超限或候选描述不符。不能先完整解码任意 IHDR 再检查尺寸。
4. fsync 文件，同卷 rename 到唯一 assets 存储键并同步目录。
5. SQLite 写事务再次检查提交门禁，一次登记 AssetRef、RunManifest 和 Succeeded，commit 后更新 UI。

文件 rename 后、DB commit 前崩溃，留下孤儿而非成功；先隔离，不自动补任务。DB commit 后、UI 通知前崩溃，从库恢复成功。失败/取消输出不登记正式资产。暂不做内容寻址去重，避免误删合法引用。

启动先获得 host.lock，再将未终态 Attempt 标 Interrupted 并撤销提交资格。旧租约未释放前允许查看/编辑，但不启动新生成；旧 scratch 不复用或删除。用户显式重试，不恢复采样步号。

### 8. 草稿、界面与资产访问

默认项目初始化一次。草稿自动保存以约 300ms 合并窗口串行写入；每份草稿稳定 ID 与独立编辑历史保持。IME 组合期间只更新编辑器状态，确认后才保存。每份草稿同一时间至多一笔保存，后续写入合并为最新版本，防止异步旧回包覆盖新文字；UI 只有最新版本 commit 后显示已保存。

正常关窗前 flush 待保存内容；失败明确显示并允许重试/复制文字，不能先显示已保存再丢失。强制退出只恢复最近已提交版本。恢复草稿、参数和最近选择，不需要模型加载；本轮主题与次要页面标签继续采用会话状态，不扩成完整偏好系统。

模型页只登记本轮一个型号，分别呈现运行时和原始模型下载/校验/探针状态。composer 根据当前型号显示真实可执行预设；旧草稿若含 16:9、多张等未支持值，保留并解释，要求用户明确更改，不悄悄降级。

任务页展示真实持久化记录、阶段、取消/重试及错误。正式结果进入现有画板和缩略条，可查看请求、有效种子、尺寸、来源与 manifest；示例保持“非本机生成”，应用模板只保存草稿，不创建执行记录。

前端读取图片使用受控 asset URI，仅接受资产 ID/修订并映射库内不可变路径，禁止任意 path、目录枚举和越界读取；解码/缩略图及文件 I/O 不在 UI 线程。新增 CSP 只开放本地资产 scheme，不增加远程脚本或通用文件权限。

### 9. 原生 PNG 导出

前端仅提交 AssetId；Rust 查库并打开原生保存对话框。源文件再校验散列，将字节复制到目标同目录临时文件，同步后原子提交。目标已有文件由用户确认覆盖；无论目标是否存在，都按规范化父目录确认其位于全部应用管理目录之外，拒绝内部 data/cache/runtime/model 路径及链接逃逸，不能通过导出新建内部孤儿文件。

对话框取消、权限/空间失败或源文件损坏仅影响导出，不改变成功 Job 和 manifest。没有模型或网络时也可导出已有结果；不创建第二个资产身份，不记录不必要的外部路径。

### 10. 最少 IPC 与输入边界

增加模型目录/安装计划/安装控制、草稿读取保存、generation_submit/cancel/retry、任务/资产查询和 export_asset 等业务命令；参数只包含注册 ID、有限输入与预期版本。提交幂等键由 UI 一次意图生成，宿主再次校验。前端不能提交 ExecutionPlan、Python参数、程序路径、任意 URL 或数据库语句。

Tauri capability 按命令登记，无 fs/shell/http 通用授权；下载仅宿主进行，生成严格离线。用户提示词仍按文本渲染，不写入可执行模板。保持诊断入口与能力不变，新增检查用生成专用 fixture，不能把假 worker 当实机模型结果。

## Risks / Trade-offs

- 原始模型约 32.85 GB 且量化有转换峰值 → 安装前明确体积与空间，先在目标设备做真实单模型实验，不宣传全系列 Mac 可用。
- Python/MLX/torch 运行时较重 → 隔离版本安装，只允许锁定 wheel，基础应用不依赖系统 Python；首轮不同时实现其他后端。
- SQLite 与文件不是同一事务 → 文件先落、DB 后提交，孤儿隔离并用崩溃注入验证，绝不先写成功。
- 宿主崩溃或 Python 不能及时检查取消 → 原生监督与独立执行租约，重启不凭旧 PID 清理，也不在旧执行仍占用时重启生成。
- 现有长草稿可能超过模型 token 预算 → 草稿完整保留，执行前拒绝并说明，不能默默截断。
- 本阶段每次重新加载模型 → 优先取得可靠生命周期与资源回收证据，热模型和量化缓存另行评估。

## Migration Plan

用户确认后先冻结平台安装锁与完整清单，开发和 fault fixture 验证不需要提前下载模型。随后在隔离数据根明确安装并下载模型/运行时，完成真实验收；首次 SQLite schema 不迁移现有会话内草稿或示例为真实资产。

实现过程保留无模型启动、旧诊断及前端回归。升级/回滚不覆盖已登记资产；本轮不处理未知未来 schema 的降级。所有真实输出、冷加载/生成耗时、内存、取消和恢复结果写入验收记录；材料不足保持未验证，全部任务通过后才归档。
