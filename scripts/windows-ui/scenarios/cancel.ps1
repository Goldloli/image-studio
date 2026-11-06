# Sends a prompt, cancels once sampling starts, and reports how long the turn took to settle.
param(
    [string]$Prompt = "取消冒烟",
    [int]$TimeoutSeconds = 120
)
$ErrorActionPreference = 'Stop'
$cdp = 'scripts\windows-ui\cdp.mjs'
node $cdp type "textarea" $Prompt | Out-Null
node $cdp click ".composer-send" | Out-Null
$sampling = $false
for ($i = 0; $i -lt 120; $i++) {
    Start-Sleep -Seconds 1
    $raw = (node $cdp eval "(()=>{const t=document.body.innerText;const i=t.lastIndexOf('$Prompt');const s=t.slice(i,i+200);return /正在采样|采样 \d+/.test(s)?'sampling':'wait'})()" 2>&1 | Select-Object -Last 1).Trim().Trim('"')
    if ($raw -eq 'sampling') { $sampling = $true; break }
}
$cancelAt = Get-Date
node $cdp click ".composer-send.is-stop" | Out-Null
$settled = $false
for ($i = 0; $i -lt $TimeoutSeconds * 2; $i++) {
    Start-Sleep -Milliseconds 500
    $raw = (node $cdp eval "(()=>{const t=document.body.innerText;const i=t.lastIndexOf('$Prompt');return /已取消/.test(t.slice(i,i+200))?'cancelled':'wait'})()" 2>&1 | Select-Object -Last 1).Trim().Trim('"')
    if ($raw -eq 'cancelled') { $settled = $true; break }
}
$seconds = [math]::Round(((Get-Date) - $cancelAt).TotalSeconds, 2)
$leftovers = (Get-Process inference-supervisor,python -ErrorAction SilentlyContinue | Measure-Object).Count
Write-Output "sampling=$sampling cancelled=$settled settle=${seconds}s leftovers=$leftovers"
if (-not $settled -or $leftovers -ne 0) { exit 1 }
