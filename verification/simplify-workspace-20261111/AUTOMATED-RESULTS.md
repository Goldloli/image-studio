# 自动化验收证据（implemented）

日期：2026-11-11（Asia/Shanghai）。范围：OpenSpec change `simplify-workspace-and-manage-library`。
本文件只记录**已自动执行**的验收；原生 UI 与真实模型项见同目录 `CHECKLIST.md`，结果待写入 `RESULTS.md`。

## 质量门

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 |
| `cargo test --workspace --locked` | 326 passed / 0 failed（含新增 1 项 capability 守卫测试与 3 项删除失败路径测试） |
| `npm --prefix apps/frontend test` | 67 passed（8 文件） |
| `npm --prefix apps/frontend run check` | 通过 |
| `npm --prefix apps/frontend run build` | 通过（仅既有 >500 kB chunk 提示） |
| `openspec validate simplify-workspace-and-manage-library --strict` | 通过 |
| `openspec validate --all --strict` | 20 passed / 4 failed —— 4 项均为**既有**正式规格 >500 字符警告（`image-studio-6g2` 跟踪），非本 change 引入 |

## 本轮新发现的运行期缺陷（已修复）

`conversation_delete`、`asset_set_favorite`、`asset_delete` 已加入 `generate_handler!`，但没有登记到
`apps/desktop/build.rs` 的 Tauri manifest，也没有加入 `apps/desktop/capabilities/main.json` 的窗口权限。
Rust 编译与单元测试均通过，但桌面 WebView 调用会被 Tauri ACL 拒绝——体检时发现的验收阻塞问题。
修复：两处补登记 + 新增集成测试 `apps/desktop/tests/capabilities.rs` 交叉校验
`generate_handler!` ↔ build.rs ↔ capability 权限，后续同类遗漏会直接导致测试失败（commit `58dbec3`）。
修复后生成的 `acl-manifests.json` / `desktop-schema.json` 已包含三个 `allow-*` 权限；dev 包已重新构建。

## test-plan 覆盖映射

| test-plan | 自动化证据 |
| --- | --- |
| U01/U02/U04/U05 | 组件渲染链已删除（`FeatureLibrary.tsx`、`LibraryPages.tsx` 不存在），前端 check/build 与状态单测通过；DOM 级交互仍需人工 U 项 |
| U03 | 诊断入口/抽屉/轮询删除后 `npm test/check/build` 通过，宿主诊断命令保留；人工确认无入口 |
| U06 | **未自动化**（原生窗口/主题/DPI） |
| U07 | `production-record.test.ts`（3 项）+ `cargo test` 中 `task_center_detail` 复用路径；人工确认原位展开 |
| C01/C02 | 交互组件无 DOM 测试；行状态与确认流程由 `workspace-state.test.ts` 的 `remove` 行为覆盖，人工确认 hover/focus 与 Escape |
| C03/C04/C06 | `conversation_deletion_retires_unsent_state_and_selects_a_visible_successor`、`deleting_the_last_conversation_creates_exactly_one_default_conversation`、`conversation_deletion_requires_the_confirmed_draft_version`、`conversation_delete_through_the_service_returns_the_successor_and_replays_safely` |
| C05 | `a_busy_conversation_is_refused_while_idle_conversations_stay_deletable`、`a_conversation_with_a_bound_active_job_cannot_be_deleted` |
| C07 | `deleting_the_source_conversation_keeps_retained_works_with_a_tombstone_marker` + 作品查询 `conversationDeleted`/facet `deleted` |
| A01/A02 | `asset_favorites_are_explicit_persisted_and_queryable_beyond_the_loaded_page`（跨页、重启、交集）、`asset_favorite_rejects_unknown_and_non_work_targets`；交互按钮人工确认 |
| A03/A04 | 同上（`limit=1` 下只收藏较早作品、`width` 交集）；`library-state.test.ts` 收藏筛选/交集 |
| A05 | 空态分支由前端状态实现；人工确认四种空态 |
| A06 | `confirmed_work_deletion_removes_only_registered_files_and_tolerates_missing_ones`、`asset_deletion_state_is_visible_retryable_and_never_fakes_success` |
| A07 | `a_two_output_work_is_deleted_as_one_unit`（primary+mask 同组、旧 ID 读取失败） |
| A08 | `works_referenced_by_unsent_or_active_requests_cannot_be_deleted`（草稿与活动请求两种引用，解除后可删） |
| A09 | 删除后 `asset()` 为 deleted、快照/卡片字段 `assetDeletionState`；人工确认卡片文案 |
| A10 | `applyConfirmed` 旧响应过滤（代码）+ 查询序号丢弃；人工交错操作确认 |
| S01 | `v5_to_v6_attaches_user_state_without_rewriting_execution_evidence`、`v5_migration_is_idempotent`、`migration_is_rejected_for_an_unexpected_intermediate_version`、旧版本链测试 |
| S02 | 事务/冲突测试：`conversation_deletion_requires_the_confirmed_draft_version`、`database_asset_failure_rolls_back_manifest_and_asset_together`、`real_process_exit_at_all_commit_boundaries_recovers_exactly_once` |
| S03 | `interrupted_cleanup_resumes_from_the_persisted_intent_on_restart` |
| S04 | `unrecoverable_cleanup_stays_failed_and_visible_after_restart`、`an_unwritable_asset_directory_keeps_cleanup_failed_until_permissions_return` |
| S05 | `cleanup_refuses_a_link_replacement_and_keeps_the_link_target`、`asset_deletion_group(mask)` 拒绝、未知 ID 报错 |
| S06 | 单库互斥串行裁决 + `pending_cleanup_blocks_external_reads_and_exports_before_success`；未做并发线程级压测 |
| S07 | `deleted_works_cannot_be_reused_as_sources_or_exported`（resolve/read/snapshot/export 全部拒绝） |

## 尚缺（必须人工/真实环境）

- 原生 macOS/Windows UI：窗口按钮、拖动、960×640、亮暗主题、键盘焦点、hover/focus 发现性。
- 真实文生图与去背景的执行、停止/重试、导出、删除后的文件与外部副本核对。
- Windows 目标机全部 UI 项（本机不可用）。

## 浏览器预览 smoke（无头，非原生验收）

2026-11-11 用 Vite 预览 + 无头 Chromium 在 1280×800 下检查已删除/保留的关键界面，确认没有运行时崩溃
（未启动 Tauri GUI，不替代原生验收）：

- 左侧导航只剩 创作/灵感库/模型/作品 + 主题/关于/设置；**无** 功能、任务入口，**无** 开发/诊断入口，
  页面无顶部页签；空会话中央三张示例卡存在，侧栏无“示例灵感”。
- 灵感库：无“来源与内容许可”折叠区与通用许可段落；案例带“上游示例/需要参考图”等条件标识。
- 设置 → 关于：存在“灵感库来源与许可”，含固定上游 commit `63e6176f` 的 MIT 许可、第三方声明与仓库链接。
- 作品页：渲染“全部作品 / 仅收藏”筛选；浏览器预览显示“作品库仅在桌面应用中读取本地资产”（预期）；
  网格/列表按钮可达。
- 切换深色主题成功（`data-theme=dark`），全程 `window.onerror`/`unhandledrejection` 为 0 条。

截图：`browser-works-1280-light.png`、`browser-create-1280-dark.png`。
仍缺：原生窗口按钮/拖动、960×640、键盘焦点、hover/focus 发现性、真实生成与文件核验（见 `CHECKLIST.md`）。
