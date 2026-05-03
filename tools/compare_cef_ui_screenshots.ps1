param(
    [Parameter(Mandatory = $true)]
    [string]$CpuReference,
    [Parameter(Mandatory = $true)]
    [string]$GpuCandidate,
    [int]$MaxPerChannelDiff = 8,
    [double]$MaxMeanDiff = 1.5,
    [double]$MaxChangedPixelRatio = 0.02,
    [string]$JsonOut = ""
)

$ErrorActionPreference = "Stop"

function Normalize-WorkspacePath {
    param([string]$Path)

    if ($Path.StartsWith("\\?\")) {
        return $Path.Substring(4)
    }
    return $Path
}

function Resolve-InputPath {
    param([string]$Path)

    $normalized = Normalize-WorkspacePath -Path $Path
    return (Resolve-Path -LiteralPath $normalized).Path
}

function New-Bitmap {
    param([string]$Path)

    return [System.Drawing.Bitmap]::new($Path)
}

Add-Type -AssemblyName System.Drawing

$cpuPath = Resolve-InputPath -Path $CpuReference
$gpuPath = Resolve-InputPath -Path $GpuCandidate
$cpu = New-Bitmap -Path $cpuPath
$gpu = New-Bitmap -Path $gpuPath

try {
    if ($cpu.Width -ne $gpu.Width -or $cpu.Height -ne $gpu.Height) {
        throw "CEF UI screenshot dimensions differ: cpu=$($cpu.Width)x$($cpu.Height) gpu=$($gpu.Width)x$($gpu.Height)"
    }

    $pixelCount = [int64]$cpu.Width * [int64]$cpu.Height
    $changedPixels = [int64]0
    $sumDiff = [int64]0
    $maxDiff = 0

    for ($y = 0; $y -lt $cpu.Height; $y++) {
        for ($x = 0; $x -lt $cpu.Width; $x++) {
            $a = $cpu.GetPixel($x, $y)
            $b = $gpu.GetPixel($x, $y)
            $dr = [Math]::Abs([int]$a.R - [int]$b.R)
            $dg = [Math]::Abs([int]$a.G - [int]$b.G)
            $db = [Math]::Abs([int]$a.B - [int]$b.B)
            $da = [Math]::Abs([int]$a.A - [int]$b.A)
            $pixelMax = [Math]::Max([Math]::Max($dr, $dg), [Math]::Max($db, $da))
            $sumDiff += [int64]($dr + $dg + $db + $da)
            if ($pixelMax -gt $maxDiff) {
                $maxDiff = $pixelMax
            }
            if ($pixelMax -gt 0) {
                $changedPixels++
            }
        }
    }

    $meanDiff = if ($pixelCount -eq 0) { 0.0 } else { [double]$sumDiff / ([double]$pixelCount * 4.0) }
    $changedRatio = if ($pixelCount -eq 0) { 0.0 } else { [double]$changedPixels / [double]$pixelCount }
    $passed = $maxDiff -le $MaxPerChannelDiff -and $meanDiff -le $MaxMeanDiff -and $changedRatio -le $MaxChangedPixelRatio

    $summary = [ordered]@{
        status = if ($passed) { "passed" } else { "failed" }
        cpu_reference = $cpuPath
        gpu_candidate = $gpuPath
        width = $cpu.Width
        height = $cpu.Height
        pixel_count = $pixelCount
        changed_pixel_count = $changedPixels
        changed_pixel_ratio = $changedRatio
        max_per_channel_diff = $maxDiff
        mean_channel_diff = $meanDiff
        threshold_max_per_channel_diff = $MaxPerChannelDiff
        threshold_max_mean_diff = $MaxMeanDiff
        threshold_max_changed_pixel_ratio = $MaxChangedPixelRatio
    }

    $json = $summary | ConvertTo-Json -Depth 4
    if ($JsonOut -ne "") {
        $jsonPath = Normalize-WorkspacePath -Path $JsonOut
        $parent = Split-Path -Parent $jsonPath
        if ($parent -ne "" -and -not (Test-Path -LiteralPath $parent)) {
            New-Item -ItemType Directory -Path $parent | Out-Null
        }
        Set-Content -LiteralPath $jsonPath -Value $json -Encoding UTF8
    }
    Write-Output $json

    if (-not $passed) {
        exit 1
    }
}
finally {
    $cpu.Dispose()
    $gpu.Dispose()
}
