# 功能、作品与任务中心验收

验收日期：2026-10-30，Asia/Shanghai。对应 OpenSpec `improve-feature-works-and-tasks`；Beads `image-studio-0rc`。

## 范围与数据

- 原生验收使用 `target/conversation-validation` 的 APFS 写时复制副本 `target/feature-work-task-validation`，源验收根未修改。
- 副本首次打开时从 SQLite schema v2 迁移到 v3；旧任务保留，新增 `completed_at` 与作品/任务稳定排序索引。旧 v2 记录未保存精确完成时间时，UI 明确显示“时间未记录”，不使用推测值。
- 特征、作品和任务查询均由宿主只读 DTO 提供。页面没有读取 SQLite、manifest 路径或模型文件，也没有新增任务队列或第二套业务状态。

## 页面验收

| 项目 | 结果 |
|---|---|
| 功能目录 | 4 个能力；仅文生图显示“可开始”，图片编辑、去背景、放大显示“计划中”且无启动按钮 |
| 功能入口 | “在文生图中开始”返回现有会话并聚焦 composer，没有创建 Turn、Job 或资产 |
| 作品来源 | 只统计正式 `assets` 表；示例文件与未提交候选不计入 |
| 作品筛选 | 会话 A 筛选从 7 件缩小到 1 件，清除条件恢复；网格/列表状态和筛选在切页后保留 |
| 作品预览 | 显示 AssetRef、会话、型号、尺寸、种子、时间、导出和制作记录 |
| 任务查询 | 宿主侧检索“山间湖泊”得到 2 条；“失败”筛选为 0 时显示筛选空态，不冒充首次使用 |
| 任务详情 | 先显示来源、请求、Attempt、阶段、完成时间、错误/下一步；manifest 与最多 64 KiB 日志按需展开 |
| 历史归属 | 直接验收 Job 无可信 Conversation 时显示“历史生成 · 无可靠来源会话”，不绑定当前草稿 |
| 来源返回 | 会话 A 作品可定位来源 Turn 并恢复未发送输入与阅读区域；从作品查看器关闭返回任务/作品来源页 |
| 导航与布局 | 1280×800、960×640、浅色/深色均保留导航、主查询、主操作和详情；功能与任务主操作在 960 高度下可见 |

## 真实生成与导出

使用隔离副本中已安装的 Z-Image Turbo 执行一次真实中文生成：

| 字段 | 值 |
|---|---|
| Job | `job-06b32a3d1a354bbeb7117086bd7e6d1c` |
| Attempt | `attempt-60f468c99c1a4c1ea918788d021fdc68` |
| AssetRef | `asset-62c9489563c2495fa942c25723c0b1c6` |
| 提示词 | 夜色中的现代玻璃温室，暖金色灯光映在平静水面，远处薄雾与森林，细腻电影摄影，无文字 |
| 尺寸 / 种子 | 1024×1024 / 4242 |
| 创建 / 完成 | 2026-10-30 22:29 / 22:30 |
| PNG SHA-256 | `37817f53d36ab0f247cd7c414e9249fcf7e2839d6b87478d427367364d61a1cd` |

作品总数由 6 增至 7，任务总数由 9 增至 10，任务“已完成”由 6 增至 7。新资产在作品库与任务中心使用同一 AssetRef；任务详情恢复了真实创建/完成时间。

从作品查看器调用原生保存对话框，将 PNG 保存为 [real-generated-greenhouse-4242.png](feature-works-task-ui/real-generated-greenhouse-4242.png)。导出文件为 1024×1024，SHA-256 与 manifest 完全一致。可靠来源会话返回另用既有“会话验收 A”作品完成，因为本次直接验收 harness 没有创建 Conversation/Turn；页面正确将其归入“历史生成”，而不是猜测关联。

## 截图

1280×800：

- [功能浅色](feature-works-task-ui/native-feature-1280-light.jpg)、[功能深色](feature-works-task-ui/native-feature-1280-dark.jpg)、[计划功能](feature-works-task-ui/native-feature-planned-1280-light.jpg)
- [作品浅色](feature-works-task-ui/native-works-1280-light.jpg)、[作品预览](feature-works-task-ui/native-works-preview-1280-light.jpg)、[真实生成作品深色](feature-works-task-ui/native-works-real-generated-1280-dark.jpg)
- [任务浅色](feature-works-task-ui/native-tasks-1280-light.jpg)、[真实生成任务深色](feature-works-task-ui/native-tasks-real-generated-1280-dark.jpg)

960×640：

- [功能浅色](feature-works-task-ui/native-feature-960-light.jpg)、[功能深色](feature-works-task-ui/native-feature-960-dark.jpg)
- [作品浅色](feature-works-task-ui/native-works-960-light.jpg)、[作品深色](feature-works-task-ui/native-works-960-dark.jpg)
- [任务浅色](feature-works-task-ui/native-tasks-960-light.jpg)、[任务深色](feature-works-task-ui/native-tasks-960-dark.jpg)

截图来自原生 Tauri 窗口并通过辅助功能树操作。1280×800 截图实际为 2560×1600 Retina 像素，960×640 为 1920×1280；没有把窗口缩放冒充独立 DPR 认证。

## 质量门

| 命令 | 结果 |
|---|---|
| `npm run check` | 通过 |
| `npm test` | 5 个文件、23 项通过 |
| `npm run build` | 通过，Vite 生产构建完成 |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | 通过 |
| `cargo test --workspace --locked` | 162 项通过、0 失败、2 项仅需外部固定验收材料的测试忽略 |
| `openspec validate --all --strict` | 归档后 17 个主规格全部通过，无活动 change |
| `python3 scripts/check-offline-startup.py` | 网络对照 DNS 失败，Tauri 主 WebView 完成加载，进程存活；未安装模型或启动 worker |

离线启动记录见 [cherry-offline-startup.json](cherry-offline-startup.json)。

## 边界

- 计划中的图片编辑、去背景和放大没有执行按钮、worker 或假结果；第二个真实模型与编辑工具仍需独立 change。
- 本 change 不提供作品删除、重命名、批量导出、外部目录扫描、收藏/标签、后台队列或跨设备同步。
- 没有 commit、push 或远程 Beads 同步。
