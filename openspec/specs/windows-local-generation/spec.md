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
