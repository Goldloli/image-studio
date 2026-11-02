param(
    [Parameter(Mandatory = $true)][string]$TitleMatch,
    [Parameter(Mandatory = $true)][string]$ButtonMatch,
    [int]$TimeoutSeconds = 20
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class AnswerWin32 {
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    public const uint BM_CLICK = 0x00F5;
}
'@
$root = [System.Windows.Automation.AutomationElement]::RootElement
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$dialog = $null
while ((Get-Date) -lt $deadline -and -not $dialog) {
    $windows = $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($window in $windows) {
        try {
            if ($window.Current.ClassName -eq '#32770' -and $window.Current.Name -like "*$TitleMatch*") {
                $dialog = $window
                break
            }
        } catch { }
    }
    if (-not $dialog) { Start-Sleep -Milliseconds 200 }
}
if (-not $dialog) { throw "dialog not found: $TitleMatch" }
[void][AnswerWin32]::SetForegroundWindow([IntPtr]$dialog.Current.NativeWindowHandle)
$all = $dialog.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
$target = $null
$names = @()
foreach ($element in $all) {
    try {
        if ($element.Current.ControlType.ProgrammaticName -match 'Button' -and $element.Current.Name) {
            $names += $element.Current.Name
            if (-not $target -and $element.Current.Name -match $ButtonMatch) { $target = $element }
        }
    } catch { }
}
if (-not $target) {
    throw "button not found matching '$ButtonMatch' in dialog '$($dialog.Current.Name)'; buttons: $($names -join ', ')"
}
$clicked = $false
try {
    $target.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
    $clicked = $true
} catch {
    $handle = [IntPtr]$target.Current.NativeWindowHandle
    if ($handle -ne [IntPtr]::Zero) {
        [void][AnswerWin32]::SendMessage($handle, [AnswerWin32]::BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero)
        $clicked = $true
    }
}
if (-not $clicked) { throw 'could not click the dialog button' }
[pscustomobject]@{
    dialog = $dialog.Current.Name
    button = $target.Current.Name
    clicked = $clicked
    buttons = $names
} | ConvertTo-Json
