# 当前模型可用性审查

日期：2026-10-30，Asia/Shanghai。审查任务：Beads `image-studio-788`。本轮只审查、运行现有验收入口和记录结果，没有修改应用功能。

## 结论与真实接入范围

应先验证现有执行链路，再接入新模型。当前“已接入模型”只有 Z-Image Turbo；模型目录中的另外三个条目仍是计划型号，不应将它们归为验证失败，也不能计入已支持数量。

| 型号 | 宿主登记 | 安装与推理 | 本轮结论 |
|---|---|---|---|
| Z-Image Turbo / mflux-v1 | implemented | 已实现 | 当前版本在隔离目录完成正式安装器重建、禁网生成和资产核验 |
| FLUX.2 Klein 4B distilled | planned | 未实现 | 不可安装或执行，需另行接入 |
| BiRefNet general epoch_244 | planned | 未实现 | 不可安装或执行，需另行接入 |
| Real-ESRGAN x4plus | planned | 未实现 | 不可安装或执行，需另行接入 |

依据：[权威目录](../resources/model-catalog/catalog.json)、[宿主目录投影](../apps/desktop/src/adapters/catalog.rs)、[首批选型范围](../doc/08-first-model-batch.md)。当前没有活动中的 OpenSpec change，审查开始时 Beads 没有未完成任务。

## 日常启动与验收环境的差异

原有安装位于 `target/first-generation-validation/data`，包含完整 Z-Image、固定 MFLUX 运行时、激活记录以及五张正式 PNG。本轮逐一复核这五张图片，文件大小和数据库保存的 SHA-256 全部一致。

默认用户数据目录 `~/Library/Application Support/ImageStudio` 已存在，但检查时没有 `models/`、`runtimes/` 或安装激活记录，任务、Attempt 和正式资产均为零。因此，隔离环境的历史成功不能解释为普通启动已经安装就绪。这属于当前安装位置的事实，不是模型推理失败。

运行时含绑定安装位置的虚拟环境链接，直接复制旧验收目录到新根会被完整性校验拒绝。后续应通过正式安装器在目标位置建立环境，不能手工修改 inventory 来绕过身份检查。当前没有自动迁移或导入已安装模型的产品能力。

## 本轮自动回归

| 检查 | 结果 |
|---|---|
| `npm --prefix apps/frontend test` | 5 个测试文件，23 项通过 |
| `npm --prefix apps/frontend run check` | TypeScript 检查通过 |
| `cargo test --workspace --locked --offline` | 162 项通过，0 失败，2 项默认忽略 |
| `python3 -m unittest discover -s tools/mflux-worker/tests -v` | 8 项通过 |
| `python3 scripts/verify-generation-lock.py` | 17 个模型组件、59 个 runtime wheel 的清单关系通过；内嵌 worker wheel 与源码一致 |
| 验收 CLI 构建 | 通过 |

两个默认忽略的 Rust 用例分别要求显式开启原生监督强杀验收、固定运行时禁网安装验收。本轮没有将这两个用例写成重新运行通过。Python 单元测试也不替代真实模型执行。Rust 日志保存在 `target/model-availability-review-20261030/rust-tests.log`。

## 当前源码的真实执行

复用现有数据的 APFS 隔离副本和完整下载缓存，在 `target/model-availability-review-20261030/smoke` 调用现有 `generation-check`，通过正式 `--install` 重建私有运行时。整个宿主、安装器、监督与 worker 进程树使用现有 `verification/deny-network.sb` 禁止网络。没有重新下载模型，也没有改动默认用户数据目录。

| 项目 | 本轮观测 |
|---|---|
| 设备 | Apple M3 Max / 128 GiB / macOS arm64 |
| 运行时 | MFLUX 0.24.0 / Python 3.13.12 / MLX 0.32.4 |
| 配方 | q4 / 9 步 / guidance 0.0 / linear / seed 20261030 |
| 输入 | 中英文混合：窗边橙猫、秋叶、温暖自然光、水彩插画 |
| 输出 | 1024×1024 PNG，1,471,252 bytes |
| SHA-256 | `1fc7b4a2e4f22b5c4cd294e3e31730b615e99103b116fc7796be6f4f55bced62` |
| Job | `job-b37ca33383c540f5a8ad587dc62c36c9` |
| Attempt | `attempt-79cdcf687ed84e26ba99a6ec815f560e`，Succeeded，无错误 |
| 制作记录 | `evidence_kind=execution_record`，数据库、PNG 实际尺寸/字节/散列一致 |
| Job 接收到资产提交 | 69.676 秒，来自数据库时间戳差 |
| 宿主总墙钟 | 146.465 秒，含复用缓存的安装、运行时重建、校验、生成与提交 |

已目视检查图片：橙猫、窗边秋叶和水彩效果可见，没有空白或黑图。该检查不代表中文文字渲染、人体结构或复杂布局质量评测通过。

结果：[本轮可持久查看的摘要](model-availability-review-2026-10-30.json)。完整日志、计划和 RunManifest 位于 `target/model-availability-review-20261030/verified-result.json`、`real-generation.jsonl`、`real-generation.stderr.log`。真实图片位于该隔离根 `data/assets/asset-d7f44573bb264f5fbac305057b586497.png`。清理 `target` 会删除这些本地大文件证据。

测试准备中的失败尝试也保留：直接移动运行时被链接身份检查拒绝；随后一次复制缓存时多嵌套了一层目录，导致禁网安装找不到缓存。纠正缓存位置后使用正式安装器重跑成功，没有通过修改产品校验或放开网络获得结果。相关日志分别为 `relocated-clone-rejected.*` 和 `cache-layout-attempt-*`。

## 仍需区分的验证边界

本轮实机执行是一张方图和既有缓存下的正式运行时重建，不是无缓存全网络安装、原生 UI 全流程、三个尺寸重新出图、长时间连续生成或其他硬件认证。三个尺寸、取消、原生导出和宿主强杀恢复的已有证据见[首次生成验收](first-generation-results.md)，本轮没有将历史覆盖写成本次重新执行。

## 建议的下一步及 Beads 跟踪

优先推进 `image-studio-2px`：验证日常启动路径，并把当前唯一已接入模型的验收输入和证据固化。重点是普通启动后通过正式模型页安装、生成、保存、重启查看和导出；补充中英文、三个尺寸、连续生成、取消与显式重试。先复用已有 helper，确有缺口再提出工具或行为变更。

随后推进 `image-studio-u3q`：评估 Qwen-Image 2.1/Turbo。当前固定的 MFLUX 源码及已安装包已经包含 `qwen21`，但 Image Studio 的安装器、核心配方、请求验证和 Python 后端仍只执行 Z-Image。应围绕第二个模型做必要的安装身份、模型路由和参数扩展；不预先建设庞大的通用插件系统。首轮优先文生图，编辑、多参考和 RGBA 分别验收。Qwen 基础版与 Turbo 必须保留各自明确的采样配方。

再推进 `image-studio-g37`：评估 MOSS-Image-SFT，聚焦中文文字与布局。固定 MFLUX 已包含 `moss_image`；仍需组件清单、专用参数与实机证据。Base 和社区 4/8 步 LoRA 不自动并入 SFT 认证。此顺序是本次建议，未批准任何模型的实现。

官方执行依据：[固定 MFLUX revision 的 Qwen 2.1 文档](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/qwen21/README.md)、[同 revision 的 MOSS 文档](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/moss_image/README.md)。上游实现可复用不等于本产品已经支持，资料中的其他设备性能不作为本机性能承诺。

Klein、BiRefNet 和 Real-ESRGAN 可按实际编辑、去背景与放大需求调整顺序；它们作为计划条目存在，不应成为“现有模型验证”阶段必须先完成的隐含实现任务。任何新增功能仍须先完成 OpenSpec change 文档并获用户确认。
