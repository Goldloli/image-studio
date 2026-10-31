# 模型可用性验收报告 `20261031T151058Z-ed3c2d57`

- Profile：`model`
- 结论：**incomplete**
- 通过 / 失败 / 未运行 / 阻塞：7 / 3 / 0 / 0
- 数据根：`/Users/gejiawei/Desktop/ai_code/image-studio/target/model-usability-offline-cache-20261031`（isolated）

| 用例 | 优先级 | 状态 | 证据类别 | 结论或原因 |
|---|---|---|---|---|
| G01 | P0 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G02 | P0 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G03 | P0 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G04 | P0 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G05 | P0 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| G06 | P0 | passed | real_execution | fixed input produced a decodable formal PNG with a successful execution record |
| L02 | P0 | failed | real_execution | explicit retry helper failed; explicit retry did not create exactly one new Attempt; retried job did not reach Succeeded |
| L04 | P0 | passed | real_execution | real loading cancellation was cleaned up without a formal asset |
| L05 | P0 | failed | real_execution | explicit retry helper failed; explicit retry did not create exactly one new Attempt; retried job did not reach Succeeded |
| L06 | P0 | failed | real_execution | host-crash acceptance script failed; execution is not marked as full-model; real sampling progress was not observed before the kill; pre-retry job was not Interrupted; execution lease release was not proven; old execution lease was not observed busy during recovery; explicit retry did not succeed; Attempt history did not retain Interrupted then Succeeded; host recovery and retry did not use unique tokens |

## 证据与口径

- 命令输出保存在同目录 `logs/`，每个证据文件在 JSON 中记录 SHA-256。
- 默认忽略测试、历史记录和计划模型不计入当前通过数。
- 存在任何 P0 失败时整体为 failed；否则存在 P0 未运行或阻塞时为 incomplete。
- 人工质量审阅：not_run。
