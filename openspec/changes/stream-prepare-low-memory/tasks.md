# Tasks: stream-prepare-low-memory

> 状态（2026-11-08）：低内存 prepare 已实现并在**模拟 16 GiB** 条件下完成产品内安装验收；Qwen Turbo 生成在修复一个独立的文本阶段释放缺陷后通过。剩余：MOSS 产品级安装/生成、真实 16 GiB 机器证据、规格归档。

## 0. 确认

- [x] 0.1 项目所有者确认按本提案（子进程分片量化 + 8 GiB/6 GiB 门槛）实施（2026-11-08）。
- [x] 0.2 建立 bd issue `image-studio-2f5`；撤回 `fix-prepare-preflight-threshold`（已合并进本 change）。

## 1. 技术验证（spike）

- [x] 1.1 单张量/分组量化内存实测：子进程只读目标区间（`open+seek+read`），避免 `safe_open` 提交整文件；单张量 bnb 量化临时量约 0.6–1 GiB，组预算冻结为 1 GiB 源字节。
- [x] 1.2 产出 bnb 键集合/格式与现有 prepared safetensors 对齐：`weight`(U8) + `absmax`/`nested_absmax`/`nested_quant_map`/`quant_map`/`quant_state.bitsandbytes__nf4`，dtype/shape 一致。
- [x] 1.3 fragment → 流式拼装 → `from_pretrained` 加载闭环验证（Qwen/MOSS 四组件全部通过，见 3.x）。

## 2. 实现

- [x] 2.1 新增 `backends/low_memory.py`：coordinator + `prepare_shard` 子进程入口（`--job`），子进程加载指定张量、量化、写 fragment、退出。
- [x] 2.2 coordinator：meta 骨架推导张量顺序/量化集合、1 GiB 分组、子进程超时、fragment 校验、流式 safetensors 拼装（dtype 分组 + 字典序，复刻 safetensors 写出顺序）、config/generation_config 写入、inventory 摘要。
- [x] 2.3 取消与失败：子进程超时/失败抛出并保留原快照；旧的整组件路径保留为 `STUDIO_PREPARE_LEGACY=1` 降级选项。
- [x] 2.4 Rust 侧预检改为可用物理内存 ≥8 GiB **且** commit 余量 ≥6 GiB；错误文案与单测更新（`prepare_memory_preflight` 边界 8/6）。

## 3. 缓存身份与数值等价

- [x] 3.1 Qwen transformer：新缓存 `010c2c3bfd110179…` 与 2026-11-04 reference **逐字节一致**。
- [x] 3.2 Qwen text encoder `13c2f7cdb087b336…`、MOSS text encoder `de9b3f5f02e69270…`、MOSS DiT `fe8c7bcbf4c63366…` 均逐字节一致。
- [x] 3.3 权重逐字节一致即数值等价；config.json 的 `_name_or_path`/rope 字段序列化与非路径元数据与旧缓存不同 → prepared 缓存身份按新 inventory 登记（不静默混用）。
- [x] 3.4 产品内生成验证：Qwen Turbo 标准档 1024² 8 步在修复文本阶段释放缺陷后通过（见 S3c）。

## 4. 受限内存验收（模拟 16 GiB）

- [x] 4.1 内存压制工具 `target/reserve_memory.py`：预留并触碰 11 GiB，launch 时可用物理内存 11.35 GiB、commit 余量 13.59 GiB。
- [x] 4.2 该状态下产品内安装 Qwen-Image 2.1 Turbo：预检 11.75 GiB/12.95 GiB 通过；low-memory prepare 文本 125.17 s（worker commit 峰值 2.927 GiB、系统可用最低 9.353 GiB）、transformer 105.32 s（2.911 GiB / 9.223 GiB）；总安装 395 s；缓存逐字节一致。证据 `verification/windows-12gb-standard-tier/S3b-*.json`。
- [x] 4.3 安装后 Qwen Turbo 标准档 1024² 8 步生成通过（40.7 s，PNG 2,010,413 B，SHA-256 `92a9d4f8…`）——修复见 `fix-qwen-standard-text-cleanup`（S3c）。
- [x] 4.4 拒绝路径：Rust 单测覆盖 8/6 GiB 边界（7/5 拒绝、8/6 通过、任一单项不足拒绝）；门槛文案已更新。
- [ ] 4.5 本机正常状态安装 Qwen-Image 2.1 / MOSS 并各生成一次：Qwen base 未安装、MOSS 产品级安装/生成未跑（低内存 prepare 的 MOSS 四组件已逐字节验证）。
- [ ] 4.6 证据 README 明确"模拟 16 GiB、非实机"：见 `verification/windows-12gb-standard-tier/README.md`（待补本 change 小节）；真实 16 GiB 实机证据待所有者提供机器后补。

## 5. 证据与规格

- [ ] 5.1 更新 `verification/windows-12gb-standard-tier/`：S3b/S3c/S3d 已写入；补 README 小结。
- [ ] 5.2 更新 `doc/15-windows-8gb-tier.md` 安装门槛说明（16 GiB 机器可安装）。
- [ ] 5.3 同步 `openspec/specs/model-installation/spec.md` 的低内存量化要求与场景（归档时执行）。
- [x] 5.4 更新 `certify-windows-10-target-machine` 的 Qwen blocked 状态（S9 改为通过，MOSS 仍 pending）。
- [ ] 5.5 `openspec archive stream-prepare-low-memory` + `openspec validate --all --strict`：MOSS 产品级证据与真实 16 GiB 机器证据补齐后执行。

## 6. 质量门

- [x] 6.1 `cargo test -p image-studio-desktop` 通过；`openspec validate --all --strict` 28/28 通过；前端未改动。
- [ ] 6.2 Windows release 构建与 worker wheel 重建已执行（wheel `543966ce…`）；完整端到端 MOSS 待补。
