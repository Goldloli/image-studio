# Tasks: fix-windows-data-root-review-findings

## 1. macOS 编译 blocker

- [x] 1.1 `host.rs` 的 `PlatformError` 导入与 `DataPathTooLong` 分支按 `cfg(windows)` 隔离；日志从被拒绝的 data 路径推导。commit `732fac9`。
- [x] 1.2 Windows 全 workspace 测试/clippy 全绿；macOS 编译由 cfg 结构保证，首次 macOS CI run 作为 user-gated 证据（本机无 macOS 工具链）。

## 2. 数据路径预算、短运行时目录与 canonical 校验

- [x] 2.1 `MAX_DATA_PATH_UNITS` 改为 94：`data + 10 + 15 (rt-<12 hex>) + 1 + 130 (最长 Python 可打开) + 10 <= 260`；编译期断言锁定预算。commit `e972da7`/`e4b6e46`。
- [x] 2.2 创建子目录前 absolute 预检 + canonicalize 后对 **data** 复检（strip `\\?\` 后按 UTF-16 计数）；cache 只存短 hash 文件名，不再用同一边界误拒。commit `e4b6e46`。
- [x] 2.3 单元测试：94 通过/95 拒绝、中文/surrogate pair 计数、verbatim 前缀 strip、canonical 超长别名拒绝。
- [x] 2.4 正式运行时目录改为 `rt-<12 hex>`（15 units）：`runtime_directory_name` 确定性/长度/唯一性测试；旧 `runtimes/<runtime_id>` 同卷迁移一次，冲突不覆盖；deep root 迁移后 ready 并成功生成。commit `e5150e5`。

## 3. 暂存目录清理

- [x] 3.1 `is_stale_runtime_staging_name` 同时匹配新格式与旧格式（`.` 前缀 + 最后 `.staging-` + 两段十六进制），非匹配目录不删。commit `e446074`。
- [x] 3.2 清理调用从 `prepare_python_runtime` 移到 `install()` 取得独占锁之后、空间预检/下载之前。
- [x] 3.3 单元测试：旧格式目录删除、正式目录保留、非目录跳过、缺失 runtimes 目录容忍。

## 4. 启动失败日志路径

- [x] 4.1 从 `DataPathTooLong.path` 推导 `<data 根>/logs`（默认根与 `--data-dir` 都正确），不再用 CWD 回退。commit `732fac9`。
- [x] 4.2 超限根启动后日志位于该根的 `logs/startup-failure.log`（data=127 实测，exit 3）。

## 5. 安装版复验与证据

- [x] 5.1 重建安装包：installer `3492a8c4…`（245,005,633 B），installed exe `72f38b61…`。
- [x] 5.2 深数据根（data 88）：旧 runtime 目录迁移为 `rt-7137668922e0`，Klein/BiRefNet ready，真实生成 Succeeded `asset-b4f090d2…`。
- [x] 5.3 新边界（data 94）：安装 + probe + 生成 Succeeded `asset-d085ab42…`；data=127 真实根启动 exit 3、日志落在该根。
- [x] 5.4 旧格式残留：root-a 真实 4.23 GB 旧 staging 在 Real-ESRGAN 安装开始时（空间预检前）回收；正式运行时目录保留。
- [x] 5.5 证据 `verification/windows-data-root-guard/review-fix/` + README/review 报告更新；188 desktop tests、fmt/clippy、安装包构建（含全 workspace/前端门）全绿。

## 6. 归档

- [x] 6.1 归档 change，同步 `windows-desktop` 与 `model-installation` 主规格。
- [x] 6.2 关闭 bd `image-studio-kf5`；更新 review 报告 F1–F6 状态。
