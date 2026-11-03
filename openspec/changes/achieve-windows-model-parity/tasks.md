# 实施与验收范围

状态：用户已确认（2026-11-03），按本清单顺序实施。范围是 Windows 与 macOS 七型号/八能力完全对齐；Gate 0 可行性实测已完成（见 verification/windows-parity-gate0）。任务复选框在完成最小验证后勾选，Git 本地小步提交，不自动 push。

## 1. Gate 0 与运行时

- [ ] 1.1 为 Klein、BiRefNet、Real-ESRGAN、Qwen-Image 2.1、Qwen-Image 2.1-Turbo、MOSS-Image-SFT 分别执行隔离 Gate 0：冻结来源 revision/逐文件 SHA-256，记录加载/单次真实执行、独占显存、进程 commit、可用 RAM、质量样图与耗时。
- [x] 1.2 冻结 Windows v2 运行时（Python 3.12.10 / torch 2.10.0+cu126 / torchvision 0.25.0 / diffusers 0.41 / transformers（Qwen/MOSS 兼容版本）/ accelerate / bitsandbytes / onnxruntime），登记 wheel 与运行时清单 SHA-256，证明离线可安装。证据：`resources/windows-parity/runtime-v2/`、`verification/windows-parity-runtime/`（51 artifacts，冻结解释器离线安装 exit 0，49/49 pin 命中）。
- [x] 1.3 冻结 Real-ESRGAN Windows ncnn Vulkan 构建与 x4plus 权重，记录 SHA-256 与 Vulkan 设备身份。证据：`resources/windows-parity/realesrgan-ncnn-vulkan/`、`verification/windows-parity-realesrgan/`（官方 v0.2.5.0 Windows zip 45,474,481 B / sha256 abc02804…，两次真实 4× 运行 pass，RTX 3080 Ti Vulkan 设备身份已记录）。
- [ ] 1.4 决定每个大模型在 12 GB/32 GB 上的执行策略：分阶段量化装载（NF4/INT8/GGUF）或硬件门槛；把决定、预算与失败证据写入 design 补充与目录元数据。
- [x] 1.5 对 MOSS 的 custom pipeline 做固定 revision 的代码审阅与安全边界记录（不联网取代码、不执行未审阅内容）。证据：`verification/windows-parity-gate0/moss-pipeline-review.md`（固定 25,060 B / sha256 1dac1aca…，ModelScope 上游 API 哈希一致；仅 torch/diffusers/transformers，无网络/进程/eval；Windows 采用 worker vendored 代码 + 离线运行）。

## 2. 执行器与 worker

- [ ] 2.1 实现分阶段量化装载执行器（文本→采样→解码，释放与清空缓存、独占显存采样、超预算失败），覆盖 Qwen 基础/Turbo 与 MOSS。
- [ ] 2.2 实现 Windows diffusers worker（Klein 文生图 + 单图编辑；Qwen；MOSS），沿用 generation v2 帧协议、有界日志、取消令牌与租约。
- [ ] 2.3 实现 Windows ONNX Runtime CPU worker（BiRefNet 双输出，复用 macOS 合同）。
- [ ] 2.4 实现 Windows ncnn Vulkan worker（Real-ESRGAN x4plus，固定 4×/tile/单 GPU）。
- [ ] 2.5 接入取消、强杀、整树回收与执行租约，验证每个 worker 不提前允许下一任务、不伪造资产。

## 3. 身份、安装与目录

- [ ] 3.1 登记六个型号独立的 model-variant/recipe/capability/install/runtime/worker 身份，扩展有界平台 profile；不修改历史 ID。
- [ ] 3.2 生成每个型号的 Windows 安装清单（固定来源、逐文件 bytes/SHA-256、许可证归档、空间预算），空缓存与离线安装验证。
- [ ] 3.3 目录与模型页新增 `min_vram_gb`/`min_ram_gb`/`quantization`/`license`/Windows 状态；低于门槛时禁用入口并解释原因；Qwen 显示非商用许可。
- [ ] 3.4 安装空间预检、坏摘要/缺件拒绝、取消恢复沿用第一轮机制并对新模型回归。

## 4. UI 与产品闭环

- [ ] 4.1 功能页/模型页投影七个型号与八项能力；Windows 状态与已安装/可执行/设备满足分离展示。
- [ ] 4.2 原生 UI 中完成每型号真实发送/处理、阶段进度、查看、导出 PNG/RGBA/mask、任务与作品查询。
- [ ] 4.3 验证跨会话互斥、旧记录只读兼容、缺件/旧环境保留、重启恢复。
- [ ] 4.4 人工审阅每个型号质量样图（中英文、写实、文字/布局、编辑、去背景、放大）。

## 5. 验收、证据与归档

- [ ] 5.1 按 test-plan 汇总每个型号 P0 结果，记录未运行/失败/阻塞原因，不以壳通过替代模型验收。
- [ ] 5.2 每型号保存独立 `verification/windows-parity-*` 证据（安装、生成、取消/强杀、UI、资源）。
- [ ] 5.3 更新 README、模型目录、许可证与兼容范围文档。
- [ ] 5.4 OpenSpec 严格校验通过后归档本 change，同步五份主规格；关闭对应 bd issue。
- [ ] 5.5 与用户确认 CI/release 另立 change（GitHub Actions 双平台），本 change 不含。

## 6. 用户已确认的边界

- [ ] 6.1 只认证目标机 12 GB/32 GB；其他硬件不承诺。
- [ ] 6.2 接受 Qwen Research License（非商用）并在产品内标注；MOSS 按 Apache 2.0。
- [ ] 6.3 若某模型 12 GB 内无法安全执行，模型页写明显存/内存门槛并禁用入口，不静默失败。
