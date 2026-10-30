# ZCode 本地参考记录

研究日期：2026-10-30（Asia/Shanghai）。用户指定路径：`/Users/gejiawei/Desktop/ai_code/项目参考/ZCode`。读取的是该路径当前工作区源码，package.json 为 3.14.3，Git HEAD `29628c9acdb81b703bbd4080c207a0e7ce5e276e`；HEAD 不是所有未提交源文件内容的保证。没有修改参考项目、读取 env、启动程序或执行其质量门禁；以下是源码交互结构观察，不是截图或运行验收。

已阅读该项目 AGENTS.md 与 DESIGN.md，无 .codegraph。参考目标是会话导航、会话内多轮与输入底栏布局，不迁入它的远程/权限/队列架构，不复制品牌素材。

| 观察 | 本地源码定位 | 本项目采用方式 |
|---|---|---|
| 独立会话分区、新建入口、按活动会话切换 | `packages/ui/src/WorkspaceSidebar.tsx:1592`、`:1604`、`:1620` | 文字会话侧栏和新建按钮，图片作为会话内结果 |
| 新建先进入对话工作区再建立草稿 | `packages/ui/src/root/useConversationWorkspaceActions.ts:48` | 新建是持久会话与空输入，不复制当前图片 |
| 列表标题、键盘切换、重命名 | `packages/ui/src/TaskListItem.tsx:245`、`:519`、`:532`、`:906` | 标题+选中态，支持新建/切换/重命名；本次不做删除/归档 |
| 切换恢复独立配置 | `packages/ui/src/store/zcodeSessionStoreWorkspaceSlice.ts:236`、`:510` | 模型、参数、输入与会话身份绑定 |
| 当前 composer 草稿按会话保存正文与 modelSelection | `packages/ui/src/v4/composer/composerDraftStore.ts:6` | 本项目继续由 Rust SQLite 保存会话草稿，参考其身份隔离；旧 chatComposerDraftStorage.ts 是 janitor，不作为当前实现依据 |
| 多轮按稳定 turnId 分组 user/assistant | `packages/ui/src/v4/conversationTurnRenderUnits.ts:32`、`:438` | Conversation → 多 Turn/Job → 多次图片结果 |
| 会话阅读位置独立、状态更新不扰动排序 | `packages/ui/src/lib/chatSessionScrollMemory.ts:38`、`packages/ui/src/lib/taskListOrdering.ts:36` | 分会话滚动缓存，采样刷新不推动列表跳动 |
| 输入底栏 leading/trailing 分组，右侧靠右 | `packages/ui/src/prompt-editor/ChatPromptEditor.tsx:388`、`:413` | 参数在左、模型与发送控制在右 |
| 模型控制簇先于停止/发送 | `packages/ui/src/v4/ConversationComposer.tsx:2038`、`:2042`、`:2064` | 采用相同右侧控制分组 |
| 模型簇实际包含 usage→model→thought | `packages/ui/src/v4/composer/V4ComposerToolbar.tsx:1013`、`:1043`、`:1074` | 按用户要求调整为 `[模型 ▾][发送/停止]`；不夹入 thought/usage |
| 模型选择只改变下一次提交，发送时冻结 | `packages/ui/src/v4/composer/V4ComposerToolbar.tsx:9`、`:15` | 选择保存到本会话输入，既有消息和运行中计划不改变 |
| 接收和输入清空区分 revision | `packages/ui/src/v4/ConversationComposer.tsx:1364`、`:1401` | 沿用本项目持久接受+CAS消费设计，防迟到ACK清空新输入 |

此检出版本的部分中文文案已把会话分区称为“任务/新建任务”（`packages/ui/src/i18n/locales/zh-CN.ts:1505`）。本项目按用户语义显示“会话/新建会话”，不宣称参考项目运行界面使用了相同文案。
