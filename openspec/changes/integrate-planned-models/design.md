## Context

用户已确认希望补齐三个计划型号并先测试，再接Qwen/MOSS；本设计已获实施确认，按 Klein → BiRefNet → Real-ESRGAN → 三型号整批验收的顺序执行。现有实现是一个Z-Image安装组合、v1 Python worker、无源图Turn和单候选PNG提交。具体来源和证据缺口见[调研](research.md)。本设计按实际执行边界扩展，不引入通用插件框架。

## Goals / Non-Goals

目标为三个精确型号各自完成正式安装、真实执行、取消/重试/恢复、原生产品操作及结果追溯，整批通过后才推进Qwen/MOSS。所有历史Z-Image资产、请求、安装与v1重试保持原义。

本批不做多源图、任意Python/模型URL、参数工作流编辑器、批量队列、后台常驻模型、Core ML/MPS认证、人脸修复、其他倍率或Windows支持。首轮工具限制明确展示；不以超出输入范围时自动缩图的方式扩大支持。

## Decisions

### 1. 固定型号与能力配方

保留三个现有目录ID，分别映射内部变体、快照、运行时和版本化recipe；不得通过家族名猜测路由。Klein一个权重快照拥有两个能力配方，各自独立认证；安装Ready不意味着两个能力均有实机证据。

| 配方ID（v1） | 请求 | 固定执行 | 输出 |
|---|---|---|---|
| klein4b-text | 提示词，u32或空种子，无源图 | 1024²，q4，4步，guidance1.0，flow_match_euler_discrete | primary：RGB PNG |
| klein4b-edit | 一张不带有效透明度的源图、指令、种子 | 同上；Flux2KleinEdit单参考分支，不使用img2img强度 | primary：1024² RGB PNG |
| birefnet-general-cpu | 一张RGB/RGBA源图，无指令/种子 | CPUExecutionProvider、float32、所选图预处理v1 | primary：原尺寸RGBA；mask：原尺寸8bit灰度PNG |
| realesrgan-x4plus-t128 | 一张不带有效透明度的源图，无指令/种子 | ncnn、4×、tile128、1:1:1线程、单GPU、TTA关闭 | primary：源图各边精确4倍的RGB PNG |

“不带有效透明度”包括RGB或所有alpha=255的RGBA；有透明像素时Klein编辑和放大拒绝，提示当前范围，不暗自丢alpha。BiRefNet接受RGBA，最终alpha=原alpha×预测mask（归一化后四舍五入为8位），mask资产保存最终alpha；原RGB通道保留为straight alpha，不能预乘后错误显示。输入RGB等价于原alpha=1。

Klein原始指令最多16KiB UTF-8，使用该配方实际tokenizer的含模板512预算且不截断；thinking关闭。固定参数只进入有效计划，不伪装成用户填写。BiRefNet/放大不受非空prompt限制，不为满足旧接口填虚构文本；UI以操作名显示请求，原始请求仍保留真实空指令和对应instruction_required=false。

### 2. 边界与依赖方向

| 边界 | 责任 |
|---|---|
| studio-core | 固定能力/输入角色、配方、冻结请求和计划及输出约束；不依赖下载、UI、ONNX或ncnn |
| desktop catalog/installation | 按型号解析manifest、安装状态、完整性、运行时和设备记录 |
| desktop studio/library/generation | 源图导入、会话快照、单活动提交、输入校验、事务/恢复、worker路由 |
| worker-protocol + inference-supervisor | v1/v2严格解码、身份/候选、日志上限、进程树监督与回收 |
| 新MFLUX worker v2 | 仅Klein两分支，加载已核验本地模型 |
| 独立ONNX worker | CPU图检查、预处理、推理、mask与RGBA候选 |
| tools/ncnn-worker | 独立Rust进程包装固定ncnn二进制，协议适配与输入/输出验证 |
| frontend | 单一composer内的四种能力表单，受控源图预览、结果/蒙版、真实状态 |

ncnn包装器是实际部署进程，允许新增一个workspace binary成员；不额外拆分一套调度服务。它以参数数组启动已核验二进制，禁用shell和任意路径参数。ncnn子进程属于同一监督执行树；父包装器被杀后不能留下GPU进程。ONNX不放进旧MFLUX环境，避免新依赖影响历史Z-Image。

### 3. 分型号安装与运行时锁

保留原`resources/local-generation/`、旧wheel及其源码。新增Klein model manifest、`studio-mflux-worker` 0.2.0、`mflux-0.24.0-py3.13.12-macos-arm64-recipes-v2` runtime锁及独立`tools/mflux-worker-v2/`；仍固定MFLUX0.24.0及已有依赖，换worker包必须产生新build/runtime身份，不能复用旧摘要。共享下载缓存只按相同内容散列复用，不复制路径绑定的venv。后续Qwen/MOSS另用0.3.0包及recipes-v3锁，generation通信协议仍是v2。

BiRefNet独立manifest和Python3.13/ONNX Runtime1.31.1 CPU私有runtime，解释器复用已锁PBS来源；依赖以预构建wheel固定版本/平台/字节/SHA-256。ncnn使用官方v0.2.0程序包与主仓库v0.2.5.0包内指定模型对，分别核验归档和提取文件；不把原.pth当目标.bin，也不在用户机器现场编译或用brew修补环境。原生runtime另记录包装器、程序、依赖、架构、GPU/MoltenVK和许可证清单。

激活记录位于`data/installations/models/<install-id>.json`及`data/installations/runtimes/<runtime-id>.json`，包含manifest/runtime摘要与探针身份；旧`installation-ready.json`仅读作旧Z-Image组合。状态按install_id投影，运行时共享不能替代权重齐全。列出模型不散列大文件或做探针；执行前后台完整核验。

单数据根保持一个安装流程；安装/修复与模型执行/清理互斥，无隐式队列。停止/继续和事件明确携带install_id，切页面不改变动作归属。清单预算区分下载缓存、目标完整文件、解释器/wheel展开、归档解压与临时保留，按实际卷检查；不假定硬链接、共享块或量化可节约下载。

代理设置沿用项目已验证的安装路径，支持HTTP(S)_PROXY。2026-10-31用户另外授权在官方直连或代理不可用时，使用已登记的国内HF镜像作为同一固定快照的传输备用端点；正式manifest中的官方来源、revision、字节数和SHA-256仍是内容身份，镜像返回的每一个文件都必须与这些值一致，任何差异立即拒绝。下载、解压和探针完成前不激活；新增zip提取器需拒绝越界、绝对路径和逃逸链接。镜像不能替代内容核验、许可审计或正式安装成功。

### 4. 受控源图、规范化与保存

输入从已有正式作品选择，或由原生文件选择器导入本地PNG/JPEG。前端只持有AssetId；文件导入采用原生选择器授权的本次选择，处理提交不接收任意路径或URL。最小范围为单帧8位PNG RGB/RGBA、JPEG RGB；拒绝动画、CMYK、16位和未知色彩配置。无配置输入按sRGB解释，有明确sRGB标识可接受；其他ICC先解释拒绝，不能静默改变色彩。

宿主先检查编码字节≤32MiB、解码尺寸每边≤4096且总像素≤4,194,304，再用受限解码器完整解码、应用JPEG EXIF朝向，写入应用自有规范化PNG并算SHA-256。实施时为解码依赖固定版本和来源；不能仅按扩展名判断。保存原文件摘要、原尺寸/朝向与规范化记录，执行只引用规范化资产；外部文件改动或删除不影响已导入输入。超限时不先全量解码再报错。

导入源图登记到独立`imported_sources`，不伪造成功模型Job，不增加作品生成数量。规范化文件写入`inputs/`，孤儿清理必须认识该表的引用。首轮不做自动GC或源图删除管理；保留占用说明。已有作品作为输入时不复制身份，执行前复核原资产hash，并在Attempt私有目录生成只读输入快照，worker不持有可写源资产句柄。

Klein编辑最多4MP输入，参考编码按固定上游保比例缩放至≤1MP，并居中裁去非16倍数余量，输入任意边<16直接拒绝。UI在发送前展示处理说明和参考预览；计划冻结原源图身份、朝向归一化及参考编码尺寸/裁边算法版本，重试复用同一规则。输出固定1024²，由用户明确选择该范围，不把源图缩放当输出尺寸变更。

BiRefNet最多4MP输入，输出恢复源图规范化后的尺寸；1024²仅为网络预处理尺寸。放大要求输入每边≤1024且总像素≤1MP，输出每边×4、总像素≤16,777,216；超限拒绝，不能自动减小原图、倍率或改模型。去背景结果有透明像素时不能直接提交首轮放大，产品提前解释限制。

### 5. ONNX图及原生ncnn适配

BiRefNet批准后先下载核验epoch_244，检查图opset、输入/输出名、dtype、shape、外置权重和算子。预处理基于固定官方参考：RGB、1024²双线性采样、float32 NCHW、[0,1]和ImageNet归一化；固定插值/antialias语义并和官方参考逐样对照，不能认为不同图像库同名resize就逐像素相同。

只有确认所选输出是logits才应用sigmoid；若图已带sigmoid不得二次应用。输出处理将概率图双线性恢复到源尺寸、clip至[0,1]并按固定舍入生成8位alpha，不默认增加二值阈值、腐蚀/膨胀、羽化或前景颜色去污染。若图的真实合同需要调整，先记录同权重检查结果并更新预处理版本，不能沿用另一个checkpoint的假设。CPU session显式只配置CPUExecutionProvider，线程数固定并写入计划，首轮拟intra_op=4/inter_op=1、sequential执行；这是待测配方，不是速度保证。

ncnn以固定参数执行，`-m`指向只含指定模型对的已核验目录，`-g`选安装探针记录的单GPU，不依赖默认模型或默认GPU。固定tile128必须在基线中证明分块确实发生并检查接缝；必要的模型边缘padding由固定上游实现处理。小图全图/较大tile对照可作参考，但不能把参考配方当正式t128认证。退出码0仍需PNG验证，stderr中的百分比只在语义有可靠解析/测试时转换为tile进度，否则显示真实活动阶段。

### 6. 协议v2与生命周期

保留LegacyZImageV1的严格消息与专用运行时。新三种后端共享generation v2有界外壳，载荷按能力/recipe内置分支解码，包含model_id、recipe_id/version、snapshot/runtime身份、源图描述（只读Attempt路径、角色、hash、尺寸）、有限参数和冻结输出规格。没有源图的Klein文生图输入为空；不把Python模块名、shell、下载URL或任意JSON交给前端控制。

v2候选为有界输出数组，每项有primary/mask角色、相对文件名、像素类型、尺寸、字节数和SHA-256；最多两项，只接受配方规定角色集合。旧v1仍只接受result.png，不因为新路径允许mask而放宽旧载荷。新worker从自身构建、锁和内置分支声明能力，不能仅回显expected身份。

继续使用64KiB帧、独立有界日志、单调序号、会话/请求/Job/Attempt/token门禁；一个执行进程只接受一个Attempt。握手最高30秒，加载/编码600秒，计算1800秒，写出/退出120秒，合作取消5秒后强制回收观察；这些是版本化故障边界，不是实测延迟承诺。实际阶段不可观测时不伪造细分；Klein真实total=4，ONNX通常无步数，ncnn可没有百分比。

所有后端受同一数据根执行租约约束。取消接受先于提交则两份候选均不能登记；资源回收前保持cleanup。原生包装器及ncnn后代都纳入监督，宿主消失触发整树回收；不能仅杀Python或Rust包装进程后就宣称GPU释放。回归强杀、断连、超时、旧token和取消/成功竞态。

### 7. SQLite迁移、输出与原子提交

现有v4的assets.attempt_id UNIQUE不支持两输出。批准后做事务性v4→v5迁移：重建assets并增加受限output_role，唯一约束改为(attempt_id,output_role)，旧行全部为primary；新增imported_sources及必要来源元数据；Turn新增capability_id和source_asset_id（旧行默认文生图/空源图）。迁移须重建原索引/外键，保留所有ID/路径/原请求与plan JSON，验证foreign_key_check、数量及哈希。未知/损坏库不重建，迁移失败整体回滚；不承诺旧二进制能打开v5。

草稿保存能力和源图引用；旧草稿缺字段按文生图读取。发送意图的幂等比较包含能力、源图身份/hash和参数，不能只比较prompt；新的mask不是第二张“生成张数”。领域ImageOutput.count仍表示一个创作结果，配方的输出角色集合表达其附件；RunManifest.outputs本已有数组，primary/mask按AssetKind唯一判定并保留角色映射。

宿主验证每份候选后先写入/同步唯一文件，再在一个写事务里登记全部输出、RunManifest和成功终态。mask失败、取消先提交或任一文件/事务故障，都不产生部分成功；已经移动但未登记的文件按现有孤儿规则隔离。副产mask与主图不分别创建Job；作品主列表只统计primary，mask在详情/导出中可取，任务按Job计数。

输出白名单由冻结配方决定：旧Z-Image保持≤1MP/32MiB/RGB或RGBA；Klein只允许1MP RGB；BiRefNet允许≤4MP RGBA及同尺寸灰度Mask、每文件≤32MiB；放大允许≤16MP/每文件≤128MiB RGB。总解码缓冲预算有硬上限（拟256MiB，逐文件解码并复用缓冲），先核验IHDR与计划，拒绝尺寸乘法溢出、伪头部/类型、重复角色、zip/png解码炸弹和路径逃逸。不把16MP上限传播到旧计划或任意未知配方。

结果在棋盘格预览RGBA，mask独立预览。导出主图/蒙版使用系统保存对话框，逐文件字节与内部资产一致；取消导出不改内部库。制作记录展示源图、能力、真实模型/配方/运行时与输出角色，导入记录不能冒充模型生成来源。

### 8. 产品流程

功能页文生图可选Z-Image或Klein；编辑/去背景/放大定位统一composer的对应能力和固定型号。模型页“用于创作”遇到Klein提供生成/编辑选择，遇到工具型号进入对应操作；当前会话有未发送内容时保留草稿，切能力不静默清空文本或源图。

工具表单显示源图、约束及目标尺寸，不显示无效种子/画幅/提示词控件；切回文生图恢复该能力草稿。源图选择、移除与预览可用键盘完成；IME未确认不提交。点击发送即冻结目标Conversation及所有输入，后续切会话/换图只影响下一请求。源图不足或未就绪时不产生空Turn/Job。

三种处理结果沿用当前会话卡、任务/作品及制作记录；重试沿原源图/hash/配方，新编辑创建新Job。没有第二套任务中心。设置里的“新会话默认生成模型”仅允许已接入文生图能力，不能把BiRefNet设为文生图默认；环境信息按各runtime显示。未知历史型号显示原ID及原因。

### 9. 阶段与后续衔接

| 阶段 | 出口 |
|---|---|
| G0 文档/授权 | 用户明确确认；Z-Image历史认证可定位 |
| G1 公共边界 | 输入/多输出/迁移/v2/安装fixture与旧v1回归通过，来源缺口逐项有处理 |
| G2 Klein | 两个能力各有基线、正式安装/执行/取消恢复和原生日常证据 |
| G3 BiRefNet | 真实epoch_244图合同核对，透明/蒙版质量与双输出事务通过 |
| G4 Real-ESRGAN | 官方包清单/架构/依赖明确，原生GPU4×、分块与整树回收通过 |
| G5 整批 | 全部必需用例、原生交替操作及Z-Image当前构建回归通过；报告完整 |
| G6 后续 | 本change归档；Qwen/MOSS自身方案已获批后先Qwen两版、再MOSS |

Qwen/MOSS复用已交付按型号安装和v2严格外壳，新能力分支的增加仍产生新worker/runtime锁，已认证Klein环境不原地覆盖。后续不再重新创建本批公共边界，不假定“共享MFLUX”意味着所有权重/快照/许可可共用。

## Alternatives Considered

只把三个planned改成implemented无法产生执行能力；继续硬编码第二个单模型安装会使状态互相覆盖。选固定recipe登记和按ID安装满足实际四型号，无需任意插件系统。

把BiRefNet和ncnn塞进旧Z-Image venv会改变旧运行时身份且扩大故障面，采用各自私有环境/原生包。直接依赖ncnn stdout日志驱动状态不可靠，由包装器提供受限协议，百分比没有证据时保持不确定活动。

去背景只存RGBA再临时生成mask可以规避数据库迁移，但不满足已登记的蒙版产物与追溯要求；选择两资产同事务提交。首轮放大透明图需要另行固定alpha处理策略，因此先明确拒绝有效透明输入，不隐式丢透明度。

## Risks / Trade-offs

权重、私有runtime及不可变源图增加磁盘占用；用正式清单预算，不引入提前GC。ONNX实际图和ncnn包内容未取得是确定的验证任务，失败必须如实阻塞，不能以元数据完成代替运行。

不同后端/量化可能有像素差异，测试分别对固定同配方基线核对，人工质量报告描述具体问题；不保证跨后端/机器位相同。去背景发丝、透明物体、阴影和复杂背景是质量用例，输出PNG合法不等于抠图效果好。

16MP输出和两资产提交扩大存储验证面，必须先做解码上限与事务故障测试。SQLite迁移与原生子进程回收是首批关键风险，必须在真实模型认证前通过对应fixture与真实生命周期用例。

## Migration Plan

新增模型/运行时只写新版本目录并逐项激活，旧Z-Image环境保留。数据库迁移先在旧库fixture验证事务性、外键和历史回读；原生默认目录不清空/搬走用户内容。无活动执行时才允许代码回退；旧二进制遇到v5应明确拒绝而非改库，升级前备份策略和实际回退限制写入交付说明。

所有阶段报告记录构建、快照、runtime、recipe、输入、设备和证据类别。三型号全部通过才更新实机证据/关闭实施Beads/归档change，归档后主规格才表达已交付；任一缺失保留not_run/blocked，后续Qwen/MOSS只可做方案工作。
