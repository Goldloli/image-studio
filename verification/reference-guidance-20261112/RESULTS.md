# 作品删除被引用提示的解除指引 —— 验收结果

日期：2026-11-12（Asia/Shanghai）。Change：`clarify-work-reference-guidance`
（Beads `image-studio-huq`）。执行方式：Rust/Vitest 回归 + 原生 computer use
（`target/dev/Image Studio.app`，v6 真实数据根，构建于 `627bd5f`/`60fb070` 与能力显示名修复之后）。

## 自动化

- Rust `works_referenced_by_unsent_or_active_requests_cannot_be_deleted` 扩展为三类断言：
  - 当前草稿源图 → `kind=draft`、`capability_id=None`、detail 含“移除源图或更换源图”；
  - 未激活能力槽位 → `kind=retained`、`capability_id=default_capability_id()`、detail 含“切回该能力”；
  - 活动请求 → `kind=active`、detail 含“等待”；解除后冲突清空。
- 前端 81 tests：`referenceConflictsMessage` 三类格式、`capabilityDisplayName` 的文本 recipe /
  Klein 编辑 / BiRefNet / 未知 id 回退、空列表不崩溃。
- 前端 `check`、`build` 通过。

## 原生复验

用真实历史作品 A（`Cinematic sports fashion collage…`，保持未删除）构造引用，未实际删除用户作品：

1. **draft**：Klein 编辑草稿把 A 设为当前源图 → 作品页删除 A 被拒，消息：
   `仍被以下位置用作源图，请先解除后重试：「新会话」请在对应会话中移除源图或更换源图后重试`
   （截图 `native-guidance-draft.png`）。
2. **retained**：仅切换模型到 Z-Image 后（槽位保留 A，当前参数无源图）再次删除：
   `…「新会话」请在对应会话切回该能力后移除源图，或更换源图后重试（对应能力：FLUX.2 Klein 4B 单图编辑）`
   （截图 `native-guidance-retained.png`）。首版显示名曾回退原始 capability id，已补映射
   （提交 `fix(frontend): name the Klein edit capability in retained references`）并以当前构建复验。
3. **active**：真实发起 Klein 编辑请求（A 为源图）后立即删除 A，同一条消息同时列出三类引用：
   `…「验收-活动引用」请在对应会话中移除源图或更换源图后重试；「验收-活动引用」请在对应会话切回该能力后移除源图，或更换源图后重试（对应能力：FLUX.2 Klein 4B 单图编辑）；「验收-活动引用」请等待该请求完成或取消后再试`
   （截图 `native-guidance-active.png`）。
4. A 全程保持 active、文件未动；随后取消该请求（DB attempt `cancelled`），删除测试会话，
   `stale_refs=0`，应用退出。

## 质量门

- 前端 test/check/build 通过；Rust `fmt`/`clippy -D warnings`/`test --workspace` 通过；
- `openspec validate clarify-work-reference-guidance --strict` 通过；全量 strict 仅剩既有 4 条
  长度警告（`image-studio-6g2`）。

## 结论

规格要求的“标明关联会话和解除方式”已满足：拒绝消息按引用类型给出可执行动作，并区分当前源图、
未激活能力槽位（含能力名）与活动请求；用户按提示解除后可正常删除。
