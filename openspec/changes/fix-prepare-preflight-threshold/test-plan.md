# Test plan: fix-prepare-preflight-threshold

环境：Windows 10 Pro 22H2 / i7-13700K / RTX 3080 Ti 12 GiB / 31.7 GiB RAM；全新根 `C:\ist-accept`（当前构建已在该根完成 4 型号标准档回归，见 `verification/windows-12gb-standard-tier/`）。对照证据：`C:\ist-a` 的 2026-11-04 安装日志（prepare 峰值 23.003 GiB、预检 commit 余量 27.06 GiB）。

## 覆盖矩阵

| ID | 项 | 判定方式 | 证据 |
|---|---|---|---|
| U1 | 预检阈值边界（单元） | 23.99 GiB 拒绝 / 24.00 GiB 通过 / 28 GiB 可用物理内存通过；错误文案含 24 GiB 与可行动建议 | `cargo test` 日志 |
| U2 | 可复用缓存跳过预检（单元） | prepared 摘要匹配时不调用预检、不启动 prepare，结果 `reused` | `cargo test` 日志 |
| R1 | 本机全新根首次安装 Qwen-Image 2.1 Turbo | 预检通过、prepare 完成、ready；记录实测 commit 余量与 prepare 峰值 | `S3b` JSON + 日志 |
| R2 | 本机安装 Qwen-Image 2.1 / MOSS（可选串行） | 同上；记录各自 prepare 耗时 | `S3b` JSON |
| R3 | Qwen/MOSS 标准档生成 | 1024²、PNG 尺寸/字节/SHA-256、stages、host GPU 采样 | 生成 JSON + PNG |
| R4 | 重启与导出 | 重启后就绪；导出与资产逐字节一致 | JSON + SHA-256 |
| R5 | 低于新门槛的机器仍被拒（单元模拟） | 可用物理 20 GiB + commit 23 GiB → 量化前错误、无半成品 | 单元测试 |
| R6 | 8 GB 参考机口径回归 | 其 commit 余量 60+ GiB 仍通过，不需要重测机器 | 既有 `verification/windows-8gb-tier/` |
| Q1 | 质量门禁 | cargo fmt/clippy/test、前端 test/check/build、openspec strict 全绿 | 构建日志 |
| D1 | 规格与归档 | 主规格更新为 24 GiB 口径；change 归档；strict 通过 | openspec 输出 |

## 判定规则

- 修复前 blocked 证据保留（S3），修复后通过证据单独记录（S3b），不得覆盖历史。
- prepare 峰值若超过 23.5 GiB，需要重新评估 24 GiB 余量是否仍合理（记录并暂停归档）。
- 安装失败时不得留下半成品 prepared 目录或 ready 记录。
- 生成数字与 GPU 采样方法沿用 `certify-windows-10-target-machine` 的规则（host 采样含桌面基线）。

## 执行顺序

1. U1/U2 单测 → 2. 构建与质量门禁 → 3. R1（全新根 Qwen Turbo）→ 4. R3 生成 → 5. R2（可选 Qwen base / MOSS）→ 6. R4 → 7. D1 归档。
