# Acceptance-only helper: run the nine real-generation cases (eight capabilities) through the UI,
# sampling resources before/during each generation. Case prompts are read from a UTF-8 JSON file
# so Chinese literals never depend on how Windows PowerShell 5.1 reads this script.
param(
    [string]$RepoRoot = 'C:\Users\XIAOKA-PC\Desktop\image-studio',
    [string]$CdpPort = '9222',
    [Parameter(Mandatory = $true)][string]$EvidenceDir,
    [string]$CasesFile = ''
)
$ErrorActionPreference = 'Continue'
$env:CDP_PORT = $CdpPort
if (-not $CasesFile) { $CasesFile = Join-Path $EvidenceDir 'generation-cases.json' }
$casesDocument = Get-Content -LiteralPath $CasesFile -Raw -Encoding UTF8 | ConvertFrom-Json
$cases = $casesDocument.cases
$sampler = Join-Path $RepoRoot 'scripts\windows-ui\sample-resources.ps1'
$driver = Join-Path $RepoRoot 'scripts\windows-ui\drive-generate.mjs'
$samples = Join-Path $EvidenceDir 'samples'
$results = Join-Path $EvidenceDir 'generations'
$shots = Join-Path $EvidenceDir 'screenshots\generations'
New-Item -ItemType Directory -Force -Path $samples, $results, $shots | Out-Null

$summary = @()
foreach ($case in $cases) {
    $started = Get-Date
    $stopFile = Join-Path $samples ("{0}.stop" -f $case.id)
    Remove-Item -LiteralPath $stopFile -ErrorAction SilentlyContinue
    $csv = Join-Path $samples ("{0}.csv" -f $case.id)
    $sampleJob = Start-Job -ScriptBlock {
        param($script, $csv, $label, $stop, $max)
        & $script -OutputCsv $csv -Label $label -StopFile $stop -MaxSeconds $max
    } -ArgumentList $sampler, $csv, $case.id, $stopFile, 2400
    Start-Sleep -Milliseconds 1200
    $out = Join-Path $results ("{0}.json" -f $case.id)
    $shot = Join-Path $shots ("{0}.png" -f $case.id)
    $nodeArgs = @($driver, '--model', $case.model, '--kind', $case.kind, '--timeout', $case.timeout, '--out', $out, '--shot', $shot)
    if ($case.prompt) { $nodeArgs += @('--prompt', $case.prompt) }
    if ($case.source) { $nodeArgs += @('--source', $case.source) }
    Write-Host ("[{0}] {1} {2} starting" -f (Get-Date -Format o), $case.id, $case.model)
    node @nodeArgs 2>&1 | ForEach-Object { Write-Host $_ }
    $code = $LASTEXITCODE
    Set-Content -LiteralPath $stopFile -Value 'stop'
    Wait-Job $sampleJob -Timeout 30 | Out-Null
    Receive-Job $sampleJob | Out-Null
    Remove-Job $sampleJob -Force -ErrorAction SilentlyContinue
    $status = 'missing-result'
    if (Test-Path -LiteralPath $out) {
        try {
            $parsed = Get-Content -LiteralPath $out -Raw -Encoding UTF8 | ConvertFrom-Json
            $status = $parsed.status
        } catch { $status = 'unparsed-result' }
    }
    $elapsed = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
    $summary += [pscustomobject]@{ id = $case.id; model = $case.model; kind = $case.kind; status = $status; exit = $code; elapsed_seconds = $elapsed; result = $out; samples = $csv; shot = $shot }
    Write-Host ("[{0}] {1}: status={2} elapsed={3}s exit={4}" -f (Get-Date -Format o), $case.id, $status, $elapsed, $code)
}
$summary | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $results 'generation-summary.json') -Encoding UTF8
Write-Host 'GENERATION-RUN-DONE'
