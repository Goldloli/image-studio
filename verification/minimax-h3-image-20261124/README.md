# MiniMax-H3-Image 2026-11-24 Evidence

## Passed

- 固定来源、SHA-256、许可证与 macOS/Windows `stable-diffusion.cpp 3b12781` 运行时清单。
- macOS Apple M3 Max 上游 CLI Gate：Q3_K_S + Q2_K_M，1024²、2048²、相同 seed 复现与取消。
- 隔离根产品安装、许可证确认归档、四个模型组件、Metal 设备探针与 Ready 记录。
- 专用 `minimax-h3-worker` 真实协议：1024²、2048²、推理中取消和候选删除。
- 原生 UI：1024²/2048² 生成、PNG 导出、取消、重试、重开历史恢复，以及新安装根中的许可证表单。
- 导出的 1024² 与 2048² PNG 均与作品库资产 SHA-256 完全一致。
- 在 `sandbox-exec` 禁止网络的隔离环境中，从已验证缓存完成产品安装（52.4 s）和 product worker 1024² 生成（63.5 s）。

## Failed / Corrected

- Abiray Q2_K 与固定 runtime 不兼容；保留为失败候选，不进入产品 profile。
- Unsloth Q2_K 能运行，但固定样例文字质量不合格；不进入默认 profile。
- 开发期 2048² 曾因轮次绑定白名单和旧 1 MP 输出像素上限失败；修复后最终原生 UI 生成与导出通过。

## Not Run

- Windows x64 / RTX 3080 Ti 12 GB / 32 GB 真机安装、生成、资源与导出。
- Windows 8 GB 补充机；首期没有 compact profile。
- 4096²、图像编辑、多参考、透明背景、批量输出和 Prompt Expander。
- 冷缓存下载、磁盘满、缓存破坏恢复和强杀整个产品宿主仍未运行；本轮只验证了预热缓存下的禁网安装与 worker 生成。

## Evidence Boundary

本轮只认证当前 macOS arm64 Apple M3 Max / 128 GiB 隔离 profile。不能外推到其他 Apple Silicon、Windows、8 GB 显存或未测试画幅。
