## Why

当前工作区已证明 Rust 桌面基础可用，但界面仅有大标题、左侧提示词和主区空态，无法体现图片创作工具的结构与完成度。用户明确要求详细查看 GitHub 上的 Cherry Studio 并复刻其界面。

本提案以 Cherry Studio 桌面版 v2.1.8 固定 commit 的官方绘图截图和实际组件源码为基准，先把 demo 做成完整、可操作的创作工作区。研究结果见 [界面研究](../../../../doc/cherry-studio-ui-research.md)，主要参照见 [官方绘图截图](../../../../doc/references/cherry-studio-v2.1.8/drawing-light-en.webp)。

**批准状态：已确认。** 用户于 2026-10-29 回复“开始吧”，批准按本提案实施。确认前完成研究与规格，确认后按 tasks 实现和验收。用户随后明确指示改为 Rust core + Tauri 2 + React，本次更新按该指示执行，不改变已确认的界面功能范围。

## What Changes

- 以紧凑的顶部标签栏、50pt 图标导航、68pt 作品缩略图条、大画板、底部提示词 composer 替换现有布局；按绘图页复刻，不增加常驻右参数栏。
- 建立浅色和深色语义主题、统一的图标/字级/间距/圆角，以及 hover、selected、focus、disabled 状态。采用非透明表面，保留原生窗口控制和离线中文字体。
- 提供创作、模型、任务与设置页面；导航和标签切换可用，模型与任务页显示真实空态。设置支持主题切换及工作区/版本说明。
- 用 3 张应用自带、来源明确的原创示例图构成首次使用展示；可查看大图、切换缩略图、缩放、平移、旋转和重置。所有示例均明确标注，不记作本机生成结果。
- 将中文多行提示词放在底部 composer；支持展开/收起、模型空态弹层和明确标注的草稿参数弹层。导航、主题和面板变化不丢会话草稿。
- 保留离线启动、已支持模型数 0、生成禁用和开发诊断隔离；验证小窗口、高 DPI、键盘和后台事件下的交互。

本轮不包含模型下载/安装、真实推理、项目数据库、跨重启恢复、外部图片导入、图层与蒙版、生成导出，也不复制 Cherry 的聊天、供应商、知识库等产品功能。示例查看和参数编辑不创建 Job、Attempt 或运行记录。

## Capabilities

### New Capabilities

- workspace-canvas：在无推理后端时提供来源明确的示例展示与可操作画板，区分示例、空白草稿和真实输出。

### Modified Capabilities

- desktop-workspace：替换工作区布局，扩展中文草稿保持、真实能力表达及窗口适配要求；增加页面导航、主题与底部输入交互。

core-contracts、worker-protocol、platform-foundation 的现有契约不在本 change 中修改。

## Impact

根据用户于同日补充的明确指示，桌面技术栈调整为 Rust core + Tauri 2 + React。apps/desktop 改为 Tauri 原生宿主，新增 apps/frontend 的 React/TypeScript/Vite 界面，图片、字体与前端产物离线内嵌；独立 core/protocol crate、后台诊断 supervisor 和路径边界继续使用。前后端通过有限的类型化命令交换状态，不授予前端通用文件、Shell 或网络访问权限。

验收交付包括可运行的原生 demo、同构图的亮暗截图、中文输入与状态保持验证、必要的状态/变换边界测试及现有工作区回归。实现须经本提案确认后开始，完成并验证后才归档。
