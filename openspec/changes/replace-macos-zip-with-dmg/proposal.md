# Proposal: replace-macos-zip-with-dmg

## Why

当前 macOS Release 只提供 `.app` zip，用户需要先解压再手动移动应用；用户已明确要求
后续 macOS 发布改为 DMG，以获得标准的“拖入 Applications”安装体验。现有已发布
的 `v0.2.1` 不动，新格式从下一版开始生效。

## What Changes

- **BREAKING**：后续 macOS GitHub Release 只发布
  `Image-Studio-<version>-macos-arm64.dmg`，不再发布 macOS `.zip`。
- DMG 内包含经过身份、版本、架构与许可检查的 `Image Studio.app`，以及指向
  `/Applications` 的快捷方式；用户打开磁盘映像后可将应用直接拖入应用程序目录。
- 构建流程先生成 `.app`，再生成只读压缩 DMG；checksum、Release 附件数量与文件类型
  校验同步改为 DMG。
- GitHub Actions 在 macOS runner 上挂载、只读检查和卸载 DMG，验证包身份、arm64
  主程序与 sidecar、最低系统版本及许可资源后，才允许进入 draft Release。
- README、中文 README、Release Notes、CI 发布文档与验收模板删除新版本的 zip
  下载说明，保留未签名/未公证和 Gatekeeper quarantine 指引。
- 已发布的 `v0.1.0`、`v0.1.1`、`v0.2.0`、`v0.2.1` 历史 Release 保持原样；不为
  新格式重写旧 tag 或旧附件。

## Capabilities

### New Capabilities

无。

### Modified Capabilities

- `ci-release`：把 macOS arm64 发布产物从 `.app` zip 改为只读 DMG，并定义构建、
  挂载验证、checksum、Release 附件和未来版本下载契约。

## Impact

- 修改 `.github/workflows/build.yml` 的 macOS 打包与 artifact 名称。
- 修改 `.github/workflows/release.yml` 的 macOS artifact 检查、附件数量断言、
  文档准备和 draft Release 输入。
- 新增或调整 macOS DMG 打包与验证脚本，并保留现有 `.app` 准备脚本作为内部步骤。
- 修改 `README.md`、`README.zh-CN.md`、`RELEASE_NOTES.md`、`doc/14-ci-release.md`
  与后续 Release 验收记录。
- 改动发布物格式与下载文件名，调用 GitHub Release 附件的自动化脚本需要从 zip
  切换到 DMG；应用数据、模型、运行时和本地工作区行为不变。
- 不修改已发布 Release，也不新增代码签名或 Apple notarization。
