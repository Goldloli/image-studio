# Seed an isolated Windows data root cache from the verified Gate 0 artifacts.
# Usage: pwsh -File target/seed-windows-cache.ps1 -DataRoot <path>
param(
    [Parameter(Mandatory = $true)][string]$DataRoot
)
$ErrorActionPreference = "Stop"
$repo = Split-Path -Parent $PSScriptRoot
$runtimeManifest = Get-Content (Join-Path $repo "resources/windows-foundation/python-runtime/product-runtime-manifest.json") -Raw | ConvertFrom-Json
$modelManifest = Get-Content (Join-Path $repo "resources/windows-foundation/z-image-diffusers/product-model-manifest.json") -Raw | ConvertFrom-Json
$workerWheel = Join-Path $repo "resources/windows-foundation/z-image-diffusers/studio_zimage_cuda_worker-0.1.0-py3-none-any.whl"
$runtimeSource = Join-Path $repo "target/windows-gate0/downloads"
$modelSource = Join-Path $repo "target/windows-gate0/model"
$cache = Join-Path $DataRoot "cache/downloads"
New-Item -ItemType Directory -Force -Path $cache | Out-Null

$artifacts = @()
$artifacts += [pscustomobject]@{ Path = $runtimeManifest.interpreter.path; Bytes = $runtimeManifest.interpreter.bytes; Sha256 = $runtimeManifest.interpreter.sha256; Source = Join-Path $runtimeSource $runtimeManifest.interpreter.path }
foreach ($wheel in $runtimeManifest.wheels) {
    if ($wheel.url -eq "bundled:worker-wheel") {
        $artifacts += [pscustomobject]@{ Path = $wheel.path; Bytes = $wheel.bytes; Sha256 = $wheel.sha256; Source = $workerWheel }
    } else {
        $artifacts += [pscustomobject]@{ Path = $wheel.path; Bytes = $wheel.bytes; Sha256 = $wheel.sha256; Source = Join-Path $runtimeSource $wheel.path }
    }
}
foreach ($file in $modelManifest.files) {
    $artifacts += [pscustomobject]@{ Path = $file.path; Bytes = $file.bytes; Sha256 = $file.sha256; Source = Join-Path $modelSource $file.path }
}

$linked = 0
$copied = 0
$cacheHits = 0
foreach ($artifact in $artifacts) {
    $target = Join-Path $cache $artifact.Sha256
    if (Test-Path -LiteralPath $target) {
        $cacheHits++
        continue
    }
    if (-not (Test-Path -LiteralPath $artifact.Source)) {
        throw "missing source for $($artifact.Path): $($artifact.Source)"
    }
    $length = (Get-Item -LiteralPath $artifact.Source).Length
    if ($length -ne $artifact.Bytes) {
        throw "size mismatch for $($artifact.Path): $length != $($artifact.Bytes)"
    }
    $hash = (Get-FileHash -LiteralPath $artifact.Source -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($hash -ne $artifact.Sha256) {
        throw "sha256 mismatch for $($artifact.Path): $hash != $($artifact.Sha256)"
    }
    try {
        New-Item -ItemType HardLink -Path $target -Target $artifact.Source | Out-Null
        $linked++
    } catch {
        Copy-Item -LiteralPath $artifact.Source -Destination $target
        $copied++
    }
}
[pscustomobject]@{
    data_root = $DataRoot
    artifacts = $artifacts.Count
    hard_links = $linked
    copies = $copied
    cache_hits = $cacheHits
    cache_entries = (Get-ChildItem $cache -File).Count
} | ConvertTo-Json
