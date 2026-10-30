# 05 · 一手来源与研究证据

研究日期：2026-10-28，Asia/Shanghai。返回[总览](README.md)。

## 1. 研究方法与证据边界

本次优先阅读模型作者、运行时维护者和平台提供者的一手仓库、模型卡与官方文档。没有使用论坛性能截图或二手排行榜来证明本产品的可用性，也没有把任一上游 README 当成实机验收。

已完成的工作：核对公开架构、界面设计、运行时接口、模型家族与发布状态、组件要求、平台执行路线、模型下载缓存和竞品公开资料。未完成、也未声称完成的工作：模型权重下载、真实图片推理、Mac/Windows 性能测试、安装包签名公证、运行时依赖锁定或商业使用许可审查。

来源状态为“上游文档已阅读”，与“Image Studio 已实现或认证”严格分开。Apple 公证文档只确认了官方入口，正文工具获取不完整，报告没有据此给出已验证的签名命令。

## 2. 一手资料中对架构最有影响的发现

| 发现 | 影响 | 来源 |
|---|---|---|
| Photocraft 采用 Rust UI 与独立引擎边界，并维护统一 UI 规则 | 设计系统和核心分层可以分别借鉴 | S001、S002 |
| 主流推理生态同时存在原生、MLX、Python 与图工作流路线 | Rust 产品核心应接入异构 worker | S007–S017 |
| 新图片模型包含多参考、布局与透明生成等能力 | 能力 schema 要能表达不同输入输出 | S029、S031、S035 |
| 像素空间生成并不要求通用 VAE 组件 | 组件图不能把 VAE/UNet 写死 | S034、S069 |
| Z-Image 的部分介绍变体在所读表中仍待发布 | 目录收录与可下载需要分开 | S032 |
| Qwen 2.1 有专用组件兼容要求 | 家族名相近不能替代组件校验 | S070 |
| 模型运行量化与原始下载大小可能不同 | 下载、安装、驻留与峰值需分别表达 | S068、S069 |
| Windows ONNX 路线已有 DirectML 与 WinML 的演进说明 | Windows 选型需复核当前 provider 状态 | S020 |

这些是产品设计依据，不是所有相关项目的完整综述。报告中的总体产品蓝图、调度、状态、目录、分期和场景是原创建议，不代表这些上游项目已经采用同样方案。

## 3. 完整来源目录

每个来源在 [sources.json](sources.json) 中具有相同 ID、URL、说明和核对日期。链接指向研究时访问的原始入口或文件；未选择运行时发行版本，后续接入需固定 commit 或 release。

<a id="s001"></a>

### S001 · Photocraft 架构

[原始资料](https://github.com/storytold/photocraft/blob/main/docs/architecture.md)。Rust 界面与图像引擎分层参考。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s002"></a>

### S002 · Photocraft 界面设计系统

[原始资料](https://github.com/storytold/photocraft/blob/main/docs/ui-design.md)。主题 token、共用控件和截图验证参考。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s003"></a>

### S003 · egui

[原始资料](https://github.com/emilk/egui)。Rust 即时模式 GUI 与 eframe 集成。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s004"></a>

### S004 · Iced

[原始资料](https://github.com/iced-rs/iced)。Rust 跨平台声明式 GUI 候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s005"></a>

### S005 · Slint

[原始资料](https://github.com/slint-ui/slint)。声明式 GUI 候选；具体分发许可需按选定版本复核。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s006"></a>

### S006 · GPUI

[原始资料](https://gpui.rs/)。GPU 加速的 Rust UI 候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s007"></a>

### S007 · Candle

[原始资料](https://github.com/huggingface/candle/blob/main/README.md)。Rust ML 框架与模型示例；不等于自动支持所有图片模型。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s008"></a>

### S008 · stable-diffusion.cpp

[原始资料](https://github.com/leejet/stable-diffusion.cpp/blob/master/README.md)。多图片模型、量化和多硬件后端的 C/C++ 推理实现。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s009"></a>

### S009 · stable-diffusion.cpp 构建说明

[原始资料](https://github.com/leejet/stable-diffusion.cpp/blob/master/docs/build.md)。Metal、CUDA、Vulkan 构建及性能注意事项。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s010"></a>

### S010 · MLX

[原始资料](https://github.com/ml-explore/mlx/blob/main/README.md)。MLX API、统一内存与运行时范围。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s011"></a>

### S011 · MFLUX

[原始资料](https://github.com/mflux-community/mflux/blob/main/README.md)。MLX 图片模型实现与功能入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s012"></a>

### S012 · Diffusers

[原始资料](https://github.com/huggingface/diffusers)。广泛的图片模型 pipeline 生态。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s013"></a>

### S013 · Diffusers MPS

[原始资料](https://huggingface.co/docs/diffusers/main/en/optimization/mps)。Apple MPS 路径；文档不构成每个模型的实机验证。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s014"></a>

### S014 · Modular Diffusers

[原始资料](https://github.com/huggingface/diffusers/blob/main/docs/source/en/modular_diffusers/modular_pipeline.md)。可组合 pipeline 思路。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s015"></a>

### S015 · ComfyUI

[原始资料](https://github.com/Comfy-Org/ComfyUI/blob/master/README.md)。模型、节点及多设备生态。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s016"></a>

### S016 · ComfyUI Server Routes

[原始资料](https://docs.comfy.org/development/comfyui-server/comms_routes)。队列、工作流和 WebSocket 接口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s017"></a>

### S017 · Apple Core ML Stable Diffusion

[原始资料](https://github.com/apple-aiml-research/ml-stable-diffusion)。Core ML 图片模型部署参考。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s018"></a>

### S018 · ort

[原始资料](https://github.com/pykeio/ort)。ONNX Runtime 的 Rust 接口项目。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s019"></a>

### S019 · ONNX Runtime CoreML EP

[原始资料](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html)。Core ML Execution Provider 与算子约束。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s020"></a>

### S020 · ONNX Runtime DirectML EP

[原始资料](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)。DirectML 持续维护状态，以及 Windows 新功能转向 WinML 的说明。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s021"></a>

### S021 · HF Hub 文件下载

[原始资料](https://huggingface.co/docs/huggingface_hub/guides/download)。模型快照、revision 与选择下载。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s022"></a>

### S022 · HF Hub 缓存

[原始资料](https://huggingface.co/docs/huggingface_hub/guides/manage-cache)。共享对象、快照与缓存管理参考。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s023"></a>

### S023 · Safetensors

[原始资料](https://huggingface.co/docs/safetensors/index)。权重容器及安全加载边界。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s024"></a>

### S024 · Diffusers 加载 Adapter

[原始资料](https://huggingface.co/docs/diffusers/main/en/using-diffusers/loading_adapters)。LoRA、IP-Adapter 等接入；兼容性需逐配方确定。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s025"></a>

### S025 · SDXL 模型卡

[原始资料](https://huggingface.co/stabilityai/stable-diffusion-xl-base-1.0)。SDXL 基础模型与模型卡。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s026"></a>

### S026 · SD 3.5 Large 模型卡

[原始资料](https://huggingface.co/stabilityai/stable-diffusion-3.5-large)。SD 3.5 家族入口与模型许可元数据。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s027"></a>

### S027 · FLUX.1

[原始资料](https://github.com/black-forest-labs/flux/blob/main/README.md)。FLUX.1 及 Fill、Kontext、Redux 等变体。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s028"></a>

### S028 · FLUX.2

[原始资料](https://github.com/black-forest-labs/flux2/blob/main/README.md)。FLUX.2 Klein 与基础/蒸馏分支。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s029"></a>

### S029 · FLUX 3 Image Klein

[原始资料](https://github.com/black-forest-labs/flux-3-image-klein/blob/main/README.md)。生成、编辑、多参考与布局控制入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s030"></a>

### S030 · Qwen-Image

[原始资料](https://github.com/QwenLM/Qwen-Image/blob/main/README.md)。Qwen-Image 原始与编辑家族。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s031"></a>

### S031 · Qwen-Image 2.1

[原始资料](https://github.com/QwenLM/Qwen-Image-2.1/blob/main/README.md)。统一生成与编辑、透明输出等新能力。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s032"></a>

### S032 · Z-Image

[原始资料](https://github.com/Tongyi-MAI/Z-Image/blob/main/README.md)。Turbo/Base 可下载；本次 README 将 Omni-Base/Edit 仍标为 To be released。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s033"></a>

### S033 · Krea 2

[原始资料](https://github.com/krea-ai/krea-2/blob/main/README.md)。RAW 与 Turbo 分支。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s034"></a>

### S034 · Iris-3B

[原始资料](https://github.com/speridlabs/iris-3b/blob/main/README.md)。像素空间生成，不应强制要求 VAE。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s035"></a>

### S035 · MOSS-Image

[原始资料](https://github.com/OpenMOSS/MOSS-Image/blob/main/README.md)。布局、文字和生成模型家族入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s036"></a>

### S036 · Anima

[原始资料](https://huggingface.co/circlestone-labs/Anima)。插画与动漫模型家族、模型卡。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s037"></a>

### S037 · SANA

[原始资料](https://github.com/NVlabs/Sana/blob/main/README.md)。高效生成家族；图片与视频分支需分别分类。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s038"></a>

### S038 · HunyuanImage 3.0

[原始资料](https://github.com/Tencent-Hunyuan/HunyuanImage-3.0/blob/main/README.md)。生成与 Instruct 家族；Mac 路径未实测。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s039"></a>

### S039 · HiDream-I1

[原始资料](https://github.com/HiDream-ai/HiDream-I1/blob/main/README.md)。Full/Dev/Fast 家族及外部文本编码器依赖。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s040"></a>

### S040 · PixArt-Sigma

[原始资料](https://github.com/PixArt-alpha/PixArt-sigma/blob/master/README.md)。历史与扩展生成家族。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s041"></a>

### S041 · Lumina-Image 2.0

[原始资料](https://github.com/Alpha-VLLM/Lumina-Image-2.0/blob/main/README.md)。独立生成架构。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s042"></a>

### S042 · Kolors

[原始资料](https://github.com/Kwai-Kolors/Kolors/blob/master/README.md)。中文生成生态入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s043"></a>

### S043 · OmniGen2

[原始资料](https://github.com/VectorSpaceLab/OmniGen2/blob/main/README.md)。统一生成与编辑模型入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s044"></a>

### S044 · BAGEL

[原始资料](https://github.com/ByteDance-Seed/Bagel/blob/main/README.md)。理解和生成统一模型入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s045"></a>

### S045 · Janus

[原始资料](https://github.com/deepseek-ai/Janus/blob/main/README.md)。自回归多模态模型入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s046"></a>

### S046 · GLM-Image

[原始资料](https://github.com/zai-org/GLM-Image/blob/main/README.md)。多模态图片生成架构入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s047"></a>

### S047 · LongCat-Image

[原始资料](https://github.com/meituan-longcat/LongCat-Image/blob/main/README.md)。生成与编辑模型入口。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s048"></a>

### S048 · Real-ESRGAN

[原始资料](https://github.com/xinntao/Real-ESRGAN/blob/master/README.md)。实用超分与修复家族。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s049"></a>

### S049 · HAT

[原始资料](https://github.com/XPixelGroup/HAT/blob/main/README.md)。超分家族。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s050"></a>

### S050 · SUPIR

[原始资料](https://github.com/Fanghua-Yu/SUPIR/blob/master/README.md)。生成式修复候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s051"></a>

### S051 · SeedVR / SeedVR2

[原始资料](https://github.com/ByteDance-Seed/SeedVR)。修复模型；本次以仓库网页核对，不能外推全部平台能力。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s052"></a>

### S052 · GFPGAN

[原始资料](https://github.com/TencentARC/GFPGAN/blob/master/README.md)。人脸修复候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s053"></a>

### S053 · CodeFormer

[原始资料](https://github.com/sczhou/CodeFormer/blob/master/README.md)。人脸修复候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s054"></a>

### S054 · BiRefNet

[原始资料](https://github.com/ZhengPeng7/BiRefNet/blob/main/README.md)。前景分离候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s055"></a>

### S055 · rembg

[原始资料](https://github.com/danielgatis/rembg/blob/main/README.md)。抠图工作流与多模型封装参考。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s056"></a>

### S056 · SAM 2

[原始资料](https://github.com/facebookresearch/sam2/blob/main/README.md)。交互式分割候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s057"></a>

### S057 · SAM 3

[原始资料](https://github.com/facebookresearch/sam3/blob/main/README.md)。概念分割候选；与 SAM 2 不能视为同一模型包。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s058"></a>

### S058 · Depth Anything V2

[原始资料](https://github.com/DepthAnything/Depth-Anything-V2)。深度估计候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s059"></a>

### S059 · GroundingDINO

[原始资料](https://github.com/IDEA-Research/GroundingDINO)。文本定位和检测候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s060"></a>

### S060 · IP-Adapter

[原始资料](https://github.com/tencent-ailab/IP-Adapter/blob/main/README.md)。图像参考条件适配。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s061"></a>

### S061 · PhotoMaker

[原始资料](https://github.com/TencentARC/PhotoMaker/blob/main/README.md)。人物身份参考候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s062"></a>

### S062 · ControlNet

[原始资料](https://github.com/lllyasviel/ControlNet)。结构控制方法与模型生态。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s063"></a>

### S063 · PaddleOCR

[原始资料](https://github.com/PaddlePaddle/PaddleOCR)。图片中文字检测和识别辅助。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s064"></a>

### S064 · InvokeAI

[原始资料](https://github.com/invoke-ai/InvokeAI)。创作工作区与画布工作流参考。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s065"></a>

### S065 · Draw Things 社区仓库

[原始资料](https://github.com/drawthingsai/draw-things-community)。Mac 产品体验与社区接口参考，不代表整个应用源码开放。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s066"></a>

### S066 · Fooocus

[原始资料](https://github.com/lllyasviel/Fooocus)。降低参数负担的交互参考。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s067"></a>

### S067 · SwarmUI

[原始资料](https://github.com/mcmonkeyprojects/SwarmUI)。面向用户的多后端编排参考。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s068"></a>

### S068 · MFLUX Qwen 2.1 编辑说明

[原始资料](https://github.com/mflux-community/mflux/blob/main/src/mflux/models/qwen21/reference/README.md)。多参考、RGBA、下载与内存口径示例。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s069"></a>

### S069 · MFLUX Iris 说明

[原始资料](https://github.com/mflux-community/mflux/blob/main/src/mflux/models/iris/README.md)。像素空间架构、量化和冷加载注意事项。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s070"></a>

### S070 · stable-diffusion.cpp Qwen 2.1

[原始资料](https://github.com/leejet/stable-diffusion.cpp/blob/master/docs/qwen_image_2.1.md)。专用 VAE 与文本/视觉编码器兼容关系。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s071"></a>

### S071 · stable-diffusion.cpp FLUX 3

[原始资料](https://github.com/leejet/stable-diffusion.cpp/blob/master/docs/flux3.md)。多组件和参考图输入方式。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s072"></a>

### S072 · Apple macOS 公证文档

[原始资料](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)。仅核对官方文档入口；工具未取得完整正文，不据此提供已验证的签名参数。

核对状态：官方入口已确认，正文未完整获取；Image Studio 实机验证：未运行。

<a id="s073"></a>

### S073 · MFLUX 共用参数说明

[原始资料](https://github.com/mflux-community/mflux/blob/main/src/mflux/models/common/README.md)。量化、计算精度和 Apple M1 家族专用参数。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s074"></a>

### S074 · MFLUX SeedVR2

[原始资料](https://github.com/mflux-community/mflux/blob/main/src/mflux/models/seedvr2/README.md)。静态图片放大路径与内存模式。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s075"></a>

### S075 · Tokio

[原始资料](https://docs.rs/tokio/latest/tokio/)。异步任务、通道、IO 与进程管理的 Rust 候选基础。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s076"></a>

### S076 · Serde

[原始资料](https://docs.rs/serde/latest/serde/)。Rust 类型与序列化基础。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s077"></a>

### S077 · rusqlite

[原始资料](https://docs.rs/rusqlite/latest/rusqlite/)。SQLite Rust 接入候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s078"></a>

### S078 · image

[原始资料](https://docs.rs/image/latest/image/)。图片编解码与基础处理候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s079"></a>

### S079 · tracing

[原始资料](https://docs.rs/tracing/latest/tracing/)。结构化诊断与事件记录。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s080"></a>

### S080 · reqwest

[原始资料](https://docs.rs/reqwest/latest/reqwest/)。HTTP 与下载基础候选。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s081"></a>

### S081 · rfd

[原始资料](https://docs.rs/rfd/latest/rfd/)。跨平台原生文件对话框。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

<a id="s082"></a>

### S082 · keyring

[原始资料](https://docs.rs/keyring/latest/keyring/)。系统凭据候选；版本与 feature 的实际行为需集成验证。

核对状态：上游公开资料已核对；Image Studio 实机验证：未运行。

## 4. 后续复核的方法

模型接入时再次核对代码与权重来源，固定 revision、运行时版本与组件散列。新发布消息只能推进发现状态；完整文件与可用后端才能推进适配研究；真实产品链路证据才能推进本机验证与认证。

运行时文档中的支持列表需展开到任务、变体、精度、控制组件和设备。任何性能数字都应带完整环境与冷/热口径；许可信息保存原始链接和版本，按实际组件组合重新核对。

## 5. 资料限制与未解决问题

没有使用目标硬件运行模型，所以不能据本报告确定首批模型的实际排序、最低内存、最佳后端或耗时。没有选择固定 UI/推理库版本，因此没有锁定其最终 API、二进制尺寸或平台支持边界。模型目录中的路线为候选，并非可直接执行配置。

目标 Git 仓库未配置 remote，本次未验证是否为最新代码。以上不妨碍完成所请求的脑爆与研究报告，但进入实现时需要把选定范围转成正式规格和真实实验。
