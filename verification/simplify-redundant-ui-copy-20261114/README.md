# simplify-redundant-ui-copy UI 验收

日期：2026-11-14

范围：创作、灵感库、模型、作品、设置五个子页，以及灵感案例详情、模型详情、作品预览和制作记录。

## 已核对

- 删除装饰性英文眉标、重复页面标题、通用副标题和无信息量分组标签。
- 来源、许可、技术身份、完整状态与 manifest 日志保留在可展开区域。
- 灵感库来源只保留页面级说明；卡片不再逐张重复“上游示例”。
- 作品卡片只显示一次相同提示词，技术身份收进“技术信息”。
- 制作记录在父级作品预览已有摘要时不再重复摘要，Job/Attempt 收进高级记录。
- 存储全盘可用空间只显示一次，各目录保留自身占用。
- 删除确认、来源边界、硬件门槛、保存失败和资源异常说明仍保留。

## 截图

### 原生 macOS arm64

- `before-inspiration.png` / `after-inspiration-dark.png`
- `before-model-detail.png` / `after-model-detail.png`
- `before-production-record.png` / `after-production-record.png`
- `after-works-grid.png`
- `before-settings-runtime.png` / `after-settings-runtime.png`
- `after-settings-storage.png`
- `after-create-960-native.png`
- `after-inspiration-960-native.png`
- `after-models-960-native.png`
- `after-works-960-native.png`
- `after-works-960-dark-native.png`
- `after-settings-appearance-960-native.png`
- `after-settings-defaults-960-native.png`
- `after-settings-storage-960-native.png`
- `after-settings-runtime-960-native.png`
- `after-settings-about-960-native.png`

原生开发版使用当前前端构建产物。默认逻辑窗口为 1280×800；通过 `--diagnostics --diagnostic-size 960x640` 以 2× DPR 复验 960×640（截图为 1920×1280），并检查亮暗主题。

## 自动化与质量门

- `npm --prefix apps/frontend test`：9 个文件、83 项通过。
- `npm --prefix apps/frontend run check`：通过。
- `npm --prefix apps/frontend run build`：通过，保留既有 catalog chunk 体积警告。
- `openspec validate simplify-redundant-ui-copy --strict`：通过。
- `cargo fmt --all -- --check`、Clippy 与 workspace tests：通过；workspace tests 通过，包含 5 个既有显式 ignored 项。
- `python3 scripts/ci/validate-workflows.py` 与 `actionlint -no-color`：通过。
- `openspec validate --all --strict`：change 自身通过；全量仍有 4 份未修改正式规格的既有长度警告，已由 Beads `image-studio-6g2` 跟踪，本 change 未混入该规格整理。

本 change 只修改前端可见信息层级，未重新执行真实模型生成，也不把 UI 截图写成 GPU、安装、恢复或整模型能力认证。
