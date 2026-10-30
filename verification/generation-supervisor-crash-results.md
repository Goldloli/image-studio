# 原生监督异常退出回收验收

日期：2026-10-30。对应 Beads `image-studio-b88.6`，属于已批准 design §5 的实现补齐。本材料是进程故障验证，不是模型出图或性能证据。

原实现遇到监督进程本身被 SIGKILL、而计算进程及后代仍持有执行租约时，只会保持忙碌等待，缺少当前宿主的原生回收路径。

修复后，监督端在接受 Hello/Execute 前，在本次独占的 Attempt 目录发布 `compute-lifecycle.json`。文件采用 create-new 临时文件和原子 no-replace 发布，包含本次监督 PID 与新建计算进程组。宿主只在当前持有的 `Child` 异常退出后读取该文件，限制类型和大小、拒绝链接并核对当前 Child 身份。匹配后原生结束本次计算组，直到进程组消失、执行租约释放、通道结束后才提交失败或取消终态。

新的 Controller 不会读取旧 Attempt 的进程记录；重启仍只依赖租约阻止重叠执行，不凭历史 PID 发信号。身份无法核实且租约仍忙碌时保持清理异常，不谎报资源已经释放。wire 协议与原诊断均未修改。

验证结果：

- 原生监督：2 项单元测试、8 项进程测试通过。新增测试证明生命周期记录在 Execute 前可读，且不能由另一个 Child 句柄复用。
- 执行协调器：7 项默认故障/竞态测试通过。新增真实操作系统进程组测试覆盖持 GIL 的计算调用、后代、监督进程 SIGKILL，以及取消先被数据库接受的顺序。
- 显式集成验收：真实 Rust `inference-supervisor` 二进制被 SIGKILL 后，当前 Host 回收配置为 `ctypes.PyDLL(None).sleep`、不处理 EOF 的计算 fixture 及其后代。无取消时为 Failed，取消先接受时为 Cancelled，均不登记输出。该测试单独执行并通过 1 项；默认忽略是因为它需要明确提供已经构建的监督二进制路径。
- 两个相关包的 all-targets 严格 Clippy 通过；所改 Rust 文件已经 rustfmt。

复现：

```sh
cargo test -p inference-supervisor --locked --offline
cargo test -p image-studio-desktop adapters::generation::tests --lib --locked --offline
cargo build -p inference-supervisor --bin inference-supervisor --locked --offline
STUDIO_TEST_NATIVE_SUPERVISOR="$PWD/target/debug/inference-supervisor" cargo test -p image-studio-desktop adapters::generation::tests::actual_native_supervisor_sigkill_is_reclaimed_by_its_current_host --lib --locked --offline -- --ignored --exact
cargo clippy -p inference-supervisor -p image-studio-desktop --all-targets --locked --offline -- -D warnings
```

原始输出见 [测试日志](generation-supervisor-crash-tests.log)。完整模型的真实生成、GPU 取消和重启验收由主任务另行记录。
