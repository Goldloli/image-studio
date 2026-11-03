## ADDED Requirements

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
