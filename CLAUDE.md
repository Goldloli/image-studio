# Project Instructions for AI Agents

This file provides instructions and context for AI coding agents working on this project.

## 产品事实与证据边界（OpenSpec 沉淀）

- `openspec/specs/` 是已实现行为的权威规格；`doc/` 是研究、路线和背景资料，不能替代正式规格或真机证据。当前产品是本地优先的 Tauri 2 图片工作台，正式平台为 macOS arm64 与 Windows x64。
- 当前能力基线是七个本地模型、八项能力：文生图、单图编辑、去背景（RGBA + mask）和精确 4× 放大。标准档面向 ≥12 GB 显存，8 GB 紧凑档使用 768² / `float16`；Z-Image Turbo 仍要求 12 GB。
- 模型、快照、运行时、配方和 worker 协议身份必须固定。不得在既有安装 ID 下替换为另一上游快照，也不得用相似模型或手工复制的文件冒充原来源；来源或身份变化必须先更新 OpenSpec 并重新验证。
- 验收证据必须区分：CI 编译/单测、安装与原生 UI 回归、GPU 探针、单模型真实生成、完整能力矩阵。`waived` 与 `not_run` 永远不能写成通过；CI 不下载模型、不运行 GPU 推理，不能代替真机生图认证。
- 当前真机边界只覆盖已记录设备：macOS arm64；Windows 10 目标机 RTX 3080 Ti 12 GB；Windows 11 补充机 RTX 2060 SUPER 8 GB。不要把某台机器、某个 Windows 版本或某个 GPU 架构的结果外推为全部硬件支持。
- 面向用户的发布声明必须能对应到 `verification/`、OpenSpec 或当前源码。性能数字、显存口径和资源预算必须写明测量方法；worker 内部数值与宿主观测口径不得混用。
- 正式 macOS 产物未签名、未公证。文档必须保留“可能显示应用已损坏”的说明、SHA-256 校验要求和解除 quarantine 的终端命令；不得建议用户在校验失败后绕过系统保护。

## UI/UX 设计原则

- **Avoid unnecessary secondary labels and descriptions. Let the layout do the explaining.** 避免不必要的辅助标签与说明，让布局本身发挥解释作用。不要用文案重复结构、层级、图标、状态或交互已经能表达的信息；优先通过分组、留白、对齐、视觉层级和操作反馈传递含义。只有当信息无法从布局推断、可能造成关键歧义，或涉及安全与不可逆操作时，才补充最小必要说明。把主要注意力放在信息架构、视觉层次和交互体验上。

## 构建与质量门

Tauri 的 `generate_context!` 会在 Rust 编译时嵌入前端产物，因此在干净检出中必须先构建前端，再执行 Cargo 命令。

```bash
npm --prefix apps/frontend ci
npm --prefix apps/frontend run build
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm --prefix apps/frontend test
npm --prefix apps/frontend run check
npm --prefix apps/frontend run build
openspec validate --all --strict
python3 scripts/ci/validate-workflows.py
actionlint -no-color
```

修改具体功能时先跑最接近的单元/组件测试；交付前再跑适用的完整质量门。Windows 的 Clippy、测试和 Tauri 构建要求 `apps/desktop/binaries/` 中已有 CI/构建脚本生成的 sidecar。

## 版本与发布

- 发布版本必须同时更新根 `Cargo.toml` 的 `[workspace.package].version`、`apps/desktop/tauri.conf.json`、`apps/frontend/package.json` 及锁文件。tag、Cargo、Tauri 和前端版本必须完全一致。
- 版本 tag 使用 `vX.Y.Z`，只能指向已进入 `main` 的发布提交。推送分支先触发双平台 quality；推送 tag 触发双平台构建和 Release。
- `README.md` 是默认英文入口，`README.zh-CN.md` 必须同步更新功能和限制；README 与 Release Notes 应使用真实产品截图并写明证据边界。
- 发布前检查 macOS quarantine 解除说明、Windows SmartScreen 说明、checksums、未签名状态和已知限制。Release 生成后还要核对 Windows NSIS、macOS arm64 zip、包身份、checksum 和版本。
- 发布完成后，GitHub About 使用英文说明在前、中文说明在后，并维护准确 homepage 与 topics；远程 push、tag push 和 Dolt 同步仍以当前用户授权为准。

## Git 提交策略（强制）

用户已明确授权本项目进行本地提交，并要求“尽量多 commit”。当用户已授权实施某项工作、且没有明确禁止提交时，代理应默认持续做小步本地提交，不需要为每次 commit 单独询问；该授权不包含 push、pull 或远程同步。

- **小步提交**：把工作拆成可独立解释和验证的最小逻辑单元。每完成一个单元就立即 commit，不要攒到会话结束或整个功能完成后一次性提交。
- **一提交一主题**：一个 commit 只解决一件事。实现、测试/验收证据、规格文档、纯格式化、资源更新和 Agent 规则在可以安全独立时分别提交。
- **保持可审查**：按依赖顺序提交检查点；提交前运行与该单元相关的最小测试、lint 或构建，任务结束前再跑完整质量门。禁止把已知失败或无关改动伪装成已完成提交。
- **不混入无关工作**：使用精确路径暂存，提交前检查 `git diff --cached`；其他代理、用户或未确认 OpenSpec change 的改动不得顺手带入。
- **提交信息**：使用 Conventional Commits，例如 `feat:`、`fix:`、`test:`、`docs:`、`refactor:`、`chore:`；标题写结果，正文在需要时说明原因和验证。
- **远程操作须明确授权**：本地 commit 可直接执行；push、pull、Dolt 远程同步仍须用户明确授权。GitHub 私有仓库为 `Goldloli/image-studio`，`origin` 指向 `https://github.com/Goldloli/image-studio.git`。用户于 2026-11-01 授权创建并首次推送源码，供 Windows 端接续开发；该授权不自动扩展为后续任意推送或 Dolt 远程同步。

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


## Build & Test

_Add your build and test commands here_

```bash
# Example:
# npm install
# npm test
```

## Architecture Overview

_Add a brief overview of your project architecture_

## Conventions & Patterns

_Add your project-specific conventions here_
