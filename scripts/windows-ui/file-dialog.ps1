# Fills the currently open Windows common file dialog and confirms it.
# usage: file-dialog.ps1 -Path <file> [-Title <dialog title>] [-TimeoutSeconds 10]
param(
    [Parameter(Mandatory = $true)][string]$Path,
    [string]$Title = '',
    [int]$TimeoutSeconds = 10
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class StudioFileDialog {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
    [DllImport("user32.dll")] public static extern IntPtr GetDlgItem(IntPtr h, int id);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, string l);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
}
'@
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$dialog = [IntPtr]::Zero
while ((Get-Date) -lt $deadline -and $dialog -eq [IntPtr]::Zero) {
    $dialog = [StudioFileDialog]::FindWindow('#32770', $Title)
    if ($dialog -eq [IntPtr]::Zero) { Start-Sleep -Milliseconds 200 }
}
if ($dialog -eq [IntPtr]::Zero) { throw "file dialog not found (title '$Title')" }
[StudioFileDialog]::SetForegroundWindow($dialog) | Out-Null
# 1148 is the file-name edit of the common dialog; 1152/1144 are seen on some Windows builds.
$edit = [IntPtr]::Zero
foreach ($id in @(1148, 1152, 1144)) {
    $candidate = [StudioFileDialog]::GetDlgItem($dialog, $id)
    if ($candidate -ne [IntPtr]::Zero) { $edit = $candidate; break }
}
if ($edit -eq [IntPtr]::Zero) { throw 'file name edit not found' }
[StudioFileDialog]::SendMessage($edit, 0x000C, [IntPtr]::Zero, $Path) | Out-Null
Start-Sleep -Milliseconds 300
[StudioFileDialog]::SendMessage($dialog, 0x0111, [IntPtr]1, [IntPtr]::Zero) | Out-Null
Write-Output "filled $Path"
