# Acceptance-only helper: cancel one generation per engine family and record settle time.
param(
    [string]$RepoRoot = 'C:\Users\XIAOKA-PC\Desktop\image-studio',
    [string]$CdpPort = '9222',
    [Parameter(Mandatory = $true)][string]$EvidenceDir,
    [string]$CasesFile = ''
)
$ErrorActionPreference = 'Continue'
$env:CDP_PORT = $CdpPort
if (-not $CasesFile) { $CasesFile = Join-Path $EvidenceDir 'cancel-cases.json' }
$cases = (Get-Content -LiteralPath $CasesFile -Raw -Encoding UTF8 | ConvertFrom-Json).cases
$driver = Join-Path $RepoRoot 'scripts\windows-ui\drive-generate.mjs'
$results = Join-Path $EvidenceDir 'cancels'
New-Item -ItemType Directory -Force -Path $results | Out-Null
$summary = @()
foreach ($case in $cases) {
    $out = Join-Path $results ("{0}.json" -f $case.id)
    $args = @($driver, '--model', $case.model, '--kind', $case.kind, '--timeout', 600, '--cancel-after', $case.cancelAfter, '--out', $out, '--shot', (Join-Path $EvidenceDir ("screenshots\cancels\{0}.png" -f $case.id)))
    if ($case.prompt) { $args += @('--prompt', $case.prompt) }
    if ($case.source) { $args += @('--source', $case.source) }
    Write-Host ("[{0}] cancel {1} {2}" -f (Get-Date -Format o), $case.id, $case.model)
    node @args 2>&1 | ForEach-Object { Write-Host $_ }
    $status = 'missing-result'
    if (Test-Path -LiteralPath $out) {
        try { $status = (Get-Content -LiteralPath $out -Raw -Encoding UTF8 | ConvertFrom-Json).status } catch { $status = 'unparsed' }
    }
    $summary += [pscustomobject]@{ id = $case.id; model = $case.model; status = $status; evidence = $out }
    Write-Host ("[{0}] {1}: {2}" -f (Get-Date -Format o), $case.id, $status)
}
$summary | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $results 'cancel-summary.json') -Encoding UTF8
Write-Host 'CANCEL-RUN-DONE'
