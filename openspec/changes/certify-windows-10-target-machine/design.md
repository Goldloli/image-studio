# Design: certify-windows-10-target-machine

本 change 是**验收/certification 轮**，不是功能 change。设计目标是：在唯一一台 Windows 10 / 12 GB 目标机上，用当前主分支构建取得可复查的产品级证据，并把"哪些没跑、为什么没跑"写清楚，而不是把 8 GB 参考机或 CI 的结果外推。

## 1. 本机事实（2026-11-08 实测）

| 项 | 值 | 来源 |
|---|---|---|
| OS | Windows 10 Pro 22H2，build 19045，x64 | `Win32_OperatingSystem` |
| CPU / RAM | i7-13700K（16C/24T）；31.7 GiB 可见物理内存 | `Win32_Processor` / `Win32_ComputerSystem` |
| GPU | NVIDIA RTX 3080 Ti，**12,288 MiB**，sm86，驱动 596.49，CUDA 13.2 | `nvidia-smi`；另见 `verification/windows-gate0/README.md` 的 GPU UUID |
| 显示 | 3840×2160 @59 Hz，OS 缩放 **150%**（AppliedDPI=144） | `Win32_VideoController` + 注册表 |
| WebView2 | 157.0.4341.42 | EdgeUpdate 注册表 |
| 页面文件 | 系统自动管理，当前 **4.8 GiB**；commit 上限 **36.5 GiB**，已提交 14.8 GiB，**余量 21.7 GiB**；可用物理内存 **20.0 GiB** | `Get-Counter '\Memory\Commit Limit'` / `Committed Bytes` / `Available Bytes` |
| 工具链 | Node 24.19.0 / npm 11.17.0 / pnpm 11.28.10 / rustc+cargo 1.99.1 / Python 3.14.7 / git 2.55.0 | 与 CI pin（Node 24.19.0、npm 11.17.0）一致 |
| 磁盘 | C: 1,087 GiB 空闲；D: 283 GiB；E: 298 GiB；G: 207 GiB | `Win32_LogicalDisk` |
| 网络 | `github.com`、`modelscope.cn`、`hf-mirror.com` 可达；**`huggingface.co:443` 不可达**（模型正式源为 ModelScope，不受影响） | `Test-NetConnection` |
| 已安装应用 | `%LOCALAPPDATA%\Image Studio\image-studio-desktop.exe`，v0.1.0，**2026-11-04 23:57** 构建，不含 11-06 修复 | 文件时间戳 |
| 隔离数据根 | `C:\ist-a`：cache 104.9 GiB / models 6 个 + 3 份 prepared / runtimes 2 份 / 就绪记录 6 份（2026-11-04） | `C:\ist-a` 目录与 `data/installations/models/*.json` |
| 默认数据根 | `%LOCALAPPDATA%\ImageStudio`：仅 prompts/WebView2 缓存，无模型（不使用） | 目录检查 |
| CI | HEAD `b861ae5` 的 quality run **#35 success**（Windows + macOS arm64） | GitHub Actions API |

> 结论：本机是**标准档目标机**，不是 `doc/15` 与 `windows-desktop-coverage` 里的 RTX 2060 SUPER 8 GB 参考机；本机没有 `D:\ist-8gb`。

## 2. 当前仓库与证据缺口

- `main == origin/main == b861ae5`，工作区干净（仅未跟踪的 `.beads/interactions.jsonl`）；`openspec validate --all --strict` 26/26 通过，但 `windows-hardening-and-coverage` 尚未归档，其 spec delta 未进入主规格。
- 8 GB 参考机证据（`verification/windows-8gb-tier/`、`verification/windows-desktop-coverage/`）只覆盖 Win11 / 2060 SUPER / 紧凑档；本机需要的标准档证据不存在。
- `C:\ist-a` 的 6 份就绪记录由 2026-11-04 构建写入，记录的 `worker_wheels` 与当前构建嵌入的 diffusers worker wheel 不同，因此当前构建启动后**预期会拒绝这些记录并提示重新安装**——这本身就是要验证的激活身份行为。
- v0.1.0 之后 75 个提交未发布；CI 绿色记录未入库；`windows-hardening` test-plan 矩阵、`windows-8gb-tier` 的 known roughness、Real-ESRGAN asset-manifest 状态等仍是过时文本。

## 3. 证据类型与判定规则

每个用例必须落一条机器可读记录，字段固定：

```
schema_version, kind, case, status(pass|fail|blocked|not_run),
date, machine{os,build,cpu,ram,gpu,vram,driver,display_scale},
build{commit,exe_sha256,installer_sha256?}, data_root,
install_type(seeded_cache|empty_cache|none), command, metrics{}, evidence[]
```

判定规则：

- 只承认**当前构建**（HEAD `b861ae5` 或执行时的 HEAD，记录 commit）在本机取得的证据；v0.1.0/2026-11-04 构建、8 GB 参考机、CI 结果只能作为对照，不能记 pass。
- **种子缓存重装**与**空缓存真实联机安装**分开记录；种子缓存重装不得宣称验证了下载链路。
- 未执行项一律 `not_run`，被内存预检阻止记 `blocked`，不得写成 pass；手工项（真实 IME、多显示器）由所有者参与时记录 `manual`。
- 资源口径沿用 `windows-local-generation` 标准档合同：CUDA reserved ≤ 10.5 GiB、受管生成进程 private commit 峰值 ≤ 24 GiB、可用物理内存最低 ≥ 2 GiB、加载 ≤ 600 s、单次生成 ≤ 900 s、无 WDDM 共享内存回退。
- 证据写入 `verification/windows-12gb-standard-tier/` 并在 README 建立索引。

## 4. 构建与运行方式

1. **质量门禁 + 构建**：`pwsh -File scripts/build-windows-internal.ps1`（内部 NSIS + release 二进制 + sidecars，并写入 `verification/windows-installer/internal-package.json`）；只想先跑产品回归可用 `-SkipBundle` 得到 `target/release/image-studio-desktop.exe` 与 sidecars。构建日志另存证据目录。
2. **运行隔离根**：`& "C:\Program Files\...\image-studio-desktop.exe" --data-dir C:\ist-a`（实际路径以构建产物为准）；默认数据根不参与验收。
3. **CDP 驱动**：按 `scripts/windows-ui/README.md` 用 `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>` 启动，本机 Hyper-V 保留端口需重新确认（2026-11-08 的 `netsh int ipv4 show excludedportrange` 未列出 9259–9358，候选 9222/9333；被占用时记录实际端口）。
4. **反例**：先在未重新安装的状态启动当前构建，验证旧就绪记录被拒（激活身份变化 → 可行动错误），再执行重装；这解决"旧缓存与新构建是否兼容"的疑问并留下证据。

## 5. 标准档回归（S 系列）

- **档位投影**：模型页 7 个型号、6 个已安装（Z-Image 未装）；期望 6 个可执行型号显示 `standard · 1024² · bfloat16`（Z-Image 显示满足 12 GB 门槛），不出现紧凑档投影。设备探针需报 12288 MiB / sm86。
- **重装与激活**：当前构建拒绝 2026-11-04 就绪记录后，用 `C:\ist-a\cache\downloads` 的已验证缓存重装 Klein、BiRefNet、Real-ESRGAN（三者无安装期量化准备），记录 `seeded_cache` 安装的阶段、耗时、结果；清理旧的 `.prepared` 前先备份就绪记录。
- **Qwen / MOSS 与内存预检**：这两个型号 `prepare_with_model = true`。本机可用物理内存 20.0 GiB、commit 余量 21.7 GiB，均低于规格双条件（≥28 GiB / ≥26 GiB），预期在量化前被拒并给出可行动错误（对应主规格 `model-installation` 的"门槛不足时提前失败"场景）。先按规格记录该 blocked 证据（错误文本、`prepare-preflight.json`、无半成品），再由所有者从 §9 的 A/B/C 中选择。
- **逐能力标准档生成**：Klein 文生图、Klein 单图编辑、BiRefNet 去背景、Real-ESRGAN 4×（每项至少一次产品内真实生成/编辑/处理）；若 Qwen/MOSS 解锁则补 Qwen Turbo（8 步）与 Qwen base/MOSS（40/50 步，时间预算见下）。每项记录：墙钟耗时、阶段（加载/编码/采样/解码）、CUDA reserved/allocated 峰值、进程 commit、可用内存最低值、输出 PNG 尺寸（标准档文生图为 **1024²**；Klein 编辑按源图规则；Real-ESRGAN 为 4×）、资产字节数与 SHA-256。
- **资源采样方法**：生成期间以 1 s 间隔轮询 `nvidia-smi --query-gpu=memory.used,memory.total --format=csv`，同时读应用 `app.log` 的生成期内存事件与 worker 资源报告（若产品提供）；进程 commit 峰值用 `Get-Counter`（`\Process(<name>)\...`）或 Windows 性能计数器采样。方法写入证据 JSON 的 `method` 字段，避免口径歧义。
- **导出与重启**：作品页导出 PNG，与资产逐字节一致；关闭应用重开，模型仍就绪，历史作品可打开并再次导出。
- **Z-Image 标准档（可选，决策见 §9）**：本机未安装 Z-Image，需 ~32.85 GB 下载（ModelScope）；其 profile 无安装期 prepare，可走真实联机安装。若执行，作为标准档 12 GB 门槛（Z-Image 仅 12 GB 档）的关键证据；不执行则记 `not_run` 并说明。
- **不要**用 `C:\ist-a` 旧记录的 2026-11-04 生成结果替代本轮生成；旧结果只作对照。

时间预算（本机 RTX 3080 Ti 比 2060 SUPER 快，仍按保守估计登记）：Klein 加载约 30–60 s + 采样 <10 s；BiRefNet ~10 s；Real-ESRGAN ~4 s；Qwen Turbo 若解锁约 1–2 min；Qwen base 40 步约 5–10 min；MOSS 50 步约 10–20 min；Z-Image 9 步约 1–2 min（含加载）。

## 6. Windows 桌面遗留项（W 系列）

| 项 | 方法 | 判定 |
|---|---|---|
| W1 DPI 100%/150%/200% | 系统设置切换缩放后 `cdp.mjs shot` + 可见性 eval；每档至少主界面+对话页两张截图，另用 `resize-window.ps1` 固定逻辑尺寸 | 导航/输入/发送/模型状态/主要动作可见可点，无遮挡 |
| W2 960×640 / 1280×800 | `resize-window.ps1` + 截图 | 同上 |
| W3 真实中文 IME | 所有者用 Microsoft 拼音实际输入，确认候选回车不发送、组合结束后显式发送；代理只做观察与记录 | 按 `windows-desktop` 规格判断；手工项，需所有者配合 |
| W4 原生导出对话框 | `save-ui-dialog.ps1` 填名保存，与资产逐字节比对 | 文件名/路径正确，旧文件不被破坏，覆盖确认仍生效 |
| W5 多显示器/睡眠唤醒 | 接第二台显示器/切换显示器后截图；睡眠唤醒后发送一次冒烟 | 归位、模型状态与发送正常；不做则 `not_run` |
| W6 主题人工审阅 | 暗/亮主题各一张主要页面截图，所有者目测 | 记录所有者结论，不冒充自动判定 |

## 7. 可观测性与故障分支（O 系列）

- **O1 日志**：在 `C:\ist-a\logs\app.log` 验证新构建产生结构化行（时间、级别、事件、字段），发送链路有 `preflight`/`bind` 阶段耗时；轮转上限按代码与单测记录（8 MiB × 3），不在本轮人为写满 8 MiB。
- **O2 看门狗 60 s**：独占占用 `<data>\data\execution.lock` 后发起发送，断言 60 s 内轮次变为"发送失败"、错误含阶段与原因、界面不无限停留"正在接收提示词"；释放锁后重试成功。
- **O3 取消**：对 BiRefNet、Real-ESRGAN 与一个 Python 型号各发一次并中途取消，记录落定时间（规格 ≤15 s，强制回收另记原因）与残留进程数；补齐 T12 5/5。
- **O4 预检期取消（bd ipw）**：在接收提示词/预检阶段点停止，记录落定时间；验证发送快路径不再重复哈希（`38cb6fd`）后是否满足"取消等待可接受"。若通过则取得关闭 ipw 的证据，否则记录剩余延迟并保留 issue。
- **O5 控制台窗口**：生成与安装期间轮询 `conhost`/`OpenConsole`/`WindowsTerminal` 进程增量，应为 0。
- **O6 安装重试**：以 `HTTPS_PROXY=http://127.0.0.1:9` 启动应用，安装 Real-ESRGAN（45 MB，种子缓存不命中才能触发下载；可先删该 artifact 的缓存条目或换新缓存根），验证 5 次重试后给出明确错误；去掉代理后重装成功。记录退出码/错误文本。
- **O7 非 ASCII/长路径**：在 `D:\ist-12gb\数据 测试\appdata` 建新根，用 Real-ESRGAN 做一次安装+放大，验证 ncnn 探针在非 ASCII 根下不再失败（`8fce281` 修复）。
- **O8 磁盘不足**：在剩余空间受限的卷/配额下触发预检，快速回归"约 4 s 内报磁盘空间不足、不写缓存"（已有 2026-11-01 证据，本机复测一次即可）。
- **O9 强制回收与进程树**：引用本地 `cargo test -p image-studio-desktop --test forced_reap` 结果（CI 已绿），不重复人工构造；作为证据 JSON 的引用项。

## 8. CI / 发布记录（C 系列）

- **C1**：通过 GitHub API 记录 HEAD `b861ae5` 的 quality run #35（Windows + macOS 成功），写入 `verification/ci-release/` 并更新 README 表。
- **C2**：登记"v0.1.0 之后 75 个提交未发布、标签/发布需要所有者授权远程操作"，不做推送/tag。
- **C3（可选）**：下载公开发布的 v0.1.0 NSIS 包，在本机安装/启动/卸载一次，验证发布产物在 Win10 可安装；因它是旧代码，只作为"发布产物可用性"证据，不替代标准档回归。若执行需 2026-11-04 已安装版本的升级路径记录。

## 9. 需要所有者决策的三点

1. **prepare 内存预检（关键）**。本机当前页面文件 4.8 GiB，commit 余量 21.7 GiB，双条件都不满足。选项：
   - **A（推荐，若需要 Qwen/MOSS 标准档证据）**：所有者把系统虚拟内存调到初始 ≥40 GiB / 最大 ≥64 GiB（参考 8 GB 机的 75.9 GiB commit 上限），重启后复测。属系统配置变更，需管理员并显式批准；验收后不自动改回，由所有者决定。
   - **B（不改系统配置）**：用一个临时内存提交压力程序迫使 Windows 自动扩大页面文件，释放后 commit 上限保持扩大，再执行安装。可脚本化、可逆性弱（页面文件可能保持较大），需要在批准后作为实验执行，失败即回退 C。
   - **C（不冒系统风险）**：把 Qwen/MOSS 标准档生成记 `blocked`（预检证据完整），标准档回归证据由 Klein（文生图/编辑）、BiRefNet、Real-ESRGAN（+可选 Z-Image）承载；另立 change 讨论"可复用 prepared 缓存时预检是否仍应拦截"的产品问题。
   默认建议：先执行 C 的记录，若所有者同意 A 再补 Qwen/MOSS。**不得**在未批准时改系统设置。
2. **Z-Image 标准档可选回归**：需 ~32.85 GB 下载与约 1–2 min 生成；建议执行（12 GB 门槛的关键型号），但可跳过。
3. **手工项配合窗口**：真实 IME、100%/200% 缩放（切换时屏幕会闪、可能要求重新登录）、多显示器/睡眠唤醒，需要所有者本人在场；代理无法替代真实 IME 判定。

## 10. 文档、规格与 bd 收口（D 系列）

- 更新 `openspec/changes/windows-hardening-and-coverage/test-plan.md` 矩阵：C3–C7 由"未测"改为"通过"并引用 `verification/windows-desktop-coverage/`（其 tasks 与结果文件已存在），补 2.7/2.8 的真实状态（DPI/IME 本轮证据或仍手工未跑）。
- 更新 `verification/windows-8gb-tier/README.md` 的 "Known roughness"（发送卡顿与 ~2 min 取消已被 `38cb6fd`/`6022bae` 修复并有后续测量）。
- 修 `resources/windows-parity/realesrgan-ncnn-vulkan/asset-manifest.json` 的 `content_verified_pending_gate0` → `verified`（Gate 0 与后续证据已存在，审计 R10）。
- 同步 `windows-hardening-and-coverage` 的 spec delta（桌面运行日志与失败诊断）到主规格并归档该 change（其 3.3/U1 由本 change 的 S 系列证据满足；其 3.4 分支合并已由 `main == origin/main` 解决，远端 `feat/support-8gb-2060s-vram` 为内容等价旧分支，删除需所有者批准）。
- `doc/15-windows-8gb-tier.md` 增补 12 GB 标准档本机实测表（与 8 GB 表并列，明确机器不同）。
- bd：`06k`（uses 已全部 pin 到 SHA）、`q2y`（pipeline 已归档、v0.1.0 已发布）可带证据关闭；`ipw` 由 O4 结果决定关闭或保留；`ba1` 保留（原生窗口级截图/坐标点击仍需人工/工具探索）；`b65` 本地 DB 不存在，需 `bd dolt pull`（远程同步需授权）后确认。
- 本 change 完成后：写 `verification/windows-12gb-standard-tier/README.md` + `acceptance.json`，`openspec validate --all --strict` 通过后归档。

## 11. 风险与缓解

| 风险 | 影响 | 缓解 |
|---|---|---|
| 新构建拒绝 2026-11-04 就绪记录 | 需要重装，耗时 | 先记录拒绝行为作为证据；重装用本地种子缓存，不依赖外网 |
| prepare 预检阻止 Qwen/MOSS | 无法取得这两个型号标准档证据 | §9 决策 A/B/C；先记录 blocked 证据 |
| 页面文件/commit 上限变化 | 预检数值不稳定 | 每次安装前重新测量并写入证据 |
| 真实 IME/缩放需人工 | 无法自动判定 | 明确 `manual` 状态，由所有者确认；不冒充通过 |
| 长时间生成占用机器 | 会话中断风险 | 按模型串行执行，先快后慢；每步写 JSON 断点 |
| CDP 端口被 Hyper-V 保留占用 | 自动化失败 | 启动前检查 `netsh` 排除区间并记录实际端口 |
| 数据根被误用为默认根 | 污染用户数据 | 全程 `--data-dir C:\ist-a` 或新建隔离根，验收后不写默认根 |

## 12. 明确不做

- 不实现 `stream-prepare-low-memory` 的分片量化（所有者已决定暂缓）。
- 不对 8 GB 参考机 / 2060 SUPER 的紧凑档做任何重测（本机硬件不可替代）。
- 不做 macOS 用户门控项（R02–R04、跨平台 M1/M2）。
- 不做被 waive 的整树禁网 I07/P06，不做 D02/D03（干净 Win10 用户、缺失 WebView2）——需要管理员/VM 或所有者另行安排。
- 不做签名/公证发布，不自动打 tag、推送或 `bd dolt push/pull`。
- 不把测试发现的缺陷顺手修掉：先登记 bd，按影响面另立 change。
