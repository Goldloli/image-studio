## MODIFIED Requirements

### Requirement: 跨平台模型提示按当前平台解析

Composer SHALL 仅在会话/草稿保存的模型 id（文本模型或源图工具模型）不属于当前平台支持的模型集合时显示跨平台提示；提示文案 SHALL 指向当前平台对应的型号（文本型号族或具体工具名），并要求用户核对画幅/源图与参数；当该模型在当前平台存在但只是未安装/未就绪时，SHALL 只显示"未就绪"，不得显示"当前 Windows 未接入"。

#### Scenario: Windows 原生模型
- **WHEN** Windows 用户选择 Klein / Qwen / MOSS / Z-Image 或 BiRefNet / Real-ESRGAN 的 Windows 模型 id（无论是否已安装）
- **THEN** 不显示跨平台提示；未安装时显示"未就绪"

#### Scenario: 旧 macOS 模型选择
- **WHEN** Windows 用户打开或编辑保存了 macOS 模型 id 的会话/草稿
- **THEN** 显示跨平台提示，文本模型说明需选择对应的 Windows 型号族（Z-Image / Klein / Qwen / MOSS）并核对画幅与参数；工具模型说明需选择对应的 Windows BiRefNet 去背景 / Real-ESRGAN 放大 型号并核对源图

#### Scenario: macOS 打开 Windows 会话
- **WHEN** macOS 用户打开保存了 Windows 模型 id 的会话/草稿
- **THEN** 显示对应提示，文本模型说明该选择来自 Windows 端、需选择 macOS 对应型号；工具模型同样说明对应的 macOS 工具型号，而不是只显示"未就绪"
