param(
    [Parameter(Mandatory = $true)][string]$DataRoot,
    [Parameter(Mandatory = $true)][string]$OutputJson,
    [string]$ExtraEnvName = '',
    [string]$ExtraEnvValue = '',
    [switch]$PreCreateProfileFile,
    [int]$TimeoutSeconds = 25
)
$ErrorActionPreference = 'Continue'
Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
public static class W32Enum2 {
    public delegate bool EnumWindowsProc(IntPtr hWnd, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumWindowsProc proc, IntPtr lParam);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr hWnd, StringBuilder text, int count);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
    public static List<string> WindowsFor(uint targetPid) {
        var found = new List<string>();
        EnumWindows((h, l) => {
            uint pid;
            GetWindowThreadProcessId(h, out pid);
            if (pid == targetPid && IsWindowVisible(h)) {
                var sb = new StringBuilder(512);
                GetWindowText(h, sb, 512);
                if (sb.Length > 0) { found.Add(sb.ToString()); }
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }
    public static void CloseFor(uint targetPid) {
        EnumWindows((h, l) => {
            uint pid;
            GetWindowThreadProcessId(h, out pid);
            if (pid == targetPid && IsWindowVisible(h)) {
                PostMessage(h, 0x0111, new IntPtr(1), IntPtr.Zero);
                PostMessage(h, 0x0010, IntPtr.Zero, IntPtr.Zero);
            }
            return true;
        }, IntPtr.Zero);
    }
}
"@
if ($PreCreateProfileFile) {
    New-Item -ItemType Directory -Force -Path (Join-Path $DataRoot 'cache') | Out-Null
    $profilePath = Join-Path $DataRoot 'cache\webview2'
    if (-not (Test-Path -LiteralPath $profilePath)) { [System.IO.File]::WriteAllText($profilePath, 'not a directory') }
}
$exe = Join-Path $env:LOCALAPPDATA 'Image Studio\image-studio-desktop.exe'
if ($ExtraEnvName) { Set-Item -LiteralPath "env:$ExtraEnvName" -Value $ExtraEnvValue }
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = ''
$started = Get-Date
$process = Start-Process -FilePath $exe -ArgumentList '--data-dir', $DataRoot -PassThru
$dialogTitles = @()
$dialogFound = $false
$deadline = $started.AddSeconds($TimeoutSeconds)
while ((Get-Date) -lt $deadline) {
    Start-Sleep -Milliseconds 500
    if ($process.HasExited) { break }
    $titles = [W32Enum2]::WindowsFor([uint32]$process.Id)
    if ($titles.Count -gt 0) {
        $dialogFound = $true
        $dialogTitles = $titles
        [W32Enum2]::CloseFor([uint32]$process.Id)
        break
    }
}
$exitCode = 'running'
if ($process.WaitForExit(15000)) { $exitCode = $process.ExitCode } else { Stop-Process -Id $process.Id -Force; $exitCode = 'killed' }
$logPath = Join-Path $DataRoot 'logs\startup-failure.log'
$logExists = Test-Path -LiteralPath $logPath
$logText = ''
if ($logExists) { $logText = [System.IO.File]::ReadAllText($logPath, [System.Text.Encoding]::UTF8) }
$result = [ordered]@{
    data_root = $DataRoot
    extra_env = if ($ExtraEnvName) { "$ExtraEnvName=$ExtraEnvValue" } else { '' }
    pre_created_profile_file = [bool]$PreCreateProfileFile
    dialog_found = $dialogFound
    dialog_titles = $dialogTitles
    exit_code = $exitCode
    startup_failure_log = $logPath
    startup_failure_log_exists = $logExists
    startup_failure_log_text = $logText
    elapsed_seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
}
[System.IO.File]::WriteAllText($OutputJson, (($result | ConvertTo-Json -Depth 4) + "`n"), (New-Object System.Text.UTF8Encoding($false)))
Write-Output ("startup-failure check: root={0} dialogs={1} exit={2} log={3}" -f $DataRoot, ($dialogTitles -join '|'), $exitCode, $logExists)
