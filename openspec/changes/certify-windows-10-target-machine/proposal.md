## Why

本机是项目登记的第二台 Windows 目标机：**Windows 10 Pro 22H2 / i7-13700K / RTX 3080 Ti 12 GB（sm86）/ 31.7 GiB RAM / 3840×2160 @150%**。它曾在 2026-11-01～11-04 为 Windows 首轮与 parity 轮提供 Gate 0、安装版与产品级证据（`verification/windows-gate0/`、`verification/windows-parity-app/`），但 **`v0.1.0` 标签之后的 76 个提交全部晚于本机安装版构建，从未在这台机器上运行过**：

- 安装版仍是 2026-11-04 23:57 构建的 v0.1.0，不含 2026-11-06 的全部桌面修复（日志、取消计时、强制回收、安装重试、控制台窗口抑制等）与 8 GB 档改动；
- 标准档（12 GB / 1024² / bfloat16）的当前代码没有本机产品级回归证据；`windows-hardening-and-coverage` 3.3 与 `support-8gb-vram-tier` U1/U2 都把这条交给所有者执行；
- 首轮验收里 **W05 100%/200% DPI、W06 真实中文 IME、D02 干净 Win10 用户、D03 WebView2 缺失/断网** 属于"用户接受但无本机机器证据"（`verification/windows-final-acceptance.json`）；其中 DPI/IME 在本机可补；
- Windows 桌面缺陷修复轮的看门狗 60 s、真实取消、控制台窗口、安装重试等分支只有单测或 8 GB 参考机证据；本机可复测；
- `windows-hardening-and-coverage` 已任务全勾但未归档，其 test-plan 矩阵仍写着 C3–C7"未测"，其 spec delta 尚未同步到主规格；
- CI 在 HEAD（`b861ae5`）是绿的（quality run #35，Windows + macOS），但 `verification/ci-release/` 最后记录停在 2026-11-05；v0.1.0 之后 76 个提交未发布（含本 change 文档提交）。

照现状继续开发，会把"8 GB 参考机通过"和"CI 通过"当成标准档目标机通过，违反 `openspec/specs/windows-desktop` 的"真实证据"要求与 `windows-local-generation` 的档位预算合同。本 change 就是把剩余的本机验收一次做完并留下可定位证据。

## What Changes

- **在本机当前主分支构建上做 12 GB 标准档产品级回归**：设备探针（12288 MiB / sm86）、档位投影（standard 1024²/bfloat16，Z-Image 12 GB 门槛）、模型重新安装/激活身份变化、逐能力真实生成（Klein 文生图/编辑、BiRefNet、Real-ESRGAN；Qwen/MOSS 视内存预检决策而定）、资源口径、导出逐字节一致、重启后就绪。
- **记录并验证本机特有的安装期量化准备门槛**：本机自动管理页面文件仅 4.8 GiB，commit 上限 36.5 GiB、余量 21.7 GiB，可用物理内存 20.0 GiB，低于规格的"≥28 GiB 物理或 ≥26 GiB commit 余量"双条件；Qwen/MOSS 的（重新）安装会在量化前被预检拒绝。按规格场景记录该可行动失败，并由所有者决定是否调整系统虚拟内存后重试；不得静默改系统配置。
- **Windows 桌面遗留项按既有决定处理**：W05 100%/200% DPI、W06 真实 IME、parity 4.4 样图复核已在 2026-11-03 / parity 归档时由所有者接受，本轮不重测、不再询问；仅保留 960×640/1280×800、原生导出对话框、主题、多显示器等廉价可选检查（默认跳过）。
- **复测并留证**：`<data>/logs/app.log` 结构化日志与阶段耗时、看门狗 60 s 卡单、取消（BiRefNet/Real-ESRGAN 补齐 T12 5/5 + 预检期取消 ipw）、生成/安装不弹控制台、安装下载重试（代理故障注入）、坏校验/缺失文件拒绝（I2-2）、非 ASCII/长路径、磁盘不足快速回归。
- **CI/发布记录与文档收口**：记录 HEAD run #35 证据；登记未发布提交与发布决策（需所有者授权远程操作）；更新 `windows-hardening-and-coverage` test-plan 矩阵、`windows-8gb-tier` 过时表述、Real-ESRGAN asset-manifest 状态，同步其 spec delta 并归档该 change；清理 `verification/` 中已被取代的 `not_run` 行；bd 过时 issue 收口（06k/q2y/ipw），ba1 保留。
- **不修改产品行为**。若验收发现缺陷，登记 bd 并按影响面另立 OpenSpec change；本 change 不夹带实现。

## Capabilities

### New Capabilities

无。本 change 不新增产品能力。

### Modified Capabilities

- `windows-desktop`：新增"Windows 12 GB 标准档在当前主分支上有真机回归证据"要求与四个场景（档位投影与设备身份、标准档生成与资源口径、种子缓存重装与重启恢复、安装期量化准备被内存预检阻止），见 [specs/windows-desktop/spec.md](specs/windows-desktop/spec.md)。其余能力沿用既有规格，本 change 只补证据与文档。

## Impact

- **代码**：无产品代码改动。可能新增/修订验收脚本（`scripts/windows-ui/` 场景、资源采样与证据生成脚本），脚本仅用于验收，不进入产品运行时。
- **证据**：新增 `verification/windows-12gb-standard-tier/`（环境、逐用例 JSON、截图、日志、质量门禁日志）；更新 `verification/ci-release/`、`verification/windows-desktop-coverage/` 的过时矩阵。
- **数据根**：沿用隔离根 `C:\ist-a`（已有 104.9 GiB 缓存与 6 个已安装型号），先备份 2026-11-04 的就绪记录作为"旧构建状态"证据；不使用默认用户数据根。
- **系统**：若所有者选择提高虚拟内存以满足 prepare 预检，需要管理员权限并重启一次；这是本 change 唯一可能改变系统配置的操作，必须显式批准。
- **文档/流程**：`doc/15-windows-8gb-tier.md` 增补 12 GB 标准档实测表；`openspec/specs/windows-desktop` 新增证据要求；归档 `windows-hardening-and-coverage`；bd 状态更新。
- **远程操作**：仅"记录已有 CI run"属于只读网络访问；打 tag、推送、`bd dolt pull` 等远程写操作不在本 change 自动执行范围内，需另行授权。

## 批准状态

**待项目所有者确认。** 确认时请给出 design.md §9 的两个决策：内存预检处理方式（A/B/C）与是否做 Z-Image 标准档可选回归（本机可零下载种子安装）。历史手工项（W05/W06/样图复核）沿用既有接受状态，本 change 不再询问。
