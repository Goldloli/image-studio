param(
    [string]$VolumeRoot = 'C:\ImageStudioAcceptance',
    [int]$SizeMB = 40960,
    [string]$DriveLetter = 'V'
)
$ErrorActionPreference = 'Stop'
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'Run this script from an elevated PowerShell session.'
}
New-Item -ItemType Directory -Force -Path $VolumeRoot | Out-Null
$vhd = Join-Path $VolumeRoot 'image-studio-acceptance.vhdx'
if (Test-Path -LiteralPath $vhd) { throw "VHD already exists: $vhd" }
if (Test-Path -LiteralPath "${DriveLetter}:\") { throw "Drive letter $DriveLetter already in use." }
$diskpartScript = Join-Path $VolumeRoot 'create-vhd.txt'
@(
    "create vdisk file=`"$vhd`" maximum=$SizeMB type=expandable",
    "select vdisk file=`"$vhd`"",
    'attach vdisk',
    'create partition primary',
    'format fs=ntfs quick label=ImageStudioAcceptance',
    "assign letter=$DriveLetter",
    'exit'
) | Set-Content -LiteralPath $diskpartScript -Encoding ascii
& diskpart /s $diskpartScript
if ($LASTEXITCODE -ne 0) { throw "diskpart failed with exit code $LASTEXITCODE" }
Start-Sleep -Seconds 2
$volume = Get-Volume -DriveLetter $DriveLetter -ErrorAction Stop
[pscustomobject]@{
    vhd = $vhd
    drive = "${DriveLetter}:"
    size_gb = [math]::Round($volume.Size / 1GB, 2)
    free_gb = [math]::Round($volume.SizeRemaining / 1GB, 2)
    note = 'Launch the installed app with --data-dir ' + "${DriveLetter}:\acceptance-root"
} | ConvertTo-Json
