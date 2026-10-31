# 计划模型阶段验收结果

状态：**Klein 与 BiRefNet 阶段通过**。两阶段必需项均无失败、未运行或阻塞；Real-ESRGAN 尚未开始，因此三型号整批门禁仍保持关闭。

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
