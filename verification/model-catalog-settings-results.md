# 模型目录与设置页验收

验收日期：2026-10-30，Asia/Shanghai。范围对应 OpenSpec `introduce-model-catalog-and-settings`。

## 原生环境

- macOS / Apple M3 Max / arm64。
- Tauri 2 内嵌 React 生产构建。
- 数据来自 `target/first-generation-validation` 的 APFS 独立副本；没有重新下载 32.85 GB 模型。
- 副本中的真实 Z-Image 安装通过组件、运行时、MLX 设备探针和 `ReadyRecord` 检查，显示为“当前可执行”。
- 源验收目录未修改。为保持 APFS 副本自包含，只把副本内 `venv/bin/python3` 的绝对符号链接及对应身份记录改指向副本；模型文件未改写。

## 模型目录

- 目录统计为总数 4、正式接入 1、已安装 1、当前可执行 1、计划中 3。
- Z-Image Turbo、FLUX.2 Klein 4B distilled、BiRefNet general epoch_244、Real-ESRGAN x4plus 均可搜索。
- 三个计划型号只显示规划和范围，不提供安装、生成或编辑动作。
- 搜索 `esrgan` 与“可执行”组合后显示“无匹配模型”，并保留清除筛选入口；没有把计划型号提升为可执行。
- 用键盘 Tab 移至 Klein 行并按 Return 可展开详情，说明公开按钮语义可键盘操作。
- 详情分开显示产品接入、模型组件、运行时、设备探针和当前可执行，不以单一绿色/红色状态替代。
- “用于当前会话”通过宿主校验后返回创作页，当前会话保持画幅和提示词，只更新模型设置。

## 设置与持久化

- 五组设置均可在 1280×800 和 960×640 使用：通用与外观、创作默认值、存储、运行环境与诊断、关于。
- 主题从浅色切换为深色后，版本化偏好文件立即写入；重启后仍为深色。
- 新会话默认画幅保存为 4:3；重启后普通“新建会话”仍使用 4:3。
- 已有会话继续使用原来的 1:1，修改默认值没有改写旧会话。
- 默认值未保存时切换分组，出现“保存并继续 / 放弃更改 / 取消”选择；选择保存后整组提交并继续切换。
- 偏好读取/字段回退、损坏文件不覆盖、保存失败不伪报、默认值优先级、无效覆盖拒绝、计划模型不可执行均有自动测试。
- 存储页显示副本真实路径、模型、运行时、缓存、图片和日志占用，并提供宿主白名单定位动作；没有迁移、删除或任意路径输入。
- 运行环境页显示真实 M3 Max、MFLUX 0.24.0、Python 3.13.12 与就绪状态。仅打开页面不会启动诊断，日志显示“尚未主动运行”。
- 关于页显示 0.1.0、本地处理、示例来源、字体/模型来源和当前能力；未渲染自动更新、在线凭据或存储迁移假控件。

## 质量门

```text
npm run check                                  passed
npm test                                       20 passed
npm run build                                  passed
cargo fmt --all -- --check                     passed
cargo clippy --workspace --all-targets \
  --all-features --locked -- -D warnings       passed
cargo test --workspace --locked                 passed
cargo build --workspace --locked               passed
python3 scripts/check-offline-startup.py \
  --report-prefix model-settings               passed
```

工作区测试中 2 项安装/原生监督显式验收因没有设置专用验收环境变量而忽略；它们不是本次设置页失败的掩盖，真实模型就绪与 UI 使用已通过现有验收副本完成检查。

## 截图

- `model-catalog-1280-light.png`
- `model-catalog-960-dark.png`
- `model-detail-960-dark.png`
- `model-planned-detail-1280-dark.png`
- `settings-appearance-1280-light.png`
- `settings-appearance-1280-dark.png`
- `settings-appearance-960-dark.png`
- `settings-storage-1280-dark.png`

截图按 Retina 2× 输出；逻辑窗口分别为 1280×800 与 960×640。

## 未覆盖

- Klein、BiRefNet、Real-ESRGAN 未安装，未做执行或设备验证。
- 存储迁移、权重删除、自动更新、在线模型凭据与音频设置不在本阶段范围。
- 未做其他硬件、Windows GUI 或非 arm64 macOS 的模型执行认证。
