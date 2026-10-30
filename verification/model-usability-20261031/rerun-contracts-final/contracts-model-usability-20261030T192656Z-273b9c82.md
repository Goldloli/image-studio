# 模型可用性验收报告 `20261030T192656Z-273b9c82`

- Profile：`contracts`
- 结论：**passed**
- 通过 / 失败 / 未运行 / 阻塞：9 / 0 / 0 / 0
- 数据根：`/Users/gejiawei/Desktop/ai_code/image-studio/target/model-usability-i01-network-20261031.YmO4n6/data`（isolated）

| 用例 | 优先级 | 状态 | 证据类别 | 结论或原因 |
|---|---|---|---|---|
| C01 | P0 | passed | contract_fixture | all build, test and fixed-lock gates passed |
| C02 | P0 | passed | contract_fixture | explicit native-supervisor SIGKILL fixture passed |
| C03 | P0 | passed | contract_fixture | offline runtime installation passed and did not activate a model |
| C04 | P0 | passed | contract_fixture | one implemented executable entry and three non-executable planned entries |
| I04 | P0 | passed | contract_fixture | all 6 required installation fixtures passed |
| I06 | P0 | passed | contract_fixture | all 4 required installation fixtures passed |
| L07 | P0 | passed | contract_fixture | all required controlled lifecycle fixtures passed |
| L08 | P0 | passed | real_execution | controlled boundary fixtures and real offline tokenizer probe passed |
| L09 | P0 | passed | contract_fixture | all required controlled lifecycle fixtures passed |

## 证据与口径

- 命令输出保存在同目录 `logs/`，每个证据文件在 JSON 中记录 SHA-256。
- 默认忽略测试、历史记录和计划模型不计入当前通过数。
- 存在任何 P0 失败时整体为 failed；否则存在 P0 未运行或阻塞时为 incomplete。
- 人工质量审阅：not_run。
