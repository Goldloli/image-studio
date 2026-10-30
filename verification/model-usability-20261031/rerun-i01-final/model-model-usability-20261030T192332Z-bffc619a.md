# 模型可用性验收报告 `20261030T192332Z-bffc619a`

- Profile：`model`
- 结论：**passed**
- 通过 / 失败 / 未运行 / 阻塞：1 / 0 / 0 / 0
- 数据根：`/Users/gejiawei/Desktop/ai_code/image-studio/target/model-usability-i01-network-20261031.YmO4n6/data`（isolated）

| 用例 | 优先级 | 状态 | 证据类别 | 结论或原因 |
|---|---|---|---|---|
| I01 | P0 | passed | real_execution | recorded evidence reviewed |

## 证据与口径

- 命令输出保存在同目录 `logs/`，每个证据文件在 JSON 中记录 SHA-256。
- 默认忽略测试、历史记录和计划模型不计入当前通过数。
- 存在任何 P0 失败时整体为 failed；否则存在 P0 未运行或阻塞时为 incomplete。
- 人工质量审阅：passed。
