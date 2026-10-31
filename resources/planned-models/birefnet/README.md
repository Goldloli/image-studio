# BiRefNet general epoch_244

本目录固定 BiRefNet 的 CPU ONNX 执行组合：

- 官方发布：`ZhengPeng7/BiRefNet` 的 `v1` release，asset `186740004`
- 模型：`BiRefNet-general-epoch_244.onnx`，`972,666,916` bytes
- 模型 SHA-256：`58f621f00f5d756097615970a88a791584600dcf7c45b18a0a6267535a1ebd3c`
- 运行时：Python `3.13.12` + ONNX Runtime `1.31.1`，仅使用 `CPUExecutionProvider`
- worker：`studio-onnx-worker-0.1.0`

图合同为固定 float32 NCHW `1×3×1024×1024` 输入和 float32 `1×1×1024×1024`
输出。图内虽含内部 Sigmoid 节点，最终输出直接由 Conv 产生，因此配方明确只对最终
logits 应用一次 sigmoid。产品随后使用 `align_corners=True` 双线性恢复到源图规范化
尺寸，并把预测概率与源 alpha 合成最终 8-bit mask。

`model-manifest.json` 和 `runtime-manifest.json` 是安装器唯一接受的来源清单；任何
网络连接或镜像都只能传输这些清单已固定的内容。`LICENSE-BiRefNet.txt` 保存发布
仓库的 MIT 许可。wheel 的版本与哈希见 runtime manifest，各 wheel 包内 license
文件随安装内容保留。

2026-10-31 已在 Apple M3 Max / 128 GiB 完成空缓存安装、隔离根禁网安装、
真实下载中断恢复、真实 CPU 推理、RGBA/mask、质量、取消/强杀和原生日常验收。
固定配置为 `31 passed / 0 failed / 0 not_run / 0 blocked`，证据见
`verification/planned-models-20261031/birefnet-acceptance.json`。
