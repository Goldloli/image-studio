# introduce-windows-foundation

为 Windows 10 x64 / RTX 3080 Ti 12 GB / 32 GB RAM / i7-13700K 建立桌面基础与首个 Z-Image Turbo 本地生成闭环；当前仅提案，等待确认。

**当前状态：文档草案，尚未批准实施。** 本轮没有修改实现代码、下载模型或运行 Windows。OpenSpec 校验通过和可 apply 状态均不替代用户确认。

| 文档 | 内容 |
|---|---|
| [proposal.md](proposal.md) | 待确认范围、用户已知前提、交付与门槛 |
| [design.md](design.md) | 低内存实验、平台/协议/安装身份、目录、进程监督和打包选择 |
| [specs/](specs/) | 两份新能力与三份既有能力的 delta |
| [tasks.md](tasks.md) | 批准后实施与验收范围，全部未开始 |
| [test-plan.md](test-plan.md) | Gate 0、平台、安装、生命周期、产品、安装包和 Mac 回归 |
| [sources.md](sources.md) | 设备事实、仓库现状与上游一手依据 |

本轮先完成目标机的单模型路线，不承诺七型号全部迁移。Gate 0 未通过时保留失败证据并停止模型产品化，不以缩图、换模型、更大硬件或仅壳可启动替代。实现进度在用户批准后由 Beads 管理；本轮文档工作为 `image-studio-f0z`。
