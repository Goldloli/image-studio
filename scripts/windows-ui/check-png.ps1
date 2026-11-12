# Acceptance-only helper: decode a PNG and record dimensions, pixel format and basic pixel statistics.
param(
    [Parameter(Mandatory = $true)][string[]]$Path,
    [Parameter(Mandatory = $true)][string]$OutputJson
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing
$results = @()
foreach ($file in $Path) {
    if (-not (Test-Path -LiteralPath $file)) {
        $results += [pscustomobject]@{ path = $file; exists = $false; error = 'missing' }
        continue
    }
    $item = [ordered]@{ path = $file; exists = $true; bytes = (Get-Item -LiteralPath $file).Length }
    try {
        $sha = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
        $item.sha256 = $sha
        $bitmap = New-Object System.Drawing.Bitmap($file)
        try {
            $item.width = $bitmap.Width
            $item.height = $bitmap.Height
            $item.pixel_format = [string]$bitmap.PixelFormat
            $item.raw_format = [string]$bitmap.RawFormat
            $hasAlpha = [System.Drawing.Image]::IsAlphaPixelFormat($bitmap.PixelFormat)
            $minL = 255; $maxL = 0; $sumL = 0.0; $sumL2 = 0.0; $count = 0
            $alphaMin = 255; $alphaMax = 0
            $dark = 0
            $stepX = [math]::Max(1, [math]::Floor($bitmap.Width / 64))
            $stepY = [math]::Max(1, [math]::Floor($bitmap.Height / 64))
            for ($y = 0; $y -lt $bitmap.Height; $y += $stepY) {
                for ($x = 0; $x -lt $bitmap.Width; $x += $stepX) {
                    $pixel = $bitmap.GetPixel($x, $y)
                    $l = 0.2126 * $pixel.R + 0.7152 * $pixel.G + 0.0722 * $pixel.B
                    if ($l -lt $minL) { $minL = $l }
                    if ($l -gt $maxL) { $maxL = $l }
                    $sumL += $l; $sumL2 += $l * $l; $count += 1
                    if ($l -lt 8) { $dark += 1 }
                    if ($hasAlpha) {
                        if ($pixel.A -lt $alphaMin) { $alphaMin = $pixel.A }
                        if ($pixel.A -gt $alphaMax) { $alphaMax = $pixel.A }
                    }
                }
            }
            $mean = if ($count -gt 0) { $sumL / $count } else { 0 }
            $variance = if ($count -gt 0) { ($sumL2 / $count) - ($mean * $mean) } else { 0 }
            $item.sampled_pixels = $count
            $item.luma_min = [math]::Round($minL, 2)
            $item.luma_max = [math]::Round($maxL, 2)
            $item.luma_mean = [math]::Round($mean, 2)
            $item.luma_std = [math]::Round([math]::Sqrt([math]::Max(0, $variance)), 2)
            $item.near_black_ratio = if ($count -gt 0) { [math]::Round($dark / $count, 4) } else { $null }
            if ($hasAlpha) {
                $item.alpha_min = $alphaMin
                $item.alpha_max = $alphaMax
            }
        } finally {
            $bitmap.Dispose()
        }
        $item.decodable = $true
    } catch {
        $item.decodable = $false
        $item.error = $_.Exception.Message
    }
    $results += $item
}
$directory = Split-Path -Parent $OutputJson
if ($directory -and -not (Test-Path -LiteralPath $directory)) { New-Item -ItemType Directory -Force -Path $directory | Out-Null }
$json = $results | ConvertTo-Json -Depth 6
[System.IO.File]::WriteAllText($OutputJson, $json + "`n", (New-Object System.Text.UTF8Encoding($false)))
Write-Output $json
