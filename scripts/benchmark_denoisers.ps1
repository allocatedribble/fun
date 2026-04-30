param(
    [string[]]$Modes = @("off", "cheap-temporal", "balanced-fast", "balanced", "quality", "rr"),
    [string]$RenderBackend = "vulkan",
    [string]$PresentMode = "immediate",
    [string]$SolariInternalScale = "1.0",
    [string]$SolariArch = "budgeted",
    [int]$SolariTargetFps = 144,
    [int]$SolariFrameBudgetNs = 6944444,
    [int]$SolariGpuBudgetNs = 3000000,
    [string]$SolariVisualTarget = "competitive",
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30,
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$DisableMeshlets,
    [switch]$TraceDiagnostics
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

function Get-MetricMean {
    param(
        [object]$Summary,
        [string]$Metric
    )

    $property = $Summary.metrics.PSObject.Properties[$Metric]
    if ($null -eq $property) {
        return 0.0
    }
    return [double]$property.Value.mean
}

function Get-MetricP95 {
    param(
        [object]$Summary,
        [string]$Metric
    )

    $property = $Summary.metrics.PSObject.Properties[$Metric]
    if ($null -eq $property) {
        return 0.0
    }
    return [double]$property.Value.p95
}

function Round-Value {
    param([double]$Value)

    return [Math]::Round($Value, 4)
}

function Get-DenoiserTotalNs {
    param(
        [object]$Summary,
        [string]$Mode
    )

    if ($Mode -eq "rr") {
        return (Get-MetricMean $Summary "dlss_rr_gpu_ns") +
            (Get-MetricMean $Summary "solari_pass_dlss_rr_guide_resolve_ns")
    }

    return (Get-MetricMean $Summary "solari_pass_denoise_cheap_ns") +
        (Get-MetricMean $Summary "solari_pass_denoise_atrous_1_ns") +
        (Get-MetricMean $Summary "solari_pass_denoise_atrous_2_ns") +
        (Get-MetricMean $Summary "solari_pass_denoise_atrous_3_ns") +
        (Get-MetricMean $Summary "solari_pass_denoise_composite_ns")
}

function Get-ModeConfig {
    param([string]$Mode)

    switch ($Mode) {
        "off" {
            return [pscustomobject]@{ name = "off"; solari_denoise_mode = "off"; disable_dlss_rr = $true }
        }
        "cheap" {
            return [pscustomobject]@{ name = "cheap-temporal"; solari_denoise_mode = "cheap-temporal"; disable_dlss_rr = $true }
        }
        "cheap-temporal" {
            return [pscustomobject]@{ name = "cheap-temporal"; solari_denoise_mode = "cheap-temporal"; disable_dlss_rr = $true }
        }
        "balanced-fast" {
            return [pscustomobject]@{ name = "balanced-fast"; solari_denoise_mode = "balanced-fast"; disable_dlss_rr = $true }
        }
        "balanced_fast" {
            return [pscustomobject]@{ name = "balanced-fast"; solari_denoise_mode = "balanced-fast"; disable_dlss_rr = $true }
        }
        "fast" {
            return [pscustomobject]@{ name = "balanced-fast"; solari_denoise_mode = "balanced-fast"; disable_dlss_rr = $true }
        }
        "balanced" {
            return [pscustomobject]@{ name = "balanced"; solari_denoise_mode = "balanced"; disable_dlss_rr = $true }
        }
        "quality" {
            return [pscustomobject]@{ name = "quality"; solari_denoise_mode = "quality"; disable_dlss_rr = $true }
        }
        "rr" {
            return [pscustomobject]@{ name = "rr"; solari_denoise_mode = "dlss-rr"; disable_dlss_rr = $false }
        }
        default {
            throw "Unknown denoiser mode '$Mode'. Use off, cheap-temporal, balanced-fast, balanced, quality, or rr."
        }
    }
}

function Find-LatestClientBenchmark {
    param(
        [string]$ClientBenchmarkRoot,
        [datetime]$StartedAt
    )

    $candidate = Get-ChildItem -Path $ClientBenchmarkRoot -Directory -ErrorAction Stop |
        Where-Object { $_.LastWriteTime -ge $StartedAt.AddSeconds(-2) } |
        Sort-Object LastWriteTime -Descending |
        Select-Object -First 1

    if ($null -eq $candidate) {
        throw "Could not find client benchmark output newer than $StartedAt"
    }

    return $candidate
}

function Write-DenoiserReport {
    param(
        [string]$Path,
        [object[]]$Rows
    )

    $off = $Rows | Where-Object { $_.mode -eq "off" } | Select-Object -First 1
    $bestFps = $Rows | Sort-Object fps_mean -Descending | Select-Object -First 1
    $bestFrameP95 = $Rows | Sort-Object frame_p95_ns | Select-Object -First 1
    $lowestDenoiser = $Rows | Sort-Object denoiser_or_rr_mean_ns | Select-Object -First 1
    $lines = New-Object "System.Collections.Generic.List[string]"
    $lines.Add("# Denoiser Benchmark Matrix") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("- Best FPS mean: $($bestFps.mode) at $($bestFps.fps_mean)") | Out-Null
    $lines.Add("- Lowest frame p95: $($bestFrameP95.mode) at $($bestFrameP95.frame_p95_ns) ns") | Out-Null
    $lines.Add("- Lowest denoiser/RR cost: $($lowestDenoiser.mode) at $($lowestDenoiser.denoiser_or_rr_mean_ns) ns") | Out-Null
    $lines.Add("- Standard runtime denoiser: balanced-fast") | Out-Null
    $lines.Add("- Solari architecture: $SolariArch, visual target: $SolariVisualTarget, target FPS: $SolariTargetFps") | Out-Null
    $lines.Add("- Solari internal scale: $SolariInternalScale") | Out-Null
    $lines.Add("- Known RR issue: Ray Reconstruction can show a large black square/rectangle and missing or broken shadows in this project.") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("## Mode Summary") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("| mode | fps mean | frame mean ns | frame p95 ns | solari mean ns | denoiser/RR mean ns | denoiser delta vs off ns | frame p95 delta vs off ns | sample count | summary |") | Out-Null
    $lines.Add("|---|---:|---:|---:|---:|---:|---:|---:|---:|---|") | Out-Null
    foreach ($row in $Rows) {
        $denoiserDelta = if ($null -eq $off) { 0.0 } else { $row.denoiser_or_rr_mean_ns - $off.denoiser_or_rr_mean_ns }
        $frameP95Delta = if ($null -eq $off) { 0.0 } else { $row.frame_p95_ns - $off.frame_p95_ns }
        $lines.Add((
                "| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} | [{0}]({9}) |" -f
                $row.mode,
                $row.fps_mean,
                $row.frame_mean_ns,
                $row.frame_p95_ns,
                $row.solari_gpu_mean_ns,
                $row.denoiser_or_rr_mean_ns,
                (Round-Value $denoiserDelta),
                (Round-Value $frameP95Delta),
                $row.sample_count,
                $row.summary_json
            )) | Out-Null
    }

    $lines.Add("") | Out-Null
    $lines.Add("## Pass Breakdown") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("| mode | diffuse initial ns | diffuse spatial ns | guide resolve ns | external RR ns | specular regular ns | specular PSR ns | cheap temporal ns | atrous 1 ns | atrous 2 ns | atrous 3 ns | composite ns |") | Out-Null
    $lines.Add("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|") | Out-Null
    foreach ($row in $Rows) {
        $lines.Add((
                "| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} | {8} | {9} | {10} | {11} |" -f
                $row.mode,
                $row.diffuse_initial_mean_ns,
                $row.diffuse_spatial_mean_ns,
                $row.dlss_rr_guide_resolve_mean_ns,
                $row.dlss_rr_external_mean_ns,
                $row.specular_regular_mean_ns,
                $row.specular_psr_mean_ns,
                $row.denoise_cheap_mean_ns,
                $row.denoise_atrous_1_mean_ns,
                $row.denoise_atrous_2_mean_ns,
                $row.denoise_atrous_3_mean_ns,
                $row.denoise_composite_mean_ns
            )) | Out-Null
    }

    $lines.Add("") | Out-Null
    $lines.Add("`denoiser/RR mean ns` is the image-space denoiser cost for non-RR modes and `DLSS RR external pass + guide resolve` for RR mode.") | Out-Null
    Set-Content -Path $Path -Value $lines -Encoding UTF8
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Normalize-WorkspacePath ((Resolve-Path (Join-Path $scriptRoot "..")).Path)
$clientBenchmarkRoot = Join-Path $repoRoot "target\benchmarks\client"
$matrixRoot = Join-Path $repoRoot ("target\benchmarks\denoisers\" + (Get-Date -Format "yyyyMMdd-HHmmss-fff"))
New-Item -ItemType Directory -Force -Path $matrixRoot | Out-Null

$rows = New-Object "System.Collections.Generic.List[object]"

foreach ($modeName in $Modes) {
    $mode = Get-ModeConfig $modeName
    $startedAt = Get-Date
    $clientArgs = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        (Join-Path $scriptRoot "benchmark_client.ps1"),
        "-RenderBackend",
        $RenderBackend,
        "-PresentMode",
        $PresentMode,
        "-WarmupSeconds",
        "$WarmupSeconds",
        "-SampleSeconds",
        "$SampleSeconds",
        "-SolariDenoiseMode",
        $mode.solari_denoise_mode,
        "-SolariInternalScale",
        $SolariInternalScale,
        "-SolariArch",
        $SolariArch,
        "-SolariTargetFps",
        "$SolariTargetFps",
        "-SolariFrameBudgetNs",
        "$SolariFrameBudgetNs",
        "-SolariGpuBudgetNs",
        "$SolariGpuBudgetNs",
        "-SolariVisualTarget",
        $SolariVisualTarget
    )
    if ($Release) { $clientArgs += "-Release" }
    if ($StaticBevy) { $clientArgs += "-StaticBevy" }
    if ($DisableMeshlets) { $clientArgs += "-DisableMeshlets" }
    if ($TraceDiagnostics) { $clientArgs += "-TraceDiagnostics" }
    if ($mode.disable_dlss_rr) { $clientArgs += "-DisableDlssRr" }

    Write-Host "Running denoiser benchmark mode=$($mode.name)..."
    & powershell @clientArgs
    if ($LASTEXITCODE -ne 0) {
        throw "benchmark_client.ps1 failed for mode=$($mode.name) with exit code $LASTEXITCODE"
    }

    $output = Find-LatestClientBenchmark -ClientBenchmarkRoot $clientBenchmarkRoot -StartedAt $startedAt
    $sourceJson = Join-Path $output.FullName "summary.json"
    $sourceMd = Join-Path $output.FullName "summary.md"
    $modeRoot = Join-Path $matrixRoot $mode.name
    New-Item -ItemType Directory -Force -Path $modeRoot | Out-Null
    Copy-Item -LiteralPath $sourceJson -Destination (Join-Path $modeRoot "summary.json") -Force
    if (Test-Path $sourceMd) {
        Copy-Item -LiteralPath $sourceMd -Destination (Join-Path $modeRoot "summary.md") -Force
    }

    $summary = Get-Content -Path $sourceJson -Raw | ConvertFrom-Json
    $rows.Add([pscustomobject]@{
            mode = $mode.name
            summary_json = Join-Path $mode.name "summary.json"
            sample_count = $summary.samples.count
            fps_mean = Round-Value (Get-MetricMean $summary "fps")
            frame_mean_ns = Round-Value (Get-MetricMean $summary "frame_ns")
            frame_p95_ns = Round-Value (Get-MetricP95 $summary "frame_ns")
            solari_gpu_mean_ns = Round-Value (Get-MetricMean $summary "solari_gpu_ns")
            denoiser_or_rr_mean_ns = Round-Value (Get-DenoiserTotalNs -Summary $summary -Mode $mode.name)
            diffuse_initial_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_diffuse_initial_ns")
            diffuse_spatial_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_diffuse_spatial_ns")
            dlss_rr_guide_resolve_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_dlss_rr_guide_resolve_ns")
            dlss_rr_external_mean_ns = Round-Value (Get-MetricMean $summary "dlss_rr_gpu_ns")
            specular_regular_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_specular_regular_ns")
            specular_psr_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_specular_psr_ns")
            denoise_cheap_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_denoise_cheap_ns")
            denoise_atrous_1_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_denoise_atrous_1_ns")
            denoise_atrous_2_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_denoise_atrous_2_ns")
            denoise_atrous_3_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_denoise_atrous_3_ns")
            denoise_composite_mean_ns = Round-Value (Get-MetricMean $summary "solari_pass_denoise_composite_ns")
        }) | Out-Null
}

$jsonPath = Join-Path $matrixRoot "summary.json"
$mdPath = Join-Path $matrixRoot "summary.md"
$result = [ordered]@{
    created_at = (Get-Date).ToString("o")
    render_backend = $RenderBackend
    present_mode = $PresentMode
    warmup_seconds = $WarmupSeconds
    sample_seconds = $SampleSeconds
    solari_internal_scale = $SolariInternalScale
    modes = $rows
}
$result | ConvertTo-Json -Depth 8 | Set-Content -Path $jsonPath -Encoding UTF8
Write-DenoiserReport -Path $mdPath -Rows $rows

Write-Host "Denoiser matrix: $mdPath"
Write-Host "Denoiser matrix JSON: $jsonPath"
