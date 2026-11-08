## ADDED Requirements

### Requirement: Windows 12 GB 标准档在当前主分支上有真机回归证据

Windows 标准档（VRAM ≥12 GiB）的产品行为 SHALL 在目标 Windows 10 x64 / RTX 3080 Ti 12 GiB 机器上，用当前主分支构建取得完整产品级回归证据：设备探针 SHALL 识别约 12,288 MiB 显存与 sm86；模型页与发送参数 SHALL 选择 `standard` 档（1024²、`bfloat16`）；已安装的每个标准档能力 SHALL 至少完成一次产品内真实生成、编辑、去背景或放大；每次 SHALL 记录 CUDA reserved 峰值、受管生成进程 private commit 峰值、可用物理内存最低值、加载与生成墙钟；输出资产与导出文件 SHALL 逐字节一致；重启应用后已安装型号 SHALL 仍为就绪且可重开已有作品。使用种子缓存重新安装与空缓存真实联机安装 SHALL 分开记录。紧凑档（768²、`float16`）证据、旧构建（2026-11-04 / v0.1.0）证据、CI 结果或其他机器证据 MUST NOT 被记为本轮标准档回归通过；未执行项 SHALL 保持 `not_run`，被安装期内存预检阻止项 SHALL 记为 `blocked` 并保留可行动错误与测量值。

#### Scenario: 档位投影与设备身份

- **WHEN** 在当前主分支构建上打开模型页并开始一次发送
- **THEN** 设备身份显示 RTX 3080 Ti / 约 12,288 MiB / sm86
- **AND** 已安装型号显示 `standard · 1024² · bfloat16`，不出现紧凑档投影
- **AND** Z-Image Turbo 按 12 GB 门槛显示为满足（若已安装则可执行）

#### Scenario: 标准档生成与资源口径

- **WHEN** 用户通过产品 UI 对已安装型号发起一次标准档生成
- **THEN** 记录输出 PNG 的尺寸、字节数与 SHA-256，以及 CUDA reserved、进程 commit、可用内存最低值和墙钟耗时
- **AND** 资源口径逐项对照标准档合同（CUDA reserved ≤10.5 GiB、private commit ≤24 GiB、可用物理内存最低 ≥2 GiB、加载 ≤600 s、生成 ≤900 s、无 WDDM 共享内存回退）

#### Scenario: 种子缓存重装与重启恢复

- **WHEN** 当前构建因激活身份变化拒绝旧就绪记录，随后用已验证的本地种子缓存重新安装
- **THEN** 安装记录标注为种子缓存类型并完整记录阶段与耗时，不宣称验证了下载链路
- **AND** 重装后可完成一次标准档生成，导出文件与资产逐字节一致
- **AND** 重启应用后型号仍为就绪，旧作品可重开

#### Scenario: 安装期量化准备被内存预检阻止

- **WHEN** 本机可用物理内存低于 28 GiB 且 commit 余量低于 26 GiB 时安装需要量化准备的型号
- **THEN** 安装器在开始量化前以包含实测值与可行建议的中文错误失败
- **AND** 不留下部分 prepared 缓存或安装记录，且该事件以 `blocked` 记录，不得改写为通过
