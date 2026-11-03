# Windows 端接续交接（2026-11-03）

本文写给下一台 Windows 开发机上的代理/开发者。先读 [AGENTS.md](../AGENTS.md)、
[CLAUDE.md](../CLAUDE.md)、本文件和 [proposal](../openspec/changes/introduce-windows-foundation/proposal.md)。
`doc/11-windows-development-handoff.md` 是 2026-11-01 的上一份交接；本轮进度以本文和 git
历史为准。

## 0. 一句话现状

`introduce-windows-foundation` 的 Windows 可自动化 P0 已基本完成并留有证据；剩余是
需要管理员/人工配合的门槛（禁网、小卷、干净用户/WebView2、100%/200% DPI、真实 IME）
和 Mac 回归。`main` 已推送，远程 `origin/main` 与本地一致（以 `git rev-parse origin/main`
为准），工作区干净。

## 1. 远程与 push 状态（已解决）

- 仓库：`Goldloli/image-studio`，`origin` = `https://github.com/Goldloli/image-studio.git`，分支 `main`。
- 用户于 2026-11-03 明确授权本次 push。首次 push 被 GitHub 拒绝：

  ```text
  remote: error: GH007: Your push would publish a private email address.
  ! [remote rejected] main -> main (push declined due to email privacy restrictions)
  ```

  原因是历史提交作者邮箱 `Kevinforeveryeah@gmail.com` 被 GitHub 的
  “Block command line pushes that expose my email” 保护拦截。
- 用户已在 GitHub → Settings → Emails 取消该保护，随后
  `bf26b04..0106a2e main -> main` 推送成功。交接时本地与 `origin/main` 一致。
- 如果以后再次出现 GH007：同样在 GitHub 设置里取消该保护后重试
  `git push origin main`（非交互，`GIT_TERMINAL_PROMPT=0`）。
  **不要**改写历史换邮箱：大量证据 JSON/文档引用提交哈希，改写会让证据失效。
- push/pull/Dolt 远程同步每次都需要明确授权；本次授权只覆盖这次源码 push。

## 2. 新机器环境要求

- Windows 10 x64（目标机是 Win10 22H2 / RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K）。
- Rust `1.99.1`（`rust-toolchain.toml`；含 rustfmt/clippy 与 `x86_64-pc-windows-msvc`）。
- Node `24.19.0` / npm `11.17.0`（`.nvmrc`、`engines`、`packageManager`）。
- Visual Studio Build Tools 2022 + MSVC + Windows SDK；WebView2 Runtime（当前 `156.0.4314.58`）。
- OpenSpec `1.12.0`、Beads `bd` `1.2.2`。
- 克隆建议：

  ```powershell
  git clone --config core.longpaths=true --config core.autocrlf=false https://github.com/Goldloli/image-studio.git
  cd image-studio
  git switch main
  ```

- 注意：代理运行环境可能是 **Windows PowerShell 5.1**（本机 `$PSVersionTable` 5.1，没有 `pwsh`）。
  PowerShell 脚本已按 5.1 修过（`$env:OS`、native stderr 处理）；新写脚本请兼容 PS 5.1，
  或在需要 PS7 时明确检查版本。

## 3. 新机器上没有的东西（Git 不含）

`target/`、模型权重、私有 Python 运行时、Gate 0 缓存、已安装应用、SQLite 数据根都不在
仓库里。新机上注意：

1. 不要期望 `target/windows-gate0/downloads|model`、`scripts/windows-ui/seed-cache.ps1`
   能直接工作；这些是本机缓存。新机第一条完整链路要用生产源下载，例如：

   ```powershell
   cargo test -p image-studio-desktop --locked `
     actual_windows_product_installs_from_an_empty_cache_and_resumes_after_cancel `
     -- --ignored --nocapture
   ```

   注意这个测试现在不是“只下载”：它在约 256 MiB 处主动取消、验证分段保留，再同根续传
   到 Ready（本机实测 3110.89 s；早先只下载版本是 1047.50 s）。如果没有 Gate 0 缓存，
   也可以直接安装应用后从 UI 触发生产 ModelScope 下载。
   `actual_windows_product_installs_from_the_validation_cache` 需要
   `target/windows-gate0` 缓存，新机默认没有，属预期失败环境前提而非产品回归。
2. 正式 UI 验收用 `--data-dir <隔离根>` 的新目录，应用会自行创建 `cache/data/logs/work`。
   不要复制 Mac 的 runtime/Ready，也不能用 Mac 数据根冒充 Windows 就绪。
3. 验证板卡与驱动：`nvidia-smi`，确认 RTX 3080 Ti / sm86 / 驱动 596.49 这一档；
   Intel UHD 770 不能作为回退。
4. 旧证据里引用的本机路径（`target/windows-ui-probe-data-20261103` 等）新机不存在，
   这是预期的；证据本身仍在 `verification/` 中。

## 4. 先跑这些命令确认基线

```powershell
git status
git log --oneline -20
openspec validate introduce-windows-foundation --strict --no-interactive
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
npm --prefix apps/frontend test
npm --prefix apps/frontend run build
bd prime
```

期望：openspec valid；fmt/clippy 0；workspace 测试约 230+ 通过、8 个显式忽略
（Windows 产品验收测试都在 ignored 列表里）；前端 37 tests。忽略测试含义见
[windows-quality-gate.json](../verification/windows-quality-gate.json)。

如果这是全新克隆、还没跑过打包脚本，`apps/desktop/build.rs` 会因为缺少 sidecar 而
直接 panic（`resource path binaries\inference-supervisor-x86_64-pc-windows-msvc.exe
doesn't exist`）。先执行：

```powershell
cargo build --locked -p inference-supervisor -p diagnostic-worker
New-Item -ItemType Directory -Force apps/desktop/binaries | Out-Null
Copy-Item target/debug/inference-supervisor.exe apps/desktop/binaries/inference-supervisor-x86_64-pc-windows-msvc.exe
Copy-Item target/debug/diagnostic-worker.exe apps/desktop/binaries/diagnostic-worker-x86_64-pc-windows-msvc.exe
```

`apps/desktop/binaries/` 已被 .gitignore 覆盖，不进仓库。

## 5. 已完成（Windows 侧，证据都在 verification/）

| 范围 | 结论 | 证据 |
|---|---|---|
| Gate 0（七项、三独立进程、资源预算） | passed | `verification/windows-gate0/README.md` 及同目录 JSON/PNG |
| 产品安装（缓存预置 / worker / Controller 实图 / 取消与强杀恢复） | passed | 同目录 `product-install.json` 等 |
| 正式空缓存 ModelScope 安装（35.56 GB） | passed，1047.50 s | `product-empty-cache-install.json` |
| 长连接中断分段续传 | passed | 同 README 说明 |
| 主动中断 → 同根续传 → Ready | passed，3110.89 s | `product-interrupted-resume.json` |
| 运行准备/探针取消 + 重试 | passed | `product-runtime-cancel-retry.json` |
| 坏摘要/缺件激活拒绝 | passed | `product-activation-failures.json` |
| Windows 全量质量门（fmt/clippy/测试/构建/前端/清单审计） | passed | `verification/windows-quality-gate.json` / `.md` / `windows-r01-*.log` |
| 安装版原生 UI：UI 安装→Ready、中英文三次真实生成、原生对话框导出 PNG、重启恢复、任务/作品、设置/存储/关于、明暗主题、原生最大化/还原/最小化/拖拽、Tab/Shift+Enter、合成 IME 组合保护、UI 停止落 cancelled | passed（部分项有明确限制） | `verification/windows-installed-app/README.md` + `ui-acceptance.json` |
| 覆盖导出确认（P08 缺陷修复） | 代码/单元测试/安装包已就绪，确认对话框已出现；拒绝覆盖时旧文件保持不变；Yes 路径未再自动重跑 | `ui-acceptance.json` 的 `export_overwrite`、`apps/desktop/src/host.rs` |
| 安装包与更新烟测 | passed，包 244,745,354 bytes / `56837792…f047` | `verification/windows-installer/README.md`、`internal-package.json`、`install-update-smoke.json` |

## 6. 明天优先事项（按建议顺序）

1. **先解决 push**：按第 1 节关闭 GitHub 邮箱保护，然后 `git push origin main`。
   若新机是重新 clone，注意它可能落后于本地会话；以用户/本机最新提交为准。
2. **可选**：把 P08 的 Yes 分支也自动跑一遍（当前“确认覆盖”只有单元测试 + 代码路径证据）。
   用隔离数据根导出到已存在文件，确认对话框点“是”，校验文件变为 PNG 且 SHA-256 正确。
3. **~~调查 second-instance 异常~~（2026-11-03 已处理一半）**：本次重建包中，第二个实例用**不同**
   `--data-dir` 启动时退出码 101；单实例（无其他实例）正常。已在 Windows 11 开发机上尝试复现
   （不同根 0 ms/500 ms/25 s 错开、同根、强杀首实例并残留 12 个 `msedgewebview2.exe`）均未重现；
   但确认了机制：Tauri 在 `setup` 失败时于 `tauri-2.12.5/src/app.rs:1444` 直接 panic，任何 WebView2
   初始化失败都会变成静默 101。现已实现启动失败诊断（`logs/startup-failure.log` + 原生对话框 + 退出码 3）
   与每数据根独立 WebView2 用户数据目录，证据见 `verification/windows-startup-diagnosis/README.md`。
   目标机仍需重跑 W07 并复测该 101；未重现前不要当已通过。
4. 管理员配合项（脚本已准备，见 `scripts/windows-admin/README.md`）：
   - 整树禁网 I07/P06：`enable-network-denial.ps1` → 我/Copy 的 CDP 流程跑缓存安装 + 三次生成 → `remove-network-denial.ps1`；
   - 空间不足：`create-small-volume.ps1` 建 40 GB VHD → 安装版 `--data-dir V:\acceptance-root` 触发错误 → `remove-small-volume.ps1 -DeleteVhd`。
5. 人工项：100%/200% 显示缩放（当前机器是 150%/DPI 144，两逻辑尺寸已过）、
   真实微软拼音 IME、干净 Win10 测试用户（D02）、缺失/离线 WebView2（D03）。
6. Mac 回归 R02–R04（必须回 Mac）：完整质量门、隔离源码/禁网启动、原 Z-Image 三画幅/取消/强杀恢复、
   其余六型号各一条真实能力。Windows 端无法替代。
7. 全部适用 P0 通过后再：更新 test-plan 汇总、同步五份主规格、OpenSpec archive、
   重跑最终 R01（当前 `windows-quality-gate.json` 绑定的是加入导出修复前的提交）、
   关闭 bd issues、最后小步提交。

## 7. 构建与安装（Windows 内部测试包）

```powershell
# 完整构建（含质量门、sidecar、NSIS）
& scripts/build-windows-internal.ps1

# 快速重建（跳过质量门，之前已单独跑过时）
& scripts/build-windows-internal.ps1 -SkipQualityGates
```

产物：`target/release/bundle/nsis/Image Studio_0.1.0_x64-setup.exe`。
静默安装/覆盖安装：

```powershell
Start-Process -FilePath 'target\release\bundle\nsis\Image Studio_0.1.0_x64-setup.exe' -ArgumentList '/S' -Wait
```

安装到 `%LOCALAPPDATA%\Image Studio`（program 目录）；用户数据根默认
`%LOCALAPPDATA%\ImageStudio`。验收请用 `--data-dir` 隔离根。

构建脚本已知坑（均已修）：
- 必须在 Windows 上跑；用 `$env:OS` 判断（PS 5.1 没有 `$IsWindows`）。
- 脚本会 `Push-Location` 到前端目录，`Tee-Object` 日志必须用**绝对路径**。
- PS 5.1 下 native 命令写 stderr 会变成 NativeCommandError；`Invoke-Checked` 已局部放宽 EAP。

## 8. 安装版 UI 自动化方法

前提：应用由 `scripts/windows-ui` 里的 harness 用隔离数据根启动，并设置：

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9333'
& "$env:LOCALAPPDATA\Image Studio\image-studio-desktop.exe" --data-dir "<隔离根绝对路径>"
```

常用命令（详见 `scripts/windows-ui/README.md`）：

```powershell
node scripts/windows-ui/cdp.mjs text
node scripts/windows-ui/cdp.mjs eval "document.querySelector('textarea').value"
node scripts/windows-ui/cdp.mjs type textarea '雨后的上海街角，一位撑伞的行人'
node scripts/windows-ui/cdp.mjs key Enter
node scripts/windows-ui/cdp.mjs shiftenter
node scripts/windows-ui/cdp.mjs tab
node scripts/windows-ui/cdp.mjs shot target/ui.png
& scripts/windows-ui/resize-window.ps1 -Width 1280 -Height 800
& scripts/windows-ui/save-ui-dialog.ps1 -Path "$PWD\target\export.png" -TitleMatch '导出'
& scripts/windows-ui/answer-dialog.ps1 -TitleMatch '覆盖' -ButtonMatch '否|No'
target/windows-gate0/runtime/python.exe scripts/windows-ui/extract-installed-records.py <data-root> <out.json>
```

注意：
- `--data-dir` 必须指向隔离根；不要对用户真实数据根做自动化操作。
- 覆盖确认对话框的标题/按钮是系统语言相关（中文“是/否”），`answer-dialog.ps1`
  用 regex 匹配按钮名；若系统为英文，传 `-ButtonMatch 'Yes|No'`。
- 应用是 release 构建，WebView2 调试端口只在设置了该 env var 时开启；用完关闭应用。

## 9. Beads（bd）与跨机任务

- `.beads/` 的 Dolt 数据库不随 git 传输；新机 `bd prime` 看到的是本地空库/新建库。
  本轮 Windows 本地 issue 只存在于本机，**未做 `bd dolt push/pull`**（未授权）。
- 本轮相关 issue（本机 ID，仅供参考）：`550`（UI/IME/DPI）、`76x`（安装包/禁网）、
  `ba1`（原生窗口捕获）、`neo.7`（空间不足）、`ipw`（预检取消延迟）、`g03`（导出覆盖修复）。
- 记录任务仍用 `bd`；不要用 markdown TODO 代替。跨机同步前先找用户确认。

## 10. 规则与纪律（重申）

- **OpenSpec**：新增/修改功能必须先写 change、等用户确认后才写代码；本轮 P08 属于
  已批准 change 内的缺陷修复。未确认的 change 不得顺手实现。
- **Git**：本地小步提交、一提交一主题、精确暂存；本轮起用户已授权一次 push，
  后续 push/pull/Dolt 同步仍需单独授权。不要提交模型权重/大文件/个人数据/系统敏感路径。
- **证据**：每项验收真实记录 passed/failed/blocked；未运行不能写成通过；不把安装包
  烟测当完整模型认证；不重做已通过的 Gate 0；不复制 Mac runtime 或缩小范围。
- **质量**：提交前跑相关最小测试；任务结束前跑完整质量门。已知失败不得伪装成完成。
- **目标机**：任何"Win11/虚拟机/CI 通过"都不能替代本机 Win10 结果；UI/IME/DPI 必须
  有原生环境证据。

## 11. 交接检查清单

- [x] `git push origin main` 已成功（2026-11-03，`bf26b04..0106a2e`；本交接文档本身也已在后续推送中）
- [ ] 工具链版本与第 2 节一致
- [ ] `openspec validate ... --strict` 通过
- [ ] 质量门基线（第 4 节命令）通过
- [ ] 读 `verification/windows-installed-app/README.md` 和 `windows-gate0/README.md`
- [ ] 读 `scripts/windows-admin/README.md` 了解需要用户配合的验收
- [ ] 选一个第 6 节事项开始，并在 bd 建/认领对应 issue
