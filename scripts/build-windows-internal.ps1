[CmdletBinding()]
param(
    [switch]$SkipQualityGates,
    [switch]$SkipBundle
)

$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$frontend = Join-Path $repo "apps/frontend"
$desktop = Join-Path $repo "apps/desktop"
$target = Join-Path $repo "target/release"
$sidecars = Join-Path $desktop "binaries"
$tauriTools = Join-Path $repo "target/.tauri"
$nsisRoot = Join-Path $tauriTools "NSIS"
$nsisUtils = Join-Path $nsisRoot "Plugins/x86-unicode/additional/nsis_tauri_utils.dll"

if ($env:OS -ne "Windows_NT") {
    throw "Windows internal packaging must run on Windows."
}

$hostTriple = (& rustc -vV | Select-String '^host: ').ToString().Substring(6).Trim()
if ($hostTriple -ne "x86_64-pc-windows-msvc") {
    throw "Expected x86_64-pc-windows-msvc, found $hostTriple."
}

function Invoke-Checked {
    param(
        [Parameter(Mandatory = $true)][string]$Program,
        [string[]]$Arguments = @()
    )
    $previousPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        & $Program @Arguments
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $previousPreference
    }
    if ($exitCode -ne 0) {
        throw "$Program failed with exit code $exitCode."
    }
}

function Get-VerifiedDownload {
    param(
        [Parameter(Mandatory = $true)][string]$Target,
        [Parameter(Mandatory = $true)][string]$Sha256,
        [Parameter(Mandatory = $true)][string[]]$Urls
    )
    if (
        (Test-Path -LiteralPath $Target -PathType Leaf) -and
        (Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash -eq $Sha256
    ) {
        return
    }
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Target) | Out-Null
    foreach ($url in $Urls) {
        & curl.exe -L --fail --retry 4 --connect-timeout 20 --max-time 180 -o $Target $url
        if (
            $LASTEXITCODE -eq 0 -and
            (Test-Path -LiteralPath $Target -PathType Leaf) -and
            (Get-FileHash -LiteralPath $Target -Algorithm SHA256).Hash -eq $Sha256
        ) {
            return
        }
    }
    throw "Unable to download and verify $Target."
}

Push-Location $frontend
try {
    if (-not (Test-Path (Join-Path $frontend "node_modules/.bin/tauri.cmd"))) {
        Invoke-Checked "npm.cmd" @("ci")
    }
    if (-not $SkipQualityGates) {
        Invoke-Checked "npm.cmd" @("test")
    }
    Invoke-Checked "npm.cmd" @("run", "build")
}
finally {
    Pop-Location
}

if (-not $SkipQualityGates) {
    Invoke-Checked "cargo.exe" @("fmt", "--all", "--", "--check")
    Invoke-Checked "cargo.exe" @("clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings")
    Invoke-Checked "cargo.exe" @("test", "--workspace", "--locked")
}

Invoke-Checked "cargo.exe" @("build", "--release", "--locked", "-p", "inference-supervisor", "-p", "diagnostic-worker", "-p", "realesrgan-worker")

New-Item -ItemType Directory -Force -Path $sidecars | Out-Null
$sidecarCopies = @{
    "inference-supervisor.exe" = "inference-supervisor-$hostTriple.exe"
    "diagnostic-worker.exe" = "diagnostic-worker-$hostTriple.exe"
    "realesrgan-worker.exe" = "realesrgan-worker-$hostTriple.exe"
}
foreach ($sourceName in $sidecarCopies.Keys) {
    $source = Join-Path $target $sourceName
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "Missing release sidecar: $source"
    }
    Copy-Item -LiteralPath $source -Destination (Join-Path $sidecars $sidecarCopies[$sourceName]) -Force
}

if (-not (Test-Path -LiteralPath (Join-Path $nsisRoot "makensis.exe") -PathType Leaf)) {
    $nsisZip = Join-Path ([System.IO.Path]::GetTempPath()) "image-studio-nsis-3.13-$PID.zip"
    $extractRoot = Join-Path ([System.IO.Path]::GetTempPath()) "image-studio-nsis-3.13-$PID-$([guid]::NewGuid())"
    Get-VerifiedDownload `
        -Target $nsisZip `
        -Sha256 "BA63DFFC4410EE89193E1CB5A41989991BD77C61068DA17E3156D136B7B0B3D8" `
        -Urls @(
            "https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip",
            "https://ghfast.top/https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip",
            "https://ghproxy.net/https://github.com/tauri-apps/binary-releases/releases/download/nsis-3.13/nsis-3.13.zip"
        )
    Expand-Archive -LiteralPath $nsisZip -DestinationPath $extractRoot
    New-Item -ItemType Directory -Force -Path $tauriTools | Out-Null
    Move-Item -LiteralPath (Join-Path $extractRoot "nsis-3.13") -Destination $nsisRoot
    [System.IO.File]::Delete($nsisZip)
    [System.IO.Directory]::Delete($extractRoot, $true)
}

Get-VerifiedDownload `
    -Target $nsisUtils `
    -Sha256 "5BA143B5DB4A87D32D6E7802E033330AAE56CBCEABE0D1E3BA41948385AD4709" `
    -Urls @(
        "https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll",
        "https://ghfast.top/https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll",
        "https://ghproxy.net/https://github.com/tauri-apps/nsis-tauri-utils/releases/download/nsis_tauri_utils-v0.5.3/nsis_tauri_utils.dll"
    )

if ($SkipBundle) {
    Write-Output "Windows release binaries and sidecars are ready."
    return
}

Push-Location $desktop
try {
    Invoke-Checked (Join-Path $frontend "node_modules/.bin/tauri.cmd") @("build", "--bundles", "nsis")
}
finally {
    Pop-Location
}

$bundleRoot = Join-Path $target "bundle/nsis"
$installer = Get-ChildItem -LiteralPath $bundleRoot -Filter "*.exe" |
    Sort-Object LastWriteTimeUtc -Descending |
    Select-Object -First 1
if ($null -eq $installer) {
    throw "Tauri did not produce an NSIS installer under $bundleRoot."
}

$evidence = [ordered]@{
    schema_version = 1
    kind = "windows-internal-nsis-package"
    status = "built"
    installer = $installer.FullName.Substring($repo.Length + 1).Replace("\", "/")
    installer_sha256 = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    installer_bytes = $installer.Length
    target = $hostTriple
    install_mode = "currentUser"
    webview2_install_mode = "offlineInstaller"
    signed = $false
    model_weights_bundled = $false
    private_python_bundled = $false
    sidecars = @(
        $sidecarCopies.Keys | Sort-Object | ForEach-Object {
            $name = $sidecarCopies[$_]
            $path = Join-Path $sidecars $name
            [ordered]@{
                name = $name
                sha256 = (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
                bytes = (Get-Item -LiteralPath $path).Length
            }
        }
    )
}

$evidenceDirectory = Join-Path $repo "verification/windows-installer"
New-Item -ItemType Directory -Force -Path $evidenceDirectory | Out-Null
$evidencePath = Join-Path $evidenceDirectory "internal-package.json"
$pendingPath = "$evidencePath.pending"
[System.IO.File]::WriteAllText(
    $pendingPath,
    ($evidence | ConvertTo-Json -Depth 5) + "`n",
    [System.Text.UTF8Encoding]::new($false)
)
Move-Item -LiteralPath $pendingPath -Destination $evidencePath -Force

Write-Output "NSIS installer: $($installer.FullName)"
Write-Output "Evidence: $evidencePath"
