# Test plan: windows-hardening-and-coverage

环境：Windows 11 Enterprise 22631 / i9-9900K / RTX 2060 SUPER 8 GB（sm75）/ 31.9 GiB RAM；
隔离数据根 `D:\ist-8gb\appdata`（已装 Qwen Turbo、Klein、MOSS、BiRefNet）。

## 覆盖矩阵

| ID | 能力 | 当前状态 | 判定方式 | 证据 |
|---|---|---|---|---|
| C1 | Qwen-Image 2.1 Turbo 文生图 | 通过（20 次连续发送） | 图片生成 + 耗时 | `verification/windows-desktop-defects/` |
| C2 | FLUX.2 Klein 4B 文生图 | 通过（177 s） | 图片生成 | 本 change 执行时补录 |
| C3 | FLUX.2 Klein 4B 图片编辑 | 通过（28.1 s，2026-11-08 12 GB 档） | 导入源图 → 编辑 → 结果与源图同场景 | `verification/windows-12gb-standard-tier/S4-S8-current-build-regeneration.json` |
| C4 | MOSS-Image-SFT 文生图 | 通过（143 s，50 步，2026-11-08） | 完整 50 步出图 | `verification/windows-12gb-standard-tier/S14-moss-install-generation.json` |
| C5 | BiRefNet 去背景 | 通过（25.1 s，RGBA + mask，2026-11-08） | 输出 RGBA + mask，主体正确 | `verification/windows-12gb-standard-tier/S4-S8-current-build-regeneration.json` |
| C6 | Real-ESRGAN 4× 放大 | 通过（9.5 s，1024²→4096²，2026-11-08） | 输出 4× 尺寸正确 | 同上 |
| C7 | Qwen-Image 2.1 基础版 | 通过（84 s，40 步，2026-11-08） | 40 步出图，记录时间 | `verification/windows-12gb-standard-tier/S15-qwen-base-install-generation.json` |
| C8 | Z-Image Turbo 8 GB | 不可行（已关闭） | 不适用 | `doc/16` |

## 缺陷分支

| ID | 分支 | 当前状态 | 判定方式 |
|---|---|---|---|
| B1 | 取消强制回收 | 通过（`tests/forced_reap.rs`，2026-11-08） | stub supervisor 忽略取消，断言 15 s 内 `cancelled` 且无残留进程 |
| B2 | 看门狗 60 s 超时 | 通过（注入滞留 pending 回合，2026-11-08） | 看门狗 5 s 内回收并给出可行动错误 | `verification/windows-12gb-standard-tier/O2-watchdog-injected-reap.json` |
| B3 | 下载真实重试 | 通过（代理故障 + 恢复，2026-11-08） | 故障时明确错误；修复后重装成功 | `verification/windows-12gb-standard-tier/O6-install-download-retry.json` |
| B4 | 发送快路径不哈希 | 已修复 | 只读元数据下 `ready_paths()` 仍成功 |

## Windows 回归

| ID | 项 | 判定 |
|---|---|---|
| W1 | 100%/150%/200% DPI | user-accepted（2026-11-03，不重测） |
| W2 | 960×640 与 1280×800 | 通过（2026-11-08；导航/输入/发送可见） | `verification/windows-12gb-standard-tier/W2-window-sizes.json` |
| W3 | 中文 IME 组合确认 | 单测覆盖（`shouldSendOnEnter`），真实 IME user-accepted 不重测 |
| W4 | 原生导出对话框 | 通过（导出与资产逐字节一致，2026-11-08） | `W4-native-export-dialog.json` |
| W5 | 磁盘不足 | pass-by-reference（2026-11-01 本机实测 + 单测） | `O8-disk-shortage.json` |
| W6 | 非 ASCII / 长路径 | 通过（`C:\ist-验收 中文` 安装 + 4× 放大，2026-11-08） | `O7-non-ascii-path.json` |

## 判定规则

- 任何未执行项保持 `not_run`，不得写成通过。
- 每项证据包含：命令/脚本、截图或 JSON、耗时。
- 时间以墙钟为准，标注是否包含组件加载。
