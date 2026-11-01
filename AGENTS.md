# Agent Instructions

## OpenSpec 流程（强制）

本项目使用 **OpenSpec**（openspec/）做规格驱动开发。**新增或修改任何功能**，必须走 OpenSpec change 流程，禁止直接写代码：

1. **先写文档，不写代码**：用 `openspec` 创建 change 文档（`/opsx:propose "想法"` 或 `openspec change new`），写清 proposal、specs（delta）、design 与 tasks。
2. **等用户确认**：change 文档写好后，向用户展示文档内容并明确等待确认。**未获用户确认前，一律不得开工写实现代码。**
3. **确认后开工**：用户确认后，才可按 tasks 开始实现；实现完成后按 OpenSpec 流程归档（archive）change 并更新 specs。

例外：不涉及行为的琐碎改动（文档错字、代码格式化等）可不走 change；bug 修复是否走 change 视影响面而定，凡涉及行为、接口、结构的改动一律走 change。任务跟踪仍用 `bd`（见下），change 获确认后可在 bd 中建对应 issue 跟踪实现进度。

## Git 提交策略（强制）

用户已明确授权本项目进行本地提交，并要求“尽量多 commit”。当用户已授权实施某项工作、且没有明确禁止提交时，代理应默认持续做小步本地提交，不需要为每次 commit 单独询问；该授权不包含 push、pull 或远程同步。

- **小步提交**：把工作拆成可独立解释和验证的最小逻辑单元。每完成一个单元就立即 commit，不要攒到会话结束或整个功能完成后一次性提交。
- **一提交一主题**：一个 commit 只解决一件事。实现、测试/验收证据、规格文档、纯格式化、资源更新和 Agent 规则在可以安全独立时分别提交。
- **保持可审查**：按依赖顺序提交检查点；提交前运行与该单元相关的最小测试、lint 或构建，任务结束前再跑完整质量门。禁止把已知失败或无关改动伪装成已完成提交。
- **不混入无关工作**：使用精确路径暂存，提交前检查 `git diff --cached`；其他代理、用户或未确认 OpenSpec change 的改动不得顺手带入。
- **提交信息**：使用 Conventional Commits，例如 `feat:`、`fix:`、`test:`、`docs:`、`refactor:`、`chore:`；标题写结果，正文在需要时说明原因和验证。
- **远程操作须明确授权**：本地 commit 可直接执行；push、pull、Dolt 远程同步仍须用户明确授权。GitHub 私有仓库为 `Goldloli/image-studio`，`origin` 指向 `https://github.com/Goldloli/image-studio.git`。用户于 2026-11-01 授权创建并首次推送源码，供 Windows 端接续开发；该授权不自动扩展为后续任意推送或 Dolt 远程同步。

---

This project uses **bd** (beads) for issue tracking. Run `bd prime` for full workflow context.

> **Architecture in one line:** Issues live in a local Dolt database
> (`.beads/dolt/`); cross-machine sync uses `bd dolt push/pull` (a
> git-compatible protocol), stored under `refs/dolt/data` on your git
> remote — separate from `refs/heads/*` where your code lives.
> `.beads/issues.jsonl` is a passive export, not the wire protocol.
>
> See [sync-concepts](https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md)
> for the one-screen overview and anti-patterns (don't treat JSONL as the
> source of truth; don't `bd import` during normal operation; don't
> reach for third-party Dolt hosting before trying the default).

## Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work atomically
bd close <id>         # Complete work
bd dolt push          # Push beads data to remote
```

## Non-Interactive Shell Commands

**ALWAYS use non-interactive flags** with file operations to avoid hanging on confirmation prompts.

Shell commands like `cp`, `mv`, and `rm` may be aliased to include `-i` (interactive) mode on some systems, causing the agent to hang indefinitely waiting for y/n input.

**Use these forms instead:**
```bash
# Force overwrite without prompting
cp -f source dest           # NOT: cp source dest
mv -f source dest           # NOT: mv source dest
rm -f file                  # NOT: rm file

# For recursive operations
rm -rf directory            # NOT: rm -r directory
cp -rf source dest          # NOT: cp -r source dest
```

**Other commands that may prompt:**
- `scp` - use `-o BatchMode=yes` for non-interactive
- `ssh` - use `-o BatchMode=yes` to fail instead of prompting
- `apt-get` - use `-y` flag
- `brew` - use `HOMEBREW_NO_AUTO_UPDATE=1` env var

<!-- BEGIN BEADS INTEGRATION v:1 profile:minimal hash:1105d646 -->
## Beads Issue Tracker

This project uses **bd (beads)** for issue tracking. Run `bd prime` to see full workflow context and commands.

### Quick Reference

```bash
bd ready              # Find available work
bd show <id>          # View issue details
bd update <id> --claim  # Claim work
bd close <id>         # Complete work
```

### Rules

- Use `bd` for ALL task tracking — do NOT use TodoWrite, TaskCreate, or markdown TODO lists
- Run `bd prime` for detailed command reference and session close protocol
- Use `bd remember` for persistent knowledge — do NOT use MEMORY.md files

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.

## Agent Context Profiles

The managed Beads block is task-tracking guidance, not permission to override repository, user, or orchestrator instructions.

- **Conservative (default)**: Use `bd` for task tracking. Do not run git commits, git pushes, or Dolt remote sync unless explicitly asked. At handoff, report changed files, validation, and suggested next commands.
- **Minimal**: Keep tool instruction files as pointers to `bd prime`; use the same conservative git policy unless active instructions say otherwise.
- **Team-maintainer**: Only when the repository explicitly opts in, agents may close beads, run quality gates, commit, and push as part of session close. A current "do not commit" or "do not push" instruction still wins.

## Session Completion

This protocol applies when ending a Beads implementation workflow. It is subordinate to explicit user, repository, and orchestrator instructions.

1. **File issues for remaining work** - Create beads for anything that needs follow-up
2. **Run quality gates** (if code changed) - Tests, linters, builds
3. **Update issue status** - Close finished work, update in-progress items
4. **Handle git/sync by active profile**:
   ```bash
   # Conservative/minimal/default: report status and proposed commands; wait for approval.
   git status

   # Team-maintainer opt-in only, unless current instructions forbid it:
   git pull --rebase
   git push
   git status
   ```
5. **Hand off** - Summarize changes, validation, issue status, and any blocked sync/commit/push step

**Critical rules:**
- Explicit user or orchestrator instructions override this Beads block.
- Do not commit or push without clear authority from the active profile or the current user request.
- If a required sync or push is blocked, stop and report the exact command and error.
<!-- END BEADS INTEGRATION -->

<!-- BEGIN BEADS CODEX SETUP: generated by bd setup codex -->
## Beads Issue Tracker

Use Beads (`bd`) for durable task tracking in repositories that include it. Use the `beads` skill at `.agents/skills/beads/SKILL.md` (project install) or `~/.agents/skills/beads/SKILL.md` (global install) for Beads workflow guidance, then use the `bd` CLI for issue operations.

### Quick Reference

```bash
bd ready                # Find available work
bd show <id>            # View issue details
bd update <id> --claim  # Claim work
bd close <id>           # Complete work
bd prime                # Refresh Beads context
```

### Rules

- Use `bd` for all task tracking; do not create markdown TODO lists.
- Run `bd prime` when Beads context is missing or stale. Codex 0.129.0+ can load Beads context automatically through native hooks; use `/hooks` to inspect or toggle them.
- Keep persistent project memory in Beads via `bd remember`; do not create ad hoc memory files.

**Architecture in one line:** issues live in a local Dolt DB; sync uses `refs/dolt/data` on your git remote; `.beads/issues.jsonl` is a passive export. See https://github.com/gastownhall/beads/blob/main/docs/core-concepts/sync-concepts.md for details and anti-patterns.
<!-- END BEADS CODEX SETUP -->
