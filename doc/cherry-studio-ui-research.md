# Cherry Studio 界面研究与 Image Studio 复刻映射

研究日期：2026-10-29。本文记录实际阅读的官方截图和界面源码，以及本项目的待批准适配方案；不是已实现界面的验收报告。

## 1. 参考版本与证据边界

- 官方桌面仓库：[CherryHQ/cherry-studio](https://github.com/CherryHQ/cherry-studio)。
- 本次查询的最新稳定发布：[v2.1.8](https://github.com/CherryHQ/cherry-studio/releases/tag/v2.1.8)，发布时间 2026-10-26T08:56:13Z。
- 固定源码 commit：9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1。后文源链接均固定此版本。
- 查询时 main 为 da43afa57cd2456838c3692bbccc85b69ab85812；不以变化中的 main 作为复刻验收基线。
- 参考对象为桌面版 v2 默认界面，不混用移动端、旧版 v1 或社区自定义主题。
- 本次做了截图视觉检查和源码阅读，没有安装运行 Cherry Studio，没有完成其功能实机测试。
- 上游仓库许可为 [AGPL-3.0](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/LICENSE)。本地截图副本用于有出处的研究对照，不编入产品；产品保持 Image Studio 名称、原创示例及图形资源，在 Rust/egui 中重新实现界面。

官方三张截图均为 2208×1444 像素，已逐张检查；它们不是 2208×1444 个逻辑点，也不能据此认定缩放率。

| 截图 | 主要检查内容 | 本地副本 |
|---|---|---|
| [绘图页](https://raw.githubusercontent.com/CherryHQ/cherry-studio/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/.github/images/readme/drawing-light-en.webp) | 顶部标签、图标导航、作品条、大图、浮动工具、底部输入框 | [drawing-light-en.webp](references/cherry-studio-v2.1.8/drawing-light-en.webp) |
| [聊天页](https://raw.githubusercontent.com/CherryHQ/cherry-studio/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/.github/images/readme/chat-light-en.webp) | 外壳一致性、二级列表、选中态、正文与输入框密度 | [chat-light-en.webp](references/cherry-studio-v2.1.8/chat-light-en.webp) |
| [模型提供商设置](https://raw.githubusercontent.com/CherryHQ/cherry-studio/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/.github/images/readme/provider-light-en.webp) | 返回入口、分类导航、列表和详情、字段分组 | [provider-light-en.webp](references/cherry-studio-v2.1.8/provider-light-en.webp) |

文件来源、尺寸、SHA-256 见 [references.json](references/cherry-studio-v2.1.8/references.json)。

## 2. 截图中的实际结构

绘图页是本项目主要参照：顶部是一条包含原生窗口按钮的紧凑标签栏，最左是图标导航，内侧是一列图片缩略图，剩余区域让给图片。画布左侧有竖向胶囊工具条，下方是居中的大圆角提示词输入框；模型选择、参数摘要和发送按钮集中在输入框底部。主内容是连续的浅色表面，没有把每一部分包成重阴影卡片。

聊天页的宽助手列表属于聊天任务，不是绘图页必有的第二列。设置页采用分类、对象列表、详情的分层；不能把整套供应商配置内容搬进本地图片工具。

因此本轮复刻先保持绘图页结构，不增加常驻右参数栏。原调研 [01-product-vision.md](01-product-vision.md) 第 5 节中的三栏只是建议，且明确允许通过原型调整；本轮改用弹层承载少量草稿参数，保留未来按真实任务扩展检查器的空间。

## 3. 外壳、导航与尺寸

以下是源码事实。Tailwind 数字换算使用其默认 4px spacing；CSS px 到 egui 逻辑点的对应是本项目适配选择。

| 部位 | 上游值与结构 | 固定源码 |
|---|---|---|
| 主窗口顶部栏 | 当前主 shell 高 44px；不要误用遗留 titleBar 常量 37.5px | [AppShellTabBar.tsx:918](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/layout/AppShellTabBar.tsx#L918) |
| 标签 | 高 30px，宽 56–160px，圆角 10px，图标 14px，标题 12px，间隔 4px | [AppShellTabBar.tsx:337](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/layout/AppShellTabBar.tsx#L337) |
| 内容外壳 | 右、下留 6px，圆角 12px，边框 0.5px | [AppShell.tsx:182](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/layout/AppShell.tsx#L182) |
| 图标导航 | 固定图标模式 50px；上游另有可展开/隐藏模式 | [constants.ts:3](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/Sidebar/constants.ts#L3) |
| 导航按钮 | 36×36px 圆形 hitbox、18px glyph、4px 间隔、左右 6px padding | [SidebarList.tsx:95](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/Sidebar/SidebarList.tsx#L95) |
| 绘图作品条 | 68px 宽，44×44px 缩略图，圆角 12px、8px gap，新增按钮 sticky，纵向滚动 | [paintingPrimitives.ts:19](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/paintingPrimitives.ts#L19) |
| 底部输入框 | 最大宽约 1024px，20px 圆角、0.5px 边框；工具行高 40px，常用胶囊按钮高 28px | [外壳](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/composer/ComposerSurfaceRuntime.tsx#L2286)、[宽度](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/PaintingPage.tsx#L220)、[胶囊](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/composer/variants/shared/ComposerControlScaffolding.tsx#L10) |
| 模型选择 | 触发宽不超过 200px、图标 18px；选择弹层最大 420px | [触发器](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/PaintingComposer.tsx#L320)、[选择器](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/PaintingModelSelector.tsx#L76) |
| 参数 | 输入框底部向上打开，最大宽 300px、内部滚动内容最高 60vh、另有 12px padding；根据模型配置动态生成 | [弹层](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/PaintingComposer.tsx#L182)、[动态字段](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/PaintingSettings.tsx#L36) |
| 画布工具 | 画布左侧 16px、垂直居中，竖向圆角胶囊、16px 图标 | [位置](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/paintingPrimitives.ts#L32)、[图标](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/Artboard.tsx#L600) |

上游 macOS 标签栏为原生红黄绿留出空间，只有空白标题区拖窗，按钮和标签不参与拖窗。本项目继续使用原生窗口控制；不画装饰性的假红黄绿。

## 4. 主题与字体

Cherry 的 [DESIGN.md](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/DESIGN.md) 强调中性底色、语义颜色、表面层级和可见焦点，具体尺寸由组件而非设计宣言决定。

实际默认设置为 system 主题、transparent 窗口、主色 #00b96b；主色由运行时覆盖。不能将 primitive brand token 误称为应用默认主色。来源：[preferenceSchemas.ts:922](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/shared/data/preference/preferenceSchemas.ts#L922)、[useUserTheme.ts:20](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/hooks/useUserTheme.ts#L20)。

| 语义 | 浅色源码值 | 深色源码值 |
|---|---|---|
| 主背景 | white | oklch(.209 0 0 / .55) |
| 正文 | oklch(.213 0 0) | oklch(.931 0 0) |
| 次要文字 | oklch(.556 0 0) | oklch(.708 0 0) |
| 三级 / 禁用文字 | oklch(.669 0 0) | oklch(.559 0 0) |
| 导航背景 | oklch(.9672 0 0) | oklch(.2393 0 0) |
| 导航强调底色 | oklch(.97 0 0) | oklch(.269 0 0) |
| 结构边框 | 黑色 10% | 白色 10% |
| 轻边框 | 黑色 4% | 白色 5% |
| card / popover | white / white | neutral900(.2) / neutral800(.27) |

来源：[providers.css:22](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/packages/ui/src/styles/tokens/colors/providers.css#L22)、[providers.css:100](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/packages/ui/src/styles/tokens/colors/providers.css#L100)。

导航与标签的选中态主要是中性填充，并非一律绿色。透明 Mac 分支浅色 white78%、深色 white10%；非透明分支浅色 black8%、深色 sidebar-accent。hover 更轻，禁用控件不靠 hover 假装可用；导航按钮的 focus-visible 为内侧 1px ring，不能据此推断标签使用相同焦点样式。来源：[SidebarList.tsx:30](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/Sidebar/SidebarList.tsx#L30)、[Sidebar.css:51](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/Sidebar/Sidebar.css#L51)、[AppShellTabBar.tsx:611](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/components/layout/AppShellTabBar.tsx#L611)。

实际 body 为 14px、line-height 1.6；macOS / 通用字体分支 Ubuntu 优先再平台回退，Windows 默认 Microsoft YaHei UI 优先。虽然 typography token 声明 Inter，不能据此断言整窗用 Inter。[index.css:73](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/assets/styles/index.css#L73)、[font.css:1](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/assets/styles/font.css#L1)、[Windows 分支](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/assets/styles/font.css#L16)。

## 5. 画布与输入框行为

- 图片 contain-fit，初始不放大超过原图尺寸；提示词摘要与尺寸位于图片上方，不跟图片变换。
- 缩放 0.25×–4×，步长 0.25×；到边界禁用对应按钮。
- 支持左键拖动平移、左右各 90° 旋转、重置。拖动达到 4px 不同时触发点击预览。
- 上游切换作品 / 当前文件会重置变换；多图才显示前后切换，底部页码表示当前文件 / 总数。
- 历史条为首张图的 cover 缩略图，选中内描边，删除按钮在 hover 时出现且不传播为选择。
- 新草稿有标题、模板图片展示与说明，不是整页单句“还没有图片”。模板最多 5 张可见，中心放大、两侧轻微旋转；点击模板填提示词。
- 编辑模型的参考图输入是有条件的，不能无模型时把附件入口表现为已支持图片编辑。

来源：[Artboard.tsx:63](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/Artboard.tsx#L63)、[Artboard.tsx:256](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/Artboard.tsx#L256)、[artboardImageScale.ts:1](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/utils/artboardImageScale.ts#L1)、[PaintingStrip.tsx:36](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/PaintingStrip.tsx#L36)、[PaintingTemplateShowcase.tsx:65](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/components/PaintingTemplateShowcase.tsx#L65)。

需要特别区分截图和源码：README 绘图截图是输入框展开态；v2.1.8 的页面逻辑会在已有结果时默认折叠，空白草稿或生成时展开，并提供手动切换。本项目以截图中的展开态作为主要视觉验收状态，另实现手动收起 / 展开，不声称复制全部自动折叠时序。来源：[PaintingPage.tsx:129](https://github.com/CherryHQ/cherry-studio/blob/9e7bb8a17d002c31bfb83ea4acd6c3b501bf40f1/src/renderer/pages/paintings/PaintingPage.tsx#L129)。

## 6. 改造前的差距

研究阶段的原生窗口已经实际检查，仍是 72pt 品牌顶栏、306pt 提示词左栏、主区固定空态、34pt 状态底栏。没有导航、作品条、图片画板和主题切换，单纯更换颜色无法达到参照的界面完成度。

对应代码：apps/desktop/src/ui/mod.rs 的 StudioApp、header、prompt_panel 与 workspace；theme.rs 固定 Light 和暖灰绿颜色；studio-core/application.rs 仍真实报告 inference=false、model=0。

已完成的中文字体、输入法、剪贴板、后台通信有界处理、原生窗口尺寸和离线启动基础需保留。诊断面板是开发工具，不改成面向用户的生成进度或参数面板。

## 7. 复刻映射

| Cherry 元素 | Image Studio 本轮映射 | 差异说明 |
|---|---|---|
| 顶部多应用标签 | 创作、模型、任务、设置的页面标签；创作始终保留 | 不做标签拖出新窗、固定、任意排序 |
| 50px 图标导航 | 创作、模型、任务；底部设置和主题切换 | 使用本产品图形与名称，无聊天、知识库等无关入口 |
| 68px 作品条 | 新建草稿、会话草稿、3 个标注示例的缩略图 | 示例不是本机生成历史 |
| Artboard | 自带原创示例的查看、缩放、平移、旋转、重置 | 不含图层、蒙版、真实生成或外部图片导入 |
| 空白模板页 | 标题、3 张示例卡、“查看示例”和“使用提示词” | 明确区分打开示例图与将文字复制进当前草稿 |
| Composer | 底部中文多行输入、模型空态、草稿参数弹层、禁用发送 | 始终说明未配置推理；不启动 Job |
| 参数弹层 | 画幅、计划数量、种子等会话草稿预设 | 标为“草稿参数，仅演示”；不宣称任何模型支持 |
| 设置分层 | 外观、工作区说明、关于 | 不复制供应商、API Key、MCP、知识库等功能 |
| 亮暗语义 token | 非透明浅色 / 深色；绿色主动作和必要状态 | 本轮不承诺 macOS vibrancy；保留 Noto 中文字体 |

初始仍选中新草稿，用有内容的示例卡改善空白页；打开示例后才显示示例大图。导航、缩略图、输入框、主题、画布工具和参数弹层应真实可操作。模型与任务中心用真实 0 状态和有层级的完整页面，不填伪造下载、进度或成功记录。

## 8. 复刻完成的判断

在 1280×800 与 960×640 逻辑窗口、1× / 2× 渲染下，对照同一构图检查浅色首屏、示例画布、输入框展开和收起、参数弹层、深色画布、模型空态、设置。关键尺寸误差不超过 2pt（原生标题区和字体字形单独说明），图像不拉伸，边缘不裁掉主要入口。

颜色以源码语义 token 转换为准，不用官方截图上受透明度、壁纸和缩放影响的合成色倒推全局颜色。不设虚假的整图像素一致率；品牌、示例内容、中文与平台字形差异应单独列出。

具体规范和实现顺序见 [OpenSpec change](../openspec/changes/archive/2026-10-29-recreate-cherry-style-workspace/proposal.md)。用户于 2026-10-29 回复“开始吧”批准实施，随后明确指示改为 Rust core + Tauri 2 + React；该技术调整已先写入同一 change 再实施。上文组件源码事实仍是视觉参照，实际实现和验收以该 change 与 verification/cherry-workspace-results.md 为准。
