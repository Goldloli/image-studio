## MODIFIED Requirements

### Requirement: 关于页披露真实产品和能力

关于分组 SHALL 显示真实应用版本、平台、本地处理说明、示例来源、项目与依赖许可入口和当前已接入能力范围。应用没有公开更新链路时 MUST 不显示可用的自动更新承诺；在线服务凭据、ASR/VAD、ffmpeg 和音频功能等领域设置不得混入图片工作台。

#### Scenario: 查看当前能力

- **WHEN** 用户打开关于分组
- **THEN** Z-Image、Klein、BiRefNet和Real-ESRGAN按本批实际接入与验证范围分别显示，未接入上游能力仍明确为范围外，版本和许可来源可核对

#### Scenario: 不显示无作用控件

- **WHEN** 当前版本尚未支持存储迁移、自动更新或在线模型服务
- **THEN** 页面不渲染会让人误以为可用的按钮，相关能力以边界说明出现

## ADDED Requirements

### Requirement: 默认生成模型排除工具专用能力

新会话默认生成模型 SHALL 只允许已登记文生图能力的型号；BiRefNet与Real-ESRGAN不得作为文生图默认，即使当前可执行。Klein默认值 SHALL 遵守方图/单图配方并整组原子验证，保存不修改已有会话或请求。运行环境分组 SHALL 分别展示旧MFLUX、新Klein、ONNX CPU及ncnn原生组合的真实版本/状态，不以单个Z-Image就绪代表所有环境。

#### Scenario: 保存BiRefNet作为生成默认
- **WHEN** 调用者尝试把去背景专用型号保存为新会话文生图默认
- **THEN** 整组拒绝并保留旧值，说明能力不匹配；用户仍可从对应功能进入去背景

#### Scenario: 仅ONNX环境异常
- **WHEN** BiRefNet CPU环境不可用而Z-Image和Klein完整
- **THEN** 设置分别显示真实环境及修复范围，其他已就绪能力保持可用
