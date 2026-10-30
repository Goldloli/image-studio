# 生成协议与进程边界验证

日期：2026-10-30。范围为 generation v1、原生监督及桌面执行协调器；本材料不是 MFLUX 整模型推理证据。

## 已实现边界

- generation 使用独立 `image-studio.generation` 协议族及首版版本，不改诊断 v1；运行时/模型/worker 身份、会话、执行请求、Attempt/token 与逐方向序号均有门禁。
- 原生监督持有执行租约，由 Python 和其受管理后代继承；宿主 EOF、合作取消、无响应取消、加载/生成超时和异常退出均先清理进程组。未确认回收时保留忙碌与租约。
- 控制帧 64 KiB，关键队列 8 项、进度合并、日志内存窗口 128 KiB、每次磁盘日志 8 MiB。`CLEANUP_INCOMPLETE` 是可继续接收最终结果的非终态状态。
- 桌面协调器在后台监督交换，并等待监督退出和执行租约释放后调用 Library。候选、9/9 进度都不直接代表成功；SQLite 门禁裁决取消/成功及取消/失败竞争。
- 启动失败不会留下不可重试的活动 Attempt；数据库保存失败不伪称已登记 Failed，而明确显示存储错误。准备前已接受的取消仍以 Cancelled 结束。

## 已通过检查

| 命令 / 范围 | 结果 |
| --- | --- |
| `cargo test -p inference-supervisor -p worker-protocol --locked --offline` | 39 项通过：protocol 30（既有诊断 24 + generation 6），监督单测 2、进程集成 7 |
| `cargo clippy -p inference-supervisor -p worker-protocol --all-targets --locked --offline -- -D warnings` | 通过 |
| `cargo test -p image-studio-desktop adapters::generation --locked --offline` | 6 项通过 |

原生集成覆盖正常候选、宿主 EOF、后代残留、旧租约与继承、异常退出、非法/超限/截断帧、合作/不合作取消、两类超时、日志与进度洪泛。修复了截断/超限缓冲区不能变成正常 EOF、导致清理后仍无法结束的错误；流被放弃后仍须等待实际进程组回收。

协调器覆盖：缺失监督程序、准备前取消、外部执行租约、候选已发但进程尚未退出、清理异常后仍能取消、非法帧关闭通道并回收。候选测试使用标准库构造的 PNG；其图片及隔离测试数据库只验证文件和状态语义，不表示模型成功生成。

## 未由本材料证明

完整模型下载、真实 GPU 生成质量/速度/内存、模型加载和采样期间真实取消、原生 UI 交互、离线整模型执行仍由 change 的实机验收任务另行记录。本材料不证明 Windows 桌面或推理兼容性。
