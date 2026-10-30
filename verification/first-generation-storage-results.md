# 首个生成闭环：存储与核心契约定向验收

验收日期：2026-10-30（Asia/Shanghai）。对应已批准 change `deliver-first-local-generation`，Beads `image-studio-b88.3`。

本报告只证明本地库、执行身份、文件提交和恢复行为。所有图片均为测试生成的纯色 PNG fixture，数据根使用临时目录；这些测试不是 MFLUX 或模型实机出图证据。

## 已通过门禁

| 命令 | 结果 |
| --- | --- |
| `cargo test --locked --offline -p image-studio-desktop adapters::library -- --nocapture` | 21 项通过，0 失败；包含子进程崩溃 fixture 入口 |
| `cargo test --locked --offline -p studio-core` | 5 项单元、17 项契约、1 项 compile-fail doctest 通过 |
| `rustfmt --edition 2024 --check crates/studio-core/src/generation.rs apps/desktop/src/adapters/library.rs apps/desktop/src/adapters/library/assets.rs apps/desktop/src/adapters/library/tests.rs` | 通过 |

完整存储测试输出见 [测试日志](first-generation-storage-tests.log)。阶段性全桌面严格 clippy 检查在存储/核心文件未报告问题，其他模块仍在并行修复；最终全工作区门禁由主任务统一记录。

## 关键行为

- 先取得数据根宿主锁，再打开、检查或恢复数据库。同根第二实例不干扰已有任务。
- SQLite 使用 foreign keys、WAL、synchronous=FULL 和首版 schema；损坏、未知版本不静默清空。
- 草稿使用 CAS 版本保存。SQLite 有符号整数边界显式检查，不把超界 ID/版本或负数据库值转换为另一条记录。
- 通过索引按项目提交键查询完整历史；总数按 Job 当前 Attempt 计算，显式重试不增加 Job 数。查询与计数不受首 100 条历史限制。
- DB 接受后若无法准备 scratch，记录 `STORAGE_PREPARE` 失败；没有 worker 启动，也不留下永久占用的 Submitted。
- 候选只接受当前 Attempt 的普通 `result.png`，拒绝链接、FIFO、越界、大小或 SHA-256 不符、损坏或巨型 IHDR；先核对头部尺寸，再受限完整解码。
- 文件复制、验证、同步和移动之后，数据库同事务关联 AssetRef、RunManifest 与成功终态。取消、成功、失败的竞争按提交顺序裁决；旧 Attempt 无法改写当前执行。
- 五个边界使用独立子进程直接 `exit(86)` 跳过析构：复制后、同步后、移动后、数据库提交前、数据库提交后。前四者恢复为 Interrupted 且不提升孤儿；最后者恢复唯一成功结果。
- 恢复不删除旧 runs；暂存目录只有确认回收且终态后才可显式清理。
- 101 个真实存储提交后，旧选中资产仍能跨重启解析；任务分页仍是 100 条，不用扩大任务页面掩盖分页边界。
- 轻量状态查询采用最多 512 条、按 Unix inode/ctime/mtime/大小失效的完整性结果缓存。URI 读取与导出对实际发送字节重新散列；同大小文件替换后可用性变为异常，历史 manifest 不改写。
- 原生对话框给定的导出目标必须位于全部管理目录之外；中文/空格路径、覆盖拒绝、目录或链接目标均已检查。已有成功记录及内部图片不因导出失败改变。

没有执行 git commit、push 或远程同步。
