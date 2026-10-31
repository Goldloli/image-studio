# 计划模型阶段验收结果

状态：**Klein、BiRefNet 与 Real-ESRGAN 三阶段全部通过**；按配置展开后的型号/能力必需项114个，加整批R组4个，共118/118通过。当前可归档并进入已条件确认的Qwen阶段。



## 整批结论

[逐配置认证报告](batch-acceptance.json)逐项展开 K-T 25、K-E 29、B-R 31、R-U 29 和整批 R01–R04 共 4 项：**118 passed / 0 failed / 0 not_run / 0 blocked**。历史 Klein 报告的共享配置/合并编号在此展开，两个额外源图检查保留为附加证据，不抬高必需项计数。

- R01：默认目录原生顺序完成 Z → Klein生成 → Klein编辑 → BiRefNet → Real-ESRGAN → Z，v1/v2归属、原输入和工具无种子记录正确。
- R02：当前Z中英/三个尺寸、随机种子、加载/计算取消、宿主强杀与历史v1冻结计划重试通过；原生重开导出hash相同。最初暴露的重试问题与修复后重跑分别保留。
- R03：四型号状态/运行环境计数正确，生成默认菜单仅Z/Klein；专用会话7个Job显示7件作品，BiRefNet mask不重复计数，四型号筛选正确。
- R04：Rust216、前端36、Python38、锁与worker源码、离线启动、干净源码构建通过。干净构建排除了依赖与生成目录，复用npm/Cargo缓存，不宣称无缓存冷构建。

可执行只读复核：`python3 verification/planned-models-20261031/validate-reports.py`；同时校验118个配置/用例、80个直接证据文件、质量与原生导出图片hash及固定Real manifests。当前应用构建身份与历史执行身份分别保留。手动IME重复检查按用户要求免测，没有写成新执行通过。


## Real-ESRGAN 阶段

- 正式配置：`realesrgan-x4plus-ncnn` / `realesrgan-x4plus-ncnn-macos-v0.2.0-v1` / `studio-realesrgan-worker-0.1.0`。
- 固定 `x4plus`、GPU 0、tile 128、线程 `1:1:1`、4 倍 RGB PNG；输入最长边 1024、最大 1 MP、无透明像素，输出最长边 4096、最大 16 MP。
- 实测设备：Apple M3 Max / 128 GiB / macOS 27.0.1 / arm64。两份官方归档、模型对、universal 程序及系统动态依赖均已校验；不安装动漫专用模型。
- 结构化汇总：[realesrgan-acceptance.json](realesrgan-acceptance.json)，**29 passed / 0 failed / 0 not_run / 0 blocked**。每次执行和当前原生应用的身份见 [realesrgan-run-identity.json](realesrgan-run-identity.json)。
- 空根冷安装下载 60,349,360 bytes；另一根整树禁网缓存安装同一身份。默认目录真实停止下载、正常退出重开、显式继续后到 Ready。
- 三种质量输入的正式输出与同快照 tile128 上游基线逐字节相同。1024² 测试有 64 个真实 tile 进度；4096² 全图的 14 条内部边界均检查了完整 4096 像素长度，未见可见拼缝。
- 大 tile 4096 仅用于参考，和产品 tile128 的通道平均绝对差为 0.510、最大差 45；微纹理存在差异，放大可能推测出原图不存在的细节。
- 真加载取消、完成至少一个 tile 后的计算取消、宿主强杀、包装器及 ncnn 整树回收、旧 Attempt 零资产及显式新 token 重试均通过；强杀后租约约 3.10 秒释放。
- 同进程三个独立任务逐一等待空闲，保留每次耗时、进程 RSS 和输出；没有把统一内存或 GPU 峰值猜成 RSS，也没有性能 SLA。当前实测数值见 [连续任务报告](P03-P08-realesrgan-continuity.json)。
- 正常重开后的 4096² 原生系统导出与内部资产 SHA-256 相同。1280×800 和 960×640 可查看制作记录和导出；错误输入保留原合法源图，键盘发送焦点可达。
- 完整质量门：Rust 216 passed / 4 个显式外部环境 helper ignored；前端 36 passed；Python 38 passed / 0 skip；格式、Clippy、锁/wheel 源码审计、禁网启动和排除生成文件的离线干净源码构建均通过。必需真实模型验收由独立运行证据覆盖。
- 按用户 2026-11-01 的意见，本轮不重复要求手动中文 IME 检查；保留组合输入防误提交自动化和 Klein 阶段原生证据，不将免测写成新的手动通过。

## BiRefNet 阶段

- 模型：`birefnet-general-epoch-244-onnx`
- 运行时：`onnxruntime-1.31.1-py3.13.12-macos-arm64-cpu-v1`
- worker：`studio-onnx-worker-0.1.0`
- 设备：Apple M3 Max / 128 GiB / macOS arm64 / `CPUExecutionProvider`
- 最终应用二进制：`29ef71fa71a903b050db11ecce3581da891241e222c72480e3c8f307dbcbc1dd`
- 结构化汇总：[birefnet-acceptance.json](birefnet-acceptance.json)
- schema 校验：[birefnet-report-schema-validation.json](birefnet-report-schema-validation.json)
- 必需项：**31 passed / 0 failed / 0 not_run / 0 blocked**

已通过：

- 官方 release asset `186740004`，`972,666,916` bytes，SHA-256 `58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c`；opset 17、固定 float32 `1×3×1024×1024` 输入和 `1×1×1024×1024` 输出。图末输出为 Conv logits，产品明确只做一次 sigmoid。
- 真正空缓存冷安装下载 `1,022,243,898` bytes，随后完成权重哈希、隔离 Python/ONNX Runtime 安装、CPU 图探针和原子激活；另一个隔离根在整树禁网环境完成同身份安装。
- 真实下载中断后重开：模型部分前缀保留；变化后的 ONNX Runtime wheel Range/ETag 被拒绝并清除，显式重试完成下载、校验、安装和激活，最终身份与冷安装一致。
- 固定 RGB/归一化/`cv2.INTER_LINEAR` 预处理、`align_corners=True` 恢复、straight alpha 和源 alpha 乘法；产品 alpha 与 8-bit mask 逐像素一致。
- 六张真实质量样图覆盖发丝、人物与长毛猫、硬边香水瓶、透明玻璃鱼、复杂同色背景和细枝/阴影。样图全部来自本轮原创生成或项目已有原创资产，未使用授权不明的第三方图片。
- 同进程连续三次任务、重复提交键、忙碌请求、加载和真实 `session.run` 取消、宿主 SIGKILL、整树回收、约 3.09 秒租约释放及显式重试。
- 单一 Job 原子提交 primary RGBA 与 mask；作品页和 facets 只统计 primary，原生页面 2 个 Job 显示 2 件作品，蒙版仍可单独查看和导出。
- 系统选图、已有作品选图、透明棋盘格、蒙版查看、中文空格目录双导出、正常重开、1280×800、960×640、缺源图与无效 ICC 恢复。
- 最终 Tauri 构建及 960×640 禁网启动重新通过；Rust、前端、Python、模型锁与 OpenSpec strict 校验通过。

质量局限：

- 极细、低对比度发丝或枝杈可能被软化或少抠。
- 半透明玻璃物体保留内部 alpha 变化，但极细肋片和歧义反射可能碎片化。
- 这些结论只适用于固定 epoch_244 快照和已记录样图，不外推到其他 BiRefNet checkpoint。

### Klein 结论

- 运行标识：`planned-models-20261031`
- 产品与验收代码检查点：`d08359c`
- 来源授权文档检查点：`ec1144a`，产品代码未再变化
- 模型：`flux2-klein-4b-distilled`
- 模型快照：`flux2-klein-4b-e7b7dc27f91deacad38e78976d1f2b499d76a294`
- 运行时：`mflux-0.24.0-py3.13.12-macos-arm64-recipes-v2`
- worker：`studio-mflux-worker-0.2.0`
- 设备：Apple M3 Max / 128 GiB / macOS 27.0.1
- 结构化汇总：`klein-acceptance.json`
- 必需项：50 passed / 0 failed / 0 not_run / 0 blocked

## 已通过

- 固定 18 个模型组件、完整运行依赖、许可和 SHA-256 身份；模型组件 `15,980,131,745` bytes。
- 正式冷安装、隔离根禁用网络缓存安装、默认 macOS 路径安装和 GPU probe。
- 真实下载中断后续传：分段前缀保持、Range 续传与损坏分段显式修复均通过；国内镜像只承担传输，18/18 文件仍匹配官方固定 revision、字节数和 SHA-256。
- 镜像补齐后由正式安装器在整树禁网环境重新完整校验、安装 runtime、执行 probe 并激活；最终 `install_id`、模型快照、runtime 与 worker 身份一致。
- 中文/英文文生图、seed 42/73；三种真实编辑：改物体颜色、换背景、改变氛围。
- 大横图、奇数竖图、方向标记、非 16 倍数裁边和大于 1 MP 参考处理；小于 16 像素和有效透明源图在采样前拒绝。
- 实际 Klein tokenizer 的含模板 512 token 边界；513 token 不截断，完整原文保留并失败于 GPU 采样前。
- 文生图和编辑分别连续三次同进程任务；每次有独立 Job/Attempt/Asset，worker 回收后空闲。
- 加载阶段和真实采样阶段取消；原 Attempt 无输出，显式重试新 token 成功。
- 文生图和编辑宿主在实际采样后 SIGKILL；执行租约保持约 3.17 秒并释放，后代进程回收，无 AssetRef 提升，显式重试成功。
- 重复提交键返回原 Turn；忙碌时新请求不创建 Turn，未发送草稿保留。
- 整个宿主、supervisor 与 Python worker 在 `deny network*` 环境完成文生图和编辑。
- 原生模型页展示两项能力和真实 Ready 事实；系统 PNG 选图、已有作品选图、取消替换与缺源图门禁通过。
- 执行中编辑下一条、切会话与切模型不改变冻结请求；正常退出重开恢复结果、来源和草稿。
- 1280×800 与 960×640 布局、键盘焦点、原生中文 IME、无效 seed 错误恢复通过。
- 通过系统保存面板导出到中文和空格路径；两个导出文件的 SHA-256 与内部资产完全一致。
- Rust、前端、Python、格式、Clippy 和构建质量门通过。

## 续传与来源

- 真实 HTTP 下载在部分组件写入后中断；续传从已有字节继续，覆盖此前“先截断再 seek”造成零填充空洞的问题。
- `segmented_range_resume_preserves_the_existing_prefix` 使用真实 Range 响应验证分段前缀不被覆盖；完整分段与部分分段均和可信文件逐字节一致。
- 原代理上游持续 SSL 超时后，使用用户明确授权的 `hf-mirror.com` 补齐传输。镜像不是内容身份来源，官方 manifest 的 revision、字节数和 SHA-256 才决定接受或拒绝。
- `I07-mirror-cache.json` 记录 18/18 文件哈希匹配；`klein-resume-install.json` 记录最终正式安装、probe 与激活。相关边界已写入 OpenSpec。

## 证据

- 质量审阅：`klein-quality-review.json`
- 质量接触表：`klein-real-quality-contact-sheet.jpg`
- 连续任务：`P03-P08-batch.json`
- 取消/重试：`P04-P05-cancellation.json`
- 强杀恢复：`P07-klein-host-crash.json`、`P07-klein-edit-host-crash.json`
- 禁网：`P02-B10-offline.json`
- 真实续传：`I07-resume.json`、`logs/klein-install-resume-final.log`
- 国内镜像传输核验：`I07-mirror-cache.json`
- 禁网最终激活：`klein-resume-install.json`、`logs/klein-install-resume-final-activated.log`
- 输出完整性与哈希：`P01-integrity.json`
- Tokenizer 边界：`B04-result.json`
- 参考处理：`B03-reference-landscape.json`、`B03-reference-portrait.json`
- 源图与重试冻结：`S01-S07-source-integrity.json`
- 安装身份：`installation-identity.json`
- 原生日常验收：`U-native.json`、`native-klein-contact-sheet.jpg`
- 中文空格路径导出：`导出 验收/Klein 原生夜景编辑.png`、`导出 验收/Klein 原生文生图.png`
- 应用二进制哈希：`application-binaries.sha256`
- 报告 schema 校验：`report-schema-validation.json`
- 完整质量门日志：`logs/quality-gates.log`

诊断运行 `klein-edit.png` 只把无纹理渐变改为橙色，明确排除在 B02 质量证据之外。
