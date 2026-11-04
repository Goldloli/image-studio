# Image Studio

源码仓库：[Goldloli/image-studio](https://github.com/Goldloli/image-studio)（私有）。Windows 目标机已完成 Gate 0、产品安装与生成闭环、安装版 UI 验收和 NSIS 内部测试包；Windows 首轮交付已于 2026-11-03 由用户验收收口并归档 change、推送源码；收口记录见 [windows-final-acceptance.json](verification/windows-final-acceptance.json)，区分机器已验证项、用户 waive 的 I07/P06，以及按用户验收接受的人工/Mac 项。，明细和接力步骤见 [Windows 进度与接力](#windows-进度与接力)，完整交接见 [doc/12](doc/12-windows-handoff-2026-11-03.md)。

Rust 本地图片创作工作台。桌面栈为 Rust core + Tauri 2 + React，工作区参考 Cherry Studio 绘图页。已接入七个精确型号：Z-Image Turbo、FLUX.2 Klein 4B distilled、BiRefNet general epoch_244、Real-ESRGAN x4plus、Qwen-Image 2.1 基础版、Qwen-Image 2.1 Turbo 与 MOSS-Image-SFT；覆盖文生图、Klein 单图编辑、CPU 去背景/RGBA/mask 和 ncnn 精确 4× 放大。Qwen/MOSS 默认从 ModelScope 固定文件 revision 下载，同时逐文件核对冻结上游快照。创作页以多会话组织多轮提示词和图片，发送后立即显示占位与真实阶段进度，正式结果在同一卡片原位出现。功能页只展示真实可执行能力，作品页跨会话浏览正式 AssetRef，任务中心提供宿主侧状态筛选、检索、稳定分页和可读制作记录。模型页由宿主目录驱动，可检索七个正式目标并分开展示接入、安装、运行时、设备与可执行状态；设置页提供持久外观偏好、显式新会话默认值、真实存储和诊断信息。M3 Max / 128 GiB 已完成七个型号的真实执行、禁网、原生导出、取消和恢复；完整证据见[七型号整批交接](verification/qwen-moss-20261101/moss-stage-report.md)、[Qwen 阶段报告](verification/qwen-moss-20261101/qwen-stage-report.md)与[前置计划模型阶段验收](verification/planned-models-20261031/results.md)。

## Windows 进度与接力

当前 Windows 目标机为 **Windows 10 22H2 / RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K**，已完成：

- Gate 0：三个独立进程、1024×1024、9 步、guidance 0.0，CUDA reserved 峰值 8.87 GiB、进程树 private commit 峰值 23.69 GiB，证据见 [Windows Gate 0](verification/windows-gate0/README.md)。
- 独立 Z-Image CUDA NF4 配方与私有 Python/wheel 运行时、空缓存 ModelScope 安装与中断续传、坏摘要/缺件拒绝、运行准备与探针取消。
- 产品 worker 与 `GenerationController` 实图、加载/采样取消、宿主/监督强杀恢复、跨实例执行租约。
- Windows 目录/功能/设置投影（只投影一个已认证 Z-Image profile）、NSIS 当前用户安装包、安装版 UI 全路径（中英文三次生成、PNG 导出、重启恢复、任务/作品、明暗主题、原生窗口控件、合成 IME 保护）。
- 2026-11-03 新增：启动失败诊断（`logs/startup-failure.log` + 原生对话框 + 退出码 3）与每数据根独立 WebView2 用户数据目录，已在 Windows 10 目标机以重建的 `9e5ecc40…a8ba2` 安装包复验通过（W07-a/a2/b）。
- 2026-11-03 新增：导出覆盖确认的系统「确认另存为」与应用「覆盖导出文件」双确认 Yes 路径已在安装版跑通，导出文件 SHA-256 与正式 Asset 一致（P08 两个分支均有原生证据）。
- 2026-11-03 新增：安装空间不足已在 27.74 GiB 可移动卷上验证——安装前约 4 秒失败、无缓存下载/Job/资产、已有标记文件保留（I03，证据 `verification/windows-gate0/product-space-shortage.json`）。

剩余 P0 与收口状态见下表；该表是归档后的记录，不再是阻塞项：

| 类别 | 内容 | 说明 |
|---|---|---|
| 机器已验证 | Gate 0、W01–W03/W07、L01–L08、P01–P05/P07、I01–I06、D01/D04、R01、P08 双分支、安装包重建与静默安装 | 证据见 `verification/` 各目录与 [收口记录](verification/windows-final-acceptance.json) |
| 用户 waive | 整树禁网 I07/P06 | 2026-11-03 用户决定本轮不运行（waived/not_run，见 [决定记录](verification/windows-network-denial-decision.json)）；不视为通过 |
| 用户验收接受（未附本轮独立机器证据） | W05 100%/200% 显示缩放、W06 真实微软拼音 IME、D02 干净 Win10 用户、D03 缺失/离线 WebView2、Mac R02–R04 | 按所有者验收记录，不得读作代理实测通过 |

### 在新 Windows 机器上接力

```powershell
git clone --config core.longpaths=true --config core.autocrlf=false https://github.com/Goldloli/image-studio.git
cd image-studio
npm --prefix apps/frontend ci
cargo build --locked -p inference-supervisor -p diagnostic-worker
New-Item -ItemType Directory -Force apps/desktop/binaries | Out-Null
Copy-Item target/debug/inference-supervisor.exe apps/desktop/binaries/inference-supervisor-x86_64-pc-windows-msvc.exe
Copy-Item target/debug/diagnostic-worker.exe apps/desktop/binaries/diagnostic-worker-x86_64-pc-windows-msvc.exe
openspec validate introduce-windows-foundation --strict --no-interactive
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm --prefix apps/frontend test
npm --prefix apps/frontend run build
bd prime
```

注意事项：

- Rust 由 `rust-toolchain.toml` 固定为 **1.99.1**，Node/npm 为 **24.19.0 / 11.17.0**。国内网络下 `rustup` 与 `crates.io` 直连会反复卡住：rustup 可先并发下载组件到本地目录，再用 `RUSTUP_DIST_SERVER` 指向本地 HTTP 镜像；cargo 可用 `--config` 临时把 `crates-io` 指向 sparse 镜像（如 rsproxy），不要改全局配置。
- `apps/desktop/build.rs` 要求 `apps/desktop/binaries/` 已有 sidecar，否则 `cargo clippy/test` 会直接 panic。
- 不要复制 Mac 的 runtime/Ready 记录；验收一律用 `--data-dir` 隔离根，正式下载走生产 ModelScope 固定源。
- 启动失败时写入 `<数据根>/logs/startup-failure.log`，显示「Image Studio 启动失败」对话框，退出码 **3**；独立数据根各自使用 `<数据根>/cache/webview2` 的 WebView2 profile。

### 两套验证环境

| 环境 | 用途 | 已复核内容 |
|---|---|---|
| Windows 10 22H2 / RTX 3080 Ti 12 GB / i7-13700K | 目标机 | 234 项 Rust 测试、fmt/clippy/构建、前端 37 项、安装版 UI 全路径、真实 Z-Image 生成、禁网与强杀恢复、NSIS 安装包、W07 启动失败诊断与双根 WebView2 隔离 |
| Windows 11 22631 / RTX 2060 SUPER 8 GB / i9-9900K | 补充环境 | 234 项 Rust 测试、fmt/clippy/构建、前端 37 项、启动失败诊断、双隔离根 WebView2、主题跨重启 |

模型安装、真实生成与资源预算只在 3080 Ti 目标机认证。2060 SUPER 为 sm75 / 8 GiB：低于配方实测 8.87 GiB 的 CUDA reserved 峰值，且不具备配方使用的 BF16 计算，不在支持范围；Windows 11 结果按 [Windows 规格](openspec/specs/windows-desktop/spec.md) 只作补充。记录见 [平台一致性](verification/windows-platform-parity/README.md)。

### Windows 全模型对齐（2026-11-04）

目标机已补齐 macOS 的六个型号（`openspec/changes/archive/2026-11-04-achieve-windows-model-parity`）：FLUX.2 Klein 4B distilled（文生图 + 单图编辑）、BiRefNet epoch_244（CPU 去背景 RGBA+mask）、Real-ESRGAN x4plus（ncnn Vulkan 精确 4×）、Qwen-Image 2.1 / 2.1-Turbo（分阶段 NF4 + prepared cache）与 MOSS-Image-SFT（分阶段 NF4、vendored pipeline）。七个型号在安装版原生 UI 中完成真实生成/处理、查看、PNG/RGBA/mask 导出、任务与作品查询；模型页投影七个型号、硬件门槛与许可（Qwen 非商用标注），设置页展示 v1/v2/ncnn 运行时。

- 证据：`verification/windows-parity-{gate0,runtime,realesrgan,worker,gpu,manifests,ui-projection,review,app}/`；安装版总账见 [Windows parity app acceptance](verification/windows-parity-app/README.md)。
- 安装版验收使用隔离根与本地已校验缓存种子完成六型号安装；空缓存真实网络安装受当前 PyPI 吞吐阻塞、未冒充通过；整树禁网验收沿用 2026-11-03 用户 waive 决定。质量样图人工签收与最终归档待用户确认。
- Windows 内部包 sidecar 现在包含 `inference-supervisor`、`diagnostic-worker` 与 `realesrgan-worker`，由 `scripts/build-windows-internal.ps1` 一并构建。

## 构建和启动

已验证环境为 macOS 27.0.1 / Apple M3 Max / arm64，Rust 工具链固定为 1.99.1。前端已验证 Node.js 24.12.0 / npm 11.6.2。首次构建需要取得 npm 与 Cargo 依赖；应用启动使用内嵌字体，不下载模型、Python 或字体资源。

```sh
npm --prefix apps/frontend ci
npm --prefix apps/frontend run build
cargo build --workspace --locked
cargo run -p image-studio-desktop --locked
```

准备一个本地开发用 macOS app 包：

```sh
python3 scripts/prepare-macos-dev-app.py
open "target/dev/Image Studio.app"
```

该 helper 只整理已经构建的桌面、诊断与原生推理监督二进制，不安装应用、不签名或公证。桌面应用保存已确认的草稿与参数；主题、字号、密度、减弱动效、侧栏状态和新会话默认值跨重启恢复。打开的次要页面标签仍为会话状态。前端生产产物内嵌，启动不需要开发服务器。

仅预览/开发 React 界面：

```sh
npm --prefix apps/frontend run dev
```

浏览器预览不提供原生安装、生成、持久化或通信诊断；修改界面后，重新运行前端 build 和 cargo build 以刷新原生内嵌页面。

## 本地模型

在模型页明确点击“安装模型与运行环境”，安装完成并通过组件校验和设备探针后，才能提交真实生成。普通启动不会下载或加载模型。各型号只使用已登记的固定清单、运行时和配方；应用不修改系统 Python，也不接收任意下载 URL 或 Python 命令。

### Z-Image Turbo

固定组合为 revision `f332072aa78be7aecdf3ee76d5c247082da564a6`、MFLUX 0.24.0 和应用管理的 Python 3.13.12。

- 原始模型组件约 **32.85 GB / 30.59 GiB**，另需运行时和安装临时空间。q4 在加载时执行，不减少原始下载量。
- 每次一张 PNG，支持 1024×1024、1024×768、768×1024，固定 9 步。种子可为空或 u32；实际种子保存在制作记录中。
- 提示词最多 16 KiB UTF-8，且含固定模板不超过 512 token；超限会保留草稿并明确拒绝，不静默截断。
- 一次只运行一个任务。取消先请求合作退出，必要时由原生监督强制回收；清理完成前不会显示资源已释放。
- 安装完整后生成只用本地组件。探针只证明版本和设备小算子通过；整模型验证范围以验收材料为准。目前仅面向 macOS arm64 / M3 Max 128 GiB 验收。

### FLUX.2 Klein 4B distilled

固定快照为 revision `e7b7dc27f91deacad38e78976d1f2b499d76a294`，使用独立 `mflux-0.24.0-py3.13.12-macos-arm64-recipes-v2` 运行时与 `studio-mflux-worker-0.2.0`。18 个模型组件共 `15,980,131,745` bytes，安装与执行前逐文件校验 SHA-256。

- 文生图固定 1024×1024、q4 加载、4 步、guidance 1.0；单图编辑使用 Klein 单参考条件分支，不把 img2img 强度伪装成指令编辑。
- 两者共用同一已核验模型快照和运行时，但分别完成真实执行、质量、生命周期和原生 UI 验收。
- Klein 不借用 Z-Image 的安装或就绪状态；多参考编辑、其他 Klein 变体和替换文本编码器不在当前范围。

### BiRefNet general epoch_244

固定官方 ONNX release asset `186740004`，模型 `972,666,916` bytes、SHA-256 `58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c`，使用独立的 Python 3.13.12 + ONNX Runtime 1.31.1 CPU 运行时和 `studio-onnx-worker-0.1.0`。

- 每次处理一张受控 RGB/RGBA PNG 或 JPEG，最长边不超过 4096、像素数不超过 4 MP；输出保持源图规范化尺寸。
- 固定 `CPUExecutionProvider`、float32、4 个 intra-op 线程、1 个 inter-op 线程；网络输入固定 1024×1024。
- 实际 ONNX 输出是 logits，配方只做一次 sigmoid，再按 `align_corners=True` 恢复尺寸；最终 mask 与源 alpha 按规定合成。
- 一个 Job 原子提交 RGBA primary 和 8-bit mask 两个资产。作品页、计数和 facets 只统计 primary，蒙版仍可从结果卡片查看、选择和独立导出。
- 同一快照已完成空缓存安装、禁网安装、CPU 图探针、真实 ONNX 推理、加载/推理取消、宿主强杀、系统选图与导出；其他 BiRefNet checkpoint 和 GPU provider 不在当前范围。

### Real-ESRGAN x4plus

固定官方 ncnn 程序与 x4plus 模型对，两个组件共 `33,540,549` bytes，使用独立的 `realesrgan-x4plus-ncnn-macos-v0.2.0-v1` 原生运行时。

- 输入必须是无透明像素的 RGB PNG/JPEG，最长边至多 1024、总像素至多 1 MP；输出为 RGB PNG，每边精确 4 倍，最大 4096×4096。
- 固定 Metal GPU、tile 128、1:1:1 线程和关闭 TTA；不接受倍率、种子、动漫/人脸模型或任意后端参数。
- 当前构建已在本机重跑输入、取消、显式重试、4096² 输出和原生导出；导出 SHA-256 与正式 Asset 一致。

### Qwen-Image 2.1 / Turbo

基础版固定 25 个组件、`33,131,609,424` bytes；官方 Turbo 固定 16 个组件、`32,451,607,098` bytes。两者共享 `mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3`，但模型快照、transformer、配置和采样身份分别登记。

- 基础版固定 1024×1024、q8、40 步、guidance 1.0；Turbo 固定 1024×1024、q8、8 步、guidance 1.0，并使用其官方采样曲线，不仅是将基础版步数改成 8。
- 本地真实 tokenizer 预检预算为 2048 token，计入模型自身模板与特殊 token；超限不截断。
- 默认下载地址使用 ModelScope 固定逐文件 revision；全部 60 个组件与冻结 HF 内容的路径、字节数和 SHA-256 一致。
- 两个配方均完成上游基线、产品连续生成、采样取消、宿主强杀恢复、完整禁网执行和原生发送/重开/系统导出。

### MOSS-Image-SFT

固定 `OpenMOSS-Team/MOSS-Image-SFT` revision `f972bd4b4b9e3852133cc5da954d87bc32cd8fb9`，19 个组件共 `27,219,783,878` bytes，与 Qwen 共用已核验的 recipes-v3 运行时但保留独立模型身份。

- 固定 1024×1024、q8、50 步、guidance 3.0、空负提示词与负分支跳层 `[22]`。
- 真实 tokenizer 预算为 4096 token，计入 chat template 与特殊 token；4095 可接受、4096 为上界，超限不截断。
- T01–T04 覆盖中英文输入及原始 JSON 布局文本；“周末市集 / OPEN”和左右布局样例在基线与产品链路中均清晰。
- 已完成 ModelScope 固定源安装、独立数据根禁网缓存安装、产品四连、禁网四连、取消重试、宿主强杀恢复和原生发送/重开/系统导出。

## 模型目录与应用设置

模型页的权威目录位于 [catalog.json](resources/model-catalog/catalog.json)，当前登记七个正式接入、已安装且可执行的型号，计划中为 0。列表搜索、一级筛选与能力筛选组合在本地有界目录中完成，不触发下载、散列、设备探针或推理。安装动作按登记 `install_id` 调用对应型号的安装器，不接收前端提供的路径、地址或命令。

设置页分为“通用与外观、创作默认值、存储、运行环境与诊断、关于”：

- 主题、字号、界面密度、减弱动效和侧栏状态即时保存。
- 默认模型、画幅、数量和种子显式整组保存；未保存切换分组时会要求保存、放弃或取消。
- 新会话使用“创建入口显式覆盖 → 持久默认值 → 内置默认值”的唯一优先级，已存在会话不被改写。
- 存储页只展示宿主解析路径、元数据占用、可用空间和受控定位，不提供迁移或删除。
- 打开运行环境页不会自动启动诊断；关于页不显示未实现的自动更新或在线服务控件。

详细字段、错误回退与验收边界见 [模型目录与应用设置](doc/10-model-catalog-and-settings.md)。

## 功能、作品与任务

- 功能页只把宿主登记为已接入且依赖真实的能力显示为可开始；当前七型号的八项能力均可进入统一 composer，不创建无后端支撑的假任务。
- 作品页只投影正式 AssetRef，可按日期、型号、尺寸与会话组合筛选，支持网格/列表、预览、原生 PNG 导出、制作记录和可靠的来源会话返回。示例、候选与失败 Attempt 不计入作品。
- 任务中心在宿主持久库执行状态筛选、提示词/会话/型号检索和稳定游标分页，同条件返回计数。详情先显示请求、来源、实际种子、Attempt、阶段、错误与下一步，完整 manifest/日志按需展开。
- 没有可靠会话归属的旧任务或直接验收任务明确显示“历史生成”，不会按提示词猜测绑定到当前草稿。

默认数据位于 `~/Library/Application Support/ImageStudio`：SQLite schema v5 保存项目、会话、轮次、受控源图、任务、完成时间和制作记录；版本化 `application-preferences.json` 保存应用偏好与新会话默认值，`models/`、`runtimes/` 与 `inputs/` 保存版本化安装和受控来源，`assets/` 保存不可变结果。v1 草稿会逐一迁移成会话，无法确认归属的旧 Job 统一进入“历史生成”，不按提示词猜关联。旧任务仍可完整读取，历史记录没有保存精确完成时间时会显示“时间未记录”。缓存与日志分别位于 `~/Library/Caches/ImageStudio`、`~/Library/Logs/ImageStudio`。可用 `--data-dir PATH` 创建独立测试根；同一数据根只允许一个宿主。

只有图片验证并完成数据库提交后，任务才成功。重启把未终结执行标为中断，用户明确重试才创建新 Attempt；不恢复采样步数。模型缺失不影响查看和导出已有图片。外部删除或改写图片会显示资产异常，原制作记录保留。保存失败时请重试或复制草稿；强制结束应用只恢复最近已提交版本。

开发者可显式使用验收 helper（必须指定隔离根，不使用默认用户数据）：

```sh
cargo build --workspace --examples --locked
target/debug/examples/generation-check --data-dir "$PWD/target/first-generation-validation"
# 明确安装，及安装后真实执行；这两条会下载/运行完整模型
target/debug/examples/generation-check --data-dir "$PWD/target/first-generation-validation" --install
target/debug/examples/generation-check --data-dir "$PWD/target/first-generation-validation" --prompt "清晨的山间湖泊，柔和自然光" --seed 42
# 打开已经安装模型、保存验收草稿与结果的隔离工作区
open -n "target/dev/Image Studio.app" --args --data-dir "$PWD/target/first-generation-validation"
```

## 开发诊断

开发构建可从右上角“开发 → 启用通信诊断”进入，也可使用命令行：

```sh
cargo run -p image-studio-desktop --locked -- --diagnostics
```

启用界面后仍须点击“启动诊断”才运行子进程。成功只表示通信探针通过，不表示 GPU、模型或图片生成通过。界面提供显式停止、错误摘要、受限日志、窗口尺寸验证。

可选参数：

| 参数 | 用途 |
|---|---|
| `--data-dir PATH` | 指定隔离的数据、缓存、日志和诊断工作目录 |
| `--diagnostic-worker PATH` | 指定已构建的诊断程序；默认与桌面程序同目录 |
| `--diagnostic-mode MODE` | 正常或故障模式，如 normal、hang、crash、flood、descendant |
| `--diagnostic-size 960x640` | 开发验证窗口尺寸 |
| `--diagnostic-scale 1` | 兼容旧诊断请求 1 或 2；WebView 实际 DPR 由系统决定，不用于改变页面缩放 |
| `--help` | 查看帮助 |

所有 `--diagnostic-*` 参数均需要显式 `--diagnostics`。普通启动不会运行诊断进程，release 构建不显示开发菜单。

## 工程边界

| Workspace 成员 | 职责 |
|---|---|
| `apps/desktop` | Tauri 宿主、受限 IPC、安装器、SQLite/资产与原生进程 adapter |
| `apps/frontend` | React/TypeScript 界面、草稿保存状态、模型/任务与画板 |
| `crates/studio-core` | 类型化身份、请求/计划/制作记录、固定配方与 Attempt 状态 |
| `crates/worker-protocol` | 彼此隔离的诊断/生成协议、帧边界、握手与身份校验 |
| `tools/diagnostic-worker` | 诊断协议及可控故障进程 |
| `tools/inference-supervisor` | 宿主 EOF、执行租约、Python/后代监督与回收 |
| `tools/mflux-worker` | 固定 MFLUX API、本地 tokenizer 预检与图片候选输出 |
| `tools/onnx-worker` | 固定 BiRefNet 图合同、CPU 推理、RGBA/mask 双候选 |

核心和协议不依赖 GUI、推理框架或平台专用 API。worker stdout 传有界协议，stderr 传有界日志；产品进程启动使用参数数组，不经 shell。

## 验证

```sh
npm --prefix apps/frontend run build
npm --prefix apps/frontend test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
python3 -m pytest -q scripts/tests tools/mflux-worker/tests tools/mflux-worker-v2/tests tools/mflux-worker-v3/tests tools/onnx-worker/tests
python3 scripts/verify-planned-model-locks.py
cargo check -p studio-core -p worker-protocol --target x86_64-pc-windows-msvc --locked
```

macOS 开发辅助脚本使用 Python 3 标准库：

```sh
python3 scripts/check-clean-source.py
python3 scripts/check-offline-startup.py
```

前者在独立源码副本中执行离线 npm 安装、前端构建和 Rust 构建，复用隔离 target 缓存，不宣称冷构建；后者仅对测试进程施加禁止联网的 sandbox profile，观测 Tauri 主 WebView 页面加载完成后自动结束测试进程，不修改系统网络设置。

会话、发送、进度、迁移和恢复结果见 [会话式生成验收](verification/conversation-generation-results.md)，机器可读记录见 [会话验收数据](verification/conversation-validation.json)。模型目录、设置持久化和窄窗结果见 [模型设置验收](verification/model-catalog-settings-results.md) 与 [机器可读记录](verification/model-catalog-settings-acceptance.json)。功能目录、作品库、任务中心与真实生成联动见 [功能作品任务验收](verification/feature-works-task-results.md) 和 [机器可读记录](verification/feature-works-task-acceptance.json)。原始工作区结果见 [Cherry 工作区验收](verification/cherry-workspace-results.md)，基础框架记录见 [基础框架验收记录](verification/foundation-results.md)、[依赖与版本](verification/dependencies.json) 和 [字体来源及许可](resources/fonts/README.md)。

2026-10-31 的 Klein 阶段认证覆盖安装、文生图、单图编辑、质量、生命周期、安全边界和原生日常操作，结果为 50 通过、0 失败、0 未运行、0 阻塞。真实中断续传使用用户授权的国内镜像补齐传输，但 18/18 文件仍按官方固定 revision、字节数和 SHA-256 核验；正式安装器随后在全树禁网环境完成校验、运行时探针和激活。完整结果见 [Klein 阶段报告](verification/planned-models-20261031/results.md) 与 [机器可读明细](verification/planned-models-20261031/klein-acceptance.json)。

BiRefNet epoch_244 随后完成 31 项适用 P0：空缓存与禁网安装、真实 ONNX 图、透明 RGBA/mask、六类原创质量样图、加载/`session.run` 取消、宿主强杀、双输出事务、原生选图/查看/导出和最终禁网启动均通过。详细结果见 [BiRefNet 机器可读验收](verification/planned-models-20261031/birefnet-acceptance.json) 与 [质量接触表](verification/planned-models-20261031/birefnet-quality-contact-sheet.jpg)。Real-ESRGAN 与三型号整批回归仍未执行。

2026-10-31 的当前模型复认证覆盖 37 个 P0 用例，结果为 37 通过、0 失败、0 阻塞。空缓存安装经固定 Hugging Face revision 完成组件校验、运行时激活和 M3 Max GPU 探针；原生 SCIM 拼音候选确认与未确认组合门禁通过。快速双击误取消、作品页首次旧计数和图片查看器重开问题也已完成原生回归。完整结果见 [模型复认证报告](verification/model-usability-20261031/results.md)、[机器可读明细](verification/model-usability-20261031/acceptance.json) 与 [质量审阅](verification/model-usability-20261031/quality-review.json)。

2026-11-01 的整批回归完成七型号/八能力统计、41 个正式作品与 1 个 mask、九步跨后端顺序、跨会话互斥、4:3→1:1 恢复、缺件与旧环境保留、1280×800/960×640 原生布局。R01 重跑 Z-Image 三个尺寸、真实取消重试和采样后宿主强杀恢复；R02 对原 v1 cancelled Job 追加新 Attempt 并保持 request/plan/manifest 不变；R05 重跑 Klein 文生图/编辑、BiRefNet 双输出、Real-ESRGAN 4×，并完成三条 cancelled 任务的会话内原生重试。R03/R04 覆盖事务、旧事件、未来版本、非法候选/帧、提交崩溃窗口、前端/Rust/Python、锁审计、禁网启动和隔离源码构建。报告见 [MOSS/整批阶段报告](verification/qwen-moss-20261101/moss-stage-report.md)。

## 规格与研究

- [正式基础规格](openspec/specs/)
- [OpenSpec 变更记录](openspec/changes/)
- [产品与模型研究总览](doc/README.md)

会话式生成已完成实现、实机验收并归档为 [2026-10-30-introduce-generation-conversation](openspec/changes/archive/2026-10-30-introduce-generation-conversation/proposal.md)。模型目录、设置与新会话默认值已完成并通过 [归档 change](openspec/changes/archive/2026-10-30-introduce-model-catalog-and-settings/proposal.md) 更新正式 `model-catalog`、`application-settings` 与 `conversation-management` specs。功能、作品与任务中心已按确认后的 [归档 change](openspec/changes/archive/2026-10-30-improve-feature-works-and-tasks/proposal.md) 完成实现、真实生成验收和主规格更新。发送幂等、作品页激活刷新与查看器返回状态已通过 [交互修复归档](openspec/changes/archive/2026-10-31-fix-daily-generation-interactions/proposal.md) 更新正式规格。当前模型 37 项 P0 认证已通过并归档为 [2026-10-31-certify-current-model-usability](openspec/changes/archive/2026-10-31-certify-current-model-usability/proposal.md)，门禁契约已合并到 [model-validation 主规格](openspec/specs/model-validation/spec.md)。Qwen/MOSS 变更已归档为 [2026-11-01-introduce-qwen21-and-moss-image](openspec/changes/archive/2026-11-01-introduce-qwen21-and-moss-image/proposal.md)，七个正式型号与 `multi-model-recipes` 主规格已同步。 Windows 首轮交付已归档为 [2026-11-03-introduce-windows-foundation](openspec/changes/archive/2026-11-03-introduce-windows-foundation/README.md)，`windows-desktop` 与 `windows-local-generation` 主规格已同步；收口记录见 [windows-final-acceptance.json](verification/windows-final-acceptance.json)。

用户选定的黑色应用图标已接入 macOS 开发包、左栏和关于页，源图与派生说明见 [品牌资源](resources/brand/README.md)。[应用图标品牌规格](openspec/specs/product-identity/spec.md)已获确认；[验收记录](verification/app-icon-branding-results.md)包含 Finder、浅暗主题、小窗口、回退与离线启动结果。Dock/应用切换器已由用户人工核对确认，change 已完成验收并归档；Windows ICO 已生成但未做 Windows GUI 实测。

实现任务使用 Beads；功能扩展先提出 OpenSpec change，经确认后实现。验收按实际硬件、版本与执行证据记录；诊断、示例和设备小算子均不替代真实模型结果。
