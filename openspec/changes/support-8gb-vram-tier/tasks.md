# Tasks: support-8gb-vram-tier

> 未获用户确认前，以下任务一律不得开始实现。

## 1. 文档与确认

- [ ] 1.1 proposal / design / tasks / test-plan 与四个 spec delta 完成并展示；等待项目所有者确认。
- [ ] 1.2 确认后在本仓库 bd 中建立 change 跟踪 issue，并把任务映射到 issue。

## 2. 设备事实与档位选择（无 GPU 依赖，先做）

- [ ] 2.1 `DeviceFacts` 增加 `compute_capability`：Windows 用 `nvidia-smi --query-gpu=compute_cap` 读取，超时/缺失降级为未知；macOS 与无 NVIDIA 设备保持未知。单测覆盖 7.5、8.6、无法读取三种输入。
- [ ] 2.2 新增档位选择：`standard`（显存 ≥ 条目 standard 门槛）/ `compact`（≥ 8 GB）/ 不可执行。单测覆盖 8 GB、12 GB、显存未知、内存不足四种组合。
- [ ] 2.3 目录 schema 增加 `tiers[]`（id、minVramGb、minRamGb、resolution、computeDtype）与 `standardMinVramGb`；`hardwareEligible` 改为"任一档满足"，`canInstall` / `canUse` 随之调整。现有 `unsatisfied_hardware_disables_entry_points_with_a_chinese_reason` 需改写为"8 GB 机器可选择紧凑档，只有标准档被禁用"。
- [ ] 2.4 模型页与发送区投影：显示当前档位、画幅、精度与降级原因（如"显存 8 GB < 12 GB，自动使用紧凑档 768²"）。前端 `model-catalog.test.ts` 增补 8 GB/12 GB 两组。
- [ ] 2.5 验证：`cargo test -p image-studio-desktop` 相关用例、`npm test` 通过；无 GPU 依赖。

## 3. 协议与配方身份

- [ ] 3.1 在 `crates/worker-protocol` 增加 compact 配方白名单与参数映射：Qwen 基础/Turbo、MOSS、Klein 文生图/编辑、Z-Image；standard 映射逐字不变。
- [ ] 3.2 在 `crates/studio-core/src/recipes.rs` 增加对应 `Recipe` 变体：`id()`、`from_request`、`effective_parameters`、`accepts_output`（768²/512²）、`max_output_bytes`、`Timeouts`。
- [ ] 3.3 协议测试：compact 请求被接受；参数多键/少键/错值被拒绝；macOS 身份仍被拒绝；standard 参数映射快照不变。
- [ ] 3.4 验证：`cargo test -p worker-protocol -p studio-core` 全绿。

## 4. 安装期量化准备的低内存路径

- [ ] 4.1 `prepare.py`（diffusers worker）改为按 safetensors 分片加载→量化→保存→释放，逐组件落盘；保持 prepared 缓存格式与逐组件摘要不变。
- [ ] 4.2 预检从"可用 RAM ≥ 28 GiB"改为分段门槛（目标：可用 RAM ≥ 8 GiB 且 commit 余量 ≥ 6 GiB），失败信息给出可行动建议；常量在 4.3 实测后冻结。
- [ ] 4.3 本机实测：在可用内存 ≈ 19.6 GiB 的当前条件下完成一份真实模型的 prepare，记录 commit/RAM 峰值。
- [ ] 4.4 验证：prepared 缓存可被 standard 与 compact 共用；摘要不匹配时仍拒绝复用。

## 5. Qwen-Image 2.1-Turbo compact（先打通整条链路）

- [ ] 5.1 diffusers worker 支持 `resolution`、`compute_dtype`、`load_strategy` 参数；文本阶段改为逐层上卡；`--probe` 的 NF4 自检按 `compute_dtype` 构造。
- [ ] 5.2 安装：空缓存下载 30.2 GB 模型 + 运行时，逐文件 SHA-256 校验。
- [ ] 5.3 prepare：本机完成 NF4 prepared 缓存（目标 ≤ 10.05 GB）。
- [ ] 5.4 Gate 0：768²、8 步、真实生成；记录 CUDA reserved 峰值（要求 ≤ 6.0 GiB）、commit、可用 RAM 最低值、加载/生成耗时、输出图与 SHA-256；确认无共享内存回退。
- [ ] 5.5 证据：`verification/windows-8gb-tier/qwen21-turbo-gate0.json` + 样图 + README。

## 6. MOSS-Image-SFT 与 Qwen-Image 2.1 compact

- [ ] 6.1 MOSS：下载 25.4 GB → prepare → Gate 0（50 步、CFG 3.0、skip 22、768²）；`vae_precision` 由实测决定。
- [ ] 6.2 Qwen 基础版：下载 30.9 GB → prepare → Gate 0（40 步、768²）。
- [ ] 6.3 两个模型各自记录峰值、耗时、画质样图与 SHA-256；超 6.0 GiB 时按 design R3 降到 512² 重测。

## 7. FLUX.2 Klein 4B compact

- [ ] 7.1 Klein 增加 prepared 预量化（文本编码器 + transformer），消除卡上量化瞬时。
- [ ] 7.2 worker 改为顺序 offload（逐层）+ 768²；`vae_precision` 由实测决定（FLUX VAE 的 fp16 风险）。
- [ ] 7.3 下载 14.9 GB → prepare → Gate 0：文生图与单图编辑各一次，记录峰值与耗时。
- [ ] 7.4 编辑路径不得用 img2img 伪装；源图条件仍走 `source` 输入角色。

## 8. Z-Image Turbo compact（高风险，R1）

- [ ] 8.1 先验证 v1 运行时 + bnb 0.48.2 在 sm75 上能否完成 NF4 自检与装载。
- [ ] 8.2 可行：改造 `studio_zimage_cuda_worker` 支持 768² 与文本编码器 staged。
- [ ] 8.3 不可行：停止并回报，由用户决定是否把 Z-Image 迁到 v2 运行时（新运行时身份，范围扩大）。
- [ ] 8.4 下载 32.8 GB → Gate 0 → 证据。

## 9. 全流程界面验收（8 GB 机器）

- [ ] 9.1 安装后启动应用，模型页对五个模型显示"紧凑档可用"，设置页显示 `显存 8 GB · 内存 32 GB`。
- [ ] 9.2 从界面发送五个模型的真实请求，分别查看结果与导出 PNG。
- [ ] 9.3 取消：每个模型至少一次在采样中取消，任务转 Cancelled，无残留进程。
- [ ] 9.4 强杀：宿主或 worker 被强杀后重启应用，恢复可用状态且不产生半个资产。
- [ ] 9.5 重启后 Ready 记录仍有效，不触发重新下载或重新 prepare。
- [ ] 9.6 证据：`verification/windows-8gb-tier/` README + 截图 + JSON。

## 10. 质量门、文档与归档

- [ ] 10.1 `cargo fmt --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、workspace 测试、前端 test/check/build 全绿。
- [ ] 10.2 `openspec validate --all --strict` 全绿。
- [ ] 10.3 12 GB 档回归：standard 配方参数、身份与预算不变；本机无法执行 12 GB 真机回归，须标注 user-gated 并提供可在 12 GB 机器上一键复跑的命令。
- [ ] 10.4 `doc/` 增加 8 GB 档说明（支持范围、档位选择、实测数据、已知限制），README 支持范围同步。
- [ ] 10.5 汇总证据、归档 change、同步主规格。
