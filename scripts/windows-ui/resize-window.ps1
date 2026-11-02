param(
    [Parameter(Mandatory = $true)][int]$Width,
    [Parameter(Mandatory = $true)][int]$Height
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class WinSize {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left; public int Top; public int Right; public int Bottom; }
    [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int X, int Y, int cx, int cy, uint flags);
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int cmd);
}
'@
$process = Get-Process image-studio-desktop | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $process) { throw 'app main window not found' }
$hwnd = [IntPtr]$process.MainWindowHandle
[void][WinSize]::ShowWindow($hwnd, 9)
Start-Sleep -Milliseconds 500
for ($attempt = 0; $attempt -lt 3; $attempt++) {
    $client = New-Object WinSize+RECT
    $window = New-Object WinSize+RECT
    [void][WinSize]::GetClientRect($hwnd, [ref]$client)
    [void][WinSize]::GetWindowRect($hwnd, [ref]$window)
    $clientWidth = $client.Right - $client.Left
    $clientHeight = $client.Bottom - $client.Top
    if ($clientWidth -eq $Width -and $clientHeight -eq $Height) { break }
    $outerWidth = ($window.Right - $window.Left) + ($Width - $clientWidth)
    $outerHeight = ($window.Bottom - $window.Top) + ($Height - $clientHeight)
    [void][WinSize]::SetWindowPos($hwnd, [IntPtr]::Zero, $window.Left, $window.Top, $outerWidth, $outerHeight, 0x0004)
    Start-Sleep -Milliseconds 700
}
$client = New-Object WinSize+RECT
[void][WinSize]::GetClientRect($hwnd, [ref]$client)
[pscustomobject]@{
    requested_width = $Width
    requested_height = $Height
    client_width = $client.Right - $client.Left
    client_height = $client.Bottom - $client.Top
} | ConvertTo-Json
