# 当前模型验收与测试结果

结论：**INCOMPLETE**

- 必需用例：35 passed / 0 failed / 0 not_run / 2 blocked
- 固定模型：Z-Image Turbo / MFLUX 0.24.0 / macOS arm64 / Apple M3 Max 128 GiB
- 人工质量终审：待完成；自动视觉检查已覆盖六张固定生成样图。

## P0 结果

| Case | 状态 | 证据类别 | 结论 |
|---|---|---|---|
| C01 | passed | contract_fixture | all build, test and fixed-lock gates passed |
| C02 | passed | contract_fixture | explicit native-supervisor SIGKILL fixture passed |
| C03 | passed | contract_fixture | offline runtime installation passed and did not activate a model |
| C04 | passed | contract_fixture | one implemented executable entry and three non-executable planned entries |
| I01 | blocked | real_execution | 网络中断：text_encoder/config.json：error sending request |
| I02 | passed | real_execution | verified cache installed the private runtime and model with networking denied |
| I03 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| I04 | passed | contract_fixture | all 6 required installation fixtures passed |
| I05 | passed | real_execution | missing component was rejected before execution while existing assets stayed readable |
| I06 | passed | contract_fixture | all 4 required installation fixtures passed |
| S01 | passed | native_ui | native evidence reviewed |
| S02 | passed | native_ui | 使用禁网离线外壳与新隔离根启动；embedded React 页面正常完成，模型页/会话显示未就绪，未启动 inference-supervisor 或 Python worker。 |
| G01 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G02 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G03 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G04 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G05 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G06 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| L01 | passed | native_ui | 三个请求在同一原生“日常验收”会话内逐个等待前一轮完成后发送；数据库与 UI 均为三个独立 Job、三个独立成功 Attempt 和三个正式 AssetRef，没有覆盖旧图或隐式并行队列。 |
| L02 | passed | real_execution | random seed was frozen, cancellation was cleaned up and explicit retry succeeded |
| L03 | passed | native_ui | 快速双击只创建一个持久 Job；第二击被多击保护忽略，原取消数未增加 |
| L04 | passed | real_execution | real loading cancellation was cleaned up without a formal asset |
| L05 | passed | real_execution | real sampling cancellation was cleaned up and an explicit retry succeeded |
| L06 | passed | real_execution | real GPU execution was killed, the old lease was busy, and explicit retry succeeded |
| L07 | passed | contract_fixture | all required controlled lifecycle fixtures passed |
| L08 | passed | real_execution | controlled boundary fixtures and real offline tokenizer probe passed |
| L09 | passed | contract_fixture | all required controlled lifecycle fixtures passed |
| D01 | passed | native_ui | native evidence reviewed |
| D02 | passed | native_ui | native evidence reviewed |
| D03 | blocked | native_ui | 宿主启用的 SCIM.ITABC 是简体拼音源，但当前 CUA 键盘注入没有触发候选组合，Ctrl+Space 也未切换；ASCII meishu 保持为字面文本。需要真实人工键盘确认候选后才能完成本项。中文文本编辑和 Shift+Enter 多行粘贴已通过，但不替代 IME 证据。 |
| D04 | passed | native_ui | native evidence reviewed |
| D05 | passed | native_ui | native evidence reviewed |
| D06 | passed | native_ui | 生成完成后首次进入作品页即显示 5 个正式 AssetRef，无需手动刷新；列表布局在标签往返后保持 |
| D07 | passed | native_ui | native evidence reviewed |
| D08 | passed | native_ui | native evidence reviewed |
| D09 | passed | native_ui | native evidence reviewed |
| D10 | passed | native_ui | native evidence reviewed |

## 剩余阻塞

- `I01` blocked：Hugging Face 冷下载连接超时，没有合法冷安装证据；后续缓存安装没有冒充 I01。
- `D03` blocked：CUA 未产生真实验证的中文 IME 候选事件；中文粘贴和多行编辑已通过，但需要人工键盘补验。

## 已修复回归

- `L03`：快速双击发送只新增一个 Job，第二击不取消提交。
- `D06`：作品页首次进入直接读取最新正式资产，筛选和布局在标签往返后保持。
- `d67`：关闭或返回作品预览后再切换标签不会重开旧查看器，显式点击仍可预览。
- 修复证据见 [日常生成交互修复验收](../daily-generation-interaction-fix-20261031/results.md) 与 [机器可读记录](../daily-generation-interaction-fix-20261031/fix-acceptance.json)。

## 已验证能力

- contracts 9/9 通过，包括固定锁、禁网 runtime 安装、真实 tokenizer、安装/生命周期故障 fixture。
- 六组中英文、三种尺寸真实生成通过，Job/Attempt/Asset/PNG 结构和 SHA-256 均核对。
- 缓存禁网安装与禁网真实生成通过；冷下载 I01 仅阻塞。
- 加载/采样取消、随机种子冻结重试、GPU 回收后宿主强杀与显式重试通过。
- 默认目录原生三轮生成、会话切换、任务/作品追溯、重启恢复、原生导出、四组布局矩阵和禁网生成通过。
- 修复后的 29 项前端测试、Rust 全工作区门禁及原生快速双击/作品激活/查看器返回回归均通过。

机器可读明细见 `acceptance.json`；完整命令日志位于 `target/model-usability-*`。
