# 08 · 第一批模型选型

核对日期：2026-10-29，Asia/Shanghai。本文是首批接入目标的选型建议，不是模型运行注册表，也不代表已安装、已适配或已通过实机验证。

**2026-10-30 交付更新：** Z-Image Turbo 已按用户批准的 `deliver-first-local-generation` 完成本地安装和实机闭环，支持 q4 / 9 步、三个尺寸、取消、恢复重试与 PNG 导出。验证设备为 M3 Max / 128 GiB，证据见[完整验收报告](../verification/first-generation-results.md)。其余三个型号仍为后续接入建议，未下载或认证。下文保留选型时的来源与取舍，不把历史计划当作当前执行清单。

现有产品为 Rust core + Tauri 2 + React。首个验证设备沿用当前工作区已记录的 Apple M3 Max / 128 GiB / macOS；不把这台机器的目标外推为所有 Mac 的兼容承诺。

## 1. 建议名单与顺序

第一批定为 **4 个具体模型**，按“生成 → 编辑 → 抠图 → 放大”逐个完成产品验收。

| 顺序 | 明确型号 | 在产品中的职责 | 推荐 Mac 路线 |
|---|---|---|---|
| 1 | Tongyi-MAI/Z-Image-Turbo | 默认文生图，先打通真实创作闭环 | MFLUX / MLX，隔离 Python worker |
| 2 | black-forest-labs/FLUX.2-klein-4B，蒸馏版 | 补充文生图与自然语言图片编辑 | 与第一个模型共用 MFLUX 运行时 |
| 3 | BiRefNet-general，epoch_244 | 主体分离、去背景、透明 PNG | 官方 ONNX，先验证 CPU provider |
| 4 | RealESRGAN_x4plus | 通用图片 4× 放大 | Real-ESRGAN-ncnn-vulkan，Mac 原生 worker |

这是本轮建议锁定的首批目标；实现仍需单独完成 OpenSpec 并获用户确认。下一条 change 只围绕第 1 个模型构建安装、生成、取消、保存、重启查看和导出闭环，不把四个模型塞进第一次实现。

## 2. Z-Image Turbo：第一个接入

具体来源为 [Tongyi-MAI/Z-Image-Turbo](https://huggingface.co/Tongyi-MAI/Z-Image-Turbo/tree/f332072aa78be7aecdf3ee76d5c247082da564a6)，本次查询 revision 为 f332072aa78be7aecdf3ee76d5c247082da564a6，官方模型卡标注 Apache-2.0。

选择理由：它是 6B 蒸馏文生图模型，上游提供中英文文字与提示词相关能力，MFLUX 已明确实现此家族，适合作为默认创作入口。这里的“默认”是产品职责安排，不是本机速度或质量测试的结论。

第一轮只认证文生图；不把 Z-Image Base、Omni、Edit 或任意 LoRA 自动计入已支持范围。起始实验使用 1024×1024、固定种子和中英文提示词，先验证 4-bit 运行配置，再与较高精度基线核对差异。

采样配方必须按执行器分别记录。MFLUX 固定版本中的 Turbo 示例使用 steps=9；不能把另一实现的 NFE 或步数标签直接当成本产品相同参数。权重、文本编码器、VAE、tokenizer 等组成一个完整快照，不能只检查 transformer 文件。

执行依据：[MFLUX v0.24.0 Z-Image 文档](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/z_image/README.md)。首次不加 LoRA，避免把额外权重和解析时下载混入最小闭环。

## 3. FLUX.2 Klein 4B：同一模型补齐生成与编辑

具体选择 [black-forest-labs/FLUX.2-klein-4B](https://huggingface.co/black-forest-labs/FLUX.2-klein-4B/tree/e7b7dc27f91deacad38e78976d1f2b499d76a294)，revision 为 e7b7dc27f91deacad38e78976d1f2b499d76a294，官方模型卡标注 Apache-2.0。

选择的是 **蒸馏 4B**，不是 4B Base、9B、9B Base 或 9B KV。官方卡说明它支持文生图与多参考编辑；MFLUX 提供生成和编辑两个入口。第一轮产品认证先限制为单图指令编辑，例如替换背景、改变画面氛围、调整物体颜色；多参考、批量编辑和身份保持需后续分别验收。

起始实验为 4 步、固定种子、4-bit 配置，参数语义跟随该型号。模型包包含 transformer、Qwen3-4B 文本编码器、专用 FLUX.2 VAE 等组件，不复用 Z-Image 的 VAE；第一轮不启用额外的小文本编码器替代方案。

理由是它能在同一个 MFLUX 运行时里增加真实编辑能力，减少首批依赖的扩散。上游的其他 GPU 延迟和显存宣传值不作为这台 Mac 的性能保证。

执行依据：[MFLUX v0.24.0 FLUX.2 文档](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/flux2/README.md)。

## 4. BiRefNet-general：选定一个固定通用权重

选 [官方 v1 release 的 BiRefNet-general-epoch_244.onnx](https://github.com/ZhengPeng7/BiRefNet/releases/tag/v1)，不把整个 BiRefNet 家族视为同一模型。release API 记录该 ONNX 文件为 972,666,916 字节；这只是上游文件元数据，未下载校验内容哈希。

第一轮任务是输入一张图，生成前景 mask 并导出透明 PNG；原图不覆盖。先使用官方 ONNX 的 CPU 路线验证正确性、边缘和耗时，再决定是否增加加速 provider。没有本轮证据能承诺 Core ML、MPS 或 Apple GPU 已经可用。

官方 [ONNX notebook](https://github.com/ZhengPeng7/BiRefNet/blob/f7079632d681ecf12161fcea8905682934e59e51/tutorials/BiRefNet_pth2onnx.ipynb) 显式包含 CPUExecutionProvider / CUDAExecutionProvider，示例转换为 opset 17、1024×1024。执行时需要核对选定文件的实际输入输出与预处理；不因为新版 README 提到 opset 22 就把现有 release 权重标成新版转换。

[官方 HF 模型卡](https://huggingface.co/ZhengPeng7/BiRefNet/tree/e2bf8e4460fc8fa32bba5ea4d94b3233d367b0e4) 与 [源仓库](https://github.com/ZhengPeng7/BiRefNet/tree/f7079632d681ecf12161fcea8905682934e59e51) 标注 MIT。发布模型包时仍需附上该具体文件的来源和许可记录，不混入 RMBG 等其他产品权重。

## 5. Real-ESRGAN x4plus：明确选择通用 4×

模型身份为 [RealESRGAN_x4plus.pth，官方 v0.1.0](https://github.com/xinntao/Real-ESRGAN/releases/tag/v0.1.0)，release API 记录为 67,040,989 字节。实际推荐的 Mac 执行格式是 ncnn 的 realesrgan-x4plus，对应 .param + .bin；原始 PyTorch 权重与转换权重必须分别记录格式、来源和哈希。

第一轮只认证通用 4× 放大和分块执行，不默认增加人脸修复、动漫专用模型或任意倍率重采样。执行命令必须显式指定 realesrgan-x4plus，因为上游程序默认模型是动漫视频型号。

选择 [Real-ESRGAN-ncnn-vulkan](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/tree/37026f49824c5cf84062e7c6a5dd71445dcf610f) 路线：其 [release workflow](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/blob/37026f49824c5cf84062e7c6a5dd71445dcf610f/.github/workflows/release.yml) 包含 arm64 / x86_64 构建、lipo 和静态 MoltenVK；这是明确的 Mac 构建路径，不是本机运行通过的证据。

上游 ncnn 仓库的 v0.2.0 Mac zip 是程序包，workflow 未拷贝 models。主仓库另有 [v0.2.5.0 Mac 便携整包](https://github.com/xinntao/Real-ESRGAN/releases/tag/v0.2.5.0)，本次元数据记录 51,817,124 字节。安装 manifest 必须分别核对可执行程序、依赖和所选模型文件，不能用“zip 存在”代替完整性检查。本轮没有下载或检查这些压缩包的实际内容。

许可分别记录：[Real-ESRGAN 主仓库为 BSD-3-Clause](https://github.com/xinntao/Real-ESRGAN/blob/a4abfb2979a7bbff3f69f58f58ae324608821e27/LICENSE)，[ncnn 应用仓库为 MIT](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/blob/37026f49824c5cf84062e7c6a5dd71445dcf610f/LICENSE)。不据程序许可证自动推导所有转换权重的分发条件，具体模型包在接入时保留对应声明。

## 6. 第一批暂缓的候选

| 候选 | 本轮决定与理由 |
|---|---|
| SDXL Base 1.0 / Refiner | 排到第二批的生态兼容线。它有成熟的经典工作流价值，但不与首批 MLX 路线共用执行器；初轮以新增生成、编辑和工具能力为主。以后先 Base，Refiner 单独评估。 |
| Qwen-Image 2.1 / Turbo | Turbo 当前权重许可是 qwen-research，不能沿用旧 Qwen 家族的 Apache-2.0 判断；先不进入默认首批。 |
| Qwen-Image-Edit-2511 | 保留为进阶中文/指令编辑候选，具体大模型资源开销和产品收益在 Klein 首轮实测后再评估。 |
| FLUX 3 Image Klein | 已发布且存在上游路线，安排后续能力增量评估；第一轮不同时引入新布局语义和更多模型组件。 |
| FLUX.2 Klein 9B / Base | 不继承 4B 蒸馏版的许可证或采样配方，不纳入本轮。 |
| 多种 LoRA、ControlNet、动漫放大与人脸修复型号 | 按具体能力和文件作为后续扩展，不把家族支持等同于任意派生权重都可运行。 |

**本次许可核查更新：** [Qwen-Image-2.1-Turbo 的固定版本 LICENSE](https://huggingface.co/Qwen/Qwen-Image-2.1-Turbo/blob/fc13ba1986fcb905854dc8de2f52eee395f49b5b/LICENSE) 明确只授予非商业研究/评估用途，商业用途需另行授权。这里记录上游条款事实，不把旧版本或同家族许可套到这个模型。

## 7. 推理运行时的选择

推荐首个生成运行时为 **MFLUX 0.24.0 / MLX**，固定 tag v.0.24.0，源码 commit 8a7fd0a486635f670ef90e02efe1c051ed3f133c；首个 Z-Image 闭环现已交付[完整依赖锁](../resources/local-generation/requirements.lock)与[运行时安装清单](../resources/local-generation/runtime-manifest.json)。

该版本的 [pyproject.toml](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/pyproject.toml) 要求 Python ≥3.11，macOS MLX 0.32.x，并包含 torch、transformers 等依赖；[代码许可证](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/LICENSE) 是 MIT。不能把它描述为无 Python、无 PyTorch 依赖的轻量原生包。

首个闭环已按 OpenSpec 批准并完成：Rust 管任务、资产、安装和协议，Python 仅位于受管理的推理 worker 中。基础应用不依赖用户手动配置系统 Python；只在需要模型时安装隔离运行时。BiRefNet 的 ONNX 与 Real-ESRGAN 的 ncnn 在后续任务里独立接入，不要求第一次同时交付三套执行器。

若 Z-Image Turbo 在此设备上不能通过关键门槛，再对同一模型用固定版 stable-diffusion.cpp 做替代路线实验；不能以“有 Metal 支持”直接给出快慢排名。

## 8. 从“入选”到“产品支持”的门槛

选型时四个目标均未实现。2026-10-30，Z-Image Turbo 的指定组合已完成实现和本机验证，模型中心反映真实就绪状态；其余三个目标仍为 product_status=not_implemented、local_hardware_verification=not_run。时间、最大 RSS 和采样磁盘占用的口径与限制见验收报告；更高精度画质对照和其他硬件留后续变更。

进入正式目录前，每个精确配置需提供完整组件与文件哈希、运行时锁、设备与参数、真实输出、取消/失败/资源回收、结果保存/重启查看/导出证据。4-bit 配置不等于下载字节或峰值内存按比例缩小；普通内存规格 Mac 需另行认证。

首个闭环的验收只针对 Z-Image Turbo 的指定组合；通过后再逐个增加其余三个。机器可读的选型记录见 [first-model-batch.json](first-model-batch.json)；它不得直接作为下载或执行 manifest。
