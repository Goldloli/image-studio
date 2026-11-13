# Design

## Context

动机与范围见 [proposal.md](proposal.md)。当前 macOS build 先通过
`scripts/prepare-macos-dev-app.py` 组装 `.app`，再由 `ditto` 生成 zip；
release workflow 在 Ubuntu 上 `unzip` 后检查包身份。DMG-only 方案需要把最终
产物验证移到具备 `hdiutil` 的 macOS runner，并保持现有 `.app` 组装、正式 bundle
identifier、arm64 sidecar 与未签名限制不变。

## Goals / Non-Goals

**Goals:**

- 从下一个版本开始，macOS Release 只提供一个可校验、可挂载、可直接拖入
  Applications 的 DMG。
- DMG 生成与验证不依赖第三方打包程序，使用 macOS 自带 `hdiutil`。
- Release workflow 在最终 draft 前重新下载并挂载验证 Mac artifact。
- checksum、安装文档、失败排查与验收证据都使用 DMG 口径。

**Non-Goals:**

- 不重新签发或修改已发布的 `v0.1.0` 至 `v0.2.1` tag、Release 或附件。
- 不新增代码签名、Apple notarization、pkg 安装器或自动更新。
- 不改应用 UI、本地数据、模型、运行时、worker 协议或业务功能。
- 不增加自定义 Finder 背景、图标位置或 AppleScript 驱动的窗口布局。

## Decisions

### 1. DMG 是唯一 macOS 发布附件

Future macOS Release SHALL 只附
`Image-Studio-<version>-macos-arm64.dmg`。build 与 release 的文件数量断言从“一个
zip”改为“一个 DMG、零个 macOS zip”。不保留双格式过渡期，因为用户已明确要求
不要 zip，且双格式会扩大 checksum、文档与发布审阅范围。

### 2. 保留 `.app` 作为内部已验证 bundle

继续由现有 `prepare-macos-dev-app.py` 生成和校验 `.app`。新增
`scripts/package-macos-dmg.py` 只负责把已验证 bundle 封装为 DMG，避免把应用身份、
sidecar 复制和版本规则复制到第二个打包器。DMG 脚本接收 app 路径、输出路径和
volume name，并在失败时返回非零。

### 3. DMG 内只放应用和 Applications 快捷方式

脚本创建临时 stage 目录：

```text
Image Studio.app
Applications -> /Applications
```

随后执行 `hdiutil create -volname "Image Studio" -srcfolder <stage> -ov -format UDZO`
生成压缩只读映像。不添加背景图、DS_Store 或 AppleScript 排版。这样 CI 不依赖
Finder 状态，也不容易因隐藏元数据产生非确定性差异。

备选方案是启用 Tauri 自带 DMG bundler；当前仓库使用自定义 `.app` 组装并关闭
`bundle.active`，引入另一条 bundling 路径会增加身份漂移风险。另一个备选是
`create-dmg` 等第三方工具，会新增供应链依赖并且并非安装功能所必需。

### 4. 验证在 macOS 上完成

DMG build 先在 macOS job 内执行：

1. `hdiutil verify <dmg>` 检查磁盘映像结构；
2. `hdiutil attach -nobrowse -readonly -mountpoint <unique>` 挂载；
3. 检查根目录中的 `Image Studio.app` 和 `Applications` symlink；
4. `plutil -lint` 检查 `Info.plist`；
5. 核对 identifier、短/构建版本与 `LSMinimumSystemVersion 11.0`；
6. 用 `file` 核对主程序及 diagnostic、supervisor、Real-ESRGAN sidecar 均为
   arm64；
7. 核对许可资源；
8. `hdiutil detach`，失败时通过 trap 尽力卸载唯一 mountpoint。

release workflow 增加只读 `verify-macos-dmg` job；它重新下载 build artifact、
校验 checksum 并重复挂载/身份检查。最终 Ubuntu release job 只做附件数量、类型、
checksum 和文档完整性检查，依靠已通过的 macOS verification job，不在 Linux 上
尝试解析 DMG 内部文件系统。

### 5. 旧版本保持不可变

历史上 zip 附件与旧 checksum 不改名、不重新上传，也不在验收文档中伪造为 DMG。
新的 DMG 契约从包含本 change 的第一个发布版本开始；旧 Release 仍作为
zip 历史记录保留。发布版本号在实现阶段使用下一个未占用 patch tag（当前预期
`0.2.2`），不得重写 `v0.2.1`。

## Risks / Trade-offs

- **DMG 挂载在 CI 中失败** → 使用唯一 mountpoint、`trap` 卸载、`hdiutil verify`
  和显式日志；失败时不进入 release job。
- **Linux release job 无法验证 DMG 内部** → 增加 macOS verification job，最终
  Linux job 只汇总已验证产物与 checksum。
- **DMG 比 zip 增加构建步骤和体积** → 接受少量构建时间，换取用户明确要求的
  原生安装体验；不引入第三方打包工具。
- **未签名应用仍触发 Gatekeeper** → DMG 只改善下载/安装流程；README 和 Release
  Notes 继续要求校验 SHA-256，并保留解除 quarantine 的路径说明。
- **历史文档与新契约混淆** → 文档明确旧版本为 zip，未来版本为 DMG；验收记录按
  版本目录保存，不覆盖旧证据。
