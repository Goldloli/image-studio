# Proposal: fix-cross-platform-model-selection

## Why

Windows 端在选择**已经接入且可执行**的文本模型（Klein / Qwen 基础 / Qwen Turbo / MOSS）时，输入框下方仍显示：

> 此会话保留旧的跨平台模型选择。请从模型菜单明确选择 Windows Z-Image，并核对画幅后再发送。

实测复现（安装版，`C:\ist-a`，2026-11-04）：选择 `FLUX.2 Klein 4B` 或 `Qwen 2.1 Turbo`（两者均为 Windows 原生 id、ready 可执行）后该警告立即出现，模型标签正常。原因见 design.md：警报条件在 Windows 上恒真，且未随 Windows 全模型接入更新。

同一根因在 macOS 上表现为另一面：打开 Windows 端保存的会话时，Windows 模型 id 不在 macOS 目录中，界面只显示"未就绪"并禁用发送，没有"来自 Windows 端选择，请选择 macOS 对应型号"的说明；用户只能自己猜。

## 用户决定（2026-11-04）

用户报告该提示并询问是否画幅原因；确认后按 OpenSpec 流程出 change 文档，待用户确认后修复。

## What Changes

- 用"当前平台支持的模型集合"替换 `legacyWindowsTextModel` 的恒真判断：只有会话/草稿保存的文本模型 id 属于**另一平台**时才提示。
- Windows：提示改为"请选择对应的 Windows 型号（Z-Image / Klein / Qwen / MOSS），并核对画幅与参数"；macOS：新增对应提示"该会话保留 Windows 端的模型选择，请选择对应的 macOS 型号"。
- 未就绪标签按平台动态显示（"当前平台未接入"），不再固定写"当前 Windows 未接入"。
- 增加平台 × 模型 id 的纯函数与前端单元测试矩阵，覆盖 Windows 原生 id 不告警、旧 macOS id 告警、macOS 反向场景。
- 不改模型身份、capability id、后端校验或任务语义。

## 成功标准

- Windows 上选择 Klein / Qwen / MOSS / Z-Image 的 Windows id 时**不再**出现跨平台警告；选择旧 macOS id（如 `z-image-turbo-mflux-v1`、`flux2-klein-4b-distilled`）时出现带目标平台与本机型号族提示。
- macOS 上打开含 Windows id 的会话时出现对应提示（而不是只有"未就绪"），选择 macOS 原生 id 时不告警。
- 前端测试覆盖上述矩阵；`npm test`、`npm run check`、`npm run build`、workspace 质量门全绿；安装版冒烟（Windows 选 Klein/Qwen 无警告，选旧 id 有警告；测试在目标机完成）。

## 不做 / 超出范围

- 不做跨平台模型自动迁移/自动改写会话记录（身份保持只读，用户显式选择）。
- 不改后端 stale-default 保留策略（只改提示与标签）。
- 不改变 macOS/Windows 的模型清单与运行时。
