param(
    [Parameter(Mandatory = $true)][string]$DataRoot
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this script from an elevated PowerShell session.'
}
$appDirectory = Join-Path $env:LOCALAPPDATA 'Image Studio'
$targets = @()
$targets += Get-ChildItem -LiteralPath $appDirectory -Filter '*.exe' -File | ForEach-Object { $_.FullName }
$runtime = Join-Path $DataRoot 'runtimes'
if (Test-Path -LiteralPath $runtime) {
    $targets += Get-ChildItem -LiteralPath $runtime -Recurse -Filter 'python.exe' -File | ForEach-Object { $_.FullName }
}
if ($targets.Count -eq 0) { throw 'No executable targets were found.' }
$created = 0
foreach ($target in $targets) {
    foreach ($direction in @('Outbound', 'Inbound')) {
        $name = 'ImageStudioAcceptance-' + $direction + '-' + ($target -replace '[^A-Za-z0-9]+', '_')
        $existing = Get-NetFirewallRule -DisplayName $name -ErrorAction SilentlyContinue
        if ($existing) { Remove-NetFirewallRule -DisplayName $name | Out-Null }
        New-NetFirewallRule -DisplayName $name -Direction $direction -Action Block -Program $target -Profile Any -Description 'Image Studio acceptance network denial' | Out-Null
        $created++
    }
}
$python = Join-Path $runtime 'python.exe'
$probe = 'runtime python not found; control probe skipped'
if (Test-Path -LiteralPath $python) {
    $probe = & $python -c "import socket
try:
    s = socket.create_connection(('www.modelscope.cn', 443), timeout=5)
    s.close()
    print('CONNECTED')
except Exception as error:
    print('BLOCKED: ' + type(error).__name__)
" 2>&1
}
[pscustomobject]@{
    rules_created = $created
    blocked_programs = $targets
    python_network_probe = $probe
} | ConvertTo-Json -Depth 4
