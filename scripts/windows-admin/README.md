# 需要管理员或人工配合的 Windows P0 验收

这些脚本只用于内部验收环境，不修改产品代码，也不会自动推送或同步。

## 1. 整树禁网（I07 / P06，需管理员）

1. 以**管理员** PowerShell 运行：

   ```powershell
   & scripts/windows-admin/enable-network-denial.ps1 -DataRoot "<隔离数据根绝对路径>"
   ```

   脚本会为目标目录下的 `image-studio-desktop.exe`、`inference-supervisor.exe`、
   `diagnostic-worker.exe` 和私有运行时 `python.exe` 建立入站/出站防火墙阻断规则，
   并用同一 `python.exe` 做一次 `www.modelscope.cn:443` 连接探针，预期返回 `BLOCKED`。
2. 保持隔离数据根已缓存齐备（可用 `scripts/windows-ui/seed-cache.ps1` 预置）。
   启动安装版应用时使用：

   ```powershell
   $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9333'
   & "$env:LOCALAPPDATA\Image Studio\image-studio-desktop.exe" --data-dir "<隔离数据根绝对路径>"
   ```

3. 由代理（我已准备好 CDP 驱动）在该根执行 UI 安装（I07）与三次真实生成（P06），
   并保存网络控制、任务、PNG、摘要和资源数据。
4. 结束立即回收规则：

   ```powershell
   & scripts/windows-admin/remove-network-denial.ps1
   ```

   验收结果里要同时记录“阻断规则生效”和“应用仍能本机生成”两个事实，不能只记录其中一条。

## 2. 空间不足（I03 / neo.7，需管理员）

1. 以**管理员** PowerShell 创建 40 GB 测试卷（默认挂载 `V:`）：

   ```powershell
   & scripts/windows-admin/create-small-volume.ps1
   ```

2. 代理用安装版应用以 `--data-dir V:\acceptance-root` 启动并点击安装。
   预期：在下载前显示明确的磁盘空间不足错误，不激活、不创建 Job/资产、不删除其他数据。
3. 验收后回收：

   ```powershell
   & scripts/windows-admin/remove-small-volume.ps1 -DeleteVhd
   ```

   该卷只有约 40 GB，小于安装计划的下载+目标+运行时+预留预算，因此会在下载前失败。

## 3. 干净测试用户（D02，需人工登录）

当前开发用户安装了 Node/Rust/Python/VS 工具，不能代表“无开发工具”的干净用户。需要：

1. 管理员创建本地标准用户（例如 `ImageStudioTest`，密码由用户设置）；
2. 把 `Image Studio_0.1.0_x64-setup.exe` 复制到该用户可访问的位置（建议不再使用
   开发者的 `target` 目录，而是复制到一个普通目录）；
3. 注销并用该用户登录（这是必须的人工步骤：Windows 的 GUI 验收需要真实交互会话）；
4. 在该用户下从安装包安装并启动，确认：
   - 显示真实“未安装模型”状态，不自动下载模型、不准备 Python/诊断；
   - 设置页可打开，存储目录可定位；
   - 普通启动不需要 Node/Rust/系统 Python；
5. 由代理或人工确认窗口标题、图标、首页与模型状态；如需代理接管，保持该会话并告知，
   我会用 `--remote-debugging-port` 和同一隔离数据根驱动 UI。

## 4. 缺失/离线 WebView2（D03，需人工或独立 VM）

不建议在开发机上直接卸载系统 WebView2。推荐任一方式：

- 在 **Windows Sandbox**（需管理员启用“Windows 沙盒”功能）或独立 Win10 VM 中运行安装包，
  删除/不预装 WebView2 后断网安装，观察包内离线 WebView2 安装分支；
- 或在一次性测试用户环境移除 WebView2 后安装，并在结束后恢复。

验收通过条件：缺失 WebView2 且断网时使用包内受控依赖完成准备；权限/依赖失败给出
可行动错误且不假成功；有 WebView2 时直接使用已满足路径。结果记录到
`verification/windows-installer/` 或新建的验收目录。

## 5. 100%/200% 显示缩放（W05，需人工切换）

当前机器为 **150%** 缩放（窗口 DPI 144），150% 下两个逻辑尺寸已通过。
切换缩放需要人工在 设置 → 系统 → 显示 → 缩放 中改为 100% 或 200%，
切换后告知我，我会用同一套 CDP/Win32 脚本复跑 1280×800、960×640 的布局测量与截图；
结束后再把缩放恢复为 150%。

## 6. 真实 Microsoft 中文 IME（W06，需人工）

合成组合事件已证明“组合期间 Enter 不发送、组合结束后显式 Enter 才发送”，
但不能替代真实输入法。需要人工在安装版应用中用微软拼音输入中文并确认候选，
记录：组合确认键不触发发送、组合结束后发送正常、导出/文件定位等原生对话框可用。

## 当前状态

| 验收项 | 状态 | 依赖 |
|---|---|---|
| 整树禁网 I07 / P06 | 待运行 | 管理员 + 本 README 第 1 节 |
| 空间不足 I03 | 待运行 | 管理员 + 第 2 节 |
| 干净用户 D02 | 待运行 | 人工登录新用户 |
| 缺失/离线 WebView2 D03 | 待运行 | 沙盒/VM 或一次性移除 |
| 100%/200% DPI W05 | 待运行 | 人工切换缩放 |
| 真实中文 IME W06 | 待运行 | 人工输入 |
