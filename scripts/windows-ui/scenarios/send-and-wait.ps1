# Sends one prompt through the running app, waits for the image, exports it and checks the bytes.
param(
    [string]$Prompt = "端到端冒烟",
    [int]$TimeoutSeconds = 600
)
$ErrorActionPreference = 'Stop'
$cdp = 'scripts\windows-ui\cdp.mjs'
$started = Get-Date
node $cdp type "textarea" $Prompt | Out-Null
node $cdp click ".composer-send" | Out-Null
$state = 'timeout'
for ($i = 0; $i -lt $TimeoutSeconds; $i++) {
    Start-Sleep -Seconds 1
    $raw = (node $cdp eval "(()=>{const t=document.body.innerText;const i=t.lastIndexOf('$Prompt');if(i<0)return 'missing';const s=t.slice(i,i+280);if(/图片已生成/.test(s))return 'ok';if(/失败|已取消|已中断/.test(s))return 'bad';return 'busy'})()" 2>&1 | Select-Object -Last 1).Trim().Trim('"')
    if ($raw -eq 'ok') { $state = 'succeeded'; break }
    if ($raw -eq 'bad') { $state = 'failed'; break }
}
$seconds = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
$assetId = (node $cdp eval "(()=>{const s=document.querySelector('article.conversation-turn:last-of-type');return JSON.stringify(s? s.getAttribute('data-turn-id') : null)})()" 2>&1 | Select-Object -Last 1).Trim().Trim('"')
Write-Output "send '$Prompt' -> $state in ${seconds}s"
if ($state -ne 'succeeded') { exit 1 }
