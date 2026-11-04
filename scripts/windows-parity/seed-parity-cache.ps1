# Seed the Windows parity installer download cache from locally verified files.
#
# This script NEVER downloads anything. It reads the checked-in installer
# manifests, resolves each declared download artifact to a local file whose
# byte count and SHA-256 match the manifest, and hard-links (or copies) it into
#   <DataRoot>/cache/downloads/<sha256>
# so an install can run from the seeded cache without network access.
#
# The installer (apps/desktop/src/adapters/installation.rs) stores downloads in
# its cache keyed by the artifact SHA-256, so this layout is exactly what
# start() consumes. Missing or mismatched sources are reported as
# `missing` / `hash_mismatch`; they are never fabricated and never downloaded.
#
# Usage:
#   pwsh -File scripts/windows-parity/seed-parity-cache.ps1 -DataRoot <data-root>
#   pwsh -File scripts/windows-parity/seed-parity-cache.ps1 -DataRoot <data-root> -DryRun
#   pwsh -File scripts/windows-parity/seed-parity-cache.ps1 -DataRoot <data-root> -OutputPath out.json

param(
    [Parameter(Mandatory = $true)][string]$DataRoot,
    [string]$OutputPath = '',
    [switch]$DryRun,
    [switch]$Copy
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent (Split-Path -Parent $PSScriptRoot)
if (-not (Test-Path -LiteralPath (Join-Path $repo 'resources/windows-parity/models'))) {
    throw "Run this script from the repository checkout; resources/windows-parity/models was not found under $repo."
}
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $repo 'verification/windows-parity-app/seed-cache-results.json'
} elseif (-not [System.IO.Path]::IsPathRooted($OutputPath)) {
    $OutputPath = Join-Path $repo $OutputPath
}
$dataRootPath = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($DataRoot)
$cacheDownloads = Join-Path $dataRootPath 'cache/downloads'
$modelsRoot = Join-Path $repo 'target/parity-gate0/models'
$runtimeRoot = Join-Path $repo 'target/parity-runtime'
$downloadsRoot = Join-Path $runtimeRoot 'downloads'
$wheelhouseRoot = Join-Path $runtimeRoot 'wheelhouse'
$bundledModelsRoot = Join-Path $repo 'resources/windows-parity/models'

$modelDirMap = @{
    'flux2-klein-4b-distilled-windows-v1'    = 'target/parity-gate0/models/klein'
    'birefnet-general-epoch-244-windows-v1'  = 'target/parity-gate0/models/birefnet'
    'qwen-image-2.1-windows-v1'              = 'target/parity-gate0/models/qwen-image-2.1'
    'qwen-image-2.1-turbo-windows-v1'        = 'target/parity-gate0/models/qwen-image-2.1-turbo'
    'moss-image-sft-windows-v1'              = 'target/parity-gate0/models/moss-image-sft'
}

$items = [System.Collections.Generic.List[object]]::new()

function Add-Artifact {
    param(
        [string]$Manifest,
        [string]$Id,
        [string]$Kind,
        [string]$Path,
        [string]$Url,
        [int64]$Bytes,
        [string]$Sha256
    )
    $items.Add([pscustomobject]@{
        manifest     = $Manifest
        id           = $Id
        kind         = $Kind
        path         = $Path
        url          = $Url
        bytes        = $Bytes
        sha256       = $Sha256.ToLowerInvariant()
        status       = 'pending'
        source       = $null
        actual_bytes = $null
        actual_sha256 = $null
        cache_path   = $null
        link_mode    = $null
        note         = $null
    })
}

function Relative-Path {
    param([string]$FullPath)
    # Windows PowerShell 5.1 compatible version of Path.GetRelativePath (the static helper is .NET Core only).
    $prefix = $repo.TrimEnd('\')
    if ($FullPath.StartsWith($prefix, [System.StringComparison]::OrdinalIgnoreCase)) {
        return ($FullPath.Substring($prefix.Length).TrimStart('\') -replace '\\', '/')
    }
    return ($FullPath -replace '\\', '/')
}

# ---- collect declared download artifacts from the installer manifests ----
$modelManifestPaths = Get-ChildItem -LiteralPath (Join-Path $repo 'resources/windows-parity/models') -Directory |
    ForEach-Object { Join-Path $_.FullName 'model-manifest.json' } |
    Where-Object { Test-Path -LiteralPath $_ } |
    Sort-Object
foreach ($manifestPath in $modelManifestPaths) {
    $manifestRelative = Relative-Path $manifestPath
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.files) {
        foreach ($file in $manifest.files) {
            Add-Artifact -Manifest $manifestRelative -Id $manifest.id -Kind 'model-file' -Path $file.path -Url $file.url -Bytes ([int64]$file.bytes) -Sha256 $file.sha256
        }
    }
    if ($manifest.archives) {
        foreach ($archive in $manifest.archives) {
            Add-Artifact -Manifest $manifestRelative -Id $manifest.id -Kind 'model-archive' -Path $archive.path -Url $archive.url -Bytes ([int64]$archive.bytes) -Sha256 $archive.sha256
        }
    }
}

$productManifestPath = Join-Path $repo 'resources/windows-parity/runtime-v2/product-runtime-manifest.json'
$productManifest = Get-Content -LiteralPath $productManifestPath -Raw | ConvertFrom-Json
$productRelative = Relative-Path $productManifestPath
Add-Artifact -Manifest $productRelative -Id $productManifest.id -Kind 'runtime-interpreter' -Path $productManifest.interpreter.path -Url $productManifest.interpreter.url -Bytes ([int64]$productManifest.interpreter.bytes) -Sha256 $productManifest.interpreter.sha256
foreach ($wheel in $productManifest.wheels) {
    Add-Artifact -Manifest $productRelative -Id $productManifest.id -Kind 'runtime-wheel' -Path $wheel.path -Url $wheel.url -Bytes ([int64]$wheel.bytes) -Sha256 $wheel.sha256
}

$nativeManifestPath = Join-Path $repo 'resources/windows-parity/realesrgan-ncnn-vulkan/runtime-manifest.json'
$nativeManifest = Get-Content -LiteralPath $nativeManifestPath -Raw | ConvertFrom-Json
$nativeRelative = Relative-Path $nativeManifestPath
foreach ($archive in $nativeManifest.archives) {
    Add-Artifact -Manifest $nativeRelative -Id $nativeManifest.id -Kind 'native-archive' -Path $archive.path -Url $archive.url -Bytes ([int64]$archive.bytes) -Sha256 $archive.sha256
}

# ---- resolve local sources and seed the cache ----
function Find-LocalSource {
    param([object]$Item)
    $leaf = Split-Path -Leaf ($Item.path -replace '/', '\')
    $candidates = [System.Collections.Generic.List[string]]::new()
    switch ($Item.kind) {
        'model-file' {
            if ($modelDirMap.ContainsKey($Item.id)) {
                $candidates.Add((Join-Path $repo ($modelDirMap[$Item.id] + '/' + $Item.path -replace '/', '\')))
            }
            $candidates.Add((Join-Path (Join-Path $bundledModelsRoot $Item.id) $leaf))
        }
        'model-archive' { $candidates.Add((Join-Path $downloadsRoot $Item.path)) ; $candidates.Add((Join-Path $downloadsRoot $leaf)) }
        'native-archive' { $candidates.Add((Join-Path $downloadsRoot $Item.path)) ; $candidates.Add((Join-Path $downloadsRoot $leaf)) }
        'runtime-interpreter' { $candidates.Add((Join-Path $downloadsRoot $Item.path)) ; $candidates.Add((Join-Path $downloadsRoot $leaf)) }
        'runtime-wheel' { $candidates.Add((Join-Path $wheelhouseRoot $Item.path)) ; $candidates.Add((Join-Path $wheelhouseRoot $leaf)) }
        default { }
    }
    foreach ($candidate in $candidates) {
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return $candidate
        }
    }
    # Fallback: search the known build roots by file name, verifying bytes+sha256 later.
    $searchRoots = switch ($Item.kind) {
        'model-file' { @($modelsRoot, $bundledModelsRoot) }
        'runtime-wheel' { @($wheelhouseRoot) }
        'runtime-interpreter' { @($downloadsRoot) }
        default { @($downloadsRoot) }
    }
    foreach ($root in $searchRoots) {
        if (-not (Test-Path -LiteralPath $root)) { continue }
        $match = Get-ChildItem -LiteralPath $root -Recurse -File -Filter $leaf -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($match) { return $match.FullName }
    }
    return $null
}

if (-not $DryRun) {
    New-Item -ItemType Directory -Force -Path $cacheDownloads | Out-Null
}
$seen = @{}
$started = (Get-Date).ToUniversalTime().ToString('o')
foreach ($item in $items) {
    $source = Find-LocalSource -Item $item
    if (-not $source) {
        $item.status = 'missing'
        $item.note = 'no local file with this name was found in target/parity-gate0/models or target/parity-runtime'
        continue
    }
    $item.source = Relative-Path $source
    $file = Get-Item -LiteralPath $source
    $item.actual_bytes = $file.Length
    $item.actual_sha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash.ToLowerInvariant()
    if ($file.Length -ne $item.bytes -or $item.actual_sha256 -ne $item.sha256) {
        $item.status = 'hash_mismatch'
        $item.note = 'local file bytes/sha256 do not match the manifest; not cached'
        continue
    }
    $cacheFile = Join-Path $cacheDownloads $item.sha256
    $item.cache_path = $cacheFile
    if ($seen.ContainsKey($item.sha256)) {
        $item.status = 'deduplicated'
        $item.link_mode = $seen[$item.sha256]
        continue
    }
    if (Test-Path -LiteralPath $cacheFile -PathType Leaf) {
        $existingSha = (Get-FileHash -Algorithm SHA256 -LiteralPath $cacheFile).Hash.ToLowerInvariant()
        if ($existingSha -eq $item.sha256) {
            $item.status = 'already_present'
            $item.link_mode = 'existing'
            $seen[$item.sha256] = 'existing'
            continue
        }
        Remove-Item -LiteralPath $cacheFile -Force
    }
    if ($DryRun) {
        $item.status = 'found_dry_run'
        $item.link_mode = 'none'
        $seen[$item.sha256] = 'dry_run'
        continue
    }
    $linkMode = 'hardlink'
    try {
        New-Item -ItemType HardLink -Path $cacheFile -Target $source -ErrorAction Stop | Out-Null
    } catch {
        $linkMode = 'copy'
        Copy-Item -LiteralPath $source -Destination $cacheFile -Force
    }
    if ($Copy -and $linkMode -eq 'copy') {
        $item.note = 'hard link failed (cross-volume?); copied instead'
    }
    $item.status = 'linked'
    $item.link_mode = $linkMode
    $seen[$item.sha256] = $linkMode
}
$finished = (Get-Date).ToUniversalTime().ToString('o')

$linked = @($items | Where-Object { $_.status -eq 'linked' }).Count
$alreadyPresent = @($items | Where-Object { $_.status -eq 'already_present' }).Count
$deduplicated = @($items | Where-Object { $_.status -eq 'deduplicated' }).Count
$dryRunFound = @($items | Where-Object { $_.status -eq 'found_dry_run' }).Count
$missing = @($items | Where-Object { $_.status -eq 'missing' })
$mismatched = @($items | Where-Object { $_.status -eq 'hash_mismatch' })
$uniqueBytes = 0
foreach ($item in $items) {
    if ($item.status -in @('linked', 'already_present', 'found_dry_run')) {
        $uniqueBytes += $item.bytes
    }
}
$uniqueShas = @($items | Where-Object { $_.status -in @('linked', 'already_present', 'deduplicated', 'found_dry_run') } | ForEach-Object { $_.sha256 } | Sort-Object -Unique)
$copiedBytes = 0
if (-not $DryRun) {
    foreach ($sha in $uniqueShas) {
        $cacheFile = Join-Path $cacheDownloads $sha
        if (Test-Path -LiteralPath $cacheFile) {
            $sourceEntry = @($items | Where-Object { $_.sha256 -eq $sha -and $_.status -in @('linked', 'already_present') } | Select-Object -First 1)
            if ($sourceEntry -and $sourceEntry[0].link_mode -eq 'copy') {
                $copiedBytes += (Get-Item -LiteralPath $cacheFile).Length
            }
        }
    }
}

$result = [ordered]@{
    schema_version       = 1
    kind                 = 'windows-parity-cache-seed'
    generated_at_utc     = $finished
    started_at_utc       = $started
    repo                 = $repo
    data_root            = $dataRootPath
    cache_downloads      = $cacheDownloads
    mode                 = if ($DryRun) { 'dry_run' } else { 'seed' }
    network_used         = $false
    downloads_performed  = 0
    manifest_count       = ($modelManifestPaths.Count + 2)
    totals               = [ordered]@{
        artifacts          = $items.Count
        unique_shas        = $uniqueShas.Count
        linked             = $linked
        already_present    = $alreadyPresent
        deduplicated       = $deduplicated
        found_dry_run      = $dryRunFound
        missing            = $missing.Count
        hash_mismatch      = $mismatched.Count
        unique_bytes       = $uniqueBytes
        copied_bytes       = $copiedBytes
    }
    notes                = @(
        'This script performs no network requests; every cache entry is a verified local file.',
        'Cache entries are keyed by SHA-256, matching the installer download cache layout.',
        'Bundled license resources and installer-embedded worker wheels are not download artifacts and are not seeded here.',
        'The Real-ESRGAN model archive and native runtime archive share one zip SHA-256; it is linked once and deduplicated.'
    )
    missing_items        = @($missing | Select-Object manifest, id, kind, path, sha256, bytes, note)
    mismatched_items     = @($mismatched | Select-Object manifest, id, kind, path, sha256, bytes, actual_bytes, actual_sha256, note)
    items                = @($items | Select-Object manifest, id, kind, path, sha256, bytes, status, source, actual_bytes, actual_sha256, cache_path, link_mode, note)
}
$outputDirectory = Split-Path -Parent $OutputPath
if ($outputDirectory) { New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null }
[System.IO.File]::WriteAllText($OutputPath, ($result | ConvertTo-Json -Depth 6), [System.Text.UTF8Encoding]::new($false))

[pscustomobject]@{
    status        = if (($missing.Count + $mismatched.Count) -gt 0) { 'incomplete' } else { 'complete' }
    mode          = $result.mode
    data_root     = $dataRootPath
    artifacts     = $items.Count
    unique_shas   = $uniqueShas.Count
    linked        = $linked
    already       = $alreadyPresent
    deduplicated  = $deduplicated
    missing       = $missing.Count
    hash_mismatch = $mismatched.Count
    unique_bytes  = $uniqueBytes
    output        = $OutputPath
} | Format-List
if (($missing.Count + $mismatched.Count) -gt 0) { exit 1 }
exit 0
