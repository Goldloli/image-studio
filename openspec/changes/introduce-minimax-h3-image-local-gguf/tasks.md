# Tasks

## 1. 批准与基线

- [ ] 1.1 用户明确批准本 proposal/spec/design 后，在 Beads 中建立实现子任务并记录批准范围，验证 `bd show <id>` 显示依赖与验收范围
- [ ] 1.2 记录当前七个稳定型号/八能力的基线测试结果，验证最接近的 Rust、前端和 OpenSpec 测试在改动前通过
- [ ] 1.3 冻结本次候选来源、文件字节、SHA-256、许可证哈希和 `stable-diffusion.cpp` commit 到实验记录，验证来源 URL 均为登记 revision 且不包含 `master`

## 2. 独立技术 Gate

- [ ] 2.1 在隔离数据根下载并校验 Q2 DiT、Q2 文本编码器、VAE 和固定原生运行时，验证三个组件摘要与登记值一致
- [ ] 2.2 使用上游 CLI 在 macOS arm64 记录 1024² 与 2048² 的真实生成、耗时、内存、取消和第二次独立执行，验证结果写入 `verification/` 且不触碰默认用户目录
- [ ] 2.3 在 Windows RTX 3080 Ti 12 GB/32 GB 记录 1024² 与 2048²、GPU dedicated/shared、commit、可用内存、耗时和进程树，验证无未披露的共享内存回退
- [ ] 2.4 在同一提示词、种子、画幅和画质设置下对照 Q2/Q3/Q4 与更高精度基线，验证样图、参数、哈希和人工评语已保存
- [ ] 2.5 根据 2.2–2.4 记录 go/no-go；若不满足预算或质量，停止产品代码实现并保留失败证据，验证 OpenSpec/Beads 状态不写成通过

## 3. 来源与资源清单

- [ ] 3.1 新增 MiniMax-H3-Image 模型、文本编码器、VAE、许可证和 NOTICE 固定清单，验证清单校验脚本接受且篡改任一字节或 revision 会失败
- [ ] 3.2 新增 Windows/macOS 原生 runtime 清单，固定 source commit、构建工具链、可执行文件和依赖哈希，验证未知平台或二进制身份会被拒绝
- [ ] 3.3 新增安装空间计算，分别覆盖权重、runtime、缓存、暂存和最终占用，验证 Q2/Q3/Q4 显示值来自清单而非参数量推算
- [ ] 3.4 更新第三方许可证与来源记录，验证 `stable-diffusion.cpp` MIT、MiniMax Community License、AUP 和 NOTICE 均可从关于页或安装记录追溯

## 4. 目录与界面

- [ ] 4.1 扩展模型目录 schema，增加 `supportLevel` 与稳定/实验统计，验证旧七型号解析默认 `stable` 且现有统计测试通过
- [ ] 4.2 以 `planned + experimental` 登记 MiniMax-H3-Image，填写完整组件、许可证、平台、支持画幅和无 compact 事实，验证未通过前无安装/生成动作
- [ ] 4.3 在模型详情中展示完整下载量、6.88 GB 仅为 DiT、实验标识、画幅、许可证和平台证据，验证 8 GB 设备与未知设备均显示不可执行原因
- [ ] 4.4 在创作模型中登记 6 步文生图 recipe 和 1024²/2048² 画幅，验证未登记画幅、负提示词、参考图和编辑请求在发送前被拒绝
- [ ] 4.5 增加许可证地域/商业用途确认流程，验证未确认时不触发下载，确认后记录许可证哈希且不会根据 IP 自动放行
- [ ] 4.6 增加前端组件测试，覆盖稳定/实验筛选、完整预算、许可证阻断、8 GB 禁用和旧七型号不变

## 5. 安装与 Ready

- [ ] 5.1 新增 MiniMax-H3-Image 独立 `InstallProfile` 和多组件 Ready 记录，验证平台、install ID、worker build 与 runtime lock 不可串用
- [ ] 5.2 接入逐文件续传、摘要校验、取消和恢复，验证网络中断、摘要错误、空间不足和组件缺失均不激活
- [ ] 5.3 将许可证与 NOTICE 归档到该 install ID，验证许可证变化或缺失会阻止 Ready
- [ ] 5.4 验证安装与生成互斥、其他型号安装/Ready 不被覆盖，以及修复失败只影响 MiniMax-H3-Image

## 6. 原生 Worker 与协议

- [ ] 6.1 新增 `tools/minimax-h3-worker` Rust crate，实现 generation v2 hello、execute、progress、cancel、候选和错误消息，验证身份、字段和枚举单测
- [ ] 6.2 包装固定 `sd-cli` sidecar，只向控制 stdout 写协议，验证原生日志不污染 JSONL 且超限日志有界
- [ ] 6.3 固定 6 步、guidance、Euler、sigma、CPU 卸载、VAE tiling 和 seed 映射，验证错误 scheduler/CFG/token/画幅在启动推理前失败
- [ ] 6.4 复用监督器执行租约并回收 worker 与 sidecar，验证取消、包装器先退出、宿主消失和强杀均回收整树
- [ ] 6.5 校验 primary PNG 的 RGB、尺寸、字节数和 SHA-256，验证多候选、错角色、错尺寸和超限文件不能提交成功
- [ ] 6.6 增加 fixture 与真实协议测试，验证错误模型 worker、旧 Attempt、迟到事件和错 runtime 均被拒绝

## 7. 产品集成

- [ ] 7.1 在 `studio-core` 增加 H3 model/capability/recipe 常量和严格计划校验，验证不能借用 Z-Image、Qwen、MOSS、Klein 或工具能力
- [ ] 7.2 在宿主导入请求、冻结计划、执行、重试和制作记录中接入 H3，验证旧七型号与历史计划字节语义不变
- [ ] 7.3 在 macOS 隔离根完成安装与真实生成，通过后仅在已通过设备上切换为 `implemented + experimental`
- [ ] 7.4 在 Windows 目标机完成同等闭环，验证只有通过设备显示可执行，其他 Windows 设备继续显示计划中或未验证
- [ ] 7.5 在原生默认目录完成安装、发送、取消、重试、重开、任务/作品查询和 PNG 导出，验证导出哈希与正式资产一致

## 8. 回归与质量门

- [ ] 8.1 运行 H3 相关 Rust、Python/worker、前端单元和组件测试，验证失败原因可定位且没有 fixture 冒充真实模型
- [ ] 8.2 回归现有七个稳定型号以及 BiRefNet/Real-ESRGAN，验证 H3 未改变其目录、安装、执行、历史和导出
- [ ] 8.3 运行项目完整质量门，验证 fmt、clippy、Rust tests、frontend test/check/build、OpenSpec strict、workflow validator 和 actionlint 通过
- [ ] 8.4 更新 README、README.zh-CN、Release Notes、第三方声明和已知限制，验证不宣称 8 GB、12 GB 通用可用或未测试 4096²

## Workflow follow-up

- 更新 evidence 索引，分别列出 `passed`、`failed`、`not_run` 和 `blocked`，不得把 POC 或 CI 写成整机型认证。
- 完成实现与验证后，按 OpenSpec archive 流程更新正式规格。
- 归档后关闭对应 Beads 任务并报告本地提交、验证结果和未覆盖硬件。
