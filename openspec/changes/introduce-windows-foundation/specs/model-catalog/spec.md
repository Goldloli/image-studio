## ADDED Requirements

### Requirement: 目录动作与证据按当前平台执行组合投影

模型目录 SHALL 在保留既有 Mac 型号/配方/历史身份的同时，投影当前平台实际接入组合。Windows 首轮 SHALL 仅为独立 Z-Image CUDA 组合提供安装/生成；其他六个既有型号保持可查询并说明当前 Windows 版本未接入，不提供无后端动作。接入、已安装、runtime 就绪、当前可执行和实机认证 SHALL 按平台/组合分别表达，不借用 Mac 或另一 GPU 证据。同一逻辑型号的多平台 profile MUST 不增加型号数量；每个平台当前可执行数按实际事实计算。

#### Scenario: Windows 首次打开模型页
- **WHEN** Windows 尚未安装任何模型
- **THEN** 七个既有逻辑型号均可查询，Z-Image 显示本平台安装条件，其余六个解释未接入 Windows，已安装/可执行不硬编码为七

#### Scenario: Mac 七型号已经认证
- **WHEN** Windows 显示同一型号的来源或历史 Mac 认证信息
- **THEN** 认证标签明确归属 Mac，不能使 Windows 显示已认证或可执行；Mac 原能力与动作保持

#### Scenario: 多后端型号统计
- **WHEN** Z-Image 有旧 MFLUX 和新 Windows CUDA 两个内部 profile
- **THEN** 当前平台目录只投影适用组合，逻辑型号仍算一个，不因新增后端把七型号误算成八型号

### Requirement: 默认选择和历史计划不隐式跨平台转换

模型页、功能入口、会话设置和新会话默认值 SHALL 使用同一当前平台能力事实。Windows 新安装的内置默认型号 SHALL 指向 Windows Z-Image 组合，未安装时解释条件；已存在草稿或偏好引用非本平台型号时 SHALL 保留原输入并提示用户明确选择，不能静默改写。历史请求、计划、Attempt 和 manifest MUST 不因打开于其他平台而改 ID 或后端；不适用的重试 SHALL 明确拒绝，用户选择新组合后另建请求。

#### Scenario: 草稿引用 Mac Z-Image
- **WHEN** Windows 读取引用旧 MFLUX 身份的草稿或默认值
- **THEN** 保留文字和旧选择，明确提示本平台不可执行；只有用户主动选择新组合才更新后续发送

#### Scenario: 重试旧 Mac 任务
- **WHEN** Windows 收到绑定旧 MFLUX 冻结计划的重试
- **THEN** 拒绝不适用组合并保留历史，不把原计划改成 CUDA 或在原 Job 下偷偷换配方
