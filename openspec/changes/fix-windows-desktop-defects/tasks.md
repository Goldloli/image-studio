# Tasks: fix-windows-desktop-defects

> 未获用户确认前，不得开始实现。

## 1. 文档与确认

- [x] 1.1 proposal / design / tasks / test-plan 与三个 spec delta 完成并展示，等待确认。
- [x] 1.2 确认后建立 bd issue 跟踪。

## 2. 连续发送卡死

- [x] 2.1 在发送链路加分段计时（prepare / freeze / scheduling 取锁 / bind / start）并写入结构化日志。
- [x] 2.2 复现：连续发送直到卡死，用日志确定阻塞段（若无法复现，记录尝试次数与状态）。
- [x] 2.3 按定位结果去掉"持锁做慢操作"：`bind_turn_job` 移出 `scheduling` 锁，SQLite 设置 `busy_timeout` 并处理 `BUSY`。
- [x] 2.4 增加 pending 无 job 的看门狗（60 s），超时把轮次置为 failed 并记录阶段与阻塞点；`generation.start` 的失败落到轮次。
- [x] 2.5 验证：连续 5 次顺序发送全部出图，预热后预检 ≤0.14 s；另有 4 次连续绑定记录（含取消与失败注入）无挂起；单次启动失败在 60 s 内给出可行动错误。

## 3. 取消落定慢

- [x] 3.1 记录取消各阶段时间戳并输出到日志与任务详情。
- [x] 3.2 超时强制回收：超过 `cancel_grace_ms + cleanup_deadline_ms` 立即落 `cancelled`（标注强制回收），后台清理。
- [ ] 3.3 界面显示已等待秒数，超过 15 s 给出说明。
- [~] 3.4 验证：Qwen Turbo 取消一次 6.69 s 落定、无残留进程；MOSS/Klein 未测，点击到 `cancelled` ≤ 15 s；无残留进程；无半成品资产。

## 3b. 子进程不弹窗

- [x] 3b.1 新增 `quiet_command()`，Windows 上统一 `CREATE_NO_WINDOW`。
- [x] 3b.2 生成、安装、探针、ncnn 的所有子进程改用它。
- [x] 3b.3 验证：重新构建后生成一次，supervisor/python 均无窗口句柄（MainWindowHandle=0），出图正常。

## 4. 安装期阶段投影

- [x] 4.1 安装进行中由宿主维护当前 phase/receivedBytes，`model_catalog` 与 `model_view` 同源读取。
- [x] 4.2 模型页在安装期有界轮询刷新，阶段文案覆盖 downloading / verifying / preparing / installing_runtime / probing。
- [x] 4.3 单元测试：每个阶段都投影为对应文案，且不得为 `not_installed`。
- [x] 4.4 验证：真实安装一次（BiRefNet，观察到 downloading→verifying→probing→ready，行内显示"正在下载"），逐阶段截图/记录。

## 5. 下载重试

- [x] 5.1 每个 artifact 失败后按 1/2/4 s 退避重试，最多 5 次，保留 `.part` 与 Range 续传。
- [x] 5.2 失败信息包含 URL、尝试次数、最后错误与恢复建议；校验失败直接重下而非重试同一字节。
- [x] 5.3 故障注入测试：连续失败 2 次后成功 → 安装继续；连续失败 5 次 → 可行动错误，进度保留。
- [~] 5.4 验证：真实下载成功但未触发重试；重试路径由单测覆盖（可用预置缓存缩短下载阶段，另用故障注入覆盖重试路径）。

## 6. 质量门与归档

- [x] 6.1 `cargo fmt --check`、`clippy -D warnings`、workspace 测试、前端 test/build 全绿。
- [ ] 6.2 `openspec validate --all --strict` 通过；证据写入 `verification/`；归档 change 并同步主规格。
