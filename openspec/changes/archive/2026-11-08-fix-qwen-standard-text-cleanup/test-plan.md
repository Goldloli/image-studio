# Test plan: fix-qwen-standard-text-cleanup

## 覆盖

| ID | 项 | 判定 | 证据 |
|---|---|---|---|
| T1 | 标准档文本装载分支前后向释放 | `reserved` after release ≤2.5 GiB（reference 与重建缓存各一次） | `target/probe-qwen-text.log` 修复后重跑 |
| T2 | 产品内 12 GB 标准档 Qwen Turbo 生成 | 成功产出 1024² PNG，资源/耗时记录完整 | `verification/windows-12gb-standard-tier/` |
| T3 | 紧凑档分支回归 | 该分支代码未改；现有 8 GB 证据保留 | `verification/windows-8gb-tier/` |
| T4 | 质量门 | cargo fmt/clippy/test、openspec strict 全绿 | 构建日志 |

## 判定规则

- 不得通过放宽 `CLEANUP_RESERVED_LIMIT_GIB` 或跳过检查来"通过"。
- 生成失败时保留 attempt 错误与 worker 日志，不伪造资产。
