# Test plan: introduce-ci-release-pipeline

## 本机可验证（Windows）

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| C1 | workflow YAML 语法 | yamllint 通过（如可用再跑 actionlint） | 本地命令输出 |
| C2 | 质量命令清单 | workflow 中的 fmt/clippy/test/前端/openspec 命令在本机逐条通过 | 日志 |
| C3 | Windows 构建脚本同参数 | `scripts/build-windows-internal.ps1 -SkipQualityGates` 产出安装包 + SHA256SUMS 样例 | 产物 + checksum 文件 |
| C4 | macOS 打包脚本静态检查 | `--profile release` 参数解析与路径常量单测/静态检查通过；标注 Windows 上不可执行 | 单元测试 |
| C5 | release 模板/checksum 脚本 | 本机生成一次 SHA256SUMS 样例并通过校验 | 文件 |
| C6 | 质量门 | fmt/clippy/tests/前端/openspec strict 全绿 | 日志 |

## 用户启用后验证（Mac / GitHub）

| ID | 场景 | 期望 | 证据 |
|---|---|---|---|
| C7 | push 后 quality workflow | Windows/macOS 两个 job 全绿 | Actions run URL |
| C8 | workflow_dispatch build | 两平台 artifact 与 SHA256SUMS 可下载 | run artifacts |
| C9 | tag `v*` release | draft Release 含两平台产物 + checksums，未自动发布 | release URL |
| C10 | 未签名产物说明 | 文档与 release notes 明确 SmartScreen/Gatekeeper 提示 | 文档 |

## 不做

- 不在 CI 跑模型下载/GPU 推理/真实生成。
- 不验证代码签名与公证（后续 change）。
