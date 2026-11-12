# Acceptance-only helper: install the seven Windows models through the real UI, sequentially.
param(
    [string]$RepoRoot = 'C:\Users\XIAOKA-PC\Desktop\image-studio',
    [string]$CdpPort = '9222',
    [Parameter(Mandatory = $true)][string]$OutDir,
    [Parameter(Mandatory = $true)][string]$ShotDir,
    [int]$TimeoutSeconds = 3600
)
$ErrorActionPreference = 'Continue'
$env:CDP_PORT = $CdpPort
$models = @(
    @{ name = 'BiRefNet'; slug = 'birefnet' },
    @{ name = 'Real-ESRGAN'; slug = 'realesrgan' },
    @{ name = 'Klein'; slug = 'klein' },
    @{ name = 'Z-Image'; slug = 'zimage' },
    @{ name = 'Qwen-Image 2.1 Turbo'; slug = 'qwen-turbo' },
    @{ name = 'Qwen-Image 2.1'; slug = 'qwen-base' },
    @{ name = 'MOSS'; slug = 'moss' }
)
New-Item -ItemType Directory -Force -Path $OutDir, $ShotDir | Out-Null
$results = @()
foreach ($model in $models) {
    $started = Get-Date
    $out = Join-Path $OutDir ("install-{0}.json" -f $model.slug)
    Write-Host ("[{0}] installing {1}" -f (Get-Date -Format o), $model.name)
    node (Join-Path $RepoRoot 'scripts\windows-ui\drive-install.mjs') --model $model.name --timeout $TimeoutSeconds --out $out --shot-dir $ShotDir 2>&1 | ForEach-Object { Write-Host $_ }
    $code = $LASTEXITCODE
    $parsed = $null
    if (Test-Path -LiteralPath $out) {
        try { $parsed = Get-Content -LiteralPath $out -Raw | ConvertFrom-Json } catch { }
    }
    $status = if ($parsed) { $parsed.status } else { 'missing-result' }
    $elapsed = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
    $results += [pscustomobject]@{ model = $model.name; slug = $model.slug; status = $status; exit = $code; elapsed_seconds = $elapsed; evidence = $out }
    Write-Host ("[{0}] {1}: status={2} elapsed={3}s exit={4}" -f (Get-Date -Format o), $model.name, $status, $elapsed, $code)
    if ($status -ne 'ready' -and $status -ne 'already_ready') {
        Write-Host ("install did not reach ready for {0}; continuing to keep evidence for the remaining models" -f $model.name)
    }
}
$results | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $OutDir 'install-summary.json') -Encoding UTF8
Write-Host 'INSTALL-MODELS-DONE'
