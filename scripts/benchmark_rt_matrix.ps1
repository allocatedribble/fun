param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$TraceDiagnostics,
    [string]$RenderBackend = "vulkan",
    [string]$PresentMode = "immediate",
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$benchmarkClient = Join-Path $scriptRoot "benchmark_client.ps1"

$matrix = @(
    [ordered]@{
        name = "baseline_solari"
        args = @()
    },
    [ordered]@{
        name = "solari_direct_only"
        args = @("-RtSampleDirect", "1", "-RtSampleIndirect", "0", "-RtSampleReflections", "0", "-RtSurfaceCache", "0")
    },
    [ordered]@{
        name = "solari_gi_only"
        args = @("-RtSampleDirect", "0", "-RtSampleIndirect", "1", "-RtSampleReflections", "0")
    },
    [ordered]@{
        name = "solari_direct_gi"
        args = @("-RtSampleDirect", "1", "-RtSampleIndirect", "1", "-RtSampleReflections", "0")
    },
    [ordered]@{
        name = "dlss_rr_diagnostic"
        args = @("-SolariDenoiseMode", "rr")
    },
    [ordered]@{
        name = "half_resolution_gi_reservoirs"
        args = @("-SolariInternalScale", "0.5")
    },
    [ordered]@{
        name = "async_readback_off"
        args = @("-RtAsyncReadback", "0")
    },
    [ordered]@{
        name = "async_readback_on"
        args = @("-RtAsyncReadback", "1")
    },
    [ordered]@{
        name = "blas_compaction_low"
        args = @("-SolariBlasCompactionVertices", "25000")
    },
    [ordered]@{
        name = "blas_compaction_high"
        args = @("-SolariBlasCompactionVertices", "250000")
    }
)

foreach ($entry in $matrix) {
    $args = @(
        "-RenderBackend", $RenderBackend,
        "-PresentMode", $PresentMode,
        "-WarmupSeconds", "$WarmupSeconds",
        "-SampleSeconds", "$SampleSeconds"
    )
    if ($Release) { $args += "-Release" }
    if ($StaticBevy) { $args += "-StaticBevy" }
    if ($TraceDiagnostics) { $args += "-TraceDiagnostics" }
    $args += $entry.args

    Write-Host "Running RT benchmark matrix lane=$($entry.name)..."
    & powershell -NoProfile -ExecutionPolicy Bypass -File $benchmarkClient @args
    if ($LASTEXITCODE -ne 0) {
        throw "benchmark_client.ps1 failed for RT matrix lane=$($entry.name) with exit code $LASTEXITCODE"
    }
}
