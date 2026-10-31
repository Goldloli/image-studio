# Klein 阶段验收结果

状态：**通过，Klein 阶段门禁关闭**。必需项全部通过；BiRefNet 尚未开始，因此三型号整批门禁仍保持关闭。

## 结论

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
