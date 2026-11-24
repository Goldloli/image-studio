# Spec Delta

## ADDED Requirements

### Requirement: MiniMax-H3-Image 使用独立原生 worker 身份

MiniMax-H3-Image SHALL 在 generation v2 中登记独立的 model、capability、recipe、snapshot、runtime lock 和 worker build 身份。握手 SHALL 同时核对平台与实验 profile；MFLUX、Diffusers、ONNX、ncnn、Z-Image 或其他 v2 worker MUST NOT 接受 MiniMax-H3-Image 请求，专用 worker 也不得把自身身份复用于其他模型。

#### Scenario: 错误 worker 连接

- **WHEN** 宿主用 MiniMax-H3-Image 计划连接只支持其他模型或错误平台的 worker
- **THEN** 握手失败并回收进程，不把通信成功解释为模型就绪

#### Scenario: 原生 sidecar 身份不符

- **WHEN** 包装 worker 启动的可执行文件、提交或版本与登记运行时不符
- **THEN** 在加载权重前拒绝执行并报告运行时身份错误

### Requirement: MiniMax-H3-Image 请求字段为封闭集合

MiniMax-H3-Image 的 execute 载荷 SHALL 只包含冻结 profile 允许的提示词、u32 种子、6 步、guidance 1.0、Euler、sigma、单张 RGB 输出、组件量化/精度、卸载策略以及宿主解析的模型和输出目录。输入资产 SHALL 为空，输出角色 SHALL 只有 primary；负提示词、参考图、编辑、任意采样、LoRA、脚本、环境变量和未知字段 MUST 被拒绝。

#### Scenario: 上游可识别但产品未开放字段

- **WHEN** 载荷携带参考图、负提示词、任意 CFG、LoRA、缓存跳步或后端命令
- **THEN** worker 在推理前拒绝，不把 JSON 参数转换为任意命令或忽略字段后继续

#### Scenario: 输出超限或角色错误

- **WHEN** 候选 PNG 尺寸、类型、字节数、散列或角色与冻结输出规格不符
- **THEN** 宿主拒绝该候选并回收执行，不登记部分成功或正式资产

### Requirement: MiniMax-H3-Image 回收包装器及其原生后代

MiniMax-H3-Image worker 及其启动的原生可执行文件 SHALL 处于同一受管进程树和执行租约中。取消、协议错误、worker 崩溃、宿主退出和强杀 SHALL 回收全部后代，确认租约释放后才能标记取消或允许下一次生成；不得只等待包装器退出。

#### Scenario: 包装器先退出

- **WHEN** 包装 worker 已退出但原生推理进程仍占用模型或 GPU
- **THEN** 执行保持清理状态并继续回收，不显示资源已释放或启动另一任务

#### Scenario: 强杀后重启

- **WHEN** 生成期间宿主或监督进程被强制结束
- **THEN** 重启时旧租约和进程后代按既有合同恢复处理，旧候选不能推进新 Attempt
