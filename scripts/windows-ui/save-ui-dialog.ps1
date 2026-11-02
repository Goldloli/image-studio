param(
    [Parameter(Mandatory = $true)][string]$Path,
    [string]$TitleMatch = '',
    [int]$TimeoutSeconds = 15
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class DialogWin32 {
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr SendMessage(IntPtr hWnd, uint Msg, IntPtr wParam, string lParam);
    [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint Msg, IntPtr wParam, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    public const uint WM_SETTEXT = 0x000C;
    public const uint BM_CLICK = 0x00F5;
}
'@
$root = [System.Windows.Automation.AutomationElement]::RootElement
$deadline = (Get-Date).AddSeconds($TimeoutSeconds)
$dialogElement = $null
while ((Get-Date) -lt $deadline -and -not $dialogElement) {
    $windows = $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition)
    foreach ($window in $windows) {
        try {
            $matchesTitle = $TitleMatch -eq '' -or $window.Current.Name -like "*$TitleMatch*"
            if ($window.Current.ClassName -eq '#32770' -and $matchesTitle) {
                $dialogElement = $window
                break
            }
        } catch { }
    }
    if (-not $dialogElement) { Start-Sleep -Milliseconds 250 }
}
if (-not $dialogElement) { throw 'save dialog not found' }
$dialogHandle = [IntPtr]$dialogElement.Current.NativeWindowHandle
[void][DialogWin32]::SetForegroundWindow($dialogHandle)
$all = $dialogElement.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition)
$editElement = $null
$saveElement = $null
foreach ($element in $all) {
    try {
        $className = $element.Current.ClassName
        $automationId = $element.Current.AutomationId
        if (-not $editElement -and $className -eq 'Edit' -and $automationId -eq '1001') {
            $editElement = $element
        }
        if (-not $saveElement -and $className -eq 'Button' -and $automationId -eq '1') {
            $saveElement = $element
        }
    } catch { }
}
if (-not $editElement) { throw 'file name edit element not found' }
if (-not $saveElement) { throw 'save button element not found' }
$editHandle = [IntPtr]$editElement.Current.NativeWindowHandle
$saveHandle = [IntPtr]$saveElement.Current.NativeWindowHandle
if ($editHandle -eq [IntPtr]::Zero) { throw 'file name edit has no native handle' }
if ($saveHandle -eq [IntPtr]::Zero) { throw 'save button has no native handle' }
[void][DialogWin32]::SendMessage($editHandle, [DialogWin32]::WM_SETTEXT, [IntPtr]::Zero, $Path)
Start-Sleep -Milliseconds 500
[void][DialogWin32]::SendMessage($saveHandle, [DialogWin32]::BM_CLICK, [IntPtr]::Zero, [IntPtr]::Zero)
[pscustomobject]@{
    dialog_title = $dialogElement.Current.Name
    edit_handle = $editHandle.ToInt64()
    save_handle = $saveHandle.ToInt64()
    path = $Path
} | ConvertTo-Json
