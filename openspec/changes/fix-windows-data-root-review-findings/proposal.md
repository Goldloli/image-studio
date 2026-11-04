# Proposal: fix-windows-data-root-review-findings

## Why

对已归档的 `harden-windows-data-root-paths` 做独立只读 review（记录见
`verification/full-review-2026-11-04/README.md`）发现 6 个真实问题，其中两个会直接破坏
后续工作：

1. **macOS 编译 blocker**：`host.rs` 无条件匹配 Windows-only 的
   `PlatformError::DataPathTooLong`，macOS 构建/CI 第一跑就会 `E0599`。
2. **路径预算是错的**：守卫按 26 字符的**暂存目录**名推算 117 上限，但激活后目录名是
   61 字符的 runtime id；接近上限的数据根会安装/probe 成功，却在最终路径上生成时再次
   长路径失败。

其余：守卫只校验 canonicalize 之前的路径（junction/短名可绕过）；旧格式暂存目录
`.{runtime_id}.staging-{pid}-{nanos}` 永远不被回收（真实残留 4.23 GB，验收时却用新
格式名字测试）；清理发生在空间预检之后，残留可能让预检先失败；默认数据根的启动失败
日志写到 CWD 而不是数据根。

本 change 修正这些问题，并重做相应验收（旧格式残留回收、深数据根真实生成、新上限边界）。

## What Changes

- `host.rs`：把 `DataPathTooLong` 的 match 分支与 import 按 `cfg(windows)` 隔离，恢复
  macOS/其他平台编译。
- 路径预算改为按**最终**运行时目录计算：`data + "\runtimes\" + 61 (v2 runtime id) + 1
  + 86 (最长装载相关相对路径) + 10 (Win32 余量) = 250`，即 data 路径上限 **92** UTF-16
  units；保留暂存目录预算检查，并在 canonicalize 之后再次校验（去掉 `\\?\` 前缀按实际
  外部路径长度计算），防止 junction/短名绕过。
- 暂存目录清理：
  - 同时匹配新格式 `.staging-<hexpid>-<8hex>` 与旧格式
    `.{runtime_id}.staging-<hexpid>-<hexnonce>`（后缀为两段十六进制，旧十进制是其子集）；
  - 移到 `install()` 取得独占 `installation.lock` 之后、空间预检与下载之前执行。
- 启动失败日志：从被拒绝的 data 路径推导 `<data 根>/logs`，默认数据根也写到正确位置。
- 更新证据与文档：`verification/windows-data-root-guard/` 增加 review-fix 验收（旧格式
  残留、深数据根真实生成、92/93 边界、超限拒绝），README 说明原 D4 用例覆盖不足。

## 成功标准

- `cargo check`/workspace 测试在 Windows 上全绿；macOS 编译由 `cfg` 结构保证，首次
  macOS CI run 作为 user-gated 证据（本机无 macOS 工具链）。
- 单元测试：92 通过/93 拒绝、canonical 长度校验、旧/新暂存目录名识别、非暂存目录不误删。
- 安装版复验：深数据根（data 88）安装后能完成一次真实生成（不是只有 probe）；新上限
  边界（data 92）安装+probe+生成通过；data >92 在启动时 exit 3；预置旧格式
  `.{runtime_id}.staging-…` 目录在安装开始时被回收（并在预检之前）。
- 全量质量门与 `openspec validate --all --strict` 通过；重建安装包并更新证据。

## 不做 / 超出范围

- 不改变正式运行时目录名（身份冻结），不实现任意长路径支持。
- 不处理跨平台工具模型提示缺口（已建 bd `image-studio-5w0`，后续小改）。
- 不修改 CI/release 流水线的其余部分（review 修复已单独提交 `8d7c881`）。
