# Test plan: support-8gb-vram-tier

## 环境

- 验收机：Windows 11 Enterprise 22631 / i9-9900K / RTX 2060 SUPER 8192 MiB（sm75）/ 31.9 GiB RAM。
- 12 GB 回归机：不具备（本机只有 8 GB）。standard 档只做代码级回归，真机回归标注 **user-gated**，不得冒充通过。

## 本机可验证项

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| T1 | 设备事实探测 | `vram_gb=8`、`compute_capability=7.5`、`ram_gb=32` | 单测 + `verification/windows-8gb-tier/device-facts.json` |
| T2 | 档位选择 | 8 GB → compact；12 GB → standard；未知 → 不可执行 | Rust 单测 |
| T3 | 目录投影 | 五个模型 `hardwareEligible=true`、`canInstall=true`，并标注 compact/768² | Rust 单测 + 应用截图 |
| T4 | 协议闭集 | compact 接受；多键/少键/错值、macOS 身份、standard 串档全部拒绝 | `cargo test -p worker-protocol -p studio-core` |
| T5 | prepare 低内存 | 可用内存 ≈ 19.6 GiB 下完成 prepare，无 OOM | prepare JSON 报告 + `nvidia-smi`/Get-Counter 采样 |
| T6 | Qwen Turbo Gate 0 | 768²、8 步成功；CUDA reserved ≤ 6.0 GiB；加载 ≤ 600 s；生成 ≤ 900 s | `qwen21-turbo-gate0.json` + 样图 + SHA-256 |
| T7 | MOSS Gate 0 | 768²、50 步、CFG 3.0 成功；峰值 ≤ 6.0 GiB | `moss-gate0.json` + 样图 |
| T8 | Qwen 基础版 Gate 0 | 768²、40 步成功；峰值 ≤ 6.0 GiB | `qwen21-base-gate0.json` + 样图 |
| T9 | Klein Gate 0 | 文生图与单图编辑各一次；峰值 ≤ 6.0 GiB；编辑非 img2img 伪装 | `klein-gate0.json` + 两张样图 |
| T10 | Z-Image Gate 0 | 768²、9 步成功；峰值 ≤ 6.0 GiB；或按 R1 停止并回报 | `zimage-gate0.json` 或 blocker 记录 |
| T11 | 共享内存回退检查 | 每个 Gate 0 期间 `nvidia-smi` 专用显存占用不超过总量，且 CUDA reserved 与 nvidia-smi 一致 | 采样日志 |
| T12 | 取消 | 五个模型至少各一次采样中取消；任务 Cancelled，无残留 worker 进程 | 任务 JSON + 进程快照 |
| T13 | 强杀恢复 | 强杀宿主/worker 后重启，无半成品资产，可再次生成 | 恢复记录 + 资产目录快照 |
| T14 | 界面发送/查看/导出 | 五个模型从界面各发送一次，查看结果并导出 PNG | 截图 + 导出文件 |
| T15 | 重启后 Ready | 重启应用后不重新下载/prepare，模型仍可执行 | Ready 记录 + 截图 |
| T16 | 空间预算 | 安装前预检正确拒绝空间不足；实际占用与预算一致 | 安装 JSON |
| T17 | 12 GB 档代码级回归 | standard 配方参数映射、身份、预算常量与改动前逐字一致 | 快照测试 |
| T18 | 质量门 | fmt/clippy/workspace tests/前端 test/check/build 全绿 | 日志 |
| T19 | 规格门 | `openspec validate --all --strict` 全绿 | 日志 |
| T20 | 既有 8 GB 行为 | Real-ESRGAN 仍可运行、BiRefNet CPU 路线不变 | 复用既有证据 + 一次复跑 |

## 判定规则

- 峰值以 `torch.cuda.memory_reserved()` 与 `nvidia-smi` 采样同时记录；两者不一致时以更高者为准，并查明原因。
- 出现 WDDM 共享内存回退即判失败，不接受"能用但溢出"。
- 画质判定：样图必须与提示词语义一致，非全黑、非 NaN、无非自然噪点；不要求与 1024² 逐像素一致。
- 单模型 768² 超预算时允许降到 512² 重测一次；512² 仍超则该模型在 8 GB 档标记不可执行并如实记录。
- 任何未执行的项必须标记 `not_run` / `user-gated`，不得写成通过。

## 用户侧（12 GB 回归，本机不可执行）

| ID | 场景 | 期望 |
|---|---|---|
| U1 | 12 GB 机器安装同一构建 | 目录自动选 standard 档，画幅 1024²，行为与改动前一致 |
| U2 | 12 GB 机器真实生成 | 五模型输出与既有 Gate 0 证据可比，峰值仍 ≤ 10.5 GiB |
