# 会话式生成与进度图片卡验收

日期：2026-10-30，Asia/Shanghai。对应已批准的 `introduce-generation-conversation` change；本轮只实现会话、发送意图、进度图片卡、迁移与恢复，不包含后续模型目录、设置页重构或功能/作品页面。

## 环境与范围

| 项目 | 验收值 |
|---|---|
| 机器 | Mac15,8 / Apple M3 Max / arm64 / 128 GiB |
| 模型 | Z-Image Turbo / MFLUX 0.24.0 已安装组合 |
| 数据根 | `target/conversation-validation`，由已验收 v1 数据根通过 APFS clone 独立复制 |
| 生成规格 | 1024×1024、单张、9 步、加载时 q4 |
| 桌面栈 | Rust core + Tauri 2 + React |

本轮未重新下载模型。迁移和真实生成只在隔离副本执行；原 `target/first-generation-validation` 未写入。模型截图、阶段截图和结果都来自原生 Tauri 窗口，不是浏览器假数据。

## v1 到 v2 迁移

升级前数据库为 `user_version=1`，包含 5 份草稿和 6 个既有 Job。首次打开后进入 `user_version=2`：

- 5 份旧草稿分别成为 5 个会话并保留提示词、参数、名称和版本；每份草稿补充当前登记 `modelId`。
- 6 个无法可靠关联旧草稿的 Job 只进入一个“历史生成”会话，未按提示词相似度猜测归属。
- 原 Job、Attempt、manifest、AssetRef 和所选资产保持不变。
- 重复启动没有重复创建会话或历史轮次。
- 迁移失败测试确认事务回滚后仍为 v1 且原草稿保留。

迁移后的隔离库随后包含 8 个会话、9 个轮次、9 个 Job、12 个 Attempt 和 6 张正式图片；数量包含本轮新建的两个验收会话、一次成功结果和两次主动取消。

## 发送、进度与结果

在新会话“会话验收 A”中手工重命名后，使用 Enter 发送中文提示词：

1. 输入框文字在本地校验通过后立即出现在右侧用户消息。
2. 对侧在同一渲染周期出现目标比例图片卡；当时状态为“正在接收提示词 2%”，输入框已在宿主持久接收后清空。
3. 卡内继续显示加载、提示词编码、采样、解码与保存。采样到 `9/9` 时仍为 85% 和“正在处理”，没有提前成功。
4. 第一次真实执行在采样阶段取消，界面先显示“正在取消”，资源回收后为“已取消”；没有提交资产。
5. 在原卡点击“重试”后生成新 Attempt。新执行完成后才进入 100% 并原位显示 PNG。

成功结果：

| 项目 | 值 |
|---|---|
| Job | `job-3b054673534040189f540f5c13283055` |
| 成功 Attempt | `attempt-ebacaa78a24a4e39bd6ca089b9eb7c67` |
| Asset | `asset-4c32f2112902415aa25610a7c48e4081` |
| 尺寸 | 1024×1024 |
| 实际种子 | 3489739187 |
| PNG 字节数 | 1,040,543 |
| SHA-256 | `7bf62fbec1e745ec8d8d6f2a74a021f297ad66f2b30716bee09d4f05e91d10cc` |

通过原生 NSSavePanel 导出为 `target/first-generation-export-验证/conversation-validation-output.png`。导出文件为 1024×1024 8-bit RGB PNG，SHA-256 与内部资产完全一致。

截图：[2% 接收态](conversation-ui/native-1280-receiving.jpg)、[真实采样](conversation-ui/native-1280-sampling.jpg)、[最终多轮会话](conversation-ui/native-1280-final-light.jpg)、[深色结果](conversation-ui/native-1280-final-dark.jpg)、[图片查看器](conversation-ui/native-1280-dark-viewer.jpg)。

## 多会话与恢复

- 同一会话成功后可继续发送第二轮，旧提示词和图片保持原位。
- 第二轮运行中点击“新建会话”，新会话 B 没有被隐式排队，也没有复制 A 的提示词或结果。
- B 的输入框可编辑；Enter 不会发送，原因显示“其他会话正在生成”。A 侧栏保留生成状态，并提供“查看正在生成的会话”。
- 切回 A 可取消当前请求；切回 B 后，B 的未发送文字仍然存在。
- 正常关闭并重开应用后，B 恢复为活动会话且输入完整；A 的图片、取消轮次、制作记录和当前限制均可恢复。
- 任务页的真实结果缩略图可跳回 A，并在返回后打开对应图片查看器。
- 历史与制作记录按需读取；每秒快照不再携带完整 manifest。

截图：[960×640 会话](conversation-ui/native-960-light-two-turns.jpg)、[960×640 侧栏收起](conversation-ui/native-960-collapsed.jpg)。

浏览器尺寸检查另见 [1280 浅色空态](conversation-ui/browser-empty-light-1280.png) 与 [960 深色空态](conversation-ui/browser-empty-dark-960.png)。浏览器预览不提供持久化或真实生成。

## 自动验证

最终实现后执行的命令：

```sh
npm --prefix apps/frontend run build
npm --prefix apps/frontend test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo check -p studio-core -p worker-protocol --target x86_64-pc-windows-msvc --locked
python3 scripts/check-offline-startup.py
python3 scripts/check-clean-source.py
```

最终结果：前端 18 项测试通过；Rust workspace 143 项通过、2 项需外部固定验收材料的显式测试忽略；格式、严格 Clippy、workspace build、Windows 核心/协议编译、禁网启动和独立源码副本构建全部通过。

自动测试覆盖 schema 迁移与回滚、会话/轮次分页、同提交键幂等、草稿 CAS 冲突、Pending 取消与重启恢复、Pending/Job 全局互斥、草稿队列迟到 ACK、IME、Enter/Shift+Enter、真实阶段进度、100% 门禁和语言无关的图片校验/导出。

## 限制

- 真实生成本轮只验证当前 M3 Max / 128 GiB 与已安装 Z-Image Turbo 组合。
- 中文 IME 的浏览器事件语义由自动测试覆盖；本轮原生窗口验证了中文文本、多行内容与 Enter 提交，没有重新做拼音候选人工录屏。
- 窗口截图来自原生 Tauri 窗口；`--diagnostic-scale` 只记录请求，实际 DPR 由 macOS/WebView 决定，不宣称重新完成独立显示器矩阵认证。
