# Tasks: harden-windows-data-root-paths

## 1. 暂存目录缩短与残留清理

- [ ] 1.1 新增 `staging_runtime_dir_name()` 纯函数：`.staging-{pid}-{8hex}`，替换 `installation.rs` 中旧的 `.{runtime_id}.staging-{pid}-{nanos}` 构造；保留同卷 rename 激活语义。
- [ ] 1.2 单元测试：暂存名长度 ≤ 26、格式匹配、不同进程/随机值不冲突；现有 `activate_staged_runtime` 测试改用新命名后仍通过。
- [ ] 1.3 新增 `cleanup_stale_runtime_staging(runtimes_dir)`：仅删除 `.staging-` 前缀普通目录，拒绝符号链接/联接点，失败只记警告；在创建新 staging 前调用。
- [ ] 1.4 单元测试：清理命中 `.staging-*`、不碰正式运行时目录、链接不跟随、清理失败不影响安装主流程。

## 2. 启动前数据根路径校验

- [ ] 2.1 在 `AppPaths::resolve` 增加 Windows data 路径长度校验（UTF-16 code units），常量 `MAX_DATA_PATH_UNITS=117` 与预算注释（运行时布局实测 + 20 字符余量）。
- [ ] 2.2 超限错误带可操作建议（中文：改用更短 `--data-dir`，示例 `C:\ImageStudioData`）；host 启动路径复用 `fail_startup` 写 `startup-failure.log` + 弹窗 + 退出码 3。
- [ ] 2.3 单元测试：边界 limit-1 通过 / limit 拒绝；中文/非 ASCII 路径按 UTF-16 计数；macOS 行为不变（cfg 隔离）。

## 3. 安装/启动路径回归与文档

- [ ] 3.1 回归：短数据根（`C:\ist-a`）安装/探针/生成行为不变；运行时激活测试、安装器现有测试全绿。
- [ ] 3.2 文档：README 数据目录说明补充路径长度边界与短目录建议；更新 `verification/windows-parity-app/not-run.json` 中 long-root 行的闭环状态（指向新证据）。
- [ ] 3.3 全量质量门：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --locked`、前端 test/check/build、`openspec validate --all --strict`。

## 4. 安装包与目标机复验

- [ ] 4.1 重建 NSIS 内部安装包（commit 记入 internal-package.json），安装到目标机。
- [ ] 4.2 深数据根复验：用本地校验缓存种子在复现原失败的 ~83 字符数据根上安装共享 v2 型号（BiRefNet 或 Klein），确认运行时安装 + probe 成功、无长路径 DLL 错误。
- [ ] 4.3 超限数据根拒绝：用 >117 字符数据路径启动，确认 `startup-failure.log` 文案、退出码 3、无下载/无 staging 目录、对话框截图。
- [ ] 4.4 残留清理复验：在 `runtimes/` 放置旧 `.staging-*` 目录，确认下一次安装开始前被回收并记录。
- [ ] 4.5 证据落盘：`verification/windows-data-root-guard/`（JSON + 日志 + 截图），README 汇总通过/未跑行。

## 5. 归档

- [ ] 5.1 `openspec validate harden-windows-data-root-paths --strict` 通过后归档 change，同步 `windows-desktop` 与 `model-installation` 主规格。
- [ ] 5.2 关闭 bd `image-studio-slk`，记录归档与证据链接。
