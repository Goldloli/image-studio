# 当前模型验收与测试结果

结论：**FAILED**

- 必需用例：33 passed / 2 failed / 0 not_run / 2 blocked
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
| L03 | failed | native_ui | 本地轮次进入“已取消”，没有创建持久 Job 或 Attempt |
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
| D06 | failed | native_ui | 三张正式资产已提交且数据库可读，但从完成生成的创作页首次进入作品页显示 0 件；点击页面刷新后才显示 3 件。单次无刷新日常流程未通过，已登记 image-studio-1yi。 |
| D07 | passed | native_ui | native evidence reviewed |
| D08 | passed | native_ui | native evidence reviewed |
| D09 | passed | native_ui | native evidence reviewed |
| D10 | passed | native_ui | native evidence reviewed |

## 失败与阻塞

- `L03` failed：快速双击发送时第二击命中已经切换的“停止”，提交在持久化前被取消；问题 `image-studio-bp9`。
- `D06` failed：已提交 3 个正式资产后，首次进入作品页仍显示 0 件，手动刷新后才显示 3 件；问题 `image-studio-1yi`。
- `I01` blocked：Hugging Face 冷下载连接超时，没有合法冷安装证据；后续缓存安装没有冒充 I01。
- `D03` blocked：CUA 未产生真实验证的中文 IME 候选事件；中文粘贴和多行编辑已通过，但需要人工键盘补验。

## 额外发现

- 从作品预览返回来源后，切回创作会重开旧图片查看器；问题 `image-studio-d67`。
- 修复以上行为问题需要按项目 OpenSpec 流程另写 change 并等待确认。

## 已验证能力

- contracts 9/9 通过，包括固定锁、禁网 runtime 安装、真实 tokenizer、安装/生命周期故障 fixture。
- 六组中英文、三种尺寸真实生成通过，Job/Attempt/Asset/PNG 结构和 SHA-256 均核对。
- 缓存禁网安装与禁网真实生成通过；冷下载 I01 仅阻塞。
- 加载/采样取消、随机种子冻结重试、GPU 回收后宿主强杀与显式重试通过。
- 默认目录原生三轮生成、会话切换、任务/作品追溯、重启恢复、原生导出、四组布局矩阵和禁网生成通过。

机器可读明细见 `acceptance.json`；完整命令日志位于 `target/model-usability-*`。
