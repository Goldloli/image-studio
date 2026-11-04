# Design: fix-windows-data-root-review-findings

## F1 macOS build blocker

`apps/desktop/src/adapters/paths.rs` 的 `DataPathTooLong` 变体是 `#[cfg(windows)]`；
`apps/desktop/src/host.rs` 的 match 分支与 `PlatformError` import 需要相同 cfg：

```rust
adapters::{
    AppPaths,
    #[cfg(windows)]
    PlatformError,
    ...
}
...
let paths = match AppPaths::resolve(options.data_dir.as_deref()) {
    Ok(paths) => paths,
    #[cfg(windows)]
    Err(error @ PlatformError::DataPathTooLong { ref path, .. }) => {
        let logs = path
            .parent()
            .map(|root| root.join("logs"))
            .unwrap_or_else(|| PathBuf::from("logs"));
        let _ = std::fs::create_dir_all(&logs);
        crate::startup::fail_startup(&logs, &error.to_string());
    }
    Err(error) => return Err(error.into()),
};
```

非 Windows 构建不再引用该变体；`--data-dir` 与默认根都通过 `path.parent()` 得到
`<root>/logs`（`path` 是 data 路径）。

## F2/F9 正确的数据路径预算与短运行时目录

最终运行时目录：`<data>/runtimes/rt-<12 hex>`（15 units；runtime 身份保留在激活记录
与 worker 身份中）。**实现阶段发现**：仅把预算从 117 降到 92 仍不够——深根 data=88 的
安装/probe 通过，但第一次真实生成时 Python 打开
`…\Lib\site-packages\transformers\models\audio_spectrogram_transformer\configuration_audio_spectrogram_transformer.py`
报 `No such file or directory`：该路径 = 88+10+61+1+103 > 260。运行时中最长的
**Python 可打开**相对路径是 130 units（diffusers 的 `.py`；torch 头文件 137 不会被
Python 打开），因此只按 86 的 DLL 预算不够。

修正后的布局与预算：

```
data + 10 (\runtimes\) + 15 (rt-<12 hex>) + 1 + 130 (最长 Python 可打开相对路径) + 10 (余量)
    = data + 166 <= 260
MAX_DATA_PATH_UNITS = 94
```

- 正式目录名由 `runtime_directory_name(runtime_id)` 生成：`rt-` + SHA-256(id) 前 12 hex
  （15 units，确定性；不同 runtime id 不同名）。旧构建按原始 id（52–61 units）命名的
  正式目录在 `new_for` 就绪检查时同卷 rename 迁移到短目录；短目录已存在时不覆盖、
  不删除旧数据。
- 校验发生在两处：
  1. 创建子目录之前，用 `std::path::absolute(data)` 做早失败（避免下载/解压）；
  2. canonicalize 之后，再按**去掉 `\\?\` 前缀**的 UTF-16 长度校验 data（runtime 树所在
     路径；cache 下只有短 hash 文件名，长 1 unit 不影响装载），防止 junction/8.3 短名把
     实际路径变长。
- 旧上限 117 会允许 data=117；仅按 DLL 预算的 92 会允许 data=92，但 Python 打开的最长
  120–130 units 文件仍会越界（实现阶段已在 data=88 复现）。最终方案 94 让边界根最长
  路径 = 94+156 = 250，保留 10 units 余量。
- 暂存目录名（≤26）仍短于正式目录（15+分隔），不是上界；清理逻辑不变。
- 单元测试：94 通过/95 拒绝；中文与 surrogate pair 按 UTF-16 计数；`\\?\` strip；
  `runtime_directory_name` 长度/确定性/唯一性；旧目录迁移一次、短目录存在时不覆盖。

## F3 canonical 校验

`canonicalize` 返回 `\\?\C:\…`（约 +4 units），但外部 Python/onnxruntime 使用的是去掉
verbatim 前缀的路径。因此定义：

```rust
fn external_path_units(path: &Path) -> usize  // strip \\?\ / \\?\UNC\ then encode_wide().count()
```

先按 absolute 预检，再对 canonical data/cache 用 `external_path_units` 复检；超限返回
同一 `DataPathTooLong`（错误文本给出数据根与建议）。

## F4/F5 暂存目录清理

匹配函数（只删除 `data/runtimes` 下、名字为普通目录的条目）：

```rust
fn is_stale_runtime_staging_name(name: &str) -> bool {
    if name.starts_with(".staging-") { return true; }        // new: .staging-<hexpid>-<8hex>
    if !name.starts_with('.') { return false; }               // old: .<runtime.id>.staging-<pid>-<nonce>
    let Some((_, suffix)) = name.rsplit_once(".staging-") else { return false };
    let Some((pid, nonce)) = suffix.split_once('-') else { return false };
    !pid.is_empty() && pid.len() <= 10 && pid.bytes().all(|b| b.is_ascii_hexdigit())
        && !nonce.is_empty() && nonce.len() <= 20 && nonce.bytes().all(|b| b.is_ascii_hexdigit())
}
```

旧格式的 pid/nanos 是十进制数字（十六进制子集），可安全匹配；`rsplit_once` 取最后一个
`.staging-`，避免 runtime id 中偶然出现同片段时误判。非匹配目录（正式运行时、任意用户
目录）不删除；符号链接/联接点只警告不跟随。

调用点从 `prepare_python_runtime` 移到 `install()`：

```rust
let lock = open_regular(&data.join("installation.lock"), true, false)?;
fs2::FileExt::try_lock_exclusive(&lock)…?;   // 已存在
#[cfg(windows)]
for warning in cleanup_stale_runtime_staging(&data.join("runtimes")) { eprintln!("{warning}"); }
// 之后才是磁盘预检、下载与 prepare_runtime
```

这样残留会在空间预检之前释放；清理处于独占安装锁内，不会误删并发安装的 staging
（review 已确认 install() 全程持有该锁）。

## F6 启动失败日志路径

见 F1 代码：`path.parent()` 来自 `DataPathTooLong.path`（绝对 data 路径），因此
`--data-dir` 与 `%LOCALAPPDATA%` 默认根都写到各自的 `logs/startup-failure.log`；
`create_dir_all` 失败不再被静默吞掉（`let _` 保留，但日志/对话框仍展示原因）。

## 验收设计

- 深数据根（data 88，原失败长度）：安装共享运行时 + 模型后完成一次真实生成（不是只有
  probe），证明最终短目录路径下 Python 可打开最长模块。
- 新上限边界（data 94，基于仓库路径构造）：安装共享运行时 + 一个模型并生成，确认
  边界通过；data 95 由单元测试覆盖，data >94 的真实根启动 exit 3。
- 旧格式残留：在 `<data>/runtimes/` 预置
  `.{runtime_id}.staging-<pid>-<nanos>` 目录（复刻 root-a 真实命名），确认安装开始时
  被回收（即使随后因预检/网络失败，也已释放）。
- 旧正式目录迁移：deep root 中保留旧 `runtimes/<runtime_id>` 目录，启动/安装后确认已
  迁移为 `rt-<12 hex>`，模型仍 ready，并完成真实生成。
- 证据：`verification/windows-data-root-guard/review-fix/`（results.json、截图、日志），
  README 增加 “review fix” 段并说明原 D4 只覆盖新格式、92 预算在生成阶段被推翻。

## 风险

| 风险 | 处理 |
|---|---|
| 降低上限到 94 后拒绝此前可工作的 95–117 数据根 | 这些根在最终路径上 unsafe；启动消息给出短目录建议，文档明确支持边界 |
| 旧格式匹配误删 | 要求 `.` 前缀 + 最后 `.staging-` + 两段十六进制；非匹配目录不动；有单测 |
| 旧正式运行目录迁移失败/冲突 | 同卷 rename、best effort；短目录已存在时不覆盖不删除；失败时 readiness 会给出明确不可用原因 |
| 清理提前到空间预检前可能与其它进程冲突 | 清理在独占 installation.lock 内；review 确认锁语义 |
| macOS 编译仅静态保证 | cfg 结构简单；首次 macOS CI run 作为 user-gated 验证，不在本机冒充 |
