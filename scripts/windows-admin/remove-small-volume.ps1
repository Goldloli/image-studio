param(
    [string]$VolumeRoot = 'C:\ImageStudioAcceptance',
    [string]$DriveLetter = 'V',
    [switch]$DeleteVhd
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this script from an elevated PowerShell session.'
}
$vhd = Join-Path $VolumeRoot 'image-studio-acceptance.vhdx'
if (Test-Path -LiteralPath $vhd) {
    $diskpartScript = Join-Path $VolumeRoot 'detach-vhd.txt'
    @(
        "select vdisk file=`"$vhd`"",
        'detach vdisk',
        'exit'
    ) | Set-Content -LiteralPath $diskpartScript -Encoding ascii
    & diskpart /s $diskpartScript
}
if ($DeleteVhd -and (Test-Path -LiteralPath $vhd)) {
    Remove-Item -LiteralPath $vhd -Force
}
[pscustomobject]@{ detached = $true; deleted = [bool]$DeleteVhd; vhd = $vhd } | ConvertTo-Json
