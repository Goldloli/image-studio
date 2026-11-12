# Acceptance-only sampler for the Windows 12 GB real-generation round.
# Samples GPU used memory, system memory/commit, and the image-studio process
# tree private commit once per interval. Not part of the product runtime.
param(
    [Parameter(Mandatory = $true)][string]$OutputCsv,
    [string]$Label = '',
    [int]$IntervalSeconds = 1,
    [int]$MaxSeconds = 1800,
    [string]$StopFile = ''
)
$ErrorActionPreference = 'Stop'
$utf8 = New-Object System.Text.UTF8Encoding($false)
$dir = Split-Path -Parent $OutputCsv
if ($dir -and -not (Test-Path -LiteralPath $dir)) { New-Item -ItemType Directory -Force -Path $dir | Out-Null }
$writer = New-Object System.IO.StreamWriter($OutputCsv, $false, $utf8)
$writer.WriteLine('label,iso_time,epoch_ms,gpu_used_mib,gpu_total_mib,available_physical_mib,commit_limit_mib,commit_available_mib,proc_tree_count,private_commit_mib,working_set_mib,pids')
$started = Get-Date
$rootPattern = '^(image-studio-desktop|inference-supervisor|python[0-9.]*|realesrgan-worker|diagnostic-worker|stub-supervisor)(\.exe)?$'
try {
    while ($true) {
        $now = Get-Date
        if (($now - $started).TotalSeconds -ge $MaxSeconds) { break }
        if ($StopFile -and (Test-Path -LiteralPath $StopFile)) { break }

        $gpuUsed = ''; $gpuTotal = ''
        try {
            $gpuLine = (& nvidia-smi --query-gpu=memory.used,memory.total --format=csv,noheader,nounits 2>$null | Select-Object -First 1)
            if ($gpuLine) {
                $parts = $gpuLine -split ',\s*'
                $gpuUsed = $parts[0].Trim(); $gpuTotal = $parts[1].Trim()
            }
        } catch { }

        $os = Get-CimInstance Win32_OperatingSystem -Property FreePhysicalMemory, TotalVirtualMemorySize, FreeVirtualMemory
        $availPhys = [math]::Round($os.FreePhysicalMemory / 1024, 1)
        $commitLimit = [math]::Round($os.TotalVirtualMemorySize / 1024, 1)
        $commitAvail = [math]::Round($os.FreeVirtualMemory / 1024, 1)

        $procs = @()
        try { $procs = @(Get-CimInstance Win32_Process -Property ProcessId, ParentProcessId, Name) } catch { }
        $byId = @{}
        foreach ($p in $procs) { $byId[[int]$p.ProcessId] = $p }
        $rootIds = @($procs | Where-Object { $_.Name -match $rootPattern } | ForEach-Object { [int]$_.ProcessId })
        $selected = New-Object System.Collections.Generic.HashSet[int]
        $queue = New-Object System.Collections.Generic.Queue[int]
        foreach ($r in $rootIds) { [void]$selected.Add($r); $queue.Enqueue($r) }
        while ($queue.Count -gt 0) {
            $parentId = $queue.Dequeue()
            foreach ($p in $procs) {
                if ([int]$p.ParentProcessId -eq $parentId -and -not $selected.Contains([int]$p.ProcessId)) {
                    [void]$selected.Add([int]$p.ProcessId); $queue.Enqueue([int]$p.ProcessId)
                }
            }
        }
        $privateBytes = 0; $workingSet = 0; $pidList = @()
        foreach ($procId in $selected) {
            $gp = Get-Process -Id $procId -ErrorAction SilentlyContinue
            if ($gp) {
                $privateBytes += $gp.PrivateMemorySize64
                $workingSet += $gp.WorkingSet64
                $pidList += "${procId}:$($gp.ProcessName)"
            }
        }
        $row = @(
            $Label,
            $now.ToUniversalTime().ToString('o'),
            ([DateTimeOffset]$now).ToUnixTimeMilliseconds(),
            $gpuUsed,
            $gpuTotal,
            $availPhys,
            $commitLimit,
            $commitAvail,
            $selected.Count,
            [math]::Round($privateBytes / 1MB, 1),
            [math]::Round($workingSet / 1MB, 1),
            ($pidList -join '|')
        ) -join ','
        $writer.WriteLine($row)
        $writer.Flush()

        $elapsed = ((Get-Date) - $now).TotalSeconds
        $sleepMs = [math]::Max(0, ($IntervalSeconds - $elapsed) * 1000)
        Start-Sleep -Milliseconds $sleepMs
    }
} finally {
    $writer.Flush()
    $writer.Close()
}
Write-Output "samples written: $OutputCsv"
