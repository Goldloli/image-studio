# Installed-app UI acceptance harness (Windows)

Small no-dependency tools used to drive the NSIS-installed Image Studio
application through its real WebView2 page and native dialogs. They are
acceptance helpers, not part of the product.

Prerequisite: launch the installed application with an isolated root and expose
the WebView2 debugging endpoint:

```powershell
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9333'
& "$env:LOCALAPPDATA\Image Studio\image-studio-desktop.exe" --data-dir <isolated-root>
```

| Tool | Purpose |
|---|---|
| `cdp.mjs` | minimal Chrome DevTools Protocol client: `text`, `eval`, `html`, `shot`, `click`, `type`, `key`, `wait` |
| `seed-cache.ps1` | hard-link the verified Gate 0 model/runtime artifacts into `<root>/cache/downloads` by SHA-256, so UI installs need no download |
| `save-ui-dialog.ps1` | set the native save dialog's file name edit and click 保存 through UI Automation + Win32 handles |
| `resize-window.ps1` | resize the native window so its client area matches a requested logical size |
| `extract-installed-records.py` | read the installed app's SQLite library and ready record into a JSON summary (run with the Gate 0 Python interpreter) |

Examples:

```powershell
node scripts/windows-ui/cdp.mjs text
node scripts/windows-ui/cdp.mjs eval "document.querySelector('textarea').value"
node scripts/windows-ui/cdp.mjs type textarea '雨后的上海街角，一位撑伞的行人'
node scripts/windows-ui/cdp.mjs key Enter
node scripts/windows-ui/cdp.mjs shot target/ui-result.png
& scripts/windows-ui/resize-window.ps1 -Width 1280 -Height 800
& scripts/windows-ui/save-ui-dialog.ps1 -Path "$PWD\target\export.png"
target/windows-gate0/runtime/python.exe scripts/windows-ui/extract-installed-records.py <data-root> <out.json>
```

The WebView2 debugging endpoint is only enabled by the environment variable at
launch time. Do not use these helpers with a normal user's data root; always
point `--data-dir` at an isolated acceptance root.
