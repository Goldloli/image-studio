## MODIFIED Requirements

### Requirement: 安装范围与来源明确

应用 SHALL 只为本阶段登记的 Z-Image Turbo、FLUX.2 Klein 4B distilled、BiRefNet general epoch_244 ONNX和Real-ESRGAN x4plus ncnn固定快照与各自锁定运行时提供安装入口；安装清单 SHALL 固定各文件来源、版本、字节数、SHA-256、平台、组件角色及必要许可记录。前端 SHALL 仅提交登记的安装标识，不得传入任意下载 URL、脚本或执行路径。安装前 SHALL 展示模型和运行时各自的下载量及预计临时空间，并以用户主动安装动作开始，普通启动不得自动下载。

#### Scenario: 查看首次安装计划
- **WHEN** 用户打开尚未安装的 Z-Image Turbo
- **THEN** 模型页说明原始模型组件约 32.85 GB，另列运行时与临时空间，明确 q4 不代表同等缩小的下载量；未点击安装前不开始下载

#### Scenario: 安装标识不受支持
- **WHEN** 调用者传入未登记的型号、revision 或安装标识
- **THEN** 宿主拒绝请求，不解析前端提供的任意远程配置或运行命令

#### Scenario: 三个计划型号的正式安装
- **WHEN** 用户在本批接入完成后查看Klein、BiRefNet或Real-ESRGAN的安装计划
- **THEN** 每项展示完整模型/程序/运行时与临时空间预算，明确官方来源和配方，只有主动安装后才开始；元数据候选清单或旧.pth不得冒充完整ncnn安装清单

## ADDED Requirements

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
