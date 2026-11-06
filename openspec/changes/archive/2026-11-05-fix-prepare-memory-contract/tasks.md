# Tasks: fix-prepare-memory-contract

- [x] 1.1 复核 prepare 实现与安装器预检的当前行为，采集同口径实测数据（见 design.md §1）。
- [x] 1.2 指出主规格中"分片流式量化 / 8 GiB 可用内存"是未交付声明。
- [x] 1.3 改写 `model-installation` 的量化准备要求为组件级量化 + 双条件预检，并把分片流式量化登记为未交付后续项。
- [x] 1.4 `openspec validate --all --strict` 通过。
- [ ] 1.5 后续项（不在本 change 内）：实现真正的分片流式量化，使可用物理内存 ≥ 8 GiB 的机器可以 prepare，并更新预检与证据。
