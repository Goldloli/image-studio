# Spec Delta

## ADDED Requirements

### Requirement: MiniMax-H3-Image 只执行冻结的文生图 profile

MiniMax-H3-Image 生成 SHALL 只从已安装、完整校验且当前设备已登记的实验 profile 启动；请求 SHALL 绑定模型快照、量化组合、原生运行时、worker、6 步、guidance 1.0、Euler、`minimax-h3-image` sigma、画幅和实际种子。未知或未通过画幅、负提示词、参考图、编辑能力及后端参数 MUST 在推理前拒绝，不静默替换模型、量化或尺寸。

#### Scenario: 合法 2048² 请求

- **WHEN** 当前 Windows 实验 profile 已通过 2048² 门槛，用户提交有效文本和种子
- **THEN** Job 与计划冻结该 profile 的完整身份、实际种子、6 步和 2048² 输出

#### Scenario: 当前画幅未验证

- **WHEN** 用户选择 4096² 或列表中不存在的画幅
- **THEN** 请求被阻止并保留输入，不缩图、不改步数或改用稳定型号

### Requirement: 原生执行进度与终态来自真实阶段

MiniMax-H3-Image 的加载、文本编码、采样、VAE 解码和结果提交 SHALL 使用原生后端可证明的阶段或步骤事实；没有确定步骤时必须显示活动阶段，不得按时间制造百分比。最后一步、候选 PNG、原生进程退出和正式资产提交 SHALL 分开处理，只有全部清理与成功事务完成才能显示 100%。

#### Scenario: 模型卸载加载

- **WHEN** 原生运行时正在加载 DiT、文本编码器或 VAE，尚未开始采样
- **THEN** 卡片显示对应准备阶段，不显示采样百分比或可导出图片

#### Scenario: 候选已生成但进程未回收

- **WHEN** worker 已报告合法候选 PNG，但原生后代仍在运行或清理未确认
- **THEN** 任务保持活动，不提交成功资产，也不允许启动另一生成

#### Scenario: 取消发生在文本编码阶段

- **WHEN** 用户在文本编码或模型卸载阶段取消
- **THEN** 取消按既有终态门禁处理，确认整棵原生进程树回收后才标记 Cancelled
