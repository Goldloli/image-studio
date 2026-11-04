# Design: harden-windows-data-root-paths

## 背景与实测

- 当前共享 v2 运行时 id：`windows-python31210-torch2100-cu126-diff041-transformers53-v2`（60 字符）。
- 在 `C:\ist-a` 数据根下，最终运行时目录路径长 84；最长文件相对路径 137（torch 头文件，Rust `\\?\` 解压可处理）；**装载相关**最长相对路径为 86（numpy openblas DLL），onnxruntime pybind11 pyd 相对路径 66。
- 旧暂存目录名 `.{runtime_id}.staging-{pid}-{nanos}` 约 95 字符，比正式目录名多约 35 字符；深数据根（约 83 字符）下 staging pyd 路径达到 252 附近，onnxruntime DLL 导入失败。
- 失败安装留下了整个 staging 树（4.23 GB / 20,621 文件），没有被自动回收。

## 决策

### D1 缩短暂存目录名（主修复）

暂存目录统一改为 `data/runtimes/.staging-{pid}-{8位十六进制随机}`：

- 最长 9 + 7 + 1 + 8 = 25 字符（预算 26）；
- 保留"创建后原子 rename 到正式运行时目录"的激活语义，避免并发/半成品目录；
- 暂存目录只在安装线程内创建；激活失败时按现有路径清理，成功 rename 后不存在；
- 新增 `staging_runtime_dir_name()` 纯函数与长度断言，避免未来回归；
- 正式运行时目录名保持不变（身份冻结，不能为路径长度重命名）。

### D2 启动前数据根路径校验（防御"仍然太长"的目录）

在 `AppPaths::resolve` canonicalize 之后、创建/探测子目录之前（或在 Windows 分支内）计算：

```
required = data_path_len + len(r"\runtimes\") + STAGING_NAME_BUDGET(26) + 1 + LOAD_RELATIVE_BUDGET(86)
```

约束 `required <= 240`（在 Win32 260 限制上留 20 字符余量，兼容临时文件名/短后缀）。等价于 data 路径 ≤ 117 字符（如 `C:\ist-a\data` 13 字符、默认 `%LOCALAPPDATA%\ImageStudio\data` 49 字符、此前失败的 ~88 字符都通过；115+ 字符的数据根拒绝）。

- 超限时返回带具体长度和建议的 `PlatformError`；host 启动路径捕获后复用现有启动失败机制：写 `<data root>/logs/startup-failure.log`、弹出中文对话框、退出码 3（`STARTUP_FAILURE_EXIT_CODE`）。
- 错误文案示例：`数据目录路径过长（{len} 字符，Windows 运行时上限 {limit}）。请把数据目录移到更短的位置，例如 C:\ImageStudioData，或用 --data-dir 指定短目录。`
- 常量 `MAX_DATA_PATH_UNITS` 与预算值集中在 `paths.rs`，注释说明来源（运行时布局实测 + 20 字符余量），并有边界单元测试（limit-1 通过 / limit 拒绝）。
- 长度按 Windows UTF-16 code unit 计算（`OsStrExt::encode_wide().count()`），避免中文/emoji 路径低估。

### D3 安装前清理历史暂存目录

- 在创建新暂存目录前调用 `cleanup_stale_runtime_staging(data/runtimes)`：
  - 仅匹配名字以 `.staging-` 开头的目录；
  - `symlink_metadata` 确认为普通目录（拒绝符号链接/联接点），删除失败只记警告不阻塞安装；
  - 不触碰任何正式运行时目录。
- 该清理同时覆盖"崩溃/取消/失败后残留"的历史现场；不依赖进程退出钩子。

### D4 证据与验收

- 深数据根（复现原失败的 ~83 字符根）用本地校验缓存重新安装共享 v2 型号（BiRefNet 或 Klein），验证运行时安装 + probe 成功；记录 staging 目录名长度与最终路径。
- 超限数据根（> 限制）启动：记录 `startup-failure.log` 内容、退出码 3、UI 对话框文案；确认没有下载/解压/暂存目录。
- 失败安装的残留清理：模拟/复用已有 staging 目录，确认下一次安装开始前被回收。
- 证据目录 `verification/windows-data-root-guard/`：JSON + 日志 + 截图（对话框）。

## 风险与权衡

| 风险 | 处理 |
|---|---|
| 阈值误拒合法路径 | 用 240 上限 + 20 余量，允许 117 字符数据路径；文档给出短目录建议；边界单测锁定 |
| 缩短暂存名破坏激活语义 | 仍为同卷 `rename`；新增命名单测 + 现有 activate 测试覆盖 |
| 清理误删 | 仅匹配 `.staging-` 前缀且拒绝链接；清理失败只警告 |
| Python/onnxruntime 仍可能对特定目录敏感 | 启动前校验 + 深根复装实测；超限用户得到可操作提示而非 cryptic 错误 |

## 迁移影响

- 无数据迁移；旧 `.staging-*` 目录由 D3 回收。
- 运行时身份、模型清单、正式运行目录名不变；已安装数据根行为不变。
