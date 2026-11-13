# Tasks

状态：**提案文档阶段。** 2026-11-13 用户已要求把后续 macOS 发布改为 DMG 且不要
zip；仍需用户确认本 proposal、spec delta、design、tasks 与 test-plan 后才能实施。

## 1. 文档与确认

- [ ] 1.1 完成 proposal、`ci-release` spec delta、design、tasks 与 test-plan，并运行 `openspec validate replace-macos-zip-with-dmg --strict`，向用户展示文档后等待确认。
- [ ] 1.2 用户确认后，将 Beads `image-studio-gga` 标记为实现中并冻结实现基线。

## 2. DMG 打包工具

- [ ] 2.1 新增 macOS DMG 打包脚本：接收已验证 `.app`、输出路径和 volume name，创建包含 `Image Studio.app` 与 `/Applications` symlink 的临时 stage，并验证 `--help`、缺失输入和错误扩展名。
- [ ] 2.2 用 `hdiutil create -format UDZO` 生成单文件 DMG，并验证脚本失败时返回非零且不遗留目标文件。
- [ ] 2.3 增加 DMG 挂载验证辅助逻辑或脚本，验证 `hdiutil verify`、只读挂载、`Applications` 链接、`Info.plist`、四个 arm64 Mach-O 与许可资源。
- [ ] 2.4 在 macOS 本地用当前 release `.app` 生成 DMG，并按 D01–D07 记录结构、身份、架构、许可与 checksum 证据。

## 3. 构建工作流

- [ ] 3.1 修改 `.github/workflows/build.yml`：macOS 先构造 `.app`，再生成 `Image-Studio-<version>-macos-arm64.dmg`，使用 `macos-dmg` artifact 上传 DMG 与 `SHA256SUMS-macos.txt`。
- [ ] 3.2 在 macOS build job 内挂载 DMG 并复核应用包；断言 artifact 中恰好一个 `.dmg`、零个 `.zip`，然后用 `actionlint` 与 workflow 结构校验脚本验证。

## 4. 发布工作流

- [ ] 4.1 修改 `.github/workflows/release.yml`：增加只读 `verify-macos-dmg` job，重新下载 artifact、核对 checksum、挂载并验证 DMG 后才解锁最终 release job。
- [ ] 4.2 最终 draft Release 只接受一个 Windows `.exe`、一个 macOS `.dmg`、对应 checksums 和文档；对 zip 存在、DMG 缺失或 checksum 失败，验证 workflow 静态检查与故意坏 fixture 均失败。
- [ ] 4.3 用 `actionlint`、`scripts/ci/validate-workflows.py` 与 YAML 结构检查验证新的 job 依赖、权限和挂载 cleanup。

## 5. 发布文档与验收模板

- [ ] 5.1 更新英文/中文 README：DMG 下载、打开映像、拖入 Applications、SHA-256 和未签名 Gatekeeper 说明；搜索新版本相关段落后确认不再引用 macOS zip。
- [ ] 5.2 更新 `RELEASE_NOTES.md`、`doc/14-ci-release.md` 与 workflow 故障排查，明确历史版本仍为 zip、本 change 之后为 DMG-only；运行 Markdown/JSON 结构检查。
- [ ] 5.3 为下一版本创建 `verification/ci-release/<version>/` 的 DMG 验收记录模板，包含 asset digest、挂载结果、包身份、arm64、启动 smoke 和历史版本不可变检查。

## 6. 集成验证与发布

- [ ] 6.1 在 macOS release 构建上执行 D01–D13，在适用环境执行 D14；将未运行的 Gatekeeper 人工项标为 `not_run`，不得写成通过。
- [ ] 6.2 运行前端 81 项测试、typecheck、build；Rust fmt、Clippy、workspace tests、release build；OpenSpec strict；workflow 校验与 actionlint。
- [ ] 6.3 更新到下一个未占用 patch 版本（当前预期 `0.2.2`），同步 Cargo、Tauri、前端 package/lock 与双语发布文档；不得修改 `v0.2.1` 或其附件。
- [ ] 6.4 用户确认发布后，推送 main 与版本 tag，等待双平台 CI/release；确认 draft 只含 DMG，不自动 publish 或覆盖历史 Release。
- [ ] 6.5 发布后重新下载 DMG，按 R01–R05 复验 checksum、挂载、安装内容、启动 smoke 与无 zip，并记录 GitHub Release URL。

## Workflow follow-up

- 用户确认后归档本 change，并合并 `ci-release` delta。
- 关闭 Beads `image-studio-gga`；若真实发布尚未执行，保留独立发布跟踪项。
