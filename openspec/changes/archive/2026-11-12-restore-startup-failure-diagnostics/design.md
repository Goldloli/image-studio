# Design

## Context

`std::panic::set_hook` 全局只有一个槽位。仓库里有两处安装钩子：

- `startup::install_panic_hook`（`apps/desktop/src/startup.rs:58`）：识别 Tauri setup 失败，
  写 `startup-failure.log`、弹原生对话框、以 3 退出；其他 panic 交给安装前的钩子。
- `HostState::new`（`apps/desktop/src/host.rs:53`）：把 panic 以 `app.panic` 写进 `app.log`。

调用顺序是 startup → logging，而后者直接覆盖，导致前者在 WebView2/profile 失败路径上永不触发。
这条路径正是 `startup.rs` 的注释里描述的“release 构建隐藏控制台，只剩 101”的场景。

## Goals / Non-Goals

**目标**

- 两个钩子同时生效，顺序无关。
- setup panic 仍然以 3 退出，并保留 `app.panic` 日志行，便于排查。
- 其他 panic 行为不变（默认钩子仍打印回溯），只多一条日志。

**非目标**

- 不引入第三方 panic hook 库，不改退出码与诊断文案。
- 不改变数据根所有权、单实例行为或 WebView2 profile 布局。

## Decisions

### 1. 日志钩子包装而非覆盖

`adapters::logging::install_panic_hook` 用 `std::panic::take_hook()` 取出当前钩子，安装一个
“先 `warn("app.panic", …)`，再调用取出的钩子”的闭包。`HostState::new` 改为调用它。

行为矩阵：

| 场景 | 链式调用 | 结果 |
|---|---|---|
| Tauri setup 失败（WebView2/profile） | logging → startup | 一条 `app.panic` 日志，随后写 `startup-failure.log`、弹窗、退出 3 |
| 超长数据目录 | 不经过 panic（显式分支） | 与今天一致：日志 + 弹窗 + 退出 3 |
| 其它 panic | logging → startup → 默认 | 日志行 + 默认回溯，退出码不变 |

### 2. 保持安装顺序无关

包装发生在 `HostState::new`，而 startup 钩子在此之前安装；如果将来有人先建 `HostState` 再装
startup 钩子，startup 的 `install_panic_hook` 自身也会 `take_hook` 并转发，链式结构仍然成立
（startup → logging → 默认），因此不需要额外的顺序断言。

### 备选方案

- **把 startup 钩子改到最后安装**：只改一行调用顺序，能修当前现象，但把正确性交给调用顺序，
  下一次有人加钩子会再次覆盖，属于同一类缺陷；不采用。
- **合并成一个钩子模块**：更彻底，但会把日志格式、对话框文案和退出码耦合进同一处，超出本次
  缺陷修复范围；留待需要第三条诊断时再重构。
- **子进程回归测试**：`fail_startup` 会弹模态对话框，自动化测试需要点击或注入隐藏开关；
  为测试增加产品环境变量不值得。验收沿用本机真实复现（见 `test-plan.md`），与 W07 的既有做法一致。

## Risks / Trade-offs

- **重复日志**：setup panic 会先写 `app.panic` 再写 `startup-failure.log`，两条记录内容相关但用途不同
  （运行日志 vs 用户可读报告），可接受。
- **静默钩子异常**：`warn` 内部若 panic 会递归；日志写入已用 `if let Ok`/`is_ok` 处理 IO 失败，
  且钩子只做字符串拼接，风险与现状相同。
- **退出码竞争**：`fail_startup` 在转发链中直接 `std::process::exit(3)`，不会继续落到默认钩子，
  因此不会出现先打印回溯再退出的混合输出。
