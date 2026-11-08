# Test plan: fix-spawned-child-console-window

| ID | 项 | 判定 | 证据 |
|---|---|---|---|
| T1 | 单测：helper 创建标志组合 | 同时含 CREATE_SUSPENDED 与 CREATE_NO_WINDOW | `cargo test -p image-studio-desktop suspended_job_spawn_combines` |
| T2 | 行为级窗口测试不可行性 | 控制台测试进程会继承控制台；以产品级实测替代 | 设计 §4 + 证据 JSON |
| T3 | 产品内生成 | supervisor/python `MainWindowHandle=0`、标题空；无 app 归属新 conhost/OpenConsole/WindowsTerminal | `O5-console-window-current-build.json`、截图 |
| T4 | 产品内安装 | 重装 BiRefNet 期间无窗口，安装完成 | 同上 JSON |
| T5 | 回归 | desktop 全量测试 + forced_reap + fmt/clippy + openspec strict 全绿 | 构建日志 |

判定规则：不得用"进程存在但窗口不可见"的观察替代窗口句柄为 0 的实测；任何生成/安装行为变化都视为失败。
