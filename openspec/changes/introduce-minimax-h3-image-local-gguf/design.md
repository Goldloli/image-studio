# Design

## Context

见 `proposal.md` 与各 capability delta。当前实现把模型身份分散在静态目录、`InstallProfile`、`Recipe`、generation v2 白名单和前端 recipe 表中；现有七型号不能通过复制名称或复用相似 worker 扩展。

本项目已有两类可复用边界：

- `tools/realesrgan-worker` 证明 Rust 原生 worker 可以包装第三方可执行文件，并复用受管进程树、候选文件校验和取消合同。
- `tools/inference-supervisor` 已负责进程后代回收、租约和帧上限，不应为 MiniMax-H3-Image 另建生命周期系统。

本设计把 MiniMax-H3-Image 作为第八个逻辑型号、第九项能力，但保持七个稳定型号的 ID、认证和默认行为不变。以下来源在 2026-11-24 核对，实施时必须按清单再次核验并冻结；资料核对不等于本机执行证据。

## Goals / Non-Goals

**Goals:**

- 用独立原生运行时执行 MiniMax-H3-Image 纯文生图。
- 固定完整组件闭包、许可证、来源和 worker/runtime 身份。
- 在 Windows 12 GB/32 GB 与 macOS arm64 分别建立可复现实验 profile。
- 让 6.88 GB DiT、完整下载量、实际显存和系统内存不再混用。
- 以真实安装、离线生成、质量和取消/恢复证据决定是否启用。

**Non-Goals:**

- 不接入 MiniMax-H3-Image 图像编辑、多参考图、透明背景、批量输出或 Prompt Expander。
- 不在首个 profile 承诺 4096²、8 GB Windows 或任意未测试 Mac/GPU。
- 不改写现有七型号的 install ID、snapshot、runtime、recipe、能力 ID 或历史结果。
- 不把 ComfyUI 作为产品运行时依赖，也不要求用户自行安装 ComfyUI。
- 不在本轮实现阶段修改代码、下载权重或声明性能。

## Decisions

### 1. 使用专用 Rust 包装 worker + 固定原生 sidecar

新增 `tools/minimax-h3-worker` crate，依赖 `worker-protocol`，实现 generation v2。它负责：

- 严格校验 H3 请求和冻结参数。
- 以受管子进程启动固定 `sd-cli` sidecar。
- 将 worker 控制通道保持在 stdout，原生日志进入 stderr 和现有有界日志路径。
- 把原生真实阶段或步骤转换为进度；没有可靠步骤数据时只报告阶段。
- 在取消、协议错误、父通道关闭和自身退出时回收 sidecar。
- 验证最终 PNG 的 RGB、尺寸、字节数和 SHA-256，再返回 primary 候选。

不使用 Python/Diffusers，因为完整 H3 权重闭包约 135 GB，且会新增 PyTorch、Transformers 和模型专用内存路径。不直接 FFI 链接 `stable-diffusion.cpp`，因为原生崩溃会带走 Rust worker，跨平台构建和升级也更难隔离。不接 ComfyUI，因为其节点、工作流和用户实例不属于当前单模型安装/执行所有权。

### 2. 固定 MiniMax-H3-Image 与量化来源

| 角色 | 固定来源 | 候选 revision / 文件 | 备注 |
| --- | --- | --- | --- |
| 官方模型 | `MiniMaxAI/MiniMax-H3-Image` | `8ced60068623def5b74b4d10558117cb61d6482d` | 身份与许可证真源 |
| DiT 量化 | `unsloth/MiniMax-H3-Image-GGUF` | `0d25c7349e007529691bbc0f4426b579fce7de01` | Q2/Q3/Q4 对照；Q3_K_S 为首个接受候选 |
| 文本编码器 | `leejet/MiniMax-H3-GGUF` | `eee1dd2f05b755b28bd1b785925cbfeebfcba9d9` | Q2/Q4 GGUF |
| 图像 VAE | `Comfy-Org/MiniMax-H3-Image` | `a70325ac34ca5c0c83df578bb5e02237460c1d8d` | `vae/minimax_h3_image_vae_fp16.safetensors` |
| 推理运行时 | `leejet/stable-diffusion.cpp` | `3b12781fac1a22eda27ad0f08d0884ef816f24d7` | MIT；发布 tag `master-1122-3b12781` |

候选文件按十进制字节计：

| 组件 | 文件 | 字节 | SHA-256 |
| --- | --- | ---: | --- |
| DiT Q2_K | `minimax_h3_image_pruned-Q2_K.gguf` | `7748879072` | `065a0b10a42736584c0c235c50bdb007380fbbb2de9e6a8cf6d5132872d42c4b` |
| DiT Q3_K_S | `minimax_h3_image_pruned-Q3_K_S.gguf` | `8768254688` | `cbf7e586952da45de3fcc6bdb51a9c5fef1dbb42a9abec943c577664be090ccd` |
| DiT Q4_K_M | `minimax_h3_image_pruned-Q4_K_M.gguf` | `12721119968` | `ef784687ff25a974191d00f941388060ef971286fbd2e98b2769fa78c8d599a8` |
| TE Q2_K_M | `qwen3vl_32b_minimax_h3-Q2_K_M.gguf` | `13102161024` | `a8ccadccd57ef34c838ffb8a7da8368bb554721b2760274a1d3b0df63960b997` |
| TE Q4_K_M | `qwen3vl_32b_minimax_h3-Q4_K_M.gguf` | `18218065024` | `11e6efe70a57ce7f4838c47bdbd1a1c4b8ce10e2b7747f1b065990b70f4b05fc` |
| VAE FP16 | `minimax_h3_image_vae_fp16.safetensors` | `1168505312` | `d80e9c2e5cfc9844869961ff5a58c9a9e52182df6bfb30b26018dd7f348721af` |

由此得到：

| 候选 | DiT + TE + VAE | 结论用途 |
| --- | ---: | --- |
| Q2 DiT + Q2 TE | `22,019,545,408` B，约 22.02 GB | 质量不通过；不得作为默认 |
| Q3_K_S DiT + Q2 TE | `23,038,921,024` B，约 23.04 GB | macOS POC 已通过 1024²/2048² |
| Q4_K_M DiT + Q2 TE | `26,991,786,304` B，约 26.99 GB | 更高质量/更高内存对照 |

运行时、许可证、NOTICE、暂存和已命中缓存必须另行计入。任何安装计划都不得只显示 `6.88 GB`。

### 3. 量化和画质设置分开验证

- Abiray Q2_K 在固定 `stable-diffusion.cpp` 上报告 `invalid MiniMax-H3 output head shape`，只保留为不兼容证据；它可能在 ComfyUI-GGUF 中存在另一条支持路线，但不属于本设计。
- Unsloth Q2_K 可生成，但固定提示词中的卡片文字镜像/损坏，产品不接受为默认。
- 首轮产品候选确定为 Unsloth Q3_K_S DiT + Q2 TE；Q4_K_M 保留为质量参照。Q3_K_S 在同一 Mac 上分别完成 1024²、2048²、字节一致的第二次 1024² 和取消测试。
- 每次安装只激活一个明确量化组合。更换 Q2/Q3/Q4 产生新的 install/model snapshot 身份，不原地覆盖。
- 不采用 P-H3-Image 的 4B 文本编码器替代官方 32B 编码器；它是不同模型/连接器组合，需另立 change。

### 4. 固定采样和输出语义

原生调用参数固定为：

```text
--diffusion-model <pinned-gguf>
--llm <pinned-text-encoder>
--vae <pinned-image-vae>
--steps 6
--cfg-scale 1.0
--sampling-method euler
--sigmas minimax-h3-image
--rng cpu
--offload-to-cpu
--vae-tiling
```

- 不接受负提示词、任意 CFG、任意 scheduler、LoRA、参考图或缓存跳步。
- 使用 u32 seed；空种子在宿主解析后写入冻结计划。
- 首期开放 1024² 和 2048²。4096² 只有在对应设备 profile 通过真实测试后另加 catalog/recipe 支持。
- 提示词沿用 16 KiB 产品上限，并以该模型本地 tokenizer 校验 4096 token；超限拒绝而非截断。
- 只输出单张 8-bit RGB PNG；4096² 候选字节上限沿用协议现有 128 MiB 上限，首个 profile 不依赖该上限。

### 5. 目录、安装和协议使用显式身份

macOS POC 已确定 Q3_K_S DiT + Q2 文本编码器为首个候选。Windows 尚未验证；其身份保留在同一候选上，但只有通过 Windows Gate 后才能启用。

| 字段 | Windows | macOS |
| --- | --- | --- |
| 模型/安装 ID | `minimax-h3-image-q3ks-sdcpp-windows-v1` | `minimax-h3-image-q3ks-sdcpp-macos-v1` |
| capability ID | `image.text-to-image.minimax-h3-image.gguf.windows-v1` | `image.text-to-image.minimax-h3-image.gguf.macos-v1` |
| recipe ID | `minimax-h3-image-txt2img-q3-q2te-6-windows-v1` | `minimax-h3-image-txt2img-q3-q2te-6-macos-v1` |
| runtime lock | `minimax-h3-sdcpp-3b12781-windows-cuda12-v1` | `minimax-h3-sdcpp-3b12781-macos-arm64-metal-v1` |
| worker build | `studio-minimax-h3-worker-windows-0.1.0` | `studio-minimax-h3-worker-macos-0.1.0` |

目录增加 `supportLevel: stable | experimental` 和稳定/实验计数，默认值为 `stable`，现有七型号不用改写身份。MiniMax-H3-Image 在通过证据前使用 `planned`，通过后为 `implemented + experimental`；单独统计时不混入七个稳定型号。

安装继续复用现有来源固定、逐文件摘要、缓存和原子激活机制，但为 H3 增加多组件 Ready 记录，记录 DiT、TE、VAE、原生可执行文件和许可证的分别路径。不得把三个文件拼成非标准单文件，也不得借用其他模型 Ready。

### 6. 平台门槛和 4K 分开登记

Windows 首个实验 profile：

- Windows 10 x64、RTX 3080 Ti 12 GB、32 GB RAM。
- 1024² 和 2048²；不登记 compact 档，不开放 4096²。
- 使用 CPU 卸载、VAE tiling 和原生 Flash Attention（可用时）。
- POC 预算：不在 WDDM 共享内存上正常回退，GPU dedicated 峰值留出至少约 1 GB 驱动/桌面余量，系统可用物理内存最低 ≥2 GiB；具体 CUDA reserved/commit 实测值写入报告，不以模型文件大小推算。
- macOS 上 Q3/Q4 的实测进程峰值约 41.6–51.5 GB；这不是 Windows commit 口径，但足以要求 Windows 32 GB 先证明能容纳完整 commit 和桌面余量。
- 2048² 必须在实验时限内完成并支持取消、重试和第二次独立执行；若 12 GB/32 GB 不通过，Windows 保持计划中，不能放宽成 8 GB。

macOS 首个实验 profile：

- 只在当前已记录的 arm64 设备上验证，优先 M3 Max 128 GiB；不能从 Windows 结果推断。
- 使用相同的模型文件身份和独立 Metal runtime，必须实测 Metal 后端、内存峰值、1024²/2048²、取消和整树回收。
- 若 macOS 原生后端不能满足质量或稳定性，可选 `h3-image-mlx` 仅作为实验室替代；它需要新的代码/权重身份与验证，不在本设计默认路线中。

已完成的 macOS 上游 CLI POC：

- Q3_K_S + Q2 TE：1024² 约 56 秒、峰值 footprint 约 43.6 GB；2048² 约 218 秒、峰值约 43.7 GB。
- 相同 seed 的第二次 1024² 独立进程输出 SHA-256 完全相同。
- 2048² 在运行约 80 秒后发送 SIGINT，进程退出且未留下输出；产品 worker 的协议取消与监督器回收仍待实现验证。
- Q4_K_M + Q2 TE：1024² 约 63 秒、峰值约 51.5 GB，质量更好但不作为默认内存档。
- Unsloth Q2_K 的卡片文字出现镜像/损坏，Abiray Q2_K 与固定 runtime 不兼容，两者均不进入正式 profile。

4096² 不进入首个 profile。后续只有在独立设备证据、耗时、内存、候选字节和 UI 交互均满足后才增加。

### 7. 许可证确认是安装前置条件

- 下载前展示完整 Community License、AUP、NOTICE、固定许可证 SHA-256，以及地域/收入/托管服务边界。
- 使用明确勾选和“适用地域 + 使用场景”用户声明；不根据 IP 或系统区域自动推断。
- 不满足或无法确认时禁用下载、安装和生成。
- 用户声明提供托管服务或达到单独授权门槛时，不提供普通本地许可路径，只展示取得 MiniMax 单独授权的说明。
- 许可证与 NOTICE 存入应用管理目录，并与模型条目、安装记录和关于页来源关联。
- 模型页和商业相关界面显示 `MiniMax-H3-Image` 名称。

许可证候选哈希：

- Community License：18,208 B，SHA-256 `0ec3bb5e6d9e885fa2aaf15e84689e50fd51f42b53fa24d3a64ed9d8c76d423b`。
- NOTICE：386 B，SHA-256 `f828a0087e386efe5e1c3039bee87bff734c375ad52fb75f35110957f16de9c1`。
- `stable-diffusion.cpp` MIT LICENSE：1,062 B，SHA-256 `b53fa08f515cb5a6fff7b9fd8fcd0961a4b80df29d74372d25e1f9171aa042ee`。

### 8. 证据门决定启用而不是文档门

依次执行：

1. 上游 CLI/原生二进制身份与许可证校验。
2. 隔离安装、逐文件摘要和空间预检。
3. 设备/后端 probe。
4. 1024² 真实生成和 2048² 真实生成。
5. Q2/Q3/Q4 固定样例质量对照。
6. 禁网第二次生成。
7. 取消、重试、强杀恢复和进程树回收。
8. 原生应用安装、发送、重开、作品查询和导出。

缺任一项时对应平台/画幅保持 `not_run` 或 `blocked`，不得以源码、CI、社区截图或另一平台结果替代。

## Risks / Trade-offs

- **32 GB 内存可能不足** → 先做 Windows POC；失败则 Windows 保持计划中，不偷偷依赖页面文件或降低输出冒充通过。
- **Q2 画质/兼容性失败** → Abiray Q2_K 不兼容固定 runtime，Unsloth Q2_K 的请求文字损坏；默认候选固定从 Unsloth Q3_K_S 开始。
- **原生后端仍在快速变化** → 固定 source commit 与二进制哈希；升级需要新 runtime 身份和重新探针。
- **macOS/Linux 预编译产物部署目标不一致** → POC 可用上游二进制，产品发布优先从固定源码在项目 CI 构建；任何重建产物都重新登记哈希和证据。
- **许可证地域限制** → 使用显式声明和前置阻断；全球发布前仍需法律审阅。
- **模型体积和冷启动大** → 显示完整空间和真实阶段；不自动下载、不默认模型、不做隐式队列。
- **多模型协议回归** → 新 worker 只接受自己的 capability，测试错误 worker、错 snapshot、错字段和错输出角色。

## Migration Plan

1. 新增资源清单、worker、runtime 和 catalog 条目，默认 `planned + experimental`，不提供可执行动作。
2. 在隔离根完成 POC，未通过前不修改产品稳定计数。
3. 某平台通过后，只在该平台的 profile 切换为 `implemented + experimental`；其他平台保持计划中。
4. 发布前更新 README、第三方声明、许可证入口和已知限制。
5. 若质量、稳定性或法务边界不可接受，删除未激活资源和 UI 计划条目；现有七型号与用户资产不受影响。

POC 失败不是实现事故，而是本 change 的合法终止条件。
