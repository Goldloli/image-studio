# 原生验收结果（macOS）

日期：2026-11-11 23:22–23:47（Asia/Shanghai）。范围：OpenSpec change
`simplify-workspace-and-manage-library` tasks 6.1。执行方式：computer use（AppleScript/System Events +
辅助功能树 + 窗口截图）操作**真实 dev 构建**，数据使用真实数据根（先备份 v5 库）。

## 环境与构建

- macOS 27.2.1，Apple M3 Max，128 GB。
- `target/dev/Image Studio.app`，debug 构建，内嵌前端生产构建；代码状态为 `2c9ea93`
  （能力白名单修复 `58dbec3` 之后，验收期间无源码改动，只有证据文档提交）。
- 数据根：`~/Library/Application Support/ImageStudio`。
- **升级前备份**：`target/acceptance-20261111-v5/library.sqlite3`（v5，929792 B，
  sha256 `e86907460617…`），首次启动后库升级为 v6。
- 7 个模型目录在升级前后保持就绪（模型页显示 目录7/接入7/已安装7/可执行7/设备满足7）。

## 结果

| 项 | 结果 | 证据 |
| --- | --- | --- |
| U01 单层导航/无页签 | **PASS** | 原生 AX 树与截图：左侧仅 创作/灵感库/模型/作品 + 主题/关于/设置；无页签、关闭按钮 |
| U02 无功能/任务入口、四类能力可达 | **PASS** | AX 树中无“功能/任务”按钮；模型菜单含 7 个可执行模型（文生图、Klein 文生图/编辑、BiRefNet、Real-ESRGAN 等），本轮实际使用文生图、去背景与 Klein 编辑源图选择 |
| U03 无开发入口/抽屉 | **PASS** | AX 树无“开发/诊断”按钮；设置五组中无诊断入口；运行环境只读事实仍显示 |
| U04 侧栏无示例灵感、中央示例卡保留 | **PASS** | `native-01-initial-1280.png`：侧栏底部仅主题/关于/设置；空会话中央三张示例卡 |
| U05 许可说明集中 | **PASS** | `native-U05-inspiration-1280.png`、`native-U05-about-license-1280.png`：灵感库无“来源与内容许可”折叠区与通用段落；关于页“灵感库来源与许可”含固定上游 `63e6176f` 的 MIT 许可、第三方声明、仓库链接；自动更新/在线服务/存储迁移显示“未提供” |
| U06 窗口/目标尺寸/主题/键盘 | **PASS（部分键盘项）** | 交通灯与拖动区正常；`native-U06-about-960.png` 为 960×640 窗口；深色主题切换成功；本轮通过 Enter/Escape/Cmd+V/按钮点击完成操作，未逐项记录纯 Tab 遍历顺序 |
| U07 原位制作记录 | **PASS** | `native-U07-record-expanded-1280.png`：会话成功卡内展开 型号/尺寸/种子/当前 Attempt/Attempt 记录/阶段 sampling，可收起，未打开任务页 |
| C01 会话行双操作/重命名 | **PASS** | 行内同时存在“重命名/删除”；原生重命名后 DB 与标题变为 `验收-去背景` |
| C02 确认框取消/Escape | **PASS** | `native-C02-confirm-dialog-1280.png`：`role=alertdialog`、绑定会话名、默认焦点“取消”；Escape 关闭且不改变数据；焦点返回在浏览器 smoke 中验证 |
| C03 删除非当前会话保留当前输入 | **PASS** | 在 A 会话输入“未发送草稿保留测试”，新建 B 并删除 B：A 被选中且输入框仍为该文本 |
| C04 后继选择 / 末项替换 | **PASS（末项为自动化+浏览器验证）** | 删除当前会话后自动选中最近活动会话；删除最后一个会话的替换路径在浏览器 smoke 与 Rust 测试覆盖，原生未逐项执行最后一项替换 |
| C05 忙碌门禁 | **PASS（UI 门禁 + 自动化宿主门禁）** | 生成中会话行删除按钮禁用（`native-C05-busy-disabled-1280.png` 悬停提示）；宿主拒绝路径由 Rust 测试 `a_busy_conversation_is_refused_while_idle_conversations_stay_deletable`、`a_conversation_with_a_bound_active_job_cannot_be_deleted` 覆盖 |
| C06 版本冲突/重放 | **PASS（自动化）** | 自动化 `conversation_deletion_requires_the_confirmed_draft_version`、service 重放测试；原生未制造并发冲突 |
| C07 删会话保留作品/来源墓碑 | **PASS** | `native-C07-delete-session-dialog-1280.png`、`native-C07-retained-work-1280.png`：删除“验收-去背景”后作品仍在（33 件），作品卡显示“验收-去背景 · 会话已删除”，预览显示“来源会话已删除 · 作品保留”且无返回按钮 |
| A01 收藏/删除按钮独立命中 | **PASS** | 网格卡片右下角存在收藏/删除；收藏不打开预览；删除打开绑定确认框 |
| A02 收藏重启持久 | **PASS** | 收藏历史作品后重启应用：作品首卡显示“取消收藏”、DB `is_favorite=1`；取消后 DB 归 0 |
| A03 仅收藏筛选 | **PASS（单页）；跨页为自动化** | 收藏后“仅收藏”显示 `1 件正式作品` 且只含该作品；跨页较早收藏由 Rust 测试覆盖 |
| A04 组合筛选 | **PASS（自动化 + UI 存在）** | 筛选区存在 全部作品/仅收藏；与型号/尺寸/会话取交集由 Rust/前端状态测试覆盖 |
| A05 空态区分 | **PASS** | `native-A05-no-favorites-1280.png`：`0 件正式作品` + “尚无收藏作品” |
| A06 删除普通作品 | **PASS** | 两件作品删除：确认文案含原图/蒙版/缓存与外部副本保留；DB `deletion_state=deleted`、登记文件消失、收藏清除；导出副本 sha 前后一致 |
| A07 双输出整组删除 | **PASS** | 去背景作品删除后 primary 与 mask 均 `deleted` 且文件消失；输入源图（inputs/）保留；导出的 primary/mask 副本保留 |
| A08 引用阻止删除 | **PASS** | `native-A08-conflict-error-1280.png`：草稿引用时确认被拒，提示“仍被以下位置用作源图：「验收-停止测试」待发送草稿仍把该作品作为源图”；解除（删除来源会话）后同一作品可删 |
| A09 已删除作品卡 | **PASS** | `native-A09-work-deleted-card-1280.png`：会话卡显示“作品已删除/图片及配套产物已移除；原制作记录仍可查看”，无查看/导出/重新加载 |
| A10 迟到响应/并发 | **PASS（自动化 + 代码防护）** | 前端旧响应过滤/查询序号 + Rust 串行互斥；原生未做压力交错 |
| S01 v5→v6 升级 | **PASS** | 真实 v5 库（23 会话/34 资产）首启升级 v6，原 ID/资产/模型就绪不变；重复启动不重复迁移 |
| S02 事务/中断边界 | **PASS（自动化）** | 事务/回滚/崩溃恢复测试；原生未注入 kill |
| S03 删除中途强杀 | **NOT RUN（自动化覆盖）** | `interrupted_cleanup_resumes_from_the_persisted_intent_on_restart` 等等价覆盖；原生未在清理窗口内强杀 |
| S04 权限/占用失败 | **PASS（自动化）** | `an_unwritable_asset_directory_keeps_cleanup_failed_until_permissions_return`；原生未制造 |
| S05 非法/链接/越界目标 | **PASS（自动化）** | `cleanup_refuses_a_link_replacement_and_keeps_the_link_target`、mask/未知 ID 拒绝 |
| S06 读取/导出与删除互斥 | **PASS（自动化）** | 单库 Mutex 串行 + pending 读取/导出拒绝测试 |
| S07 旧源图重试 | **PASS（自动化）** | `deleted_works_cannot_be_reused_as_sources_or_exported` |
| E 真实文生图 | **PASS** | Z-Image Turbo 4:3 1024×768，54.6 s，结果卡完成并可预览/导出 |
| E 真实去背景 | **PASS** | BiRefNet CPU 1024×768，12.3 s，输出透明 PNG + 8-bit 蒙版；两件一起删除并核对 |
| E 停止/取消 | **PASS** | 第二轮文生图运行 7.8 s 后停止：卡片“已取消/本次生成已取消”，DB attempt `cancelled` |
| E 导出 | **PASS** | 文生图 PNG、去背景 primary、mask 均通过原生保存对话框导出：`/tmp/imagestudio-acceptance-text2img.png`(901351 B)、`/tmp/acceptance-birefnet-primary.png`(1052164 B)、`/tmp/acceptance-birefnet-mask.png`(34129 B) |
| E 第二次真实文生图（引用测试） | **PASS** | 43.3 s，完成后用作 Klein 编辑源图并触发 A08 引用拒绝 |

## 验收中发现的问题

1. **[已修复] 新命令缺少 Tauri capability**：`conversation_delete` / `asset_set_favorite` /
   `asset_delete` 未登记 build manifest 与窗口权限，运行时会被 ACL 拒绝。修复 commit `58dbec3`，
   并新增 `apps/desktop/tests/capabilities.rs` 防回归。
2. **[既有缺陷，未修] capabilityDrafts 中毒**：4:3 会话切换到仅支持 1:1 的 Klein 后，槽位以 4:3
   保存，宿主校验拒绝后续所有 `draft_save`（新建会话、发送前 flush 全部失败，只能重启恢复）。
   验收中用“先把画幅改 1:1 再切模型”规避。已建 `image-studio-31x`，需走 OpenSpec 提案后修复。
3. **[观察] 空指令作品的删除确认标题为「」**：BiRefNet 作品 prompt 为空时确认框标题
   “删除作品「」？”；范围说明完整，未在本轮修改。

## 数据状态（验收结束时）

- 库 v6；24 个可见会话、4 个已删除会话（均为本轮测试创建）；31 个可用资产、7 个已删除资产
  （3 件用户确认由其本人删除的历史作品 + 4 件本轮测试生成物）；0 个 failed。
- 升级前 v5 库备份保留：`target/acceptance-20261111-v5/library.sqlite3`。
- 真实模型与运行时目录未改动；导入源图目录保留。
- 验收期间生成并删除的 4 件测试作品与 4 个测试会话均不影响原始历史作品（用户已确认 3 件被删作
  品为其本人操作，无需恢复）。

## 未覆盖

- **Windows 全部 UI/文件项**：本机无 Windows 目标机，未验收；不得以 macOS 结果替代。
- 原生纯 Tab 遍历顺序、IME 组合输入、`S03` 清理窗口强杀、`A10` 高频并发交错。
- 960×640 下仅检查了设置/关于页；工作区 960×640 布局由浏览器 smoke 与前端布局约束覆盖。

## 证据文件

截图（本目录）：`native-01-initial-1280.png`、`native-U02-model-1280.png`、
`native-U05-inspiration-1280.png`、`native-U05-about-license-1280.png`、`native-U06-about-960.png`、
`native-U07-record-expanded-1280.png`、`native-C02-confirm-dialog-1280.png`、
`native-C05-busy-disabled-1280.png`、`native-C07-retained-work-1280.png`、
`native-C07-delete-session-dialog-1280.png`、`native-A01-works-1280.png`、
`native-A03-favorites-filter-1280.png`、`native-A05-no-favorites-1280.png`、
`native-A08-conflict-error-1280.png`、`native-A09-work-deleted-card-1280.png`、
`native-E1-text2img-done-1280.png`、`native-E2-birefnet-done-1280.png`、
`native-E2-cancel-1280.png`，以及 `native-U07-conversation-1280.png` 等过程截图。
自动化证据见同目录 `AUTOMATED-RESULTS.md`。
