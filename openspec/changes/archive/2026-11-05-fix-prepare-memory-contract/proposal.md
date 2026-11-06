# Proposal: fix-prepare-memory-contract

## Why

`support-8gb-vram-tier` 的主规格现在要求"安装期 NF4 量化准备 SHALL 按 safetensors 分片逐个加载、量化、保存并释放"，并把预检目标写成"可用内存 ≥ 8 GiB 且 commit 余量 ≥ 6 GiB"。这两条都没有实现：

- 实现（`tools/diffusers-worker-windows/studio_diffusers_worker/backends/prepare.py`）以**整个组件**为单位量化，峰值 commit 实测 23.1 GiB。
- 安装器预检（`apps/desktop/src/adapters/installation.rs`）要求"可用物理内存 ≥ 28 GiB **或** commit 余量 ≥ 26 GiB"。

规格声明了一个未交付的能力，属于必须纠正的文档缺陷：按现状归档会让后续实现者与验收者误以为 8 GiB 机器已经可以 prepare。

## 修正内容

- 把该要求改写为已交付行为：组件级 CPU 量化、实测冻结的双条件预检（物理 28 GiB 或 commit 26 GiB）、失败前置与不留半成品、prepared 缓存跨档位共用。
- 明确写出：**逐分片流式量化未交付**，不得被当作已实现；它被登记为独立后续项（目标是让 16 GiB 内存机器也能安装）。
- 保留并强化原有场景：本机实测通过路径、门槛不足提前失败、两档共用缓存。

## 不做

- 不改动 prepare 的运行时行为或任何代码。
- 不把 16 GiB 机器的支持纳入本次范围。
- 不修改 `support-8gb-vram-tier` 的归档内容（历史记录保持原样）。

## 成功标准

- 主规格中不再出现"分片流式量化已交付"或"8 GiB 可用内存已足够"的表述。
- `openspec validate --all --strict` 通过。
