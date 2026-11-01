# MOSS-Image-SFT 与七型号整批阶段报告

日期：2026-11-01
状态：MOSS-SFT、跨模型回归和 R01–R05 全部 P0 通过；无必需项 failed、not_run 或 blocked。

## 结论

MOSS-Image-SFT 已从 ModelScope 固定文件 revision 完成首次安装，并在独立数据根禁网复用缓存重新安装、激活与执行。该型号已在 Apple M3 Max / 128 GiB 上独立通过上游基线、中文文字和 JSON 布局样例、正式产品四连、断网四连、取消重试、宿主强杀恢复、原生发送/重开/系统导出。

Qwen-Image 2.1 基础版、官方 Turbo 与 MOSS-SFT 均已完成原生产品验收。原 Z-Image、Klein、BiRefNet、Real-ESRGAN 也在当前构建重跑并完成跨后端、历史重试和原生回归。当前模型目录为 7 个正式接入、7 个已安装、7 个可执行、0 个计划中。

## 来源、安装与体积

- Qwen 基础版：25 个组件，`33,131,609,424` bytes，`qwen-image-2.1-d26bb61231c349cf6b7896fa83353113880e1ba3`。
- Qwen Turbo：16 个组件，`32,451,607,098` bytes，`qwen-image-2.1-turbo-fc13ba1986fcb905854dc8de2f52eee395f49b5b`。
- MOSS-SFT：19 个组件，`27,219,783,878` bytes，`moss-image-sft-f972bd4b4b9e3852133cc5da954d87bc32cd8fb9`。
- Qwen/MOSS 共 60 个组件与冻结 HF 快照逐文件核对路径、字节数和 SHA-256；普通文件另核对 Git blob ID。
- 所有下载 URL 使用 ModelScope 固定逐文件 revision，不使用 `master`；未登记域名、范围或摘要漂移会失败关闭。
- Qwen/MOSS 共享 `mflux-0.24.0-py3.13.12-macos-arm64-recipes-v3` 运行时，但模型快照和配方身份独立；共享运行时不使未装权重的型号就绪。
- MOSS 独立禁网安装写入 `target/qwen-moss-offline-install-20261101`，仅复用已核验缓存；MOSS `ready=true`、设备探针通过，其他型号保持未安装。

## MOSS 基线与产品

上游基线和正式产品均使用 1024×1024、q8、50 步、guidance 3.0、skip `[22]` 与 4096 token 预算。

- T01–T04 上游基线全部 50/50：四次耗时 `637.4 / 637.2 / 631.7 / 649.9` 秒。
- T01–T03 中文/英文输入与产品摄影结构正常。
- T03 海报中的“秋日咖啡 / 周末限定”完整清晰。
- T04 原始 JSON 保持原样输入；产品不解析执行布局字段，但实际图片清晰呈现“周末市集 / OPEN”和左水果篮、右面包布局。
- 正式产品 T01–T04 全部成功，单任务约 `627.6 / 623.2 / 638.9 / 743.3` 秒；PNG、Asset 和 RunManifest SHA 一致。
- 完整 `deny network*` 沙盒中 T01–T04 再次全部 50/50 并提交，完成后无残留 worker。
- tokenizer 边界：4095 可接受，4096 为上界；超限不截断。

证据：

- [上游基线](moss-baseline/results.json)
- [产品生成](moss-product/results.json)
- [断网产品生成](moss-offline/results.json)
- [tokenizer 边界](moss-token-boundaries.json)
- [布局输入](moss-layout-t04.prompt.json)

## 生命周期与原生 UI

- 采样 `1/50` 取消后旧 Attempt 为 cancelled、无资产；同 Job 新 Attempt 使用原配方成功。
- 采样 `1/50` 强杀宿主退出码 73；重开旧 Attempt 为 interrupted，租约回收后同 Job 重试成功。
- 原生 composer 发送成功：`turn-8075dada9cde46449a95dddbbff3e9aa`、`job-330628630b194bc480ee9db0f3bb6105`、`attempt-37bb1239413b46e185be1a9a9e055654`。
- 系统保存面板导出 PNG 与 Asset SHA-256 同为 `bc40b20548ac368a37aea819696b857f336481f6e470894bd3f9d20cf05c1f2d`。
- 正常退出重开后任务、图片和 MOSS 选择保留。

证据：

- [取消重试](moss-cancel-retry/results.json)
- [强杀恢复](moss-kill-retry/results.json)
- [原生 UI](moss-native-ui/results.json)

## 跨模型、异常与布局

- U03 完成 `Z-Image → Klein 文生图 → Klein 编辑 → BiRefNet → Real-ESRGAN 4× → Qwen 基础版 → Qwen Turbo → MOSS → Z-Image`，九次均经过真实 worker/runtime，最后 Z-Image 仍为 q4/9 步。
- U04 中 Qwen 运行期间切到 MOSS 会话可保存草稿但不排队；运行中发送保持禁用，Qwen 完成后仍需用户明确发送。
- U05 旧 4:3 会话切到 Qwen/MOSS 时保留提示词和无效画幅，阻止保存/发送；明确改为 1:1 后恢复。
- U06 七型号筛选、41 个 primary 正式作品和 1 个 BiRefNet mask 的计数/归属一致。
- U07 新型号缺件与旧 v1 环境缺件都保留所选型号和明确阻塞，不自动回退；16 张旧作品仍可查看和导出。
- U08 覆盖 1280×800 浅色和 960×640 深色；长型号名、来源、错误、主动作和键盘焦点可见。
- U02 的系统保存面板 Cancel 路径显示“已取消导出，原作品未改变”，资产仍为 42（41 primary + 1 mask）。

证据：

- [U03/U04](cross-backend)
- [U05–U08](u05-u08)
- [U07](u07-results.json)
- [U02 取消导出](u02-export-cancel-results.json)

## 原模型与回归

- R01：Z-Image 的 1024²、1024×768、768×1024 三个尺寸均成功，保持 q4/9 步/512 token 与 v1 worker；采样取消后新 Attempt 成功，采样第 2 步强杀后旧 Attempt interrupted、租约释放、重试成功。
- R02：原 v1 cancelled Job `job-de0914daf82245cdb4e1b6e2a8d6cfe6` 从产品 UI 显式重试；request、旧 plan、旧 manifest 不变，新 Attempt 复用原 plan，runtime 仍为 `mflux-0.24.0-py3.13.12-macos-arm64-v1`。
- R05：当前构建重跑 Klein 文生图/编辑、BiRefNet RGBA+mask、Real-ESRGAN 4096² RGB；四份系统导出与 Asset SHA 一致。
- R05 原生重试：Klein、BiRefNet、Real-ESRGAN 各创建 cancelled Attempt，再从会话中的“重试”按钮执行成功，均保留原 recipe/runtime。
- R03：事务、幂等、取消竞争、非法候选/帧、未来版本、旧事件围栏和四类提交崩溃边界均通过自动化测试。

证据：

- [R01](r01/results.json)
- [R02](r02/results.json)
- [R03](r03-results.json)
- [R05](r05/results.json)

## 最终质量门

- 前端 check：通过。
- 前端 test：37/37。
- 前端 production build：通过。
- Rust fmt：通过。
- Rust clippy `-D warnings`：通过。
- Rust test：225 passed，0 failed，5 个需要单独真实环境的显式验收按设计 ignored。
- Rust build：通过。
- Python tests：44/44。
- 模型 lock/wheel/source 审计：通过，审计未联网。
- OpenSpec strict：通过。
- 原生断网启动：前端页面加载成功，推理 worker 未启动。
- 隔离源码副本离线 npm/Cargo 构建：通过；复用现有缓存，不声明冷构建。

证据：

- [R04](r04-results.json)
- [最终门禁日志](final-gates)
- [干净源码构建](../cherry-clean-source-build.json)
- [禁网启动](../cherry-offline-startup.json)

## 限制与未运行项

- P1 的更高精度/进一步性能探索未作为交付门禁运行；现有耗时只对应本文记录的输入、缓存和进程条件，不能外推为模型平均速度或其他硬件承诺。
- MOSS JSON 布局是模型原始文本能力，不构成产品侧可执行布局协议、bbox 编辑器或像素级几何保证。
- 本轮新增配方只开放 1024×1024、单张 RGB；Qwen 编辑/RGBA/多参考、MOSS Base/社区加速 LoRA 与 Real-ESRGAN 其他倍率/模型不在范围。
- 设备认证仍只覆盖 `Apple M3 Max / 128 GiB / macOS arm64`，未验证的硬件保持未认证。
