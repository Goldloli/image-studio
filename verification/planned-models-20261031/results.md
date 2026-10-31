# Klein 阶段性验收结果

状态：**未完成，门禁保持关闭**。Klein 后端、安装、真实文生图/编辑、质量、生命周期和安全边界已通过；`I07` 的完整断点续传受当前上游代理超时阻塞，原生 `U01/U02/U03/U05/U06` 受 macOS 锁屏阻塞。未开始 BiRefNet。

## 结论

- 运行标识：`planned-models-20261031`
- 产品代码检查点：`9bf1d93`，其后仅有验收工具与报告变化
- 模型：`flux2-klein-4b-distilled`
- 模型快照：`flux2-klein-4b-e7b7dc27f91deacad38e78976d1f2b499d76a294`
- 运行时：`mflux-0.24.0-py3.13.12-macos-arm64-recipes-v2`
- worker：`studio-mflux-worker-0.2.0`
- 设备：Apple M3 Max / 128 GiB / macOS 27.0.1
- 结构化汇总：`klein-acceptance.json`
- 当前必需项：41 passed / 0 failed / 0 not_run / 9 blocked

## 已通过

- 固定 18 个模型组件、完整运行依赖、许可和 SHA-256 身份；模型组件 `15,980,131,745` bytes。
- 正式冷安装、隔离根禁用网络缓存安装、默认 macOS 路径安装和 GPU probe。
- 中文/英文文生图、seed 42/73；三种真实编辑：改物体颜色、换背景、改变氛围。
- 大横图、奇数竖图、方向标记、非 16 倍数裁边和大于 1 MP 参考处理；小于 16 像素和有效透明源图在采样前拒绝。
- 实际 Klein tokenizer 的含模板 512 token 边界；513 token 不截断，完整原文保留并失败于 GPU 采样前。
- 文生图和编辑分别连续三次同进程任务；每次有独立 Job/Attempt/Asset，worker 回收后空闲。
- 加载阶段和真实采样阶段取消；原 Attempt 无输出，显式重试新 token 成功。
- 文生图和编辑宿主在实际采样后 SIGKILL；执行租约保持约 3.17 秒并释放，后代进程回收，无 AssetRef 提升，显式重试成功。
- 重复提交键返回原 Turn；忙碌时新请求不创建 Turn，未发送草稿保留。
- 整个宿主、supervisor 与 Python worker 在 `deny network*` 环境完成文生图和编辑。
- Rust、前端、Python、格式、Clippy 和构建质量门通过。

## 阻塞

- `I07`：真实下载已中断并续传；两个完整分段和一个部分分段与可信文件逐字节一致，但当前 `127.0.0.1:7897` 上游连接持续超时。需代理恢复后完成剩余字节、最终 SHA-256 和激活。
- `U01/U02/U03/U05/U06`：macOS 当前锁屏，Computer Use 无法查看或操作原生窗口。需用户解锁后执行模型页、系统选图、composer、重启恢复、中文空格路径导出、1280×800/960×640、键盘和 IME 用例。

## 证据

- 质量审阅：`klein-quality-review.json`
- 质量接触表：`klein-real-quality-contact-sheet.jpg`
- 连续任务：`P03-P08-batch.json`
- 取消/重试：`P04-P05-cancellation.json`
- 强杀恢复：`P07-klein-host-crash.json`、`P07-klein-edit-host-crash.json`
- 禁网：`P02-B10-offline.json`
- 输出完整性与哈希：`P01-integrity.json`
- Tokenizer 边界：`B04-result.json`
- 参考处理：`B03-reference-landscape.json`、`B03-reference-portrait.json`
- 源图与重试冻结：`S01-S07-source-integrity.json`
- 安装身份：`installation-identity.json`
- 完整质量门日志：`logs/quality-gates.log`

诊断运行 `klein-edit.png` 只把无纹理渐变改为橙色，明确排除在 B02 质量证据之外。
