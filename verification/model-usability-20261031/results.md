# 当前模型验收与测试结果

结论：**PASSED**

- 必需用例：37 passed / 0 failed / 0 not_run / 0 blocked
- 固定模型：Z-Image Turbo / MFLUX 0.24.0 / macOS arm64 / Apple M3 Max 128 GiB
- 人工质量终审：已通过；六张固定生成样图及偏差记录见 [质量审阅](quality-review.json)。

## P0 结果

| Case | 状态 | 证据类别 | 结论 |
|---|---|---|---|
| C01 | passed | contract_fixture | all build, test and fixed-lock gates passed |
| C02 | passed | contract_fixture | explicit native-supervisor SIGKILL fixture passed |
| C03 | passed | contract_fixture | offline runtime installation passed and did not activate a model |
| C04 | passed | contract_fixture | one implemented executable entry and three non-executable planned entries |
| I01 | passed | real_execution | 从全新隔离缓存经固定 Hugging Face revision 完成真实联网安装；全部组件校验、运行时安装、原子激活与 Apple M3 Max GPU 探针通过 |
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
| D03 | passed | native_ui | 原生 SCIM.ITABC 拼音候选由系统级键盘事件触发，`meishu` 确认成“美术”；候选期间 Enter 未创建 Job，中文多行输入继续通过 |
| D04 | passed | native_ui | native evidence reviewed |
| D05 | passed | native_ui | native evidence reviewed |
| D06 | passed | native_ui | 生成完成后首次进入作品页即显示 5 个正式 AssetRef，无需手动刷新；列表布局在标签往返后保持 |
| D07 | passed | native_ui | native evidence reviewed |
| D08 | passed | native_ui | native evidence reviewed |
| D09 | passed | native_ui | native evidence reviewed |
| D10 | passed | native_ui | native evidence reviewed |

## 已闭环阻塞

- `I01`：直连 Hugging Face 仍超时；使用当前主机既有代理访问同一固定官方 revision，从全新缓存完成 `33,197,369,043` 字节下载、组件校验、私有运行时安装、激活和设备探针。没有使用镜像、手工搬运模型或缓存命中冒充冷安装。
- `D03`：系统级 HID-state 键盘事件唤出了真实 SCIM.ITABC 候选窗；确认后输入框为“美术”。组合态按 Enter 未新增 Job/Attempt/Asset，记录仍为 6/6/5。

## 已修复回归

- `L03`：快速双击发送只新增一个 Job，第二击不取消提交。
- `D06`：作品页首次进入直接读取最新正式资产，筛选和布局在标签往返后保持。
- `d67`：关闭或返回作品预览后再切换标签不会重开旧查看器，显式点击仍可预览。
- 修复证据见 [日常生成交互修复验收](../daily-generation-interaction-fix-20261031/results.md) 与 [机器可读记录](../daily-generation-interaction-fix-20261031/fix-acceptance.json)。

## 已验证能力

- contracts 9/9 通过，包括固定锁、禁网 runtime 安装、真实 tokenizer、安装/生命周期故障 fixture。
- 六组中英文、三种尺寸真实生成通过，Job/Attempt/Asset/PNG 结构和 SHA-256 均核对。
- 空缓存固定来源联网安装、缓存复用禁网安装和禁网真实生成均通过。
- 加载/采样取消、随机种子冻结重试、GPU 回收后宿主强杀与显式重试通过。
- 默认目录原生三轮生成、会话切换、任务/作品追溯、重启恢复、原生导出、四组布局矩阵和禁网生成通过。
- 中文 IME 候选确认、未确认组合不误发、快速双击幂等、作品页首次激活和查看器返回回归均通过。
- 最终 contracts 9/9、daily 11/11、I01 1/1 可复跑 profile 报告均为 passed；验收工具测试 15/15。

## 质量观察

- G01/G02 的主体、光线和语义完整，但呈现更接近写实摄影而非提示词要求的水彩插画；作为能力边界保留，不判损坏或空白。
- G05 的白色花瓶、向日葵与书本关系清楚，但书为打开状态而非提示词中的合上状态；G03/G04/G06 与固定要求一致。

机器可读明细见 `acceptance.json`；最终 profile 报告位于 `rerun-contracts-final/`、`rerun-daily-final/` 与 `rerun-i01-final/`，冷安装日志和质量审阅文件保存在本目录。
