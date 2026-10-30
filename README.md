# Image Studio

Rust 本地图片创作工作台。桌面栈为 Rust core + Tauri 2 + React，工作区参考 Cherry Studio 绘图页。已接入 Z-Image Turbo 的受控安装、独立推理进程、任务记录和 PNG 资产保存；创作页以多会话组织多轮提示词和图片，发送后立即显示图片占位与真实阶段进度，正式结果在同一卡片原位出现。功能页按创作目的展示真实能力，作品页跨会话浏览正式 AssetRef，任务中心提供宿主侧状态筛选、检索、稳定分页和可读制作记录。模型页由宿主目录驱动，可检索首批四个目标并分开展示接入、安装、运行时、设备与可执行状态；设置页提供持久外观偏好、显式新会话默认值、真实存储和诊断信息。草稿、会话、未发送输入、界面偏好与新会话默认值可跨重启恢复。M3 Max / 128 GiB 已完成三个尺寸的真实中英文生成、禁网生成、原生 PNG 导出与取消；完整范围和恢复证据见 [首个模型验收](verification/first-generation-results.md)、[会话式生成验收](verification/conversation-generation-results.md)、[模型设置验收](verification/model-catalog-settings-results.md) 与 [功能作品任务验收](verification/feature-works-task-results.md)。

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

## 首个本地模型

在模型页明确点击“安装模型与运行环境”，安装完成并通过组件校验和设备探针后，才能提交真实生成。普通启动不会下载或加载模型。固定组合为 Z-Image Turbo revision `f332072aa78be7aecdf3ee76d5c247082da564a6`、MFLUX 0.24.0 和应用管理的 Python 3.13.12；不修改系统 Python，也不接收任意下载 URL 或 Python 命令。

- 原始模型组件约 **32.85 GB / 30.59 GiB**，另需运行时和安装临时空间。q4 在加载时执行，不减少原始下载量。
- 每次一张 PNG，支持 1024×1024、1024×768、768×1024，固定 9 步。种子可为空或 u32；实际种子保存在制作记录中。
- 提示词最多 16 KiB UTF-8，且含固定模板不超过 512 token；超限会保留草稿并明确拒绝，不静默截断。
- 一次只运行一个任务。取消先请求合作退出，必要时由原生监督强制回收；清理完成前不会显示资源已释放。
- 安装完整后生成只用本地组件。探针只证明版本和设备小算子通过；整模型验证范围以验收材料为准。目前仅面向 macOS arm64 / M3 Max 128 GiB 验收。

## 模型目录与应用设置

模型页的权威目录位于 [catalog.json](resources/model-catalog/catalog.json)，只把 Z-Image Turbo 标为已接入和当前可执行；FLUX.2 Klein 4B distilled、BiRefNet general epoch_244、Real-ESRGAN x4plus 是计划型号。列表搜索、一级筛选与能力筛选组合在本地有界目录中完成，不触发下载、散列、设备探针或推理。安装动作仍只调用现有 Z-Image 安装器。

设置页分为“通用与外观、创作默认值、存储、运行环境与诊断、关于”：

- 主题、字号、界面密度、减弱动效和侧栏状态即时保存。
- 默认模型、画幅、数量和种子显式整组保存；未保存切换分组时会要求保存、放弃或取消。
- 新会话使用“创建入口显式覆盖 → 持久默认值 → 内置默认值”的唯一优先级，已存在会话不被改写。
- 存储页只展示宿主解析路径、元数据占用、可用空间和受控定位，不提供迁移或删除。
- 打开运行环境页不会自动启动诊断；关于页不显示未实现的自动更新或在线服务控件。

详细字段、错误回退与验收边界见 [模型目录与应用设置](doc/10-model-catalog-and-settings.md)。

## 功能、作品与任务

- 功能页只把宿主登记为已接入且依赖真实的文生图显示为可开始；图片编辑、去背景和放大仅展示计划范围，不创建假任务。文生图入口直接定位当前会话的现有 composer。
- 作品页只投影正式 AssetRef，可按日期、型号、尺寸与会话组合筛选，支持网格/列表、预览、原生 PNG 导出、制作记录和可靠的来源会话返回。示例、候选与失败 Attempt 不计入作品。
- 任务中心在宿主持久库执行状态筛选、提示词/会话/型号检索和稳定游标分页，同条件返回计数。详情先显示请求、来源、实际种子、Attempt、阶段、错误与下一步，完整 manifest/日志按需展开。
- 没有可靠会话归属的旧任务或直接验收任务明确显示“历史生成”，不会按提示词猜测绑定到当前草稿。

默认数据位于 `~/Library/Application Support/ImageStudio`：SQLite schema v3 保存项目、会话、轮次、每会话草稿、任务、完成时间和制作记录；版本化 `application-preferences.json` 保存应用偏好与新会话默认值，`models/` 与 `runtimes/` 保存版本化安装，`assets/` 保存不可变结果。v1 草稿会逐一迁移成会话，无法确认归属的旧 Job 统一进入“历史生成”，不按提示词猜关联。v2 任务可完整读取，但历史记录没有保存精确完成时间时会显示“时间未记录”。缓存与日志分别位于 `~/Library/Caches/ImageStudio`、`~/Library/Logs/ImageStudio`。可用 `--data-dir PATH` 创建独立测试根；同一数据根只允许一个宿主。

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

核心和协议不依赖 GUI、推理框架或平台专用 API。worker stdout 传有界协议，stderr 传有界日志；产品进程启动使用参数数组，不经 shell。

## 验证

```sh
npm --prefix apps/frontend run build
npm --prefix apps/frontend test
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --workspace --locked
cargo check -p studio-core -p worker-protocol --target x86_64-pc-windows-msvc --locked
```

macOS 开发辅助脚本使用 Python 3 标准库：

```sh
python3 scripts/check-clean-source.py
python3 scripts/check-offline-startup.py
```

前者在独立源码副本中执行离线 npm 安装、前端构建和 Rust 构建，复用隔离 target 缓存，不宣称冷构建；后者仅对测试进程施加禁止联网的 sandbox profile，观测 Tauri 主 WebView 页面加载完成后自动结束测试进程，不修改系统网络设置。

会话、发送、进度、迁移和恢复结果见 [会话式生成验收](verification/conversation-generation-results.md)，机器可读记录见 [会话验收数据](verification/conversation-validation.json)。模型目录、设置持久化和窄窗结果见 [模型设置验收](verification/model-catalog-settings-results.md) 与 [机器可读记录](verification/model-catalog-settings-acceptance.json)。功能目录、作品库、任务中心与真实生成联动见 [功能作品任务验收](verification/feature-works-task-results.md) 和 [机器可读记录](verification/feature-works-task-acceptance.json)。原始工作区结果见 [Cherry 工作区验收](verification/cherry-workspace-results.md)，基础框架记录见 [基础框架验收记录](verification/foundation-results.md)、[依赖与版本](verification/dependencies.json) 和 [字体来源及许可](resources/fonts/README.md)。

2026-10-31 的当前模型复认证覆盖 37 个 P0 用例，结果为 37 通过、0 失败、0 阻塞。空缓存安装经固定 Hugging Face revision 完成组件校验、运行时激活和 M3 Max GPU 探针；原生 SCIM 拼音候选确认与未确认组合门禁通过。快速双击误取消、作品页首次旧计数和图片查看器重开问题也已完成原生回归。完整结果见 [模型复认证报告](verification/model-usability-20261031/results.md)、[机器可读明细](verification/model-usability-20261031/acceptance.json) 与 [质量审阅](verification/model-usability-20261031/quality-review.json)。

## 规格与研究

- [正式基础规格](openspec/specs/)
- [OpenSpec 变更记录](openspec/changes/)
- [产品与模型研究总览](doc/README.md)

会话式生成已完成实现、实机验收并归档为 [2026-10-30-introduce-generation-conversation](openspec/changes/archive/2026-10-30-introduce-generation-conversation/proposal.md)。模型目录、设置与新会话默认值已完成并通过 [归档 change](openspec/changes/archive/2026-10-30-introduce-model-catalog-and-settings/proposal.md) 更新正式 `model-catalog`、`application-settings` 与 `conversation-management` specs。功能、作品与任务中心已按确认后的 [归档 change](openspec/changes/archive/2026-10-30-improve-feature-works-and-tasks/proposal.md) 完成实现、真实生成验收和主规格更新。发送幂等、作品页激活刷新与查看器返回状态已通过 [交互修复归档](openspec/changes/archive/2026-10-31-fix-daily-generation-interactions/proposal.md) 更新正式规格。当前模型 37 项 P0 认证已通过并归档为 [2026-10-31-certify-current-model-usability](openspec/changes/archive/2026-10-31-certify-current-model-usability/proposal.md)，门禁契约已合并到 [model-validation 主规格](openspec/specs/model-validation/spec.md)。

用户选定的黑色应用图标已接入 macOS 开发包、左栏和关于页，源图与派生说明见 [品牌资源](resources/brand/README.md)。[应用图标品牌规格](openspec/specs/product-identity/spec.md)已获确认；[验收记录](verification/app-icon-branding-results.md)包含 Finder、浅暗主题、小窗口、回退与离线启动结果。Dock/应用切换器已由用户人工核对确认，change 已完成验收并归档；Windows ICO 已生成但未做 Windows GUI 实测。

实现任务使用 Beads；功能扩展先提出 OpenSpec change，经确认后实现。验收按实际硬件、版本与执行证据记录；诊断、示例和设备小算子均不替代真实模型结果。
