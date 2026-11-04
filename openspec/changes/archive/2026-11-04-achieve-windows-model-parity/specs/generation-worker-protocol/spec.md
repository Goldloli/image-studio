## Purpose

让新增 Windows worker 复用既有有界帧协议、执行身份与生命周期合同。

## ADDED Requirements

### Requirement: 新 Windows worker 沿用有界协议与执行身份

Klein/Qwen/MOSS 的 diffusers worker、BiRefNet 的 ONNX worker 与 Real-ESRGAN 的 ncnn worker SHALL 使用 generation v2 的握手、帧边界、有界日志、取消令牌、租约与整树回收合同；worker 身份 SHALL 包含型号、配方、运行时与构建 ID，未知/错平台/错版本请求必须拒绝。

#### Scenario: 错平台请求
- **WHEN** 请求携带 macOS 配方或未登记型号身份
- **THEN** worker 拒绝且不启动计算，不产生候选或资产

#### Scenario: 取消与强杀
- **WHEN** 采样或推理期间用户取消，或宿主/监督进程被强杀
- **THEN** worker 与后代被回收，租约释放，重启后标记 Interrupted 且不伪造成功
