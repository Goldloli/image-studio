## ADDED Requirements

### Requirement: Klein文生图使用独立固定配方

Klein文生图 SHALL 绑定FLUX.2-klein-4B distilled固定快照与专用版本化配方，只接受非空文本、u32或空种子、单张1024×1024 RGB PNG，无源图。有效计划 SHALL 保存q4加载、4步、guidance1.0、flow_match_euler_discrete、实际种子及组件精度/运行时，使用自身chat模板的512 token预算且禁用截断。未知参数、非方图或额外输入 SHALL 明确拒绝，不沿用Z-Image的9步或更换模型。

#### Scenario: Klein与Z-Image交替生成
- **WHEN** 用户先发送Klein再发送Z-Image
- **THEN** 两任务分别使用自己的4步/v2和原9步/v1组合，型号/组件/进度及制作记录准确，先前结果不被改写

#### Scenario: 模板导致超限
- **WHEN** 指令原文未超16KiB但Klein含模板超过512 token
- **THEN** 在采样前明确拒绝并保留完整输入，不截断、不误用Z-Image模板
