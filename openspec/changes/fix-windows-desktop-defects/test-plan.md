# Test plan: fix-windows-desktop-defects

## 环境

Windows 11 Enterprise 22631 / i9-9900K / RTX 2060 SUPER 8 GB（sm75）/ 31.9 GiB RAM；隔离数据根
`D:\ist-8gb\appdata`；应用由本分支构建（NSIS 包或 `target/release` 直接启动，均需 CDP 端口）。

## 本机可验证项

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| T1 | 连续发送 20 次（Qwen Turbo，含 1 次取消、1 次失败注入） | 每次都进入 job；无"停在正在接收提示词"；相邻两次发送间隔与总耗时记录 | 会话记录 + 应用日志 + `verification/` 截图 |
| T2 | 人为让 `generation.start` 失败（占用 execution.lock） | 轮次在 60 s 内转 failed，错误含阶段与可行动原因；可立即重试 | 任务 JSON + 截图 |
| T3 | Qwen Turbo / MOSS / Klein 各取消一次 | 点击到 `cancelled` ≤ 15 s；无残留 python/supervisor；无半成品资产 | 时间戳日志 + 进程快照 |
| T4 | 真实安装一次（Qwen Turbo，清空 ready 记录） | 模型页依次显示下载/校验/准备/安装运行时/探针，不出现"尚未安装" | 逐阶段截图 |
| T5 | 下载故障注入：某 wheel 连续失败 2 次后成功 | 自动重试后安装继续完成 | 安装日志 + ready 记录 |
| T6 | 下载故障注入：某 wheel 连续失败 5 次 | 安装失败，错误含 URL 与尝试次数，已下载进度保留 | 安装日志 + 缓存字节 |
| T7 | 回归 | `cargo test --workspace`、前端 59 测试、`openspec validate --all --strict` 全绿 | 日志 |

## 判定规则

- 取消时限以"点击停止"到任务中心显示"已取消"的墙钟时间为准，从日志时间戳核对。
- 强制回收路径必须标注原因，且资产库不出现半成品。
- 重试测试使用本地可控的失败注入（例如在下载客户端注入一次失败或临时改写目标 URL），不为测试修改生产清单。
- 任何未执行项标记 `not_run`，不得写成通过。

## 用户侧（非本机）

| ID | 场景 | 期望 |
|---|---|---|
| U1 | 12 GB 机器回归 | 本轮改动不改变引擎、协议与档位行为；标准档生成与既有证据一致 |
