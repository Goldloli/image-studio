# model-installation Specification

## Purpose

为首个本地图片模型提供用户主动发起、来源固定且完整性可核对的安装流程，分别管理运行时与模型组件；通过可恢复的下载、受管理目录、环境探针和真实就绪状态，避免把文件存在、下载完成或诊断成功误认为已经可以执行生成。

## Requirements

### Requirement: 安装范围与来源明确

应用 SHALL 只为本阶段正式登记的 Z-Image Turbo、Klein distilled 4B、BiRefNet general epoch_244、Real-ESRGAN x4plus、Qwen-Image 2.1、Qwen-Image 2.1-Turbo 和 MOSS-Image-SFT 的固定 revision、版本化配方及锁定运行时提供安装入口；安装清单 SHALL 固定各文件来源、版本、字节数、SHA-256、平台、组件角色及必要许可记录。前端 SHALL 仅提交登记的安装标识，不得传入任意下载 URL、脚本或执行路径。安装前 SHALL 展示该型号和运行时各自的原始下载量、可验证缓存命中和预计临时空间，并以用户主动安装动作开始，普通启动不得自动下载。候选来源元数据未完成组件闭包和内容摘要前 SHALL 不成为正式安装清单。

Qwen与MOSS清单 SHALL 默认使用ModelScope固定文件revision作为传输来源，同时保留Hugging Face原始revision、路径和SHA-256作为内容身份；下载URL不得使用`master`。正式启用国内源前 SHALL 证明每个文件路径、字节数和SHA-256与冻结上游快照一致。安装器 SHALL 只允许登记的ModelScope HTTPS域名与LFS重定向，不得因国内源而放宽任意域名、Range身份或最终摘要校验。

#### Scenario: 查看首次安装计划
- **WHEN** 用户打开尚未安装的 Z-Image Turbo
- **THEN** 模型页说明原始模型组件约 32.85 GB，另列运行时与临时空间，明确 q4 不代表同等缩小的下载量；未点击安装前不开始下载

#### Scenario: 安装标识不受支持
- **WHEN** 调用者传入未登记的型号、revision 或安装标识
- **THEN** 宿主拒绝请求，不解析前端提供的任意远程配置或运行命令

#### Scenario: 查看新型号计划
- **WHEN** 用户打开 Qwen 基础版、Turbo 或 MOSS-SFT 的安装计划
- **THEN** 显示所选精确变体及其完整清单所需下载/缓存/临时空间，不能将 q8 当作原始文件缩小，也不能按参数量推算已测运行峰值

#### Scenario: 三个计划型号的正式安装
- **WHEN** 用户查看前置已接入的Klein、BiRefNet或Real-ESRGAN安装计划
- **THEN** 继续展示它们各自的完整程序/模型/runtime与空间预算，只接受固定来源主动安装，不因增加Qwen/MOSS删除前置安装能力

#### Scenario: Qwen或MOSS默认国内下载
- **WHEN** 用户主动安装Qwen基础版、Turbo或MOSS-SFT
- **THEN** 宿主从清单登记的ModelScope固定文件revision下载，不请求Hugging Face或ModelScope的`master`，并在完成全部Range和SHA-256校验后才激活

#### Scenario: 国内源内容漂移或伪域名
- **WHEN** ModelScope文件的大小、SHA-256、路径或范围响应与冻结清单不符，或重定向到未登记域名
- **THEN** 安装失败并保留已核验缓存和可恢复分段，不激活模型、不回退到未经确认的其他来源

### Requirement: 下载可恢复且逐文件验证

下载 SHALL 在后台进行，临时文件限定于应用管理目录；续传 SHALL 验证远端身份、范围与已保存偏移，完整文件 SHALL 在激活前匹配清单字节数及 SHA-256。网络中断、磁盘不足、服务端不支持续传或文件校验失败 SHALL 有可区分状态和重试动作；失败不得覆盖既有可用快照。并发下载、进度事件与日志 SHALL 有界。

#### Scenario: 中断后继续安装
- **WHEN** 下载中网络中断或应用退出，用户重新打开应用并明确继续
- **THEN** 核对已有部分文件后按有效范围续传，或明确重新下载该文件，不把不完整文件计为已安装

#### Scenario: 服务端返回整文件
- **WHEN** 对部分文件发起续传但服务端返回完整响应或身份已变化
- **THEN** 下载器不把新响应直接追加到旧内容，重建当前文件并重新校验

#### Scenario: 空间不足或校验失败
- **WHEN** 临时空间不足或完成文件散列不符
- **THEN** 安装保持未就绪，说明失败文件和恢复动作，现有模型及用户图片不被删除或改写

### Requirement: 运行时隔离并在探针后激活

运行时 SHALL 位于应用自有版本目录，不修改系统 Python、用户包目录或已有环境。安装 SHALL 只使用已锁定的解释器和依赖文件，不在用户机器临时解析浮动版本或从源码构建依赖。归档解压 SHALL 拒绝绝对路径、越界路径和逃逸链接。只有模型全部组件校验、运行时安装和当前目标设备探针均通过，才能原子登记就绪；安装中或失败版本不得作为用户生成环境。受控安装与激活探针可执行已完成文件校验的私有解释器，不得跳过清单校验。

#### Scenario: 环境准备成功
- **WHEN** 固定 macOS arm64 运行时完成安装，MLX 设备探针及组件完整性检查通过
- **THEN** 对应运行时与模型快照分别记录就绪、来源和验证结果，生成入口可使用这一明确组合

#### Scenario: 运行时探针失败
- **WHEN** 依赖导入、设备算子、版本身份或完整性检查失败
- **THEN** 应用显示运行时不可用原因，不仅依据下载结束就启用生成，也不回退到用户系统 Python

### Requirement: 就绪状态可重新核实且生成离线

模型页 SHALL 区分未安装、下载/准备中、失败、就绪与文件缺失/损坏，并分别呈现运行时和权重状态。本阶段就绪只覆盖被验证的目标配置，其他硬件 SHALL 保持未验证。生成 worker SHALL 使用已核对的本地快照，不在生成时隐式访问 HF、下载补件或执行远程代码；用户仍可在未安装或离线时编辑草稿、查看示例及已有结果。

#### Scenario: 安装后断网生成
- **WHEN** 已安装组合完整且网络被禁止，用户提交有效请求
- **THEN** worker 仅从本地组件执行，仍能生成并提交真实图片

#### Scenario: 文件被移走
- **WHEN** 已登记快照的必要文件缺失或验证发现损坏
- **THEN** 状态变为不可用，生成被阻止并提示修复安装，历史结果仍可查看和导出

### Requirement: 多型号安装状态和运行时引用独立

安装状态、进度、激活记录和恢复动作 SHALL 归属明确install_id、快照及runtime锁；各型号不得借用其他型号的权重齐备事实。共享缓存或runtime SHALL 仅基于完整性核验的相同内容，不能复制路径绑定的venv或覆盖旧Z-Image环境。Python、ONNX和原生程序/依赖 SHALL 分别有固定来源、版本、平台和SHA-256；新worker使用新构建身份，缺件或探针失败不得激活，也不得回退系统Python、brew或其他未登记后端。

#### Scenario: 仅Klein已安装
- **WHEN** Klein模型和MFLUX新runtime已就绪，而BiRefNet与ncnn包未安装
- **THEN** 只有Klein的模型齐备及可执行事实成立，其他型号保持各自未就绪；Z-Image原环境不被覆盖

#### Scenario: 原生包有程序无模型
- **WHEN** ncnn程序归档校验通过但指定x4plus.param/.bin或必要动态依赖缺失
- **THEN** 安装不激活，说明缺件，不以程序可启动代替真实模型完整性

### Requirement: 安装与执行在受管理资源上互斥

本阶段 SHALL 同时只运行一个正式安装流程，停止/继续与事件 SHALL 绑定活跃install_id；针对另一型号的动作不得改变当前安装。安装/修复 SHALL 与模型执行及回收互斥，不覆盖正在使用的文件；冲突 SHALL 返回明确忙碌原因，不创建隐式安装或推理队列。失败或取消只改变对应组合，不删除其他安装或用户资产。

#### Scenario: 错行停止
- **WHEN** Klein安装中收到针对BiRefNet的停止请求
- **THEN** 返回目标无活动安装或拒绝，不停止Klein

#### Scenario: 计算中修复
- **WHEN** ncnn仍计算或回收时用户请求安装/修复另一环境
- **THEN** 动作被明确阻止，原任务不受影响，空闲后须用户明确重试

### Requirement: Qwen与MOSS使用新的独立包身份

Qwen基础版、Turbo及MOSS SHALL 复用前置按install_id安装/互斥/恢复合同，但增加自己的固定manifest和新worker/runtime锁。原Z-Image、Klein、ONNX和ncnn的源/wheel/二进制及激活身份 SHALL 保持，不原地覆盖或重新解释旧计划。共享新runtime只证明该环境存在，不使未装权重的其他新型号就绪。

#### Scenario: 只安装Turbo
- **WHEN** Turbo和Qwen/MOSS新runtime已安装而基础版与MOSS权重缺失
- **THEN** 仅Turbo具备本机可执行条件，原四型号状态独立，其他两新型号仍显示缺权重

#### Scenario: 新包安装失败
- **WHEN** Qwen/MOSS新worker或MOSS探针失败
- **THEN** 对应新组合不激活，前置四型号及用户源图/蒙版/作品不变，不回退其他模型或覆盖Klein环境

### Requirement: Windows 安装与激活绑定精确平台组合

Windows 首轮 SHALL 只为已登记 Z-Image Windows 配方提供安装，计划和 Ready SHALL 绑定 Windows x64、install/profile、worker/runtime、完整模型清单和当前设备探针身份。模型仍使用固定官方 revision，补齐 pipeline、scheduler、tokenizer 等实际依赖闭包并逐文件固定来源、角色、bytes/SHA-256 和许可。下载/缓存/暂存/最终空间 SHALL 按该清单计算，NF4 不得被展示为原始下载量同等缩小。缓存可复用相同摘要内容，Mac 环境或 Ready 记录 MUST 不使 Windows 就绪。

#### Scenario: 已有 Mac 权重或 Ready 文件
- **WHEN** Windows 数据根出现相同模型权重但只有 Mac 运行时或激活记录
- **THEN** 仅可核验并复用相同内容，Windows 仍需完整本平台环境和探针，不能直接启用生成

#### Scenario: 缺少 Diffusers 配置
- **WHEN** 原 Mac 清单权重齐备但 Windows pipeline/scheduler 等必要文件缺失
- **THEN** Windows 安装保持未就绪并准确列出缺件，不在首次生成时联网补齐

#### Scenario: 查看 Windows 安装空间
- **WHEN** 用户在安装前查看计划
- **THEN** 分别显示完整模型、运行时、已核验缓存和暂存空间，不以 NF4、12 GB 显存或 32 GB RAM 估算下载量

### Requirement: Windows 私有运行时可复核并提供可行动失败

Windows runtime SHALL 使用冻结的受控解释器、官方预编译依赖与必要系统运行组件，不修改系统 Python/包，不在用户机浮动解依赖或源码编译。激活 SHALL 校验版本、DLL 导入、实际 CUDA 设备和量化小算子；不兼容驱动、缺失 DLL、无可用 GPU 或探针失败 SHALL 不激活并提供对应恢复信息，不自动安装 GPU 驱动或回退系统环境。安装/探针的取消与异常 SHALL 遵循受管理进程树合同，完整模型认证仍单独记录。

#### Scenario: 驱动或 CUDA DLL 不兼容
- **WHEN** 权重下载结束但 CUDA 导入/设备算子无法执行
- **THEN** 显示运行时具体不可用原因与修复方向，不把下载完成当可执行，不改用 CPU 或其他 Python

#### Scenario: Windows 运行时准备取消
- **WHEN** 用户停止环境准备或探针，随后重新打开应用
- **THEN** 原执行与后代被回收，未完成版本不激活，可明确继续/重试且其他已核验内容和作品不被删除

### Requirement: 新模型的 Windows 安装清单与空间预算

每个新模型 SHALL 提供独立 Windows 安装清单：固定来源（ModelScope/HF/官方 release）、逐文件 bytes 与 SHA-256、许可证归档、运行时展开与下载缓存预算；安装前 SHALL 复核数据与缓存卷空间，不足时在下载前失败且不激活、不删除已有数据。

#### Scenario: 空缓存安装
- **WHEN** 用户在新隔离根点击安装某新模型
- **THEN** 安装器按固定源逐文件校验并激活；空间不足或摘要不符时不激活且保留已有数据

#### Scenario: 缓存复用
- **WHEN** 另一型号已缓存相同摘要组件
- **THEN** 仅复用摘要一致的组件，不复制其他型号的 Ready 记录或版本事实

### Requirement: 运行时暂存目录使用短名并原子激活

Windows 共享运行时安装 SHALL 使用短暂存目录名 `.staging-<pid>-<8位十六进制随机>`（不超过 26 字符）在 `data/runtimes/` 内准备，并在成功后通过同卷原子 rename 激活为冻结的正式运行时目录；正式运行时目录名不得为路径长度而改变。

#### Scenario: 深数据根安装运行时
- **WHEN** 数据根接近支持边界（但仍在范围内）时安装共享 v2 运行时
- **THEN** 运行时安装、激活与 probe 成功，不出现 onnxruntime 长路径 DLL 导入失败

#### Scenario: 激活失败清理
- **WHEN** 暂存运行时准备或 rename 激活失败
- **THEN** 失败路径不产生部分激活的正式运行时，并尽量清理本次暂存目录

### Requirement: 安装前回收历史暂存目录

运行时安装 SHALL 在创建新暂存目录前清理 `data/runtimes/` 中匹配 `.staging-` 前缀的历史残留目录；清理 SHALL 仅作用于普通目录、拒绝符号链接/联接点，且清理失败只记录警告、不阻塞安装。

#### Scenario: 回收失败尝试的残留
- **WHEN** 上一次安装/取消/崩溃留下了 `.staging-*` 目录
- **THEN** 下一次运行时安装开始前该残留被删除并释放磁盘
- **AND** 正式运行时目录与当前安装输入不受影响

#### Scenario: 清理失败不阻塞
- **WHEN** 某个 `.staging-*` 目录无法删除（占用/权限）
- **THEN** 安装继续，日志记录警告，最终结果不因此失败
