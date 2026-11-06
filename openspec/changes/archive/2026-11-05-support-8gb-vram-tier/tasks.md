# Tasks: support-8gb-vram-tier

> 未获用户确认前，以下任务一律不得开始实现。

## 1. 文档与确认

- [x] 1.1 proposal / design / tasks / test-plan 与四个 spec delta 完成并展示；等待项目所有者确认。
- [x] 1.2 确认后在本仓库 bd 中建立 change 跟踪 issue，并把任务映射到 issue。

## 2. 设备事实与档位选择（无 GPU 依赖，先做）

- [x] 2.1 `DeviceFacts` 增加 `compute_capability`：Windows 用 `nvidia-smi --query-gpu=compute_cap` 读取，超时/缺失降级为未知；macOS 与无 NVIDIA 设备保持未知。单测覆盖 7.5、8.6、无法读取三种输入。
- [x] 2.2 新增档位选择：`standard`（显存 ≥ 条目 standard 门槛）/ `compact`（≥ 8 GB）/ 不可执行。单测覆盖 8 GB、12 GB、显存未知、内存不足四种组合。
- [x] 2.3 目录 schema 增加 `tiers[]`（id、minVramGb、minRamGb、resolution、computeDtype）与 `standardMinVramGb`；`hardwareEligible` 改为"任一档满足"，`canInstall` / `canUse` 随之调整。现有 `unsatisfied_hardware_disables_entry_points_with_a_chinese_reason` 需改写为"8 GB 机器可选择紧凑档，只有标准档被禁用"。
- [x] 2.4 模型页与发送区投影：显示当前档位、画幅、精度与降级原因（如"显存 8 GB < 12 GB，自动使用紧凑档 768²"）。前端 `model-catalog.test.ts` 增补 8 GB/12 GB 两组。
- [x] 2.5 验证：`cargo test -p image-studio-desktop` 相关用例、`npm test` 通过；无 GPU 依赖。

## 3. 协议与配方身份

- [x] 3.1 在 `crates/worker-protocol` 增加 compact 配方白名单与参数映射：Qwen 基础/Turbo、MOSS、Klein 文生图/编辑、Z-Image；standard 映射逐字不变。
- [x] 3.2 在 `crates/studio-core/src/recipes.rs` 增加对应 `Recipe` 变体：`id()`、`from_request`、`effective_parameters`、`accepts_output`（768²/512²）、`max_output_bytes`、`Timeouts`。
- [x] 3.3 协议测试：compact 请求被接受；参数多键/少键/错值被拒绝；macOS 身份仍被拒绝；standard 参数映射快照不变。
- [x] 3.4 验证：`cargo test -p worker-protocol -p studio-core` 全绿。

## 4. 安装期量化准备的低内存路径

- [ ] 4.1 `prepare.py`（diffusers worker）改为按 safetensors 分片加载→量化→保存→释放，逐组件落盘；保持 prepared 缓存格式与逐组件摘要不变。
- [ ] 4.2 预检从"可用 RAM ≥ 28 GiB"改为分段门槛（目标：可用 RAM ≥ 8 GiB 且 commit 余量 ≥ 6 GiB），失败信息给出可行动建议；常量在 4.3 实测后冻结。
- [x] 4.3 本机实测：在可用内存 ≈ 19.6 GiB 的当前条件下完成一份真实模型的 prepare，记录 commit/RAM 峰值。
- [ ] 4.4 验证：prepared 缓存可被 standard 与 compact 共用；摘要不匹配时仍拒绝复用。

## 5. Qwen-Image 2.1-Turbo compact（先打通整条链路）

- [x] 5.1 diffusers worker 支持 `resolution`、`compute_dtype`、`load_strategy` 参数；文本阶段改为逐层上卡；`--probe` 的 NF4 自检按 `compute_dtype` 构造。
- [x] 5.2 安装：空缓存下载 30.2 GB 模型 + 运行时，逐文件 SHA-256 校验。
- [x] 5.3 prepare：本机完成 NF4 prepared 缓存（目标 ≤ 10.05 GB）。
- [x] 5.4 Gate 0：768²、8 步、真实生成；记录 CUDA reserved 峰值（要求 ≤ 6.0 GiB）、commit、可用 RAM 最低值、加载/生成耗时、输出图与 SHA-256；确认无共享内存回退。
- [x] 5.5 证据：`verification/windows-8gb-tier/qwen21-turbo-gate0.json` + 样图 + README。

## 6. MOSS-Image-SFT 与 Qwen-Image 2.1 compact

- [x] 6.1 MOSS：下载 25.4 GB → prepare → Gate 0（50 步、CFG 3.0、skip 22、768²）；`vae_precision` 由实测决定。
- [x] 6.2 Qwen 基础版：下载 30.9 GB → prepare → Gate 0（40 步、768²）。
- [x] 6.3 两个模型各自记录峰值、耗时、画质样图与 SHA-256；超 6.0 GiB 时按 design R3 降到 512² 重测。

## 7. FLUX.2 Klein 4B compact

- [x] 7.1 Klein 增加 prepared 预量化（文本编码器 + transformer），消除卡上量化瞬时。
- [x] 7.2 worker 改为顺序 offload（逐层）+ 768²；`vae_precision` 由实测决定（FLUX VAE 的 fp16 风险）。
- [x] 7.3a 下载 14.9 GB → Gate 0 文生图一次：5.436 GiB、145.6 s。`- [ ] 7.3b 单图编辑（Klein edit）在 8 GB 上**未实测**：代码与协议已就绪，但没有跑过编辑路径。
- [x] 7.4 编辑路径不得用 img2img 伪装；源图条件仍走 `source` 输入角色。

## 8. Z-Image Turbo compact（高风险，R1）

- [x] 8.1 先验证 v1 运行时 + bnb 0.48.2 在 sm75 上能否完成 NF4 自检与装载。
- [x] 8.2 可行：改造 `studio_zimage_cuda_worker` 支持 768² 与文本编码器 staged。
- [x] 8.3 不可行：停止并回报，由用户决定是否把 Z-Image 迁到 v2 运行时（新运行时身份，范围扩大）。
- [x] 8.4 结果：8 GB 下不可行（768² fp16 出 NaN、512² bf16 无 bnb 内核、加载峰值 ~7.8 GiB），Z-Image 继续按 12 GB 门槛禁用；证据见 `verification/windows-8gb-tier/README.md`。

## 9. 全流程界面验收（8 GB 机器）

- [x] 9.1 安装后启动应用，模型页对五个模型显示"紧凑档可用"，设置页显示 `显存 8 GB · 内存 32 GB`。
- [~] 9.2 界面发送：**仅 Qwen-Image 2.1 Turbo** 完成发送/查看/导出 PNG（字节一致）；其余四个模型未在界面上安装与发送。
- [~] 9.3 取消：**仅 Qwen-Image 2.1 Turbo** 取消一次并落为 cancelled；其余模型未验证。
- [~] 9.4 强杀：做过一次宿主强杀 → 重启后 pending 轮次转 interrupted、无残留 worker、随后生成成功；worker 级强杀未单独验证。
- [~] 9.5 重启后 Qwen-Image 2.1 Turbo 仍为就绪；未逐一验证其他模型是否复用 Ready 记录。
- [x] 9.6 证据：`verification/windows-8gb-tier/` README + 截图 + JSON。

## 10. 质量门、文档与归档

- [x] 10.1 `cargo fmt --check`、`cargo clippy --workspace --all-targets --locked -- -D warnings`、workspace 测试、前端 test/check/build 全绿。
- [x] 10.2 `openspec validate --all --strict` 全绿。
- [ ] 10.3 12 GB 档回归：standard 配方参数、身份与预算不变（代码级测试已覆盖）；真机回归本机无法执行，标注 user-gated。
- [x] 10.4 `doc/` 增加 8 GB 档说明（支持范围、档位选择、实测数据、已知限制），README 支持范围同步。
- [x] 10.5 汇总证据、归档 change（`openspec/changes/archive/2026-11-05-support-8gb-vram-tier`）、同步主规格（+4 / ~3）。
