# support-8gb-vram-tier

**状态：2026-11-05 提案文档完成，等待项目所有者确认后开始实现。**

本 change 为 Image Studio 的 Windows 侧增加 **8 GB 显存执行档位（compact）**，并在本仓库当前的 8 GB 机器（Windows 11 Enterprise 22631 / i9-9900K / RTX 2060 SUPER 8192 MiB / sm75 Turing / 31.9 GiB RAM）上完成五个生成模型的全流程验收：安装、安装期量化准备、真实生成、取消与强杀恢复、界面发送/查看/导出。

边界：

- 不改动 12 GB 认证机的现有配方、身份与证据。`standard` 档保持 1024²、BF16 与现有 load strategy。
- 不新增模型、快照、运行时或能力数量。`compact` 是同一模型的第二种执行档位，共用安装与 prepared 量化缓存。
- 不引入 GGUF / INT8 / 新加载器；量化仍为 bitsandbytes NF4。
- 低于 8 GB 显存的设备不在本次支持范围。
- 所有 compact 预算、精度与画质结论以本机 Gate 0 实测为准；设计文档中的预估不得当作已通过结论。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 背景、用户决定、变更范围、成功标准、不做项 |
| [design.md](design.md) | 档位模型、逐模型策略、计算精度、prepare 低内存、协议与预算、风险 |
| [specs/model-catalog/spec.md](specs/model-catalog/spec.md) | 目录按档位登记门槛并投影当前档位 |
| [specs/model-installation/spec.md](specs/model-installation/spec.md) | 安装期量化准备的可用内存门槛与共用缓存 |
| [specs/windows-local-generation/spec.md](specs/windows-local-generation/spec.md) | 8 GB 档执行、预算与降级可见性 |
| [specs/generation-worker-protocol/spec.md](specs/generation-worker-protocol/spec.md) | compact 配方身份与参数绑定 |
| [tasks.md](tasks.md) | 按依赖顺序的实现与验收任务 |
| [test-plan.md](test-plan.md) | 本机全流程验证矩阵与证据路径 |
