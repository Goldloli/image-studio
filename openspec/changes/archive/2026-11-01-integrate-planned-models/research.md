# 调研：三个计划模型与后续接入顺序

核对日期：2026-10-31，Asia/Shanghai。结论来自项目现状与固定上游资料；没有下载模型或执行新后端。机器可读来源为 [HF/运行时元数据](references/hf-and-runtime-metadata.json)和 [GitHub 发布元数据](references/github-source-metadata.json)。这些文件不是正式安装 manifest。

2026-10-31 实施期间，用户明确允许在 Hugging Face 直连或代理不可用时使用已允许的国内 HF 镜像。镜像只可作为同一固定快照的传输备用端点；每个文件仍必须匹配正式 manifest 的字节数和 SHA-256，任何差异都拒绝。模型 revision、许可、来源身份和正式安装验收不因传输端点改变。

## 1. 当前产品事实

2026-10-31 阶段更新：`resources/model-catalog/catalog.json` 仍登记四项；Z-Image、`flux2-klein-4b-distilled` 与 `birefnet-general-epoch-244` 已接入并支持受控安装，`realesrgan-x4plus` 仍为 planned。功能目录的文生图和单图编辑已可执行，去背景也已启用；放大仍为 planned。Klein 与 BiRefNet 的正式结果分别为 50/50 和 31/31 P0，Real-ESRGAN 及三型号整批回归尚未执行。

以下“当前限制”表保留实施前代码审阅快照，用于解释为什么需要公共多型号安装、能力和存储扩展；其中已完成项以上述阶段更新与验收报告为准。

代码审阅通过 CodeGraph 优先定位，得到以下必要改动：

| 位置 | 当前限制 | 接入影响 |
|---|---|---|
| desktop `adapters/installation.rs`、`catalog.rs` | 一个安装组合/快照传给目录投影 | 需要按 install_id 查状态，不能借用 Z-Image Ready |
| core `generation.rs` | freeze/validate 只认 Z-Image、无图输入、9步/q4 | 新增明确能力与配方分发 |
| `tools/mflux-worker/studio_mflux_worker/backend.py` | 仅导入 ZImage，token 预算与9步固定 | 新包/运行时，不覆盖旧 worker 与历史身份 |
| desktop `library.rs` | assets.attempt_id UNIQUE，schema v4 | 一次去背景双输出需迁移与单事务提交 |
| desktop `library/assets.rs` | result.png 单文件；最多1024²、32MiB，PNG RGB/RGBA | 按角色校验 mask，以及16MP放大图的有界解码 |
| desktop `library/conversations.rs`、`studio.rs` | Turn/草稿以文生图参数组织 | 保存能力、受控源图和发送快照，重试不改源图 |
| `adapters/feature_catalog.rs` | 已接入动作仅 open-composer | 保留统一 composer，增加实际操作表单和输入条件 |

现有 AssetKind 已有 ImageRgba/Mask，UserRequest/RunManifest 已有输入和 outputs 数组；可复用领域概念，但实际存储/协议/界面没有因此自动支持图片处理。现有 Z-Image 37/37 P0 是历史基线，本批改动后仍须回归。

## 2. Klein：优先接通第二个生成模型和首个编辑模型

固定目标为 [black-forest-labs/FLUX.2-klein-4B](https://huggingface.co/black-forest-labs/FLUX.2-klein-4B/tree/e7b7dc27f91deacad38e78976d1f2b499d76a294)，revision `e7b7dc27f91deacad38e78976d1f2b499d76a294`，元数据标注 Apache-2.0、未 gating。选 distilled 4B，排除 Base/9B/KV。

复用固定 MFLUX 0.24.0 源码 `8a7fd0a486635f670ef90e02efe1c051ed3f133c` 的[生成/编辑说明](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/flux2/README.md)。两条入口分别为 Flux2Klein 与 Flux2KleinEdit，首轮4步、guidance 1.0、flow_match_euler_discrete、加载q4，固定方图；编辑传一张参考图，不能拿 img2img 的 image_strength 入口冒充指令编辑。

[组件定义](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/flux2/weights/flux2_weight_definition.py)使用独立 transformer、Qwen3-4B 文本编码器和 FLUX.2 VAE，tokenizer 为512含模板预算，chat template 关闭 thinking。预算数字虽与 Z-Image 相同，模板并不相同。q4 只作用于实际可量化层，不代表所有组件均4位。

本次 API 全仓库统计23,740,007,447 bytes；按上述组件目录加配置筛选18文件、15,980,131,745 bytes，约15.98GB。根目录另有重复格式 transformer，不能连同组件树重复下载。候选清单尚缺普通文件的内容SHA-256与无网络闭包验证；正式下载量还需加许可文件和运行时。q4 不缩小这份原始权重下载。

[参考图处理源码](https://github.com/mflux-community/mflux/blob/8a7fd0a486635f670ef90e02efe1c051ed3f133c/src/mflux/models/flux2/variants/edit/flux2_klein_edit_helpers.py)会保长宽比将大图缩到最多1024²，再居中裁掉不足16倍数的余量；它不会把参考图拉成输出方图。产品应预览并冻结这一处理，保存原始源图身份，不能隐式裁剪。首轮不启用 small-text-encoder/small-decoder、LoRA、low-ram 或额外缓存优化。

## 3. BiRefNet：独立 CPU ONNX 路线

目标为[官方 v1 release](https://github.com/ZhengPeng7/BiRefNet/releases/tag/v1)的 `BiRefNet-general-epoch_244.onnx`，asset ID 186740004、972,666,916 bytes。发布 API 没有提供内容digest，因此本轮不知道真实SHA-256；tag/文件名和大小不能替代下载后的完整哈希与图检查。源仓库及官方通用模型卡标注MIT，具体下载文件的来源/许可附件仍须进入正式清单，不混入RMBG等权重。

**实施后修订：** 最终下载内容 SHA-256 为 `58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c`。实际图为 opset 17，固定输入 `input_image` float32 `1×3×1024×1024`，固定输出 `output_image` float32 `1×1×1024×1024`，无外置 initializer；输出节点是最终 Conv，因此配方只对 logits 做一次 sigmoid。正式安装、真实 CPU 推理、双输出、质量、恢复与原生验收均已通过，见[BirefNet 阶段结果](../../../../verification/planned-models-20261031/birefnet-acceptance.json)。

[固定官方 notebook](https://github.com/ZhengPeng7/BiRefNet/blob/f7079632d681ecf12161fcea8905682934e59e51/tutorials/BiRefNet_pth2onnx.ipynb)展示CPUExecutionProvider、RGB→1024²、float32 NCHW、[0,1]及ImageNet均值/标准差归一化，示例对最后输出做sigmoid。该 notebook 当前示例使用另一个dynamic checkpoint，不能证明epoch_244图的opset、输入名或是否自带sigmoid；实施先检查选定图，避免重复sigmoid。若实际图不兼容，保留阻塞并补充设计，不能暗换权重。

拟用独立Python 3.13私有环境与 ONNX Runtime **1.31.1** CPU包；[PyPI固定版本](https://pypi.org/project/onnxruntime/1.31.1/)已有cp313 macOS14 arm64 wheel，20,881,825 bytes，SHA-256见元数据。这仅证明可获得对应wheel，不代表模型图或整套环境运行通过。完整传递依赖、解释器及worker必须另行锁定；复用现有已锁定PBS解释器来源，不在系统环境pip安装。

拟交付原尺寸straight-alpha RGBA与8位灰度mask；RGB像素取规范化源图，alpha取恢复到原尺寸的预测mask。用户可在棋盘格及黑/白底检查边缘。CPU是明确选择的provider，不把Core ML/MPS算作已验证。

## 4. Real-ESRGAN：原生 ncnn + MoltenVK

固定 [ncnn源码](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/tree/37026f49824c5cf84062e7c6a5dd71445dcf610f) `37026f49824c5cf84062e7c6a5dd71445dcf610f`。其v0.2.0官方Mac程序zip为8,532,236 bytes；[release workflow](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/blob/37026f49824c5cf84062e7c6a5dd71445dcf610f/.github/workflows/release.yml)包含arm64/x86_64、lipo及静态MoltenVK，但打包models的命令被注释。故该程序包不能当完整模型安装包。

拟从[Real-ESRGAN主仓库v0.2.5.0 Mac便携包](https://github.com/xinntao/Real-ESRGAN/releases/tag/v0.2.5.0)提取通用 `realesrgan-x4plus.param/.bin`；zip为51,817,124 bytes、asset ID 63494474。本轮未下载解包，内部文件、依赖和架构仍待核对；若不含目标格式就阻塞，不替换社区包。原PyTorch x4plus.pth的67,040,989 bytes不能充当ncnn模型体积或哈希。主仓库BSD-3-Clause、ncnn应用MIT，权重及依赖来源分别留档。

[命令源码](https://github.com/xinntao/Real-ESRGAN-ncnn-vulkan/blob/37026f49824c5cf84062e7c6a5dd71445dcf610f/src/main.cpp)默认是动漫视频模型。产品必须显式传 `-n realesrgan-x4plus -s 4 -t 128 -j 1:1:1 -f png`，指定已核验模型目录和选定GPU，关闭TTA；不接受用户拼接命令。固定tile128是本提案的起始配方，不是本机最优值或性能结论。真实测试比较接缝、全图对照、颜色、输出精确4倍及取消后子进程回收。

本批只接受不带alpha的RGB输入，最大1024×1024，输出最大4096×4096；透明图拒绝并解释，避免暗自合成背景或丢alpha。BiRefNet→放大透明图工作流因此暂不承诺。Mac平台可构建不等于本机MoltenVK路径已通过；安装后需验证实际架构、动态依赖、设备和真实模型运行。

## 5. Qwen/MOSS 后置

既有[详细方案](../../introduce-qwen21-and-moss-image/README.md)已选择Qwen 2.1基础版、官方Turbo及MOSS-SFT。本次重新查询三个固定HF revision，仍可解析，Qwen两版元数据为qwen-research，MOSS为Apache-2.0；没有升级版本或更换路线。该后续方案已获条件确认，须等待本批三型号整批验收与归档后实施。

公共多型号安装、配方路由和v2协议先由本批建立。后续只补Qwen/MOSS的manifest、模型分支和独立验收，不再另造第二套v2或覆盖Klein worker身份。先完成三个计划型号的全部必需项及旧模型回归，再做Qwen基础版/Turbo，随后MOSS。Z-Image历史认证或三个型号中的单项通过不能提前越过整批门槛。

## 6. 实施前仍需取得的证据

| 证据缺口 | 取得阶段 | 不满足时处理 |
|---|---|---|
| 普通组件文件SHA-256、完整依赖闭包 | 批准后的来源/清单准备 | 不激活安装、不声称离线 |
| epoch_244实际ONNX图与输出语义 | BiRefNet独立基线 | blocked，修正同权重适配，不静默换checkpoint |
| 两个ncnn压缩包的内容/架构/链接及转换权重来源 | Real-ESRGAN来源准备 | blocked，记录缺件；路线变化需补方案 |
| 三型号实机速度、内存、效果、取消与恢复 | 各型号真实测试 | failed/not_run/blocked据实保留 |
| 原生入口/重开/导出和全批报告 | 产品与最终验收 | 不归档、不放行Qwen/MOSS |

这些是批准后需要执行的验证工作，不是已知的产品范围选择悬案。所有性能数字在测量前为null；不引用其他机器宣传数字作本机承诺。
