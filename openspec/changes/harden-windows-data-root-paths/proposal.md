# Proposal: harden-windows-data-root-paths

## Why

`achieve-windows-model-parity` 的安装版验收发现一个真实 Windows 缺陷：当 `--data-dir` 指向较深目录时，共享 v2 运行时的暂存目录名把运行时路径推到 legacy MAX_PATH 边缘，onnxruntime 的 pybind11 DLL 导入报 `文件名或扩展名太长`，安装失败且留下 4.23 GB / 20,621 文件的 `.staging-*` 目录。失败证据见
[verification/windows-parity-app/install-failure-long-data-root.json](../../../verification/windows-parity-app/install-failure-long-data-root.json)。

该行当时按 blocked 记录并由用户接受归档；随后用户要求把"深数据根加固"作为下一步跟进（bd `image-studio-slk`）。本 change 把"深目录下从 cryptic DLL 报错 + 目录残留"改为"启动前给出可操作错误，或自动缩短暂存路径后正常安装"，并顺手回收历史暂存目录。

## 用户决定（2026-11-04）

1. 按顺序先做本 change，再做 `introduce-ci-release-pipeline`。
2. 允许先出 change 文档，确认后实现；实现完成后本地验收并归档。
3. 不要求打开系统级 long-path 注册表/组策略；本机与目标机按默认 Windows 配置验收。

## What Changes

- **缩短运行时暂存目录名**：从 `.{runtime_id}.staging-{pid}-{nanos}`（约 95 字符）改为 `.staging-{pid}-{8位随机}`（≤26 字符），消除 35+ 字符的嵌套压力；新增唯一的暂存目录名构造函数与长度断言测试。
- **启动前数据根路径校验**：在 `AppPaths::resolve` 完成 canonicalize 后，按冻结运行时布局的最坏情况（最长 DLL/pyd 相对路径 + 暂存目录名 + 固定前缀）校验 data 路径长度；超限时写入 `startup-failure.log`、弹出对话框并以文档化退出码 3 退出，错误信息给出"改用更短 `--data-dir`（如 `C:\ImageStudioData`）"的可操作建议。
- **安装前清理历史暂存目录**：新运行时安装开始前，清理 `data/runtimes/.staging-*` 残留（仅删除匹配该模式的普通目录，不触碰正式运行时），避免失败尝试长期占用磁盘；增加单元测试覆盖命名匹配与清理边界。
- 更新 README/验收文档：说明数据根路径支持边界、短目录建议，以及该 finding 的闭环状态。

## 成功标准

- 深数据根（此前失败的 ~83 字符数据根）在缩短暂存目录后能完成运行时安装与探针（用本地校验缓存复装至少一个共享 v2 型号），不再出现 onnxruntime 长路径 DLL 错误。
- 超过支持边界的数据根在启动阶段即以清晰中文提示 + `startup-failure.log` + 退出码 3 拒绝，不再进入下载/解压阶段，不留下大体积暂存目录。
- 失败/取消后可自动回收残留 `.staging-*` 目录；正常安装/探针/生成路径不回归。
- 单元测试覆盖暂存目录名长度、路径校验边界、残留清理；workspace 质量门全绿；重建安装包并在目标机复验，证据落入 `verification/windows-data-root-guard/`。

## 不做 / 超出范围

- 不承诺真正支持任意长路径（不改 registry LongPathsEnabled、不为第三方 Python/onnxruntime 注入 `\\?\`、不替换运行时布局）。
- 不做数据根自动迁移/搬迁工具；只做"短暂存 + 启动前明确拒绝 + 残留清理"。
- 不改变 macOS 路径行为；不改运行时身份/模型清单。
