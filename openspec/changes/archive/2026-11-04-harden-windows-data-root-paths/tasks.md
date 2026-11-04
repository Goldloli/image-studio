# Tasks: harden-windows-data-root-paths

## 1. 暂存目录缩短与残留清理

- [x] 1.1 新增 `staging_runtime_dir_name()` 纯函数：`.staging-{hex pid}-{8hex}`，替换旧的 `.{runtime_id}.staging-{pid}-{nanos}`；保留同卷 rename 激活语义。（commit d97b5d4）
- [x] 1.2 单元测试：暂存名长度 ≤ 26、格式、不同 pid/随机值不冲突；现有 `activate_staged_runtime` 测试通过。
- [x] 1.3 新增 `cleanup_stale_runtime_staging(runtimes_dir)`：仅删除 `.staging-` 前缀普通目录，拒绝符号链接/联接点，失败只记警告；在创建新 staging 前调用。
- [x] 1.4 单元测试：清理命中 `.staging-*`、不碰正式运行时目录、非目录项跳过并警告、缺失目录容忍。

## 2. 启动前数据根路径校验

- [x] 2.1 `AppPaths::resolve` 增加 Windows data 路径 UTF-16 长度校验，`MAX_DATA_PATH_UNITS=117`（260 − 20 余量 − 运行时布局预算），常量注释说明来源。
- [x] 2.2 超限错误带可操作建议；host 启动路径捕获后复用 `fail_startup` 写 `startup-failure.log` + 原生对话框 + 退出码 3。
- [x] 2.3 单元测试：边界 117 通过/118 拒绝、UTF-16 surrogate pair 计数、macOS 行为不变（cfg 隔离）。

## 3. 安装/启动路径回归与文档

- [x] 3.1 回归：短数据根 `C:\ist-a` 安装版启动后 6/6 型号 ready，真实 Klein 文生图成功；workspace 测试、安装器测试全绿。
- [x] 3.2 文档：README 与证据说明补充路径边界；`verification/windows-parity-app/not-run.json` 的 long-root 行标记 resolved 并指向新证据。
- [x] 3.3 全量质量门：`cargo fmt --check`、`cargo clippy --workspace --all-targets -- -D warnings`、`cargo test --workspace --locked`（183 desktop / 全 workspace 绿）、前端 test/check/build（build 脚本内）、`openspec validate --all --strict` 25/25。

## 4. 安装包与目标机复验

- [x] 4.1 重建 NSIS 安装包（installer SHA-256 `58406c92…`，245,015,583 B，installed exe `e1cd5fb3…`）。
- [x] 4.2 深数据根复验：`deep-root-b`（root 83 / data 88 units，复现原失败长度）安装共享 v2 运行时 + BiRefNet，`ready/executable`，probe `windows-cpu / Intel64 Family 6 Model 183`，无长路径 DLL 错误；onnxruntime pyd 全路径 225（原 staging pyd 252）。
- [x] 4.3 超限数据根拒绝：data 127 > 117 启动即退出码 3，`startup-failure.log` 含中文长度/上限/短目录建议，未创建 `data/` 或 `runtimes/`，对话框截图留存。
- [x] 4.4 残留清理复验：安装前预置 `.staging-legacy-acceptance`，安装后 `runtimes/` 仅剩正式运行时目录（`staging_dirs_remaining=0`）。
- [x] 4.5 证据落盘：`verification/windows-data-root-guard/`（results.json、ready record、seed、startup-failure.log、对话框截图、README）。

## 5. 归档

- [x] 5.1 `openspec validate harden-windows-data-root-paths --strict` 通过后归档 change，同步 `windows-desktop` 与 `model-installation` 主规格。
- [x] 5.2 关闭 bd `image-studio-slk`，记录归档与证据链接。
