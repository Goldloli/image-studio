# Z-Image Turbo 首个本地生成闭环验收

日期：2026-10-30，Asia/Shanghai。对应已批准 change `deliver-first-local-generation`，主验收任务为 Beads `image-studio-b88.5`。

真实安装、三个尺寸出图、界面显示、PNG 导出、加载/采样取消和宿主强杀恢复已取得实机证据；强杀后的同 Job 显式重试也已成功。本记录将模型执行、设备探针及故障 fixture 分别列出。最终质量门禁全部通过，自动化结果与实机记录分别保留。

## 环境与固定组合

| 项目 | 验收值 |
|---|---|
| 机器 | Mac15,8 / Apple M3 Max / arm64 |
| 统一内存 | 137,438,953,472 bytes，128 GiB |
| 系统 | macOS 27.0.1，build 26A434 |
| 桌面栈 | Rust core + Tauri 2 + React；Rust 1.99.1 |
| 模型 | `Tongyi-MAI/Z-Image-Turbo@f332072aa78be7aecdf3ee76d5c247082da564a6` |
| 运行时 | Python 3.13.12 / MFLUX 0.24.0 / MLX 0.32.4 / torch 2.15.0 / transformers 5.22.0 |
| 固定配方 | 每次一张 PNG；9 步；加载时 q4；`scheduler=linear`；有效 guidance 0.0 |
| 精度记录 | `native;compute_precision=None;float32=False`，没有把所有组件宣称为统一 bfloat16 |
| 执行方式 | 每 Attempt 新建原生监督与 Python 计算进程，不使用热模型池 |

机器基础记录见[基础环境](foundation-results.md)，当前设备与依赖探针见[运行时探针](first-generation-runtime-probe.json)。本轮只覆盖上述设备组合；Windows 仅做核心/协议编译检查，没有 Windows GUI 或推理认证。

## 下载、安装与就绪

原始模型的 17 个推理组件合计 **32,848,304,893 bytes，约 32.85 GB / 30.59 GiB**；正式模型清单另含 README 来源记录。q4 发生在加载阶段，没有将下载量按四分之一估算。固定运行时包含 PBS 解释器及 59 个 wheel，其大小、SHA-256、许可元数据和 worker 源码/可重复 wheel 已单独核对。

下载在独立验收根的缓存中完成，最终逐文件整 SHA-256 通过。下载 helper 最终使用两文件并发、每文件最多八个有身份校验的 Range；该 helper 的并行数不是正式安装器网络并发的性能证明。正式 `Installation::start` 随后复用了这些已校验缓存，执行目标模型复制、隔离运行时安装、探针、27,043 条运行时文件/链接 inventory，并原子登记 Ready。因此本轮属于**准备好下载缓存后的真实安装**，不宣称无缓存的一键全程网络安装。

模型与运行时激活记录、冻结计划均绑定完整清单摘要，不只比较版本标签。安装前展示保守空间预算，原始模型、下载缓存、安装展开与运行时分别计入空间。另在全新临时数据根复制已校验缓存、禁止网络，重新执行正式安装器并采样 `du -sk(data,cache)`：76.536 秒内成功进入 Ready，观测峰值和结束值均为 **67,717,607,424 bytes，约 67.72 GB / 63.07 GiB**。每轮间隔 0.5 秒另加目录遍历时间；该数值是目录分配量的采样峰值，APFS clone 可能共享块，不是独占物理块或瞬时系统全盘峰值。临时根随后删除，原有验收库保持。

证据：

- [模型正式清单](../resources/local-generation/model-manifest.json)、[运行时正式清单](../resources/local-generation/runtime-manifest.json)、[依赖锁](../resources/local-generation/requirements.lock)。
- [完整下载 receipt](../target/first-generation-validation/model-download-receipt.json)：`complete`、`full_model_verified=true`；此记录只证明下载与校验。
- [正式模型安装](first-generation-model-install.json)及[安装日志](first-generation-model-install.log)：`modelReady/runtimeReady/deviceVerified=true`。
- [正式安装磁盘采样](first-generation-install-space.json)及[日志](first-generation-install-space.log)：复用已校验缓存，完整正式安装成功，记录逐次数据/缓存目录占用。
- [固定运行时审计](first-generation-runtime-audit.json)：60 个运行时文件大小/散列、worker wheel 与源码一致、私有环境 `pip check` 通过。
- [禁止网络下的正式运行时安装](first-generation-runtime-install.json)：`network_denied=true`；没有模型齐备时不激活安装。
- [真实本地 tokenizer 预检](first-generation-tokenizer-probe.json)：短中文含模板 18 token；5008 token 输入明确拒绝，没有静默截断到 512。

## 正式生成结果

以下图片来自真实 MFLUX 模型执行。SQLite 中对应 Attempt 为 `Succeeded`，RunManifest 的 `evidence_kind` 为 `execution_record`；输出尺寸、字节数和 SHA-256 已与正式资产文件核对。

| 结果 | 入口与提示词 | 尺寸 / 种子 | PNG 字节数 | 正式输出 |
|---|---|---|---:|---|
| 方形山湖 | 验收 CLI；英文描述 | 1024×1024 / 42 | 1,494,049 | [PNG](../target/first-generation-validation/data/assets/asset-2f4148ce544f4f62808a75ff35f4e134.png) |
| 横向山湖 | 整个验收宿主处于 macOS 禁网 sandbox；中文描述 | 1024×768 / 43 | 950,791 | [PNG](../target/first-generation-validation/data/assets/asset-2d19af5408264e6496202ac44f6eb2c6.png) |
| 竖向静物 | 原生工作区提交；多行中英文描述 | 768×1024 / 44 | 813,329 | [PNG](../target/first-generation-validation/data/assets/asset-d7f0f88a467a49ca9888b2f765623f6e.png) |
| 建筑插画 | 真实采样中强杀宿主，恢复后同 Job 显式重试 | 1024×1024 / 77 | 1,277,718 | [PNG](../target/first-generation-validation/data/assets/asset-59348b34354749c29c0dbaa8225b2efe.png) |

输出 SHA-256：

- 方形山湖：`b43c302c6228190f4458f169123b8a2cc56abc1a5a6c00d1d6aed50925f4796d`。
- 横向山湖：`0ee10f6de05ccfbd9eb1357232b03066b43cfcb0daf18a88a18c193c2c51f1c2`。
- 竖向静物：`68af46f20f01228b6e0e7d1c05366c1fa6839a0d9450a20f1807f5f999d88705`。
- 建筑插画：`b8ca593192f231596bb93f4ef32801e352ffe85d8310e35204478d8af7813052`。

原始记录见[方图日志](first-generation-real-square.log)、[中文禁网横图日志](first-generation-real-landscape-offline.log)、[恢复后显式重试日志](first-generation-real-recovered-retry.log)。竖图由原生界面提交，其请求、有效种子/尺寸和来源可在任务制作记录中查看；[原生竖图截图](first-generation-ui/native-real-portrait.jpg)保留真实显示证据。

### 本次时间与内存口径

| 执行 | 本次观测时间 | 口径 | `/usr/bin/time -l` 最大 RSS |
|---|---:|---|---:|
| 方形山湖 | 109.03 秒 | CLI 宿主墙钟，含执行前校验、加载、采样、输出和提交 | 25,309,298,688 bytes，约 23.57 GiB |
| 禁网横向山湖 | 141.87 秒 | 相同 CLI 墙钟口径，整个宿主被禁止联网 | 27,300,315,136 bytes，约 25.43 GiB |
| 原生竖向静物 | 57.470 秒 | SQLite Job 接收提交至资产提交；不含 UI 提交前校验 | 未单独采集 |
| 建筑插画的重试 Attempt | 81.13 秒 | 显式重试 CLI 宿主墙钟；不含先前被强杀的 Attempt | 27,278,606,336 bytes，约 25.41 GiB |

时间与内存原文分别在[方图 time](first-generation-real-square-time.txt)、[禁网横图 time](first-generation-real-landscape-offline-time.txt)及[重试 time](first-generation-real-recovered-retry-time.txt)。Darwin 的最大 RSS 单位为 bytes；这里没有把 `peak memory footprint` 的宿主数值误当成 Python/GPU 总内存。RSS 也不是完整 Metal 统一内存分配峰值。

这些是本机单次观测，不是平均性能或容量承诺。每次重新加载模型，但会复用操作系统文件缓存和已有 Metal 编译缓存；不同入口计时范围、提示词、尺寸和系统负载也不同，不能据此横向宣布某个尺寸更快。一秒轮询记录中，方图观察到约 3 次 preparing、2 次 loading、79 次 sampling、2 次 decoding、2 次 writing；这些只是阶段快照，不是精确分段计时。第 9 步回调也没有被当成文件已提交。

## 取消、强杀、恢复与重试

| 场景 | 实际结果 |
|---|---|
| 原生 UI 的真实采样取消 | 任务页先观察到 5/9，接受取消时显示 8/9 与正在取消；随后变为 Cancelled，正式资产仍为原有三张 |
| 生成期间继续编辑 | 另一份草稿 4 的 49 字符中英文内容独立保存；运行中的请求仍是原竖图的 122 字符提示词 |
| 同 Job 在加载阶段再次取消 | 显式重试后在真实 loading 状态约 1.01 秒接受取消；旧/新 Attempt 和 token 均不同，两次终态均 Cancelled，均无输出 |
| 真正 SIGKILL 宿主 | 在 `sampling` 且 `step>=2` 后，仅对脚本持有的 live 子进程发送 SIGKILL；退出码 -9，第一 GPU 求值已返回 |
| 首次重启 | 原 Attempt 恢复为 Interrupted，输出为空；旧租约仍忙时显示 cleanup、禁止新生成，没有用历史 PID 发信号 |
| 原生资源清理 | 监督端在宿主消失后释放执行租约，验收脚本实际取得排他租约后才确认清理完成 |
| 显式恢复重试 | 保留 Job、原请求/有效计划和旧 Interrupted manifest，追加新 Attempt/token 后成功保存建筑插画 |

[真实取消与重试记录](first-generation-real-cancel-retry.json)确认两次取消没有正式输出。[真实宿主强杀记录](first-generation-real-host-crash.json)保留强杀时阶段、首个恢复快照和租约释放结果。SQLite 只读核对时共有五个 Job、七个 Attempt、四张正式图片；取消 Job 保留两次 Cancelled，恢复 Job 保留一次 Interrupted 和一次 Succeeded。

监督进程自身被杀的另一条回收路径使用受控故障 fixture 单独验证，不能算作真实模型出图证据：见[监督异常回收报告](generation-supervisor-crash-results.md)。其它越界/损坏 PNG、FIFO、散列不符、终态竞态、数据库提交崩溃窗口和 101 张资产分页恢复，见[存储验收](first-generation-storage-results.md)与[进程验收](generation-process-results.md)。

## 原生界面与导出

通过真实 macOS 窗口、辅助功能树与截图检查，而不是只读开发服务器：

- 两份中英文草稿各自保存，正常退出重开后文本与当前选择恢复；后续真实生成期间的新草稿也保持独立。
- 模型页区分已接入、已安装、可执行与设备探针；没有用诊断成功或自带示例增加模型/任务数。
- 1280×800 和 960×640 逻辑窗口、浅色/深色下导航、编辑和画布可用。截图为实际 2× 原生渲染，不声称本轮重新完成 1× 原生设备验证。
- 真实结果通过受控资产 URI 显示，带“Z-Image Turbo · 本机生成”；重开后结果恢复。
- 示例仍标注“非本机生成”，使用模板后一次撤销恢复当前草稿，不创建图片任务。
- 原生 NSSavePanel 将真实横图导出至包含中文和空格的新路径。导出文件 950,791 bytes，SHA-256 与内部资产完全一致，没有创建第二个内部资产身份。用户取消或失败不改写已成功任务的行为另由自动化存储测试覆盖。
- 显式通信诊断仍只说明通信能力，停止后草稿和真实任务记录保持；未让诊断运行于普通启动。

截图：[窄窗草稿恢复](first-generation-ui/native-restored-draft-960-light.jpg)、[深色模型页](first-generation-ui/native-model-960-dark.jpg)、[深色示例画板](first-generation-ui/native-example-960-dark.jpg)、[真实横图恢复](first-generation-ui/native-real-landscape-restored.jpg)、[真实采样任务页](first-generation-ui/native-real-sampling-editable.jpg)、[取消完成](first-generation-ui/native-real-cancelled.jpg)。导出元数据见[原生导出记录](first-generation-native-export.json)。

最后实现构建重开后，画布恢复了强杀重试成功的建筑图片及独立保存的草稿 4，任务页显示全部 5、进行中 0、已完成 4，取消任务仍保留。见[最终恢复画布](first-generation-ui/native-final-recovered-output.jpg)与[最终任务列表](first-generation-ui/native-final-tasks.jpg)。

输入法边界由本轮状态/保存回归测试覆盖：确认前快照可保存、迟到的旧草稿组合事件不串写新草稿、未确认组合不作为已确认文本落盘。本轮原生明确验证了中英文多行粘贴与编辑；基础 UI 的实际拼音候选验证见[基础原生输入记录](foundation-results.md)，不把该旧结果写成本轮重新完成的拼音输入实测。

## 自动质量门禁状态

当前[质量汇总](first-generation-quality.json)确认前端构建、11 项测试、生产依赖 audit（0 漏洞）、Rust fmt、严格 Clippy、默认并发 workspace 测试（134 passed / 0 failed / 2 ignored）和本机构建含 examples/bins 全部通过。两项默认忽略的显式验收分别有固定运行时禁网安装及原生监督被杀回收记录；Windows 仅核心/协议编译通过。

最终并发回归暴露的宿主锁继承问题已修复：[宿主锁回归报告](first-generation-host-lock-results.md)记录真实子进程持有继承描述符时的锁释放与新 owner 排他验证。失败历史日志仍保留，当前通过结论来自修复后的完整默认并发测试。

[干净源码报告](first-generation-clean-source-build.json)已在最后实现改动后重跑，83 个源码输入散列核对无变化；独立源文件副本与隔离 target 复用全局 npm/Cargo 缓存，不是冷构建或 Git checkout。[禁网启动报告](first-generation-offline-startup.json)也已重跑，网络拒绝对照生效、原生 WebView 加载完成、没有安装模型或启动 worker。无模型禁网启动与上文已安装模型的禁网生成提供各自独立证据。

## 数据位置与验收范围

本轮模型、运行时、数据库与真实图片均在 `target/first-generation-validation` 隔离根，未迁移到默认用户库。查看已验收工作区时应显式使用该根：

```sh
open -n "target/dev/Image Studio.app" --args --data-dir "$PWD/target/first-generation-validation"
```

真实图片和大模型属于本地验收数据，不作为源码净构建输入；若以后清理 target，PNG 链接可能不再存在，散列与执行日志仍是本轮留下的证据。当前不包含其他首批模型、图片编辑、多参考、LoRA、批量队列、热模型池、公开签名分发或其它硬件认证。OpenSpec 已归档为 [2026-10-30-deliver-first-local-generation](../openspec/changes/archive/2026-10-30-deliver-first-local-generation/tasks.md)，30/30 任务完成，主规格已更新；Beads 保存任务状态，本报告保存执行与验证证据。
