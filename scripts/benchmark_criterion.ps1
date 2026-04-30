param(
    [string]$Package = "game_client",
    [string]$Bench = "client_costs",
    [string]$Baseline = "",
    [string]$SaveBaseline = "",
    [double]$WarmupSeconds = 0.25,
    [double]$MeasurementSeconds = 0.75,
    [int]$SampleSize = 10,
    [switch]$OpenReport
)

$ErrorActionPreference = "Stop"

function Normalize-WorkspacePath {
    param([string]$Path)

    if ($Path.StartsWith("\\?\")) {
        return $Path.Substring(4)
    }
    if ($Path.StartsWith("\?\")) {
        return $Path.Substring(3)
    }
    return $Path
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Normalize-WorkspacePath ((Resolve-Path (Join-Path $scriptRoot "..")).Path)

$benchArgs = @(
    "bench",
    "-p",
    $Package,
    "--features",
    "benchmarks",
    "--bench",
    $Bench,
    "--"
)

if (-not [string]::IsNullOrWhiteSpace($Baseline)) {
    $benchArgs += @("--baseline", $Baseline)
}
if (-not [string]::IsNullOrWhiteSpace($SaveBaseline)) {
    $benchArgs += @("--save-baseline", $SaveBaseline)
}
$benchArgs += @(
    "--warm-up-time",
    $WarmupSeconds.ToString([System.Globalization.CultureInfo]::InvariantCulture),
    "--measurement-time",
    $MeasurementSeconds.ToString([System.Globalization.CultureInfo]::InvariantCulture),
    "--sample-size",
    $SampleSize.ToString([System.Globalization.CultureInfo]::InvariantCulture)
)

Push-Location $repoRoot
try {
    & cargo @benchArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo bench failed with exit code $LASTEXITCODE"
    }
}
finally {
    Pop-Location
}

$report = Join-Path $repoRoot "target\criterion\report\index.html"
if (Test-Path $report) {
    Write-Host "Criterion report: $report"
    if ($OpenReport) {
        Start-Process $report
    }
}
