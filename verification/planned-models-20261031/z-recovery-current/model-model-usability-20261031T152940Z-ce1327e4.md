# 模型可用性验收报告 `20261031T152940Z-ce1327e4`

- Profile：`model`
- 结论：**passed**
- 通过 / 失败 / 未运行 / 阻塞：4 / 0 / 0 / 0
- 数据根：`/Users/gejiawei/Desktop/ai_code/image-studio/target/model-usability-offline-cache-20261031`（isolated）

| 用例 | 优先级 | 状态 | 证据类别 | 结论或原因 |
|---|---|---|---|---|
| L02 | P0 | passed | real_execution | random seed was frozen, cancellation was cleaned up and explicit retry succeeded |
| L04 | P0 | passed | real_execution | real loading cancellation was cleaned up without a formal asset |
| L05 | P0 | passed | real_execution | real sampling cancellation was cleaned up and an explicit retry succeeded |
| L06 | P0 | passed | real_execution | real GPU execution was killed, the old lease was busy, and explicit retry succeeded |

## 证据与口径

- 命令输出保存在同目录 `logs/`，每个证据文件在 JSON 中记录 SHA-256。
- 默认忽略测试、历史记录和计划模型不计入当前通过数。
- 存在任何 P0 失败时整体为 failed；否则存在 P0 未运行或阻塞时为 incomplete。
- 人工质量审阅：not_run。
