# Tasks: stream-prepare-low-memory

> 未获用户确认前，不得开始实现。确认后按顺序实现并在本机模拟 16 GiB 条件下验收。

## 0. 确认

- [ ] 0.1 项目所有者确认按本提案（子进程分片量化 + 8 GiB/6 GiB 门槛）实施。
- [ ] 0.2 建立 bd issue 跟踪实现与验收；撤回 `fix-prepare-preflight-threshold`（只覆盖 32 GiB 的临时门槛修复）并记录理由。

## 1. 技术验证（spike）

- [ ] 1.1 量化单张量内存实测：在子进程中用 `bnb.functional.quantize_4bit` 量化 Qwen/MOSS 的 64 MB、256 MB、512 MB 张量组，记录 commit 峰值与耗时，冻结组字节预算。
- [ ] 1.2 产出的 bnb 键集合/格式与现有 prepared safetensors 对齐验证：键名、dtype、shape、quant_state 序列化方式一致。
- [ ] 1.3 用最小样例验证"fragment → 拼装 → `from_pretrained` 加载 → `_component_ready`"闭环。

## 2. 实现

- [ ] 2.1 新增 `prepare_shard` 子进程入口（worker 内新模块），支持 `--job <json>`：只加载指定张量、量化、写 fragment、退出。
- [ ] 2.2 coordinator：张量清单/分组、子进程派生与超时、fragment 校验、流式 safetensors 拼装（自写 header + 字节拷贝）、config/generation_config 写入、inventory digest。
- [ ] 2.3 取消与失败：整树回收子进程、失败清理 prepared 目录、保留原快照；旧路径保留为显式降级选项。
- [ ] 2.4 Rust 侧：预检门槛改为可用物理内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB；错误文案与日志（prepare 进度、组数、峰值）更新；单元测试覆盖边界。

## 3. 缓存身份与数值等价

- [ ] 3.1 Qwen transformer（已知 SHA-256 `533cede7…`）逐字节比对；一致则复用身份，不一致则登记新身份。
- [ ] 3.2 Qwen text encoder / MOSS text encoder / MOSS DiT 同样比对；记录每组件产物 SHA-256 与耗时。
- [ ] 3.3 抽样 1% 参数比较反量化权重（bit-identical 或相对误差 < 1e-6）；不一致时记录差异与影响。
- [ ] 3.4 相同提示词/种子下新缓存 vs 旧缓存输出图对比（PSNR + 目视），记录结论。

## 4. 受限内存验收（模拟 16 GiB）

- [ ] 4.1 内存压制工具：独立进程驻留内存把可用物理内存压到 ~8 GiB，记录前后数值。
- [ ] 4.2 Job Object 限制 prepare 进程 commit ≈10 GiB；在模拟条件下全新安装 Qwen-Image 2.1 Turbo 并完成 prepare，记录 commit 峰值/可用内存最低值/耗时。
- [ ] 4.3 标准档 1024² 生成一次并记录 PNG/SHA-256、资源；重启后仍就绪。
- [ ] 4.4 拒绝路径：把可用内存压到 ~7 GiB 重跑，确认量化前失败且无半成品。
- [ ] 4.5 本机正常状态（无模拟）安装 Qwen-Image 2.1 / MOSS 并各生成一次；记录证据。
- [ ] 4.6 在证据 README 明确标注"模拟 16 GiB、非实机"；所有者提供 16 GiB 实机后补实机验收。

## 5. 证据与规格

- [ ] 5.1 更新 `verification/windows-12gb-standard-tier/`：新增低内存 prepare 证据（S3c）与 16 GiB 模拟验收 JSON。
- [ ] 5.2 更新 `doc/15-windows-8gb-tier.md` 与安装门槛说明（16 GiB 机器可安装）。
- [ ] 5.3 同步 `openspec/specs/model-installation/spec.md` 的低内存量化要求与场景（8 GiB/6 GiB、子进程隔离、缓存身份）。
- [ ] 5.4 更新 `certify-windows-10-target-machine` 中 Qwen/MOSS blocked 状态与相关任务。
- [ ] 5.5 `openspec archive stream-prepare-low-memory`，`openspec validate --all --strict` 通过。

## 6. 质量门

- [ ] 6.1 `cargo fmt --check`、`clippy -D warnings`、workspace 测试、前端 test/check/build、`npm audit` 全绿。
- [ ] 6.2 构建 Windows release 并完成安装/生成端到端；测试日志与证据入库。
