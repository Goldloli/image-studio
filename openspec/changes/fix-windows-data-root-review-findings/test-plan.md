# Test plan: fix-windows-data-root-review-findings

## 单元 / 编译

| ID | 场景 | 期望 |
|---|---|---|
| U1 | 非 Windows 编译 | `host.rs` 不再引用 Windows-only 变体；Windows 上 clippy/tests 全绿 |
| U2 | 数据路径边界 | data=92 通过；data=93 拒绝；中文/surrogate pair 按 UTF-16 计数 |
| U3 | canonical 复检 | verbatim 前缀 strip 后的长度参与校验；junction 别名不绕过 |
| U4 | 暂存名匹配 | 新格式 `.staging-2a-0123abcd` 与旧格式 `.windows-…v2.staging-27684-1793797083903590600` 都识别；`.runtime-config`、正式运行时目录不匹配 |
| U5 | 清理边界 | 旧/新残留删除；非目录项跳过并警告；缺失 runtimes 目录容忍 |

## 安装版（Windows 目标机）

| ID | 场景 | 期望 |
|---|---|---|
| A1 | data=88 深根（原失败长度） | 安装共享运行时 + 模型，probe ready，并在 UI 完成一次真实生成 |
| A2 | data=92 边界根 | 安装 + probe + 真实生成通过 |
| A3 | data>92 超限根 | 启动 exit 3，日志在 `<该根>/logs/startup-failure.log`，无 data/runtimes 创建 |
| A4 | 旧格式残留 | 预置 `.{runtime_id}.staging-<pid>-<nanos>`，安装开始时（空间预检前）被回收；正式运行时目录保留 |
| A5 | 短根回归 | `C:\ist-a` 6/6 模型 ready，生成正常 |

## user-gated

| ID | 场景 | 期望 |
|---|---|---|
| M1 | macOS 首次 CI/构建 | `cargo check/test` 在 macOS runner 通过（本机无 macOS 工具链，不能冒充） |

## 质量门

`cargo fmt/clippy/test --workspace`、前端 test/check/build、`openspec validate --all --strict`、重建安装包。
