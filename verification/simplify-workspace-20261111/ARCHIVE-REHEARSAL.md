# 归档演练记录（未执行真实归档）

日期：2026-11-11（Asia/Shanghai）。目的：在**不改动工作区**的前提下验证
`openspec archive simplify-workspace-and-manage-library` 的合并结果与归档后校验，供 6.1 原生验收通过后直接执行。

## 方法

```bash
rm -rf /tmp/osc-rehearsal2 && mkdir -p /tmp/osc-rehearsal2
cp -R openspec /tmp/osc-rehearsal2/openspec
cd /tmp/osc-rehearsal2
openspec archive simplify-workspace-and-manage-library --yes --no-validate
openspec validate --all --strict
openspec list
```

真实工作区未执行 archive；临时副本用完即弃。

## 结果

合并到 9 份正式规格：

| 规格 | added | modified | removed |
| --- | --- | --- | --- |
| application-settings | 0 | 2 | 0 |
| asset-library | 4 | 5 | 1 |
| conversation-management | 4 | 0 | 0 |
| desktop-workspace | 1 | 4 | 1 |
| feature-catalog | 1 | 0 | 4 |
| generation-conversation | 1 | 2 | 1 |
| project-storage | 3 | 1 | 0 |
| prompt-library | 0 | 1 | 0 |
| task-center | 1 | 1 | 6 |
| **总计** | **15** | **16** | **13** |

- 变更目录归档为 `openspec/changes/archive/2026-11-11-simplify-workspace-and-manage-library`；
  `openspec list` → `No active changes found`。
- 归档后 `openspec validate --all --strict`：**20 passed / 4 failed (24 items)**，4 项仍为既有
  `generation-worker-protocol`、`model-installation`、`windows-desktop`、`windows-local-generation`
  的 >500 字符警告（`image-studio-6g2` 跟踪），合并未引入新失败。
- 24 份正式规格仍可解析；合并后需求数最多的规格为 model-installation(13)、windows-desktop(12)、
  project-storage(12)、model-catalog(12)，未见重复要求或空文件。

## 归档后仍需处理

1. 修正 `feature-catalog` 与 `task-center` 的 Purpose（文案已写入 change README「归档计划」）。
2. 更新 tasks.md/README 状态，关闭 Beads `image-studio-i6`。
3. 归档提交与 Purpose 修正分开提交，保持可审查。

## 前置条件

`tasks.md` 6.1（原生 UI + 真实生成验收）完成并写入同目录 `RESULTS.md` 后才能执行真实归档；
本次演练**不代表**已归档，也不代表用户批准。
