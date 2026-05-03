param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$TraceDiagnostics,
    [switch]$FrameTimeDiagnostics,
    [ValidateSet("quick", "full", "present")]
    [string]$MatrixSize = "quick",
    [string[]]$Lane = @(),
    [switch]$PlanOnly,
    [switch]$ContinueOnFailure,
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30,
    [int]$WindowWidth = 1280,
    [int]$WindowHeight = 720,
    [string]$OutputRoot = ""
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

function Get-EnvAnnotation {
    param(
        [string]$Name,
        [string]$Default = "not_collected"
    )

    $value = [System.Environment]::GetEnvironmentVariable($Name, "Process")
    if ([string]::IsNullOrWhiteSpace($value)) {
        return $Default
    }
    return $value
}

function Test-CommandAvailable {
    param([string]$Name)

    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-Dx12ParityEnvironment {
    $os = $null
    try {
        $os = Get-CimInstance Win32_OperatingSystem | Select-Object Caption, Version, BuildNumber
    }
    catch {
        $os = $null
    }

    $gpu = @()
    try {
        $gpu = @(Get-CimInstance Win32_VideoController | Select-Object Name, DriverVersion, AdapterRAM)
    }
    catch {
        $gpu = @()
    }

    return [ordered]@{
        os = $os
        gpu = $gpu
        monitor_refresh_hz = Get-EnvAnnotation -Name "FUN_BENCH_ENV_MONITOR_REFRESH_HZ"
        hdr_state = Get-EnvAnnotation -Name "FUN_BENCH_ENV_HDR"
        hags_state = Get-EnvAnnotation -Name "FUN_BENCH_ENV_HAGS"
        vrr_state = Get-EnvAnnotation -Name "FUN_BENCH_ENV_VRR"
        rebar_state = Get-EnvAnnotation -Name "FUN_BENCH_ENV_REBAR"
        overlays = [ordered]@{
            status = Get-EnvAnnotation -Name "FUN_BENCH_ENV_OVERLAYS"
            steam = Get-EnvAnnotation -Name "FUN_BENCH_ENV_OVERLAY_STEAM"
            discord = Get-EnvAnnotation -Name "FUN_BENCH_ENV_OVERLAY_DISCORD"
            geforce_experience = Get-EnvAnnotation -Name "FUN_BENCH_ENV_OVERLAY_GFE"
            amd = Get-EnvAnnotation -Name "FUN_BENCH_ENV_OVERLAY_AMD"
            xbox_game_bar = Get-EnvAnnotation -Name "FUN_BENCH_ENV_OVERLAY_XBOX_GAME_BAR"
        }
        capture_tools = [ordered]@{
            presentmon_available = Test-CommandAvailable -Name "PresentMon"
            pix_attached = Get-EnvAnnotation -Name "FUN_BENCH_ENV_PIX_ATTACHED" -Default "false"
            renderdoc_attached = Get-EnvAnnotation -Name "FUN_BENCH_ENV_RENDERDOC_ATTACHED" -Default "false"
        }
    }
}

function New-Dx12ParityLane {
    param(
        [string]$Name,
        [string]$Category,
        [string]$RenderBackend = "dx12",
        [string]$PresentMode = "immediate",
        [string]$CefUiMode = "disabled",
        [string]$CefPaintTransport = "default",
        [string]$BenchmarkLane = "full_runtime",
        [bool]$DisableClouds = $false,
        [bool]$DisableSolari = $false,
        [bool]$DisableMeshlets = $false,
        [bool]$DisableFpsOverlay = $false,
        [int]$LaneWindowWidth = 0,
        [int]$LaneWindowHeight = 0,
        [int]$MaxFrameLatency = 0,
        [string]$WindowMode = "windowed",
        [string]$EditorPreview = "off",
        [string]$Notes = ""
    )

    return [ordered]@{
        name = $Name
        category = $Category
        render_backend = $RenderBackend
        present_mode = $PresentMode
        cef_ui_mode = $CefUiMode
        cef_paint_transport = $CefPaintTransport
        benchmark_lane = $BenchmarkLane
        disable_clouds = $DisableClouds
        disable_solari = $DisableSolari
        disable_meshlets = $DisableMeshlets
        disable_fps_overlay = $DisableFpsOverlay
        window_width = $LaneWindowWidth
        window_height = $LaneWindowHeight
        max_frame_latency = $MaxFrameLatency
        window_mode = $WindowMode
        editor_preview = $EditorPreview
        notes = $Notes
    }
}

function Get-Dx12ParityLaneDefinitions {
    $lanes = New-Object "System.Collections.Generic.List[object]"

    $lanes.Add((New-Dx12ParityLane -Name "vulkan_immediate" -Category "backend_present" -RenderBackend "vulkan" -PresentMode "immediate")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "vulkan_fifo" -Category "backend_present" -RenderBackend "vulkan" -PresentMode "fifo")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "vulkan_auto_no_vsync" -Category "backend_present" -RenderBackend "vulkan" -PresentMode "auto_no_vsync")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "dx12_immediate" -Category "backend_present" -RenderBackend "dx12" -PresentMode "immediate")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "dx12_fifo" -Category "backend_present" -RenderBackend "dx12" -PresentMode "fifo")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "dx12_auto_no_vsync" -Category "backend_present" -RenderBackend "dx12" -PresentMode "auto_no_vsync")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "dx12_mailbox_if_available" -Category "backend_present" -RenderBackend "dx12" -PresentMode "mailbox")) | Out-Null

    $lanes.Add((New-Dx12ParityLane -Name "ui_hidden" -Category "cef_visibility" -CefUiMode "hidden" -DisableFpsOverlay $true)) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "ui_static" -Category "cef_visibility" -CefUiMode "static" -CefPaintTransport "cpu")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "ui_animated" -Category "cef_visibility" -CefUiMode "animated" -CefPaintTransport "cpu")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "ui_animated_1440p_surface" -Category "cef_visibility" -CefUiMode "animated_1440p_surface" -CefPaintTransport "cpu" -LaneWindowWidth 2560 -LaneWindowHeight 1440)) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "ui_animated_4k_surface" -Category "cef_visibility" -CefUiMode "animated_4k_surface" -CefPaintTransport "cpu" -LaneWindowWidth 3840 -LaneWindowHeight 2160)) | Out-Null

    $lanes.Add((New-Dx12ParityLane -Name "clouds_off" -Category "feature_toggle" -DisableClouds $true)) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "clouds_on" -Category "feature_toggle")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "solari_off" -Category "feature_toggle" -DisableSolari $true)) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "solari_on" -Category "feature_toggle")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "meshlets_off" -Category "feature_toggle" -DisableMeshlets $true)) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "meshlets_on" -Category "feature_toggle")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "editor_preview_off" -Category "feature_toggle" -EditorPreview "off")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "editor_preview_on" -Category "feature_toggle" -EditorPreview "on" -Notes "metadata lane until merged editor preview exposes a benchmarkable switch")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "cef_cpu_paint" -Category "feature_toggle" -CefUiMode "animated" -CefPaintTransport "cpu")) | Out-Null
    $lanes.Add((New-Dx12ParityLane -Name "cef_gpu_accelerated" -Category "feature_toggle" -CefUiMode "animated" -CefPaintTransport "d3d11on12")) | Out-Null

    return @($lanes.ToArray())
}

function Get-Dx12PresentLaneDefinitions {
    $lanes = New-Object "System.Collections.Generic.List[object]"
    $backends = @("vulkan", "dx12")
    $presentModes = @("immediate", "auto_no_vsync", "fifo", "auto_vsync")
    $frameLatencies = @(1, 2, 3, 4)
    $uiModes = @(
        @{ name = "ui_hidden"; mode = "hidden"; transport = "default"; disable_fps = $true },
        @{ name = "ui_static"; mode = "static"; transport = "cpu"; disable_fps = $false },
        @{ name = "ui_animated"; mode = "animated"; transport = "cpu"; disable_fps = $false }
    )

    foreach ($backend in $backends) {
        foreach ($presentMode in $presentModes) {
            foreach ($latency in $frameLatencies) {
                foreach ($ui in $uiModes) {
                    $laneName = "present_${backend}_${presentMode}_fl${latency}_$($ui.name)"
                    $lanes.Add((New-Dx12ParityLane `
                                -Name $laneName `
                                -Category "present_matrix" `
                                -RenderBackend $backend `
                                -PresentMode $presentMode `
                                -MaxFrameLatency $latency `
                                -CefUiMode $ui.mode `
                                -CefPaintTransport $ui.transport `
                                -DisableFpsOverlay $ui.disable_fps `
                                -WindowMode "windowed" `
                                -Notes "windowed lane; borderless fullscreen is not yet exposed by the stack runner")) | Out-Null
                }
            }
        }
    }

    return @($lanes.ToArray())
}

function Get-QuickLaneNames {
    return @(
        "vulkan_immediate",
        "dx12_immediate",
        "vulkan_fifo",
        "dx12_fifo",
        "ui_hidden",
        "ui_animated",
        "cef_cpu_paint",
        "cef_gpu_accelerated",
        "clouds_off",
        "solari_off",
        "meshlets_off"
    )
}

function Get-RequiredMetricNames {
    return @(
        "frame_ns",
        "fps",
        "present_wait_ns",
        "post_process_gpu_ns",
        "cloud_total_gpu_ns",
        "meshlet_visibility_gpu_ns",
        "solari_gpu_ns",
        "render_scheduler_pressure",
        "transient_texture_requests",
        "transient_texture_creates",
        "transient_texture_reuses",
        "transient_texture_aliases",
        "transient_buffer_requests",
        "transient_buffer_creates",
        "transient_buffer_reuses",
        "transient_buffer_aliases",
        "transient_texture_descriptor_miss_creates",
        "transient_texture_lifetime_conflict_creates",
        "transient_buffer_descriptor_miss_creates",
        "transient_buffer_lifetime_conflict_creates",
        "transient_texture_near_miss_size",
        "transient_texture_near_miss_format",
        "transient_texture_near_miss_usage",
        "transient_texture_near_miss_view_formats",
        "transient_texture_label_variant_descriptors",
        "transient_buffer_label_variant_descriptors",
        "transient_texture_every_frame_create_descriptors",
        "transient_buffer_every_frame_create_descriptors",
        "transient_texture_resize_like_create_descriptors",
        "transient_buffer_resize_like_create_descriptors",
        "cef_on_paint_fps",
        "cef_on_accelerated_paint_fps",
        "cef_cpu_upload_bytes",
        "cef_gpu_copy_bytes",
        "cef_gpu_copy_ns",
        "cef_gpu_frame_ready_count",
        "cef_gpu_frame_not_ready_count",
        "cef_gpu_frame_reused_count",
        "cef_gpu_frame_blocking_wait_count",
        "cef_transport_fallback_count",
        "cef_published_generation",
        "cef_sampled_generation",
        "cef_stale_frame_count",
        "render_upload_write_texture_calls",
        "render_upload_write_texture_bytes",
        "render_upload_write_buffer_calls",
        "render_upload_write_buffer_bytes",
        "render_upload_write_buffer_with_calls",
        "render_upload_write_buffer_with_bytes",
        "render_upload_callsite_count",
        "render_churn_bind_group_creations",
        "render_churn_bind_group_layout_creations",
        "render_churn_bind_group_layout_cache_hits",
        "render_churn_bind_group_layout_cache_misses",
        "render_churn_pipeline_layout_creations",
        "render_churn_render_pipeline_queued",
        "render_churn_compute_pipeline_queued",
        "render_churn_render_pipeline_creations",
        "render_churn_compute_pipeline_creations",
        "render_churn_pipeline_cache_hits",
        "render_churn_pipeline_cache_misses",
        "render_churn_material_pipeline_key_count",
        "render_churn_post_process_pipeline_key_count",
        "render_churn_cloud_pipeline_key_count",
        "render_churn_solari_pipeline_key_count",
        "render_churn_meshlet_pipeline_key_count",
        "render_churn_ui_pipeline_key_count",
        "render_churn_debug_overlay_pipeline_key_count",
        "render_churn_event_count"
    )
}

function Get-Dx12ExternalMetricPlan {
    return [ordered]@{
        status = "requires_pix_presentmon_or_gpuview_capture"
        metrics = @(
            "barrier_count",
            "descriptor_heap_switch_count",
            "pipeline_creation_count",
            "command_list_count_per_frame",
            "command_allocator_reset_count",
            "fence_wait_count",
            "cpu_side_submit_count",
            "gpu_queue_idle_intervals"
        )
    }
}

function New-MetricPresence {
    param(
        [object]$Summary,
        [string[]]$MetricNames
    )

    $presence = [ordered]@{}
    foreach ($metric in $MetricNames) {
        $entry = [ordered]@{
            present = $false
            count = 0
            mean = $null
            p50 = $null
            p95 = $null
            p99 = $null
        }
        if ($null -ne $Summary -and $null -ne $Summary.metrics) {
            $property = $Summary.metrics.PSObject.Properties[$metric]
            if ($null -ne $property) {
                $entry.present = $true
                $entry.count = $property.Value.count
                $entry.mean = $property.Value.mean
                $entry.p50 = $property.Value.p50
                $entry.p95 = $property.Value.p95
                $entry.p99 = $property.Value.p99
            }
        }
        $presence[$metric] = $entry
    }
    return $presence
}

function New-KeyMetricSnapshot {
    param([object]$Summary)

    $names = @("fps", "frame_ns", "present_wait_ns", "cef_on_paint_fps", "cef_on_accelerated_paint_fps", "cef_gpu_copy_ns", "cef_gpu_frame_not_ready_count", "cef_gpu_frame_reused_count", "cef_gpu_frame_blocking_wait_count", "render_upload_write_texture_bytes", "render_upload_write_buffer_bytes", "render_churn_render_pipeline_creations", "render_churn_compute_pipeline_creations", "render_churn_bind_group_layout_creations")
    $snapshot = [ordered]@{}
    foreach ($name in $names) {
        $property = if ($null -ne $Summary -and $null -ne $Summary.metrics) { $Summary.metrics.PSObject.Properties[$name] } else { $null }
        $snapshot[$name] = if ($null -ne $property) {
            [ordered]@{
                mean = $property.Value.mean
                p95 = $property.Value.p95
                p99 = $property.Value.p99
            }
        }
        else {
            $null
        }
    }
    return $snapshot
}

function Get-LaneMetricField {
    param(
        [object]$Lane,
        [string]$Metric,
        [string]$Field,
        [double]$MissingValue
    )

    if ($null -eq $Lane -or $null -eq $Lane.key_metrics) {
        return $MissingValue
    }
    $metricEntry = $Lane.key_metrics.$Metric
    if ($null -eq $metricEntry) {
        return $MissingValue
    }
    $value = $metricEntry.$Field
    if ($null -eq $value) {
        return $MissingValue
    }
    return [double]$value
}

function Format-Dx12PresentRecommendation {
    param(
        [object]$Lane,
        [string]$Metric,
        [string]$Field
    )

    if ($null -eq $Lane) {
        return "not enough passing DX12 lanes"
    }
    $value = Get-LaneMetricField -Lane $Lane -Metric $Metric -Field $Field -MissingValue ([double]::NaN)
    return "$($Lane.name) present=$($Lane.present_mode) max_latency=$($Lane.max_frame_latency) ui=$($Lane.cef_ui_mode) $Metric.$Field=$value"
}

function New-Dx12PresentRecommendations {
    param([object[]]$LaneResults)

    $dx12Lanes = @($LaneResults | Where-Object { $_.status -eq "passed" -and $_.render_backend -eq "dx12" })
    if ($dx12Lanes.Count -eq 0) {
        return [ordered]@{
            maximum_fps = "not enough passing DX12 lanes"
            best_p95 = "not enough passing DX12 lanes"
            lowest_present_wait = "not enough passing DX12 lanes"
            default_decision = "unchanged; collect present-matrix evidence first"
        }
    }

    $bestFps = @($dx12Lanes | Sort-Object -Property { Get-LaneMetricField -Lane $_ -Metric "fps" -Field "mean" -MissingValue -1.0 } -Descending | Select-Object -First 1)
    $bestP95 = @($dx12Lanes | Sort-Object -Property { Get-LaneMetricField -Lane $_ -Metric "frame_ns" -Field "p95" -MissingValue ([double]::PositiveInfinity) } | Select-Object -First 1)
    $lowestPresentWait = @($dx12Lanes | Sort-Object -Property { Get-LaneMetricField -Lane $_ -Metric "present_wait_ns" -Field "p95" -MissingValue ([double]::PositiveInfinity) } | Select-Object -First 1)

    return [ordered]@{
        maximum_fps = Format-Dx12PresentRecommendation -Lane $bestFps[0] -Metric "fps" -Field "mean"
        best_p95 = Format-Dx12PresentRecommendation -Lane $bestP95[0] -Metric "frame_ns" -Field "p95"
        lowest_present_wait = Format-Dx12PresentRecommendation -Lane $lowestPresentWait[0] -Metric "present_wait_ns" -Field "p95"
        default_decision = "unchanged; do not promote a present-mode default until this matrix has comparable live data"
    }
}

function Convert-LaneToBenchmarkArgs {
    param(
        [System.Collections.IDictionary]$LaneDefinition,
        [string]$BenchmarkClientPath
    )

    $laneWidth = if ([int]$LaneDefinition.window_width -gt 0) { [int]$LaneDefinition.window_width } else { $WindowWidth }
    $laneHeight = if ([int]$LaneDefinition.window_height -gt 0) { [int]$LaneDefinition.window_height } else { $WindowHeight }
    $args = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $BenchmarkClientPath,
        "-BenchmarkProfile",
        "dx12_parity",
        "-BenchmarkScenario",
        $LaneDefinition.category,
        "-BenchmarkMatrixLane",
        $LaneDefinition.name,
        "-BenchmarkLane",
        $LaneDefinition.benchmark_lane,
        "-RenderBackend",
        $LaneDefinition.render_backend,
        "-PresentMode",
        $LaneDefinition.present_mode,
        "-CefUiMode",
        $LaneDefinition.cef_ui_mode,
        "-CefPaintTransport",
        $LaneDefinition.cef_paint_transport,
        "-WarmupSeconds",
        "$WarmupSeconds",
        "-SampleSeconds",
        "$SampleSeconds",
        "-WindowWidth",
        "$laneWidth",
        "-WindowHeight",
        "$laneHeight"
    )
    if ([int]$LaneDefinition.max_frame_latency -gt 0) {
        $args += @("-RequestedMaximumFrameLatency", "$($LaneDefinition.max_frame_latency)")
    }

    if ($Release) { $args += "-Release" }
    if ($StaticBevy) { $args += "-StaticBevy" }
    if ($TraceDiagnostics) { $args += "-TraceDiagnostics" }
    if ($FrameTimeDiagnostics) { $args += "-FrameTimeDiagnostics" }
    if ($LaneDefinition.disable_clouds) { $args += "-DisableClouds" }
    if ($LaneDefinition.disable_solari) { $args += "-DisableSolari" }
    if ($LaneDefinition.disable_meshlets) { $args += "-DisableMeshlets" }
    if ($LaneDefinition.disable_fps_overlay) { $args += "-DisableFpsOverlay" }

    return $args
}

function Write-Dx12ParityMarkdown {
    param(
        [string]$Path,
        [System.Collections.IDictionary]$Summary
    )

    $lines = New-Object "System.Collections.Generic.List[string]"
    $lines.Add("# DX12 Parity Benchmark Matrix") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("- Created: $($Summary.created_at)") | Out-Null
    $lines.Add("- Git: $($Summary.git.commit)") | Out-Null
    $lines.Add("- Matrix size: $($Summary.matrix_size)") | Out-Null
    $lines.Add("- Plan only: $($Summary.plan_only)") | Out-Null
    $lines.Add("- Required metrics: $($Summary.required_metrics -join ', ')") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("| lane | status | backend | present | max latency | cef mode | cef transport | fps mean | frame p95 ns | present p95 ns | summary |") | Out-Null
    $lines.Add("|---|---|---|---|---:|---|---|---:|---:|---:|---|") | Out-Null
    foreach ($lane in $Summary.lanes) {
        $metrics = $lane.key_metrics
        $fpsMean = if ($null -ne $metrics.fps) { $metrics.fps.mean } else { "n/a" }
        $frameP95 = if ($null -ne $metrics.frame_ns) { $metrics.frame_ns.p95 } else { "n/a" }
        $presentP95 = if ($null -ne $metrics.present_wait_ns) { $metrics.present_wait_ns.p95 } else { "n/a" }
        $summaryPath = if ([string]::IsNullOrWhiteSpace($lane.summary_json)) { "n/a" } else { $lane.summary_json }
        $lines.Add("| $($lane.name) | $($lane.status) | $($lane.render_backend) | $($lane.present_mode) | $($lane.max_frame_latency) | $($lane.cef_ui_mode) | $($lane.cef_paint_transport) | $fpsMean | $frameP95 | $presentP95 | $summaryPath |") | Out-Null
    }
    if ($null -ne $Summary.dx12_present_recommendations) {
        $lines.Add("") | Out-Null
        $lines.Add("## DX12 Present Recommendations") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("- Maximum FPS: $($Summary.dx12_present_recommendations.maximum_fps)") | Out-Null
        $lines.Add("- Best p95: $($Summary.dx12_present_recommendations.best_p95)") | Out-Null
        $lines.Add("- Lowest present wait: $($Summary.dx12_present_recommendations.lowest_present_wait)") | Out-Null
        $lines.Add("- Default decision: $($Summary.dx12_present_recommendations.default_decision)") | Out-Null
    }
    $lines.Add("") | Out-Null
    $lines.Add("DX12 PIX/GPUView-only counters are listed in `dx12_external_metrics` and are not inferred from client logs.") | Out-Null
    Set-Content -Path $Path -Value $lines -Encoding UTF8
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Normalize-WorkspacePath ((Resolve-Path (Join-Path $scriptRoot "..")).Path)
$timestamp = Get-Date -Format "yyyyMMdd-HHmmss-fff"
$matrixRoot = if ([string]::IsNullOrWhiteSpace($OutputRoot)) {
    Join-Path $repoRoot "target\benchmarks\dx12_parity\$timestamp"
}
elseif ([System.IO.Path]::IsPathRooted($OutputRoot)) {
    Normalize-WorkspacePath $OutputRoot
}
else {
    Normalize-WorkspacePath (Join-Path $repoRoot $OutputRoot)
}
New-Item -ItemType Directory -Force -Path $matrixRoot | Out-Null

$benchmarkClientPath = Join-Path $scriptRoot "benchmark_client.ps1"
$baseLanes = @(Get-Dx12ParityLaneDefinitions)
$presentLanes = @(Get-Dx12PresentLaneDefinitions)
$allLanes = if ($MatrixSize -eq "present" -or ($Lane | Where-Object { $_ -like "present_*" }).Count -gt 0) {
    @($baseLanes + $presentLanes)
}
else {
    $baseLanes
}
$selectedNames = if ($Lane.Count -gt 0) {
    @($Lane)
}
elseif ($MatrixSize -eq "present") {
    @($presentLanes | ForEach-Object { $_.name })
}
elseif ($MatrixSize -eq "quick") {
    @(Get-QuickLaneNames)
}
else {
    @($allLanes | ForEach-Object { $_.name })
}
$selectedLanes = @($allLanes | Where-Object { $selectedNames -contains $_.name })
$missingLanes = @($selectedNames | Where-Object { $name = $_; ($allLanes | Where-Object { $_.name -eq $name }).Count -eq 0 })
if ($missingLanes.Count -gt 0) {
    throw "Unknown dx12 parity lane(s): $($missingLanes -join ', ')"
}

$requiredMetrics = @(Get-RequiredMetricNames)
$laneResults = New-Object "System.Collections.Generic.List[object]"
$powerShellPath = (Get-Process -Id $PID).Path
if ([string]::IsNullOrWhiteSpace($powerShellPath)) {
    $powerShellPath = "powershell"
}

foreach ($laneDefinition in $selectedLanes) {
    $args = @(Convert-LaneToBenchmarkArgs -LaneDefinition $laneDefinition -BenchmarkClientPath $benchmarkClientPath)
    $status = if ($PlanOnly) { "planned" } else { "pending" }
    $summaryJson = ""
    $stdoutTail = @()
    $summary = $null
    $exitCode = $null

    if (-not $PlanOnly) {
        $previousEditorPreview = $env:FUN_BENCH_EDITOR_PREVIEW
        [System.Environment]::SetEnvironmentVariable("FUN_BENCH_EDITOR_PREVIEW", $laneDefinition.editor_preview, "Process")
        try {
            Write-Host "Running dx12_parity lane $($laneDefinition.name)..."
            $output = @(& $powerShellPath @args 2>&1)
            $exitCode = $LASTEXITCODE
            $stdoutTail = @($output | Select-Object -Last 80)
            $jsonLine = @($output | Select-String -Pattern "^Benchmark JSON:\s*(?<path>.+)$" | Select-Object -Last 1)
            if ($jsonLine.Count -gt 0) {
                $summaryJson = $jsonLine[0].Matches[0].Groups["path"].Value.Trim()
            }
            if ($exitCode -eq 0 -and -not [string]::IsNullOrWhiteSpace($summaryJson) -and (Test-Path $summaryJson)) {
                $summary = Get-Content -Path $summaryJson -Raw | ConvertFrom-Json
                $status = "passed"
            }
            else {
                $status = "failed"
            }
        }
        catch {
            $status = "failed"
            $stdoutTail += $_.Exception.Message
            if (-not $ContinueOnFailure) {
                $exitCode = 1
            }
        }
        finally {
            if ([string]::IsNullOrWhiteSpace($previousEditorPreview)) {
                Remove-Item Env:\FUN_BENCH_EDITOR_PREVIEW -ErrorAction SilentlyContinue
            }
            else {
                [System.Environment]::SetEnvironmentVariable("FUN_BENCH_EDITOR_PREVIEW", $previousEditorPreview, "Process")
            }
        }
    }

    $laneResults.Add([ordered]@{
        name = $laneDefinition.name
        category = $laneDefinition.category
        status = $status
        exit_code = $exitCode
        render_backend = $laneDefinition.render_backend
        present_mode = $laneDefinition.present_mode
        max_frame_latency = $laneDefinition.max_frame_latency
        window_mode = $laneDefinition.window_mode
        cef_ui_mode = $laneDefinition.cef_ui_mode
        cef_paint_transport = $laneDefinition.cef_paint_transport
        benchmark_lane = $laneDefinition.benchmark_lane
        feature_toggles = [ordered]@{
            clouds_disabled = $laneDefinition.disable_clouds
            solari_disabled = $laneDefinition.disable_solari
            meshlets_disabled = $laneDefinition.disable_meshlets
            editor_preview = $laneDefinition.editor_preview
        }
        command = @($powerShellPath) + $args
        summary_json = $summaryJson
        key_metrics = New-KeyMetricSnapshot -Summary $summary
        metric_presence = New-MetricPresence -Summary $summary -MetricNames $requiredMetrics
        dx12_external_metrics = if ($laneDefinition.render_backend -eq "dx12") { Get-Dx12ExternalMetricPlan } else { $null }
        notes = $laneDefinition.notes
        stdout_tail = $stdoutTail
    }) | Out-Null

    if (-not $PlanOnly -and $status -eq "failed" -and -not $ContinueOnFailure) {
        break
    }
}

$gitCommit = (Get-RepoGitLines -RepoRoot $repoRoot -Arguments @("rev-parse", "HEAD") | Select-Object -First 1)
$gitDirty = @(Get-RepoGitLines -RepoRoot $repoRoot -Arguments @("status", "--short"))
$matrixSummary = [ordered]@{
    schema_version = 1
    profile = "dx12_parity"
    created_at = (Get-Date).ToString("o")
    repo_root = $repoRoot
    matrix_size = $MatrixSize
    plan_only = [bool]$PlanOnly
    warmup_seconds = $WarmupSeconds
    sample_seconds = $SampleSeconds
    default_window_width = $WindowWidth
    default_window_height = $WindowHeight
    required_metrics = $requiredMetrics
    git = [ordered]@{
        commit = $gitCommit
        dirty_count = $gitDirty.Count
        dirty = $gitDirty
    }
    environment = Get-Dx12ParityEnvironment
    lanes_defined = $allLanes
    lanes = @($laneResults.ToArray())
    dx12_present_recommendations = New-Dx12PresentRecommendations -LaneResults @($laneResults.ToArray())
}

$matrixJsonPath = Join-Path $matrixRoot "matrix.json"
$matrixMarkdownPath = Join-Path $matrixRoot "summary.md"
$matrixSummary | ConvertTo-Json -Depth 12 | Set-Content -Path $matrixJsonPath -Encoding UTF8
Write-Dx12ParityMarkdown -Path $matrixMarkdownPath -Summary $matrixSummary

Write-Host "DX12 parity summary: $matrixMarkdownPath"
Write-Host "DX12 parity JSON: $matrixJsonPath"

$failed = @($laneResults | Where-Object { $_.status -eq "failed" })
if ($failed.Count -gt 0 -and -not $ContinueOnFailure) {
    throw "DX12 parity matrix failed in lane(s): $($failed.name -join ', ')"
}
