# Real-ESRGAN x4plus ncnn

本目录固定 Image Studio 的 Real-ESRGAN 原生放大组合：

- 程序：`Real-ESRGAN-ncnn-vulkan` v0.2.0，macOS universal2
- 程序归档：8,532,236 bytes，SHA-256 `f1cc31f13398126ddec25a186b9e50c7ab3e4f2bc011d0222331b43ce2037d3b`
- 程序文件：26,787,080 bytes，SHA-256 `c1c35d92079085de96b9d547fd7e4464bc8a2e9ccf28d7b8c712d72ade91b7cc`
- 模型归档：Real-ESRGAN v0.2.5.0 / `realesrgan-ncnn-vulkan-20220424-macos.zip`，51,817,124 bytes，SHA-256 `e0ad05580abfeb25f8d8fb55aaf7bedf552c375b5b4d9bd3c8d59764d2cc333a`
- 模型：`realesrgan-x4plus.bin` 与 `.param`，合计 33,540,549 bytes
- 执行器：本地 ncnn + Vulkan，固定模型名 `realesrgan-x4plus`、4 倍、tile 128、单 GPU、`1:1:1` 线程，不启用 TTA

安装器只解压清单登记的归档成员；下载归档、成员文件与最终安装内容分别核验字节数和
SHA-256。归档中另有 anime、x2/x3 权重、样例图和重复程序文件，不会安装或作为默认。

`LICENSE-RealESRGAN.txt` 固定主仓库 BSD-3-Clause 许可，
`LICENSE-ncnn-app.txt` 固定 ncnn 应用 MIT 许可。模型权重与程序按上述精确发布资产
分发，不把主仓库程序许可外推到其他社区转换包。
