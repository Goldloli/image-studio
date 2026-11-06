## MODIFIED Requirements

### Requirement: 模型状态更新不伪造进度

模型页 SHALL 区分下载、校验、安装、环境准备、设备探针、失败和完成阶段；有确定字节或步骤时显示实际进度，未知阶段显示活动提示而不按时间生成百分比。安装总览与行内状态 SHALL 读取同一事实，不能各自推算不同进度。自动刷新 SHALL 有界，离开页面后迟到响应不得覆盖新筛选结果或另一安装状态。安装进行中（downloading / verifying / preparing / installing_runtime / probing）时，模型行 MUST 显示当前真实阶段与已下载字节，不得回退为"尚未安装"；`model_install` 返回后页面 SHALL 立即反映新阶段。

#### Scenario: 未知准备阶段

- **WHEN** 安装已经下载完成但正在校验或准备运行时
- **THEN** 页面显示对应活动阶段，不把下载 100% 当成安装完成或当前可执行

#### Scenario: 刷新响应迟到

- **WHEN** 用户切换筛选后收到上一查询的模型行响应
- **THEN** 旧响应不替换新筛选结果、不抢焦点，也不改变安装事实

#### Scenario: 安装进行中阶段可见

- **WHEN** 用户点击安装并进入 downloading、verifying、preparing、installing_runtime 或 probing
- **THEN** 模型行显示对应阶段（含已下载字节/总量），不显示"尚未安装"，也不需要用户手动刷新
