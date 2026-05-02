param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$DisableDlssRr,
    [switch]$DisableSolari,
    [switch]$DisableMeshlets,
    [switch]$DisableClouds,
    [switch]$DisableFpsOverlay,
    [string]$RtSampleDirect = "",
    [string]$RtSampleIndirect = "",
    [string]$RtSampleReflections = "",
    [string]$RtSurfaceCache = "",
    [string]$RtMegaGeom = "",
    [string]$RtOpacityMask = "",
    [string]$RtHair = "",
    [string]$RtAsyncReadback = "",
    [string]$RtValidation = "",
    [switch]$RenderUnknownVendor,
    [string]$RenderVendorEmulation = "",
    [switch]$TraceDiagnostics,
    [switch]$FrameTimeDiagnostics,
    [int]$FrameTimeDiagnosticInterval = 60,
    [int]$FrameTimeDiagnosticMinNs = 0,
    [int]$FrameTimeDiagnosticMaxDepth = 10,
    [int]$FrameTimeDiagnosticTopChildren = 16,
    [int]$FrameTimeDiagnosticTopSpans = 32,
    [switch]$FrameTimeDiagnosticRowEvents,
    [ValidateSet("full_runtime", "solari_floor", "meshlet_floor", "cpu_floor", "streaming_spike", "presentation_floor")]
    [string]$BenchmarkLane = "full_runtime",
    [string]$SolariArch = "budgeted",
    [int]$SolariTargetFps = 144,
    [int]$SolariFrameBudgetNs = 6944444,
    [int]$SolariGpuBudgetNs = 3000000,
    [string]$SolariVisualTarget = "competitive",
    [string]$SolariDenoiseMode = "balanced-fast",
    [string]$SolariInternalScale = "1.0",
    [int]$SolariBlasCompactionVertices = 0,
    [string]$CloudQuality = "balanced",
    [string]$CloudInternalScale = "0.5",
    [string]$CloudTemporal = "1",
    [string]$CloudShadows = "0",
    [string]$CloudProfile = "scattered",
    [string]$CloudDebugOverlay = "",
    [string]$RenderGeometryPolicy = "hybrid",
    [int]$MeshletMinTriangles = 512,
    [int]$WindowWidth = 0,
    [int]$WindowHeight = 0,
    [string]$RenderBackend = "vulkan",
    [string]$PresentMode = "immediate",
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30,
    [string]$Baseline = "",
    [string]$InputLog = "",
    [switch]$KeepRunning
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

function Resolve-RepoPath {
    param(
        [string]$RepoRoot,
        [string]$Path
    )

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return Normalize-WorkspacePath ((Resolve-Path $Path).Path)
    }

    return Normalize-WorkspacePath ((Resolve-Path (Join-Path $RepoRoot $Path)).Path)
}

function Add-Metric {
    param(
        [System.Collections.IDictionary]$Sample,
        [string]$Name,
        [string]$Text,
        [switch]$Milliseconds
    )

    if ($null -eq $Sample) {
        return
    }
    if ([string]::IsNullOrWhiteSpace($Text) -or $Text -eq "pending") {
        return
    }

    $number = 0.0
    $style = [System.Globalization.NumberStyles]::Float
    $culture = [System.Globalization.CultureInfo]::InvariantCulture
    if (-not [double]::TryParse($Text, $style, $culture, [ref]$number)) {
        return
    }

    $Sample[$Name] = [double]$number
    if ($Milliseconds) {
        $nsName = if ($Name.EndsWith("_ms")) {
            $Name.Substring(0, $Name.Length - 3) + "_ns"
        }
        else {
            $Name + "_ns"
        }
        $Sample[$nsName] = [double]($number * 1000000.0)
    }
}

function ConvertTo-MetricName {
    param([string]$Name)

    return ([regex]::Replace($Name.ToLowerInvariant(), "[^a-z0-9]+", "_")).Trim("_")
}

function Add-KeyValueMetrics {
    param(
        [System.Collections.IDictionary]$Sample,
        [string]$Payload,
        [string]$Prefix,
        [switch]$Milliseconds
    )

    foreach ($match in [regex]::Matches($Payload, "([A-Za-z0-9_\/]+)=([^\s,]+)")) {
        $key = ConvertTo-MetricName $match.Groups[1].Value
        $value = $match.Groups[2].Value
        $metricName = "$Prefix$key"
        if ($Milliseconds -and -not $metricName.EndsWith("_ms")) {
            $metricName = $metricName + "_ms"
        }
        Add-Metric -Sample $Sample -Name $metricName -Text $value -Milliseconds:$Milliseconds
    }
}

function ConvertTo-KeyValueObject {
    param(
        [string]$Payload,
        [string]$HashKey = "hash"
    )

    $result = [ordered]@{}
    foreach ($match in [regex]::Matches($Payload, "([A-Za-z0-9_\/]+)=([^\s,]+)")) {
        $key = ConvertTo-MetricName $match.Groups[1].Value
        if ($key -eq "hash") {
            $key = $HashKey
        }
        $result[$key] = $match.Groups[2].Value
    }
    return $result
}

function Parse-RenderCapabilitiesLog {
    param([string[]]$Lines)

    foreach ($line in $Lines) {
        $capabilities = [regex]::Match($line, "\[bevy render\] capabilities: (?<payload>.*)$")
        if ($capabilities.Success) {
            $result = ConvertTo-KeyValueObject -Payload $capabilities.Groups["payload"].Value -HashKey "backend_capability_hash"
            $result["status"] = "found"
            return $result
        }
    }

    return [ordered]@{
        status = "not_found"
        backend_capability_hash = $null
    }
}

function Parse-RenderFeatureGatesLog {
    param([string[]]$Lines)

    foreach ($line in $Lines) {
        $featureGates = [regex]::Match($line, "\[fun render\] RT gates: (?<payload>.*)$")
        if ($featureGates.Success) {
            $result = ConvertTo-KeyValueObject -Payload $featureGates.Groups["payload"].Value -HashKey "rt_feature_hash"
            $result["status"] = "found"
            return $result
        }
    }

    return [ordered]@{
        status = "not_found"
        rt_feature_hash = $null
    }
}

function Parse-ClientPerfLog {
    param([string[]]$Lines)

    $samples = New-Object "System.Collections.Generic.List[object]"
    $current = $null

    foreach ($line in $Lines) {
        $main = [regex]::Match($line, "\[client perf\] fps=(?<fps>\S+) frame_ms=(?<frame_ms>\S+) solari_gpu_ms=(?<solari>\S+) meshlet_visibility_gpu_ms=(?<meshlet>\S+) dlss_rr_gpu_ms=(?<dlss>\S+)")
        if ($main.Success) {
            $current = [ordered]@{}
            Add-Metric -Sample $current -Name "fps" -Text $main.Groups["fps"].Value
            Add-Metric -Sample $current -Name "frame_ms" -Text $main.Groups["frame_ms"].Value -Milliseconds
            Add-Metric -Sample $current -Name "solari_gpu_ms" -Text $main.Groups["solari"].Value -Milliseconds
            Add-Metric -Sample $current -Name "meshlet_visibility_gpu_ms" -Text $main.Groups["meshlet"].Value -Milliseconds
            Add-Metric -Sample $current -Name "dlss_rr_gpu_ms" -Text $main.Groups["dlss"].Value -Milliseconds
            $samples.Add($current) | Out-Null
            continue
        }

        $cpu = [regex]::Match($line, "\[client perf\] process_cpu_pct=(?<process_cpu>\S+) process_mem_gib=(?<process_mem>\S+) system_cpu_pct=(?<system_cpu>\S+) system_mem_pct=(?<system_mem>\S+)")
        if ($cpu.Success) {
            Add-Metric -Sample $current -Name "process_cpu_pct" -Text $cpu.Groups["process_cpu"].Value
            Add-Metric -Sample $current -Name "process_mem_gib" -Text $cpu.Groups["process_mem"].Value
            Add-Metric -Sample $current -Name "system_cpu_pct" -Text $cpu.Groups["system_cpu"].Value
            Add-Metric -Sample $current -Name "system_mem_pct" -Text $cpu.Groups["system_mem"].Value
            continue
        }

        $passes = [regex]::Match($line, "\[client perf\] solari passes gpu_ms: (?<payload>.*)$")
        if ($passes.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $passes.Groups["payload"].Value -Prefix "solari_pass_" -Milliseconds
            continue
        }

        $budget = [regex]::Match($line, "\[client perf\] solari budget: (?<payload>.*)$")
        if ($budget.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $budget.Groups["payload"].Value -Prefix "solari_budget_"
            continue
        }

        $top = [regex]::Match($line, "\[client perf\] top render timings (?<payload>.*)$")
        if ($top.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $top.Groups["payload"].Value -Prefix "top_render_" -Milliseconds
            continue
        }

        $nonSolariGpu = [regex]::Match($line, "\[client perf\] non_solari gpu_ms: (?<payload>.*)$")
        if ($nonSolariGpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $nonSolariGpu.Groups["payload"].Value -Prefix "" -Milliseconds
            continue
        }

        $cloudGpu = [regex]::Match($line, "\[client perf\] clouds gpu_ms: (?<payload>.*)$")
        if ($cloudGpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $cloudGpu.Groups["payload"].Value -Prefix "cloud_" -Milliseconds
            continue
        }

        $cloudCpu = [regex]::Match($line, "\[client perf\] clouds cpu_ns: (?<payload>.*)$")
        if ($cloudCpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $cloudCpu.Groups["payload"].Value -Prefix "cloud_"
            continue
        }

        $cloudState = [regex]::Match($line, "\[client perf\] clouds state: (?<payload>.*)$")
        if ($cloudState.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $cloudState.Groups["payload"].Value -Prefix "cloud_"
            continue
        }

        $nonSolariCpu = [regex]::Match($line, "\[client perf\] non_solari cpu_ns: (?<payload>.*)$")
        if ($nonSolariCpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $nonSolariCpu.Groups["payload"].Value -Prefix ""
            continue
        }

        $renderPaths = [regex]::Match($line, "\[client perf\] render paths: (?<payload>.*)$")
        if ($renderPaths.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $renderPaths.Groups["payload"].Value -Prefix ""
            continue
        }

        $meshletBuffers = [regex]::Match($line, "\[client perf\] meshlet buffers: (?<payload>.*)$")
        if ($meshletBuffers.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $meshletBuffers.Groups["payload"].Value -Prefix "meshlet_"
            continue
        }

        $radianceCache = [regex]::Match($line, "\[client perf\] radiance cache: (?<payload>.*)$")
        if ($radianceCache.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $radianceCache.Groups["payload"].Value -Prefix "radiance_cache_"
            continue
        }

        if ($line.Contains("transient render resource arena frame")) {
            Add-KeyValueMetrics -Sample $current -Payload $line -Prefix "transient_"
            continue
        }

        if ($line.Contains("render graph budget pressure")) {
            Add-KeyValueMetrics -Sample $current -Payload $line -Prefix "render_scheduler_"
            continue
        }

        $scheduleCpu = [regex]::Match($line, "\[client perf\] schedule cpu_ns: (?<payload>.*)$")
        if ($scheduleCpu.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $scheduleCpu.Groups["payload"].Value -Prefix "schedule_"
            continue
        }

        $scheduleDetail = [regex]::Match($line, "\[client perf\] schedule detail: (?<payload>.*)$")
        if ($scheduleDetail.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $scheduleDetail.Groups["payload"].Value -Prefix "schedule_"
            continue
        }
    }

    return $samples
}

function Get-Percentile {
    param(
        [double[]]$Values,
        [double]$Percentile
    )

    if ($Values.Count -eq 0) {
        return $null
    }

    $sorted = @($Values | Sort-Object)
    $rank = [Math]::Ceiling(($Percentile / 100.0) * $sorted.Count)
    $index = [Math]::Max(0, [Math]::Min($sorted.Count - 1, $rank - 1))
    return [double]$sorted[$index]
}

function Round-Metric {
    param(
        [double]$Value,
        [string]$MetricName = ""
    )

    if ($MetricName.EndsWith("_ns")) {
        return [Math]::Round($Value, 0)
    }

    return [Math]::Round($Value, 4)
}

function Get-SummaryStats {
    param([object[]]$Samples)

    $metricNames = @{}
    foreach ($sample in $Samples) {
        foreach ($key in $sample.Keys) {
            $metricNames[$key] = $true
        }
    }

    $stats = [ordered]@{}
    foreach ($metricName in ($metricNames.Keys | Sort-Object)) {
        $values = @()
        foreach ($sample in $Samples) {
            if ($sample.Contains($metricName)) {
                $values += [double]$sample[$metricName]
            }
        }

        if ($values.Count -eq 0) {
            continue
        }

        $measure = $values | Measure-Object -Average -Minimum -Maximum
        $stats[$metricName] = [ordered]@{
            count = $values.Count
            mean = Round-Metric -Value ([double]$measure.Average) -MetricName $metricName
            min = Round-Metric -Value ([double]$measure.Minimum) -MetricName $metricName
            max = Round-Metric -Value ([double]$measure.Maximum) -MetricName $metricName
            p50 = Round-Metric -Value (Get-Percentile -Values $values -Percentile 50) -MetricName $metricName
            p95 = Round-Metric -Value (Get-Percentile -Values $values -Percentile 95) -MetricName $metricName
        }
    }

    return $stats
}

function Get-RepoGitLines {
    param(
        [string]$RepoRoot,
        [string[]]$Arguments
    )

    Push-Location $RepoRoot
    try {
        $output = @(& git @Arguments 2>$null)
        if ($LASTEXITCODE -ne 0) {
            return @()
        }
        return $output
    }
    catch {
        return @()
    }
    finally {
        Pop-Location
    }
}

function Get-HardwareInfo {
    $gpu = @()
    $cpu = @()

    try {
        $gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion, AdapterRAM)
    }
    catch {
        $gpu = @()
    }

    try {
        $cpu = @(Get-CimInstance Win32_Processor | Select-Object Name, NumberOfCores, NumberOfLogicalProcessors, MaxClockSpeed)
    }
    catch {
        $cpu = @()
    }

    return [ordered]@{
        cpu = $cpu
        gpu = $gpu
    }
}

function New-Comparison {
    param(
        [System.Collections.IDictionary]$CurrentStats,
        [string]$BaselinePath
    )

    if ([string]::IsNullOrWhiteSpace($BaselinePath)) {
        return $null
    }
    if (-not (Test-Path $BaselinePath)) {
        throw "Baseline summary was not found: $BaselinePath"
    }

    $baselineSummary = Get-Content -Path $BaselinePath -Raw | ConvertFrom-Json
    $comparison = [ordered]@{}

    foreach ($metricName in ($CurrentStats.Keys | Sort-Object)) {
        $baselineProperty = $baselineSummary.metrics.PSObject.Properties[$metricName]
        if ($null -eq $baselineProperty) {
            continue
        }

        $currentMean = [double]$CurrentStats[$metricName].mean
        $baselineMean = [double]$baselineProperty.Value.mean
        $delta = $currentMean - $baselineMean
        $displayDelta = Round-Metric -Value $delta -MetricName $metricName
        $deltaPercent = $null
        if ([Math]::Abs($baselineMean) -gt 0.000001) {
            $deltaPercent = ($delta / $baselineMean) * 100.0
        }

        $higherIsBetter = $metricName -eq "fps"
        $lowerIsBetter = $metricName.EndsWith("_ms") -or $metricName.EndsWith("_ns")
        $better = $null
        if ([double]$displayDelta -eq 0.0) {
            $better = $true
        }
        elseif ($higherIsBetter) {
            $better = $delta -ge 0.0
        }
        elseif ($lowerIsBetter) {
            $better = $delta -le 0.0
        }

        $comparison[$metricName] = [ordered]@{
            baseline_mean = Round-Metric -Value $baselineMean -MetricName $metricName
            current_mean = Round-Metric -Value $currentMean -MetricName $metricName
            delta = $displayDelta
            delta_percent = if ($null -eq $deltaPercent -or [double]$displayDelta -eq 0.0) { 0 } else { Round-Metric -Value $deltaPercent }
            better = $better
        }
    }

    return $comparison
}

function Format-StatValue {
    param(
        [System.Collections.IDictionary]$Stats,
        [string]$Metric,
        [string]$Field
    )

    if (-not $Stats.Contains($Metric)) {
        return "n/a"
    }
    return "$($Stats[$Metric][$Field])"
}

function Write-MarkdownReport {
    param(
        [string]$Path,
        [System.Collections.IDictionary]$Summary
    )

    $stats = $Summary.metrics
    $comparison = $Summary.comparison
    $lines = New-Object "System.Collections.Generic.List[string]"
    $lines.Add("# Client Benchmark") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("- Created: $($Summary.created_at)") | Out-Null
    $lines.Add("- Git: $($Summary.git.commit)") | Out-Null
    $lines.Add("- Dirty files: $($Summary.git.dirty_count)") | Out-Null
    $lines.Add("- Backend: $($Summary.config.render_backend)") | Out-Null
    $lines.Add("- Present mode: $($Summary.config.present_mode)") | Out-Null
    $lines.Add("- Solari denoise mode: $($Summary.config.solari_denoise_mode)") | Out-Null
    $lines.Add("- Solari internal scale: $($Summary.config.solari_internal_scale)") | Out-Null
    $lines.Add("- Clouds: disabled=$($Summary.config.disable_clouds) profile=$($Summary.config.cloud_profile) quality=$($Summary.config.cloud_quality) internal_scale=$($Summary.config.cloud_internal_scale) temporal=$($Summary.config.cloud_temporal) shadows=$($Summary.config.cloud_shadows)") | Out-Null
    $lines.Add("- RT feature hash: $($Summary.rt_feature_gates.rt_feature_hash)") | Out-Null
    $lines.Add("- Backend capability hash: $($Summary.render_capabilities.backend_capability_hash)") | Out-Null
    $lines.Add("- RT gates: direct=$($Summary.config.rt_sample_direct) indirect=$($Summary.config.rt_sample_indirect) reflections=$($Summary.config.rt_sample_reflections) surface_cache=$($Summary.config.rt_surface_cache) megageom=$($Summary.config.rt_megageom) opacity_mask=$($Summary.config.rt_opacity_mask) hair=$($Summary.config.rt_hair) async_readback=$($Summary.config.rt_async_readback) validation=$($Summary.config.rt_validation)") | Out-Null
    $lines.Add("- Sample count: $($Summary.samples.count)") | Out-Null
    $lines.Add("") | Out-Null

    $primaryMetrics = @(
        "fps",
        "frame_ns",
        "frame_ms",
        "solari_gpu_ns",
        "meshlet_visibility_gpu_ns",
        "cloud_total_gpu_ns",
        "cloud_weather_update_gpu_ns",
        "cloud_shape_noise_gpu_ns",
        "cloud_raymarch_gpu_ns",
        "cloud_temporal_gpu_ns",
        "cloud_resolve_gpu_ns",
        "cloud_composite_gpu_ns",
        "cloud_weather_update_cpu_ns",
        "cloud_internal_width",
        "cloud_internal_height",
        "cloud_primary_steps",
        "cloud_light_steps",
        "cloud_history_accept_rate",
        "cloud_history_reset_count",
        "cloud_vram_bytes",
        "meshlet_path_instance_count",
        "raster_path_instance_count",
        "ray_proxy_only_count",
        "meshlet_first_pass_gpu_ns",
        "meshlet_depth_pyramid_first_gpu_ns",
        "meshlet_second_pass_gpu_ns",
        "meshlet_depth_resolve_gpu_ns",
        "meshlet_material_depth_gpu_ns",
        "meshlet_depth_pyramid_second_gpu_ns",
        "meshlet_extract_cpu_ns",
        "meshlet_prepare_cpu_ns",
        "meshlet_bind_group_prepare_cpu_ns",
        "meshlet_material_queue_cpu_ns",
        "meshlet_material_queue_dirty_instance_count",
        "meshlet_instance_full_buffer_writes",
        "meshlet_instance_range_buffer_writes",
        "meshlet_material_full_buffer_writes",
        "meshlet_material_range_buffer_writes",
        "meshlet_view_visibility_buffer_writes",
        "meshlet_view_reset_cpu_queue_writes",
        "meshlet_view_reset_cpu_queue_writes_per_view",
        "meshlet_view_count",
        "transient_texture_requests",
        "transient_texture_creates",
        "transient_texture_reuses",
        "transient_texture_aliases",
        "transient_buffer_requests",
        "transient_buffer_creates",
        "transient_buffer_reuses",
        "transient_buffer_aliases",
        "transient_cached_texture_slots",
        "transient_cached_buffer_slots",
        "render_scheduler_pressure",
        "schedule_networking_receive_ns",
        "schedule_world_stream_apply_ns",
        "schedule_movement_input_ns",
        "schedule_look_ns",
        "schedule_physics_movement_ns",
        "schedule_diagnostics_logging_ns",
        "schedule_render_config_window_ns",
        "schedule_solari_runtime_params_update_ns",
        "schedule_meshlet_extraction_ns",
        "schedule_render_interpolation_ns",
        "standard_raster_gpu_ns",
        "physics_fixed_update_cpu_ns",
        "network_receive_cpu_ns",
        "world_stream_apply_cpu_ns",
        "catalog_lookup_cpu_ns",
        "post_process_gpu_ns",
        "ui_overlay_cpu_ns",
        "present_wait_ns",
        "dlss_rr_gpu_ns",
        "solari_pass_direct_ns",
        "solari_pass_diffuse_ns",
        "solari_pass_diffuse_initial_ns",
        "solari_pass_diffuse_spatial_ns",
        "solari_pass_specular_regular_ns",
        "solari_pass_specular_psr_ns",
        "solari_pass_denoise_cheap_ns",
        "solari_pass_denoise_atrous_1_ns",
        "solari_pass_denoise_atrous_2_ns",
        "solari_pass_denoise_atrous_3_ns",
        "solari_pass_denoise_composite_ns"
    )

    $lines.Add("## Primary Metrics") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("| metric | mean | p50 | p95 | min | max | samples |") | Out-Null
    $lines.Add("|---|---:|---:|---:|---:|---:|---:|") | Out-Null
    foreach ($metric in $primaryMetrics) {
        if ($stats.Contains($metric)) {
            $row = "| {0} | {1} | {2} | {3} | {4} | {5} | {6} |" -f `
                $metric, `
                (Format-StatValue -Stats $stats -Metric $metric -Field "mean"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "p50"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "p95"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "min"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "max"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "count")
            $lines.Add($row) | Out-Null
        }
    }

    $budgetLedger = [ordered]@{
        "frame_ns" = 6944444
        "cloud_total_gpu_ns" = 1200000
        "meshlet_visibility_gpu_ns" = 1200000
        "standard_raster_gpu_ns" = 600000
        "physics_fixed_update_cpu_ns" = 350000
        "network_receive_cpu_ns" = 150000
        "world_stream_apply_cpu_ns" = 150000
        "post_process_gpu_ns" = 250000
        "ui_overlay_cpu_ns" = 50000
    }
    $lines.Add("") | Out-Null
    $lines.Add("## 144 FPS Budget Ledger") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("| bucket | target p95 ns | actual p95 ns | pass |") | Out-Null
    $lines.Add("|---|---:|---:|---|") | Out-Null
    foreach ($metric in $budgetLedger.Keys) {
        $target = [double]$budgetLedger[$metric]
        $actual = if ($stats.Contains($metric)) { [double]$stats[$metric].p95 } else { $null }
        $pass = if ($null -eq $actual) { "n/a" } elseif ($actual -le $target) { "true" } else { "false" }
        $actualText = if ($null -eq $actual) { "n/a" } else { [Math]::Round($actual, 0) }
        $lines.Add("| $metric | $target | $actualText | $pass |") | Out-Null
    }

    if ($null -ne $comparison) {
        $lines.Add("") | Out-Null
        $lines.Add("## Baseline Comparison") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| metric | baseline mean | current mean | delta | delta percent | better |") | Out-Null
        $lines.Add("|---|---:|---:|---:|---:|---|") | Out-Null
        foreach ($metric in $primaryMetrics) {
            if ($comparison.Contains($metric)) {
                $entry = $comparison[$metric]
                $row = "| {0} | {1} | {2} | {3} | {4} | {5} |" -f `
                    $metric, `
                    $entry.baseline_mean, `
                    $entry.current_mean, `
                    $entry.delta, `
                    $entry.delta_percent, `
                    $entry.better
                $lines.Add($row) | Out-Null
            }
        }
    }

    $lines.Add("") | Out-Null
    $lines.Add("Full machine-readable output: summary.json") | Out-Null

    Set-Content -Path $Path -Value $lines -Encoding UTF8
}

function Stop-StackProcesses {
    param([string]$PidFile)

    if (-not (Test-Path $PidFile)) {
        return
    }

    try {
        $entries = @(Get-Content -Path $PidFile -Raw | ConvertFrom-Json)
        foreach ($entry in $entries) {
            $process = Get-Process -Id $entry.pid -ErrorAction SilentlyContinue
            if ($null -ne $process) {
                Stop-Process -Id $entry.pid -Force
            }
        }
    }
    catch {
        Write-Warning "Could not stop stack processes cleanly: $_"
    }
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Normalize-WorkspacePath ((Resolve-Path (Join-Path $scriptRoot "..")).Path)
$runRoot = Join-Path $repoRoot "target\run-stack"
$logRoot = Join-Path $runRoot "logs"
$pidFile = Join-Path $runRoot "processes.json"
$clientLog = if ([string]::IsNullOrWhiteSpace($InputLog)) {
    Join-Path $logRoot "game_client.out.log"
}
else {
    Resolve-RepoPath -RepoRoot $repoRoot -Path $InputLog
}
$baselinePath = if ([string]::IsNullOrWhiteSpace($Baseline)) {
    ""
}
else {
    Resolve-RepoPath -RepoRoot $repoRoot -Path $Baseline
}

$benchmarkRoot = Join-Path $repoRoot "target\benchmarks\client"
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$outputRoot = Join-Path $benchmarkRoot $timestamp
$outputCounter = 1
while (Test-Path $outputRoot) {
    $outputRoot = Join-Path $benchmarkRoot "$timestamp-$outputCounter"
    $outputCounter += 1
}
New-Item -ItemType Directory -Force -Path $outputRoot | Out-Null

$ranStack = $false
$lineOffset = 0

switch ($BenchmarkLane) {
    "full_runtime" {}
    "solari_floor" {
        $DisableSolari = $true
    }
    "meshlet_floor" {
        $DisableMeshlets = $true
    }
    "cpu_floor" {
        if ($WindowWidth -le 0) { $WindowWidth = 320 }
        if ($WindowHeight -le 0) { $WindowHeight = 180 }
        $DisableFpsOverlay = $true
    }
    "streaming_spike" {
        $WarmupSeconds = 0
        if ($SampleSeconds -lt 8) { $SampleSeconds = 8 }
    }
    "presentation_floor" {
        $DisableFpsOverlay = $true
    }
}

try {
    if ([string]::IsNullOrWhiteSpace($InputLog)) {
        $runStackPath = Join-Path $scriptRoot "run_stack.ps1"
        $powerShellPath = (Get-Process -Id $PID).Path
        if ([string]::IsNullOrWhiteSpace($powerShellPath)) {
            $powerShellPath = "powershell"
        }

        $runStackArgs = @(
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            $runStackPath,
            "-RenderDiagnostics",
            "-RenderBackend",
            $RenderBackend,
            "-PresentMode",
            $PresentMode
        )
        if ($Release) { $runStackArgs += "-Release" }
        if ($StaticBevy) { $runStackArgs += "-StaticBevy" }
        if ($DisableDlssRr) { $runStackArgs += "-DisableDlssRr" }
        if ($DisableSolari) { $runStackArgs += "-DisableSolari" }
        if ($DisableMeshlets) { $runStackArgs += "-DisableMeshlets" }
        if ($DisableClouds) { $runStackArgs += "-DisableClouds" }
        if ($DisableFpsOverlay) { $runStackArgs += "-DisableFpsOverlay" }
        if (-not [string]::IsNullOrWhiteSpace($RtSampleDirect)) { $runStackArgs += @("-RtSampleDirect", $RtSampleDirect) }
        if (-not [string]::IsNullOrWhiteSpace($RtSampleIndirect)) { $runStackArgs += @("-RtSampleIndirect", $RtSampleIndirect) }
        if (-not [string]::IsNullOrWhiteSpace($RtSampleReflections)) { $runStackArgs += @("-RtSampleReflections", $RtSampleReflections) }
        if (-not [string]::IsNullOrWhiteSpace($RtSurfaceCache)) { $runStackArgs += @("-RtSurfaceCache", $RtSurfaceCache) }
        if (-not [string]::IsNullOrWhiteSpace($RtMegaGeom)) { $runStackArgs += @("-RtMegaGeom", $RtMegaGeom) }
        if (-not [string]::IsNullOrWhiteSpace($RtOpacityMask)) { $runStackArgs += @("-RtOpacityMask", $RtOpacityMask) }
        if (-not [string]::IsNullOrWhiteSpace($RtHair)) { $runStackArgs += @("-RtHair", $RtHair) }
        if (-not [string]::IsNullOrWhiteSpace($RtAsyncReadback)) { $runStackArgs += @("-RtAsyncReadback", $RtAsyncReadback) }
        if (-not [string]::IsNullOrWhiteSpace($RtValidation)) { $runStackArgs += @("-RtValidation", $RtValidation) }
        if ($RenderUnknownVendor) { $runStackArgs += "-RenderUnknownVendor" }
        if (-not [string]::IsNullOrWhiteSpace($RenderVendorEmulation)) { $runStackArgs += @("-RenderVendorEmulation", $RenderVendorEmulation) }
        if (-not [string]::IsNullOrWhiteSpace($CloudQuality)) { $runStackArgs += @("-CloudQuality", $CloudQuality) }
        if (-not [string]::IsNullOrWhiteSpace($CloudInternalScale)) { $runStackArgs += @("-CloudInternalScale", $CloudInternalScale) }
        if (-not [string]::IsNullOrWhiteSpace($CloudTemporal)) { $runStackArgs += @("-CloudTemporal", $CloudTemporal) }
        if (-not [string]::IsNullOrWhiteSpace($CloudShadows)) { $runStackArgs += @("-CloudShadows", $CloudShadows) }
        if (-not [string]::IsNullOrWhiteSpace($CloudProfile)) { $runStackArgs += @("-CloudProfile", $CloudProfile) }
        if (-not [string]::IsNullOrWhiteSpace($CloudDebugOverlay)) { $runStackArgs += @("-CloudDebugOverlay", $CloudDebugOverlay) }
        $runStackArgs += "-BenchmarkLogMinimal"
        if ($TraceDiagnostics) { $runStackArgs += "-TraceDiagnostics" }
        if ($FrameTimeDiagnostics) {
            $runStackArgs += @(
                "-FrameTimeDiagnostics",
                "-FrameTimeDiagnosticInterval",
                $FrameTimeDiagnosticInterval,
                "-FrameTimeDiagnosticMinNs",
                $FrameTimeDiagnosticMinNs,
                "-FrameTimeDiagnosticMaxDepth",
                $FrameTimeDiagnosticMaxDepth,
                "-FrameTimeDiagnosticTopChildren",
                $FrameTimeDiagnosticTopChildren,
                "-FrameTimeDiagnosticTopSpans",
                $FrameTimeDiagnosticTopSpans
            )
            if ($FrameTimeDiagnosticRowEvents) { $runStackArgs += "-FrameTimeDiagnosticRowEvents" }
        }
        if (-not [string]::IsNullOrWhiteSpace($SolariDenoiseMode)) {
            $runStackArgs += @("-SolariDenoiseMode", $SolariDenoiseMode)
        }
        if (-not [string]::IsNullOrWhiteSpace($SolariInternalScale)) {
            $runStackArgs += @("-SolariInternalScale", $SolariInternalScale)
        }
        if ($SolariBlasCompactionVertices -gt 0) {
            $runStackArgs += @("-SolariBlasCompactionVertices", "$SolariBlasCompactionVertices")
        }
        if (-not [string]::IsNullOrWhiteSpace($SolariArch)) {
            $runStackArgs += @("-SolariArch", $SolariArch)
        }
        if ($SolariTargetFps -gt 0) {
            $runStackArgs += @("-SolariTargetFps", $SolariTargetFps)
        }
        if ($SolariFrameBudgetNs -gt 0) {
            $runStackArgs += @("-SolariFrameBudgetNs", $SolariFrameBudgetNs)
        }
        if ($SolariGpuBudgetNs -gt 0) {
            $runStackArgs += @("-SolariGpuBudgetNs", $SolariGpuBudgetNs)
        }
        if (-not [string]::IsNullOrWhiteSpace($SolariVisualTarget)) {
            $runStackArgs += @("-SolariVisualTarget", $SolariVisualTarget)
        }
        if (-not [string]::IsNullOrWhiteSpace($RenderGeometryPolicy)) {
            $runStackArgs += @("-RenderGeometryPolicy", $RenderGeometryPolicy)
        }
        if ($MeshletMinTriangles -gt 0) {
            $runStackArgs += @("-MeshletMinTriangles", "$MeshletMinTriangles")
        }
        if ($WindowWidth -gt 0 -and $WindowHeight -gt 0) {
            $runStackArgs += @("-WindowWidth", "$WindowWidth", "-WindowHeight", "$WindowHeight")
        }

        Write-Host "Starting benchmark stack..."
        & $powerShellPath @runStackArgs
        if ($LASTEXITCODE -ne 0) {
            throw "run_stack.ps1 failed with exit code $LASTEXITCODE"
        }
        $ranStack = $true

        $waitUntil = (Get-Date).AddSeconds(15)
        while (-not (Test-Path $clientLog)) {
            if ((Get-Date) -gt $waitUntil) {
                throw "Client log was not created: $clientLog"
            }
            Start-Sleep -Milliseconds 250
        }

        Write-Host "Warmup: $WarmupSeconds seconds"
        Start-Sleep -Seconds $WarmupSeconds
        $lineOffset = @(Get-Content -Path $clientLog -ErrorAction SilentlyContinue).Count
        Write-Host "Sampling: $SampleSeconds seconds"
        Start-Sleep -Seconds $SampleSeconds
    }

    $allLines = @(Get-Content -Path $clientLog -ErrorAction Stop)
    $sampleLines = if ($lineOffset -gt 0) {
        @($allLines | Select-Object -Skip $lineOffset)
    }
    else {
        $allLines
    }

    $samples = @(Parse-ClientPerfLog -Lines $sampleLines)
    if ($samples.Count -eq 0) {
        throw "No [client perf] samples were found in $clientLog"
    }

    $renderCapabilities = Parse-RenderCapabilitiesLog -Lines $allLines
    $rtFeatureGates = Parse-RenderFeatureGatesLog -Lines $allLines
    $stats = Get-SummaryStats -Samples $samples
    $comparison = New-Comparison -CurrentStats $stats -BaselinePath $baselinePath
    $gitCommit = (Get-RepoGitLines -RepoRoot $repoRoot -Arguments @("rev-parse", "HEAD") | Select-Object -First 1)
    $gitDirty = @(Get-RepoGitLines -RepoRoot $repoRoot -Arguments @("status", "--short"))

    $summary = [ordered]@{
        schema_version = 1
        created_at = (Get-Date).ToString("o")
        repo_root = $repoRoot
        source_log = $clientLog
        git = [ordered]@{
            commit = $gitCommit
            dirty_count = $gitDirty.Count
            dirty = $gitDirty
        }
        hardware = Get-HardwareInfo
        config = [ordered]@{
            benchmark_lane = $BenchmarkLane
            profile = if ($Release) { "release" } else { "debug" }
            static_bevy = [bool]$StaticBevy
            render_backend = $RenderBackend
            present_mode = $PresentMode
            disable_dlss_rr = [bool]$DisableDlssRr
            disable_solari = [bool]$DisableSolari
            disable_meshlets = [bool]$DisableMeshlets
            disable_clouds = [bool]$DisableClouds
            disable_fps_overlay = [bool]$DisableFpsOverlay
            rt_sample_direct = if ([string]::IsNullOrWhiteSpace($RtSampleDirect)) { "default" } else { $RtSampleDirect }
            rt_sample_indirect = if ([string]::IsNullOrWhiteSpace($RtSampleIndirect)) { "default" } else { $RtSampleIndirect }
            rt_sample_reflections = if ([string]::IsNullOrWhiteSpace($RtSampleReflections)) { "default" } else { $RtSampleReflections }
            rt_surface_cache = if ([string]::IsNullOrWhiteSpace($RtSurfaceCache)) { "default" } else { $RtSurfaceCache }
            rt_megageom = if ([string]::IsNullOrWhiteSpace($RtMegaGeom)) { "off" } else { $RtMegaGeom }
            rt_opacity_mask = if ([string]::IsNullOrWhiteSpace($RtOpacityMask)) { "off" } else { $RtOpacityMask }
            rt_hair = if ([string]::IsNullOrWhiteSpace($RtHair)) { "off" } else { $RtHair }
            rt_async_readback = if ([string]::IsNullOrWhiteSpace($RtAsyncReadback)) { "default" } else { $RtAsyncReadback }
            rt_validation = if ([string]::IsNullOrWhiteSpace($RtValidation)) { "default" } else { $RtValidation }
            render_unknown_vendor = [bool]$RenderUnknownVendor
            render_vendor_emulation = if ([string]::IsNullOrWhiteSpace($RenderVendorEmulation)) { "auto" } else { $RenderVendorEmulation }
            render_geometry_policy = $RenderGeometryPolicy
            meshlet_min_triangles = $MeshletMinTriangles
            window_width = $WindowWidth
            window_height = $WindowHeight
            solari_arch = if ([string]::IsNullOrWhiteSpace($SolariArch)) { "legacy" } else { $SolariArch }
            solari_target_fps = $SolariTargetFps
            solari_frame_budget_ns = $SolariFrameBudgetNs
            solari_gpu_budget_ns = $SolariGpuBudgetNs
            solari_visual_target = if ([string]::IsNullOrWhiteSpace($SolariVisualTarget)) { "balanced" } else { $SolariVisualTarget }
            solari_denoise_mode = if ([string]::IsNullOrWhiteSpace($SolariDenoiseMode)) { "balanced-fast" } else { $SolariDenoiseMode }
            solari_internal_scale = if ([string]::IsNullOrWhiteSpace($SolariInternalScale)) { "1.0" } else { $SolariInternalScale }
            solari_blas_compaction_vertices = $SolariBlasCompactionVertices
            cloud_quality = if ([string]::IsNullOrWhiteSpace($CloudQuality)) { "balanced" } else { $CloudQuality }
            cloud_internal_scale = if ([string]::IsNullOrWhiteSpace($CloudInternalScale)) { "0.5" } else { $CloudInternalScale }
            cloud_temporal = if ([string]::IsNullOrWhiteSpace($CloudTemporal)) { "1" } else { $CloudTemporal }
            cloud_shadows = if ([string]::IsNullOrWhiteSpace($CloudShadows)) { "0" } else { $CloudShadows }
            cloud_profile = if ([string]::IsNullOrWhiteSpace($CloudProfile)) { "scattered" } else { $CloudProfile }
            cloud_debug_overlay = if ([string]::IsNullOrWhiteSpace($CloudDebugOverlay)) { "none" } else { $CloudDebugOverlay }
        }
        samples = [ordered]@{
            count = $samples.Count
            warmup_seconds = if ([string]::IsNullOrWhiteSpace($InputLog)) { $WarmupSeconds } else { 0 }
            sample_seconds = if ([string]::IsNullOrWhiteSpace($InputLog)) { $SampleSeconds } else { 0 }
            input_log_mode = -not [string]::IsNullOrWhiteSpace($InputLog)
        }
        metrics = $stats
        comparison = $comparison
        render_capabilities = $renderCapabilities
        rt_feature_gates = $rtFeatureGates
    }

    $jsonPath = Join-Path $outputRoot "summary.json"
    $markdownPath = Join-Path $outputRoot "summary.md"
    $summary | ConvertTo-Json -Depth 8 | Set-Content -Path $jsonPath -Encoding UTF8
    Write-MarkdownReport -Path $markdownPath -Summary $summary

    Write-Host "Benchmark summary: $markdownPath"
    Write-Host "Benchmark JSON: $jsonPath"
}
finally {
    if ($ranStack -and -not $KeepRunning) {
        Stop-StackProcesses -PidFile $pidFile
    }
}
