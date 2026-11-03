## ADDED Requirements

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
