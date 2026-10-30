# 模型可用性验收报告 `20261030T191819Z-0b6ea76e`

- Profile：`daily`
- 结论：**passed**
- 通过 / 失败 / 未运行 / 阻塞：11 / 0 / 0 / 0
- 数据根：`/Users/gejiawei/Library/Application Support/ImageStudio`（default）

| 用例 | 优先级 | 状态 | 证据类别 | 结论或原因 |
|---|---|---|---|---|
| D01 | P0 | passed | native_ui | native evidence reviewed |
| D02 | P0 | passed | native_ui | native evidence reviewed |
| D03 | P0 | passed | native_ui | 系统级键盘事件唤出了 SCIM.ITABC 的拼音候选窗，选中 `meishu` 后确认成“美术”；候选期间按 Enter 只结束组合，没有创建持久 Job。 |
| D04 | P0 | passed | native_ui | native evidence reviewed |
| D05 | P0 | passed | native_ui | native evidence reviewed |
| D06 | P0 | passed | native_ui | 首次进入作品页直接显示 5 个正式 AssetRef，无需手动刷新；列表/网格往返后数量和布局保持，来源会话与制作记录可追溯。 |
| D07 | P0 | passed | native_ui | native evidence reviewed |
| D08 | P0 | passed | native_ui | native evidence reviewed |
| D09 | P0 | passed | native_ui | native evidence reviewed |
| D10 | P0 | passed | native_ui | native evidence reviewed |
| S01 | P0 | passed | native_ui | native evidence reviewed |

## 证据与口径

- 命令输出保存在同目录 `logs/`，每个证据文件在 JSON 中记录 SHA-256。
- 默认忽略测试、历史记录和计划模型不计入当前通过数。
- 存在任何 P0 失败时整体为 failed；否则存在 P0 未运行或阻塞时为 incomplete。
- 人工质量审阅：not_run。
