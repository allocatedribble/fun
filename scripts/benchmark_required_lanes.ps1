param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$TraceDiagnostics,
    [string]$RenderBackend = "dx12",
    [string]$PresentMode = "immediate",
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$benchmarkClient = Join-Path $scriptRoot "benchmark_client.ps1"
$lanes = @(
    "full_runtime",
    "solari_floor",
    "meshlet_floor",
    "cpu_floor",
    "streaming_spike",
    "presentation_floor"
)

foreach ($lane in $lanes) {
    $args = @(
        "-BenchmarkLane", $lane,
        "-RenderBackend", $RenderBackend,
        "-PresentMode", $PresentMode,
        "-WarmupSeconds", "$WarmupSeconds",
        "-SampleSeconds", "$SampleSeconds"
    )
    if ($Release) { $args += "-Release" }
    if ($StaticBevy) { $args += "-StaticBevy" }
    if ($TraceDiagnostics) { $args += "-TraceDiagnostics" }

    Write-Host "Running client benchmark lane=$lane..."
    & powershell -NoProfile -ExecutionPolicy Bypass -File $benchmarkClient @args
    if ($LASTEXITCODE -ne 0) {
        throw "benchmark_client.ps1 failed for lane=$lane with exit code $LASTEXITCODE"
    }
}
