# Tasks: fix-windows-data-root-review-findings

## 1. macOS 编译 blocker

- [ ] 1.1 `host.rs` 的 `PlatformError` 导入与 `DataPathTooLong` match 分支按 `cfg(windows)` 隔离；非 Windows 走通用错误返回。
- [ ] 1.2 `cargo clippy --workspace --all-targets` 与 Windows 测试全绿；macOS 编译由 cfg 结构保证，首次 macOS CI run 作为 user-gated 证据。

## 2. 数据路径预算与 canonical 校验

- [ ] 2.1 `MAX_DATA_PATH_UNITS` 改为 92，注释按最终布局推导（`data + 10 + 61 + 1 + 86 + 10 <= 260`）；保留暂存目录预算常量用于说明。
- [ ] 2.2 创建子目录前 absolute 预检 + canonicalize 后对 data/cache 复检（strip `\\?\` 后按 UTF-16 计数）；超限返回同一可操作错误。
- [ ] 2.3 单元测试：92 通过/93 拒绝、中文/surrogate pair 计数、verbatim 前缀 strip、canonical 复检路径。

## 3. 暂存目录清理

- [ ] 3.1 `is_stale_runtime_staging_name` 同时匹配新格式与旧格式（`.` 前缀 + 最后 `.staging-` + 两段十六进制），非匹配目录不删。
- [ ] 3.2 清理调用从 `prepare_python_runtime` 移到 `install()` 取得独占锁之后、空间预检/下载之前。
- [ ] 3.3 单元测试：旧格式目录删除、正式目录保留、非目录跳过、缺失 runtimes 目录容忍。

## 4. 启动失败日志路径

- [ ] 4.1 从 `DataPathTooLong.path` 推导 `<data 根>/logs`（默认根与 `--data-dir` 都正确），不再用 CWD 回退。
- [ ] 4.2 单元/手工验证：超限根启动后日志位于该根的 `logs/startup-failure.log`。

## 5. 安装版复验与证据

- [ ] 5.1 重建安装包并安装（记录 installer/installed exe SHA-256）。
- [ ] 5.2 深数据根（data 88，原失败长度）：安装共享 v2 运行时 + 模型后完成一次**真实生成**（不仅 probe）。
- [ ] 5.3 新边界（data 92）：安装 + probe + 生成通过；data >92 的真实根启动 exit 3、日志落在该根。
- [ ] 5.4 旧格式残留：预置 `.{runtime_id}.staging-<pid>-<nanos>` 目录，确认安装开始时（空间预检前）被回收。
- [ ] 5.5 更新 `verification/windows-data-root-guard/`（review-fix results + README 说明原 D4 覆盖不足 + 截图/日志）；全量质量门与 `openspec validate --all --strict` 通过。

## 6. 归档

- [ ] 6.1 归档 change，同步 `windows-desktop` 与 `model-installation` 主规格。
- [ ] 6.2 关闭 bd `image-studio-kf5`；更新 review 报告中的 F1–F6 状态。
