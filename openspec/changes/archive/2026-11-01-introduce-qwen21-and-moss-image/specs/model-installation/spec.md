## MODIFIED Requirements

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

## ADDED Requirements

### Requirement: Qwen与MOSS使用新的独立包身份

Qwen基础版、Turbo及MOSS SHALL 复用前置按install_id安装/互斥/恢复合同，但增加自己的固定manifest和新worker/runtime锁。原Z-Image、Klein、ONNX和ncnn的源/wheel/二进制及激活身份 SHALL 保持，不原地覆盖或重新解释旧计划。共享新runtime只证明该环境存在，不使未装权重的其他新型号就绪。

#### Scenario: 只安装Turbo
- **WHEN** Turbo和Qwen/MOSS新runtime已安装而基础版与MOSS权重缺失
- **THEN** 仅Turbo具备本机可执行条件，原四型号状态独立，其他两新型号仍显示缺权重

#### Scenario: 新包安装失败
- **WHEN** Qwen/MOSS新worker或MOSS探针失败
- **THEN** 对应新组合不激活，前置四型号及用户源图/蒙版/作品不变，不回退其他模型或覆盖Klein环境
