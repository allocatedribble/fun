param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$EnableDx12DlssRr,
    [switch]$RequireDx12DlssRrAcceptance,
    [int]$RrStressFrameTarget = 500,
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
    [string]$BenchmarkProfile = "",
    [string]$BenchmarkScenario = "",
    [string]$BenchmarkMatrixLane = "",
    [ValidateSet("disabled", "hidden", "static", "animated", "animated_1440p_surface", "animated_4k_surface")]
    [string]$CefUiMode = "disabled",
    [ValidateSet("default", "cpu", "auto", "d3d11on12")]
    [string]$CefPaintTransport = "default",
    [int]$RequestedMaximumFrameLatency = 0,
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
    [string]$RenderBackend = "dx12",
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

function Set-BenchmarkProcessEnv {
    param(
        [string]$Name,
        [string]$Value
    )

    if ([string]::IsNullOrWhiteSpace($Value)) {
        Remove-Item "Env:\$Name" -ErrorAction SilentlyContinue
        return
    }

    [System.Environment]::SetEnvironmentVariable($Name, $Value, "Process")
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

function Parse-RenderPresentationLog {
    param([string[]]$Lines)

    $result = [ordered]@{
        status = "not_found"
        startup_status = "not_found"
        surface_status = "not_found"
        backend = $null
        adapter = $null
        driver = $null
        present_mode = $null
        desired_maximum_frame_latency = $null
        vrr_detected = $null
        hdr_active = $null
        swapchain_format = $null
        window_mode = $null
        resolution_width = $null
        resolution_height = $null
        surface_requested_present_mode = $null
        surface_selected_present_mode = $null
        surface_available_present_modes = $null
    }

    foreach ($line in $Lines) {
        $presentation = [regex]::Match($line, "\[fun render\] presentation: (?<payload>.*)$")
        if ($presentation.Success) {
            $parsed = ConvertTo-KeyValueObject -Payload $presentation.Groups["payload"].Value -HashKey "presentation_hash"
            foreach ($property in $parsed.GetEnumerator()) {
                $result[$property.Key] = $property.Value
            }
            $result["startup_status"] = "found"
            $result["status"] = "found"
            continue
        }

        $surface = [regex]::Match($line, "\[bevy render\] surface present mode requested (?<requested>[^;]+); selected (?<selected>[^;]+); available (?<available>.+)$")
        if ($surface.Success) {
            $result["surface_requested_present_mode"] = $surface.Groups["requested"].Value.Trim()
            $result["surface_selected_present_mode"] = $surface.Groups["selected"].Value.Trim()
            $result["surface_available_present_modes"] = $surface.Groups["available"].Value.Trim()
            $result["surface_status"] = "found"
            $result["status"] = "found"
            continue
        }

        $surfaceConfig = [regex]::Match($line, "\[bevy render\] surface config: format (?<format>[^;]+); width (?<width>\d+); height (?<height>\d+); present_mode (?<present>[^;]+); desired_maximum_frame_latency (?<latency>\d+)")
        if ($surfaceConfig.Success) {
            $result["swapchain_format"] = $surfaceConfig.Groups["format"].Value.Trim()
            $result["resolution_width"] = $surfaceConfig.Groups["width"].Value.Trim()
            $result["resolution_height"] = $surfaceConfig.Groups["height"].Value.Trim()
            $result["surface_selected_present_mode"] = $surfaceConfig.Groups["present"].Value.Trim()
            $result["desired_maximum_frame_latency"] = $surfaceConfig.Groups["latency"].Value.Trim()
            $result["surface_status"] = "found"
            $result["status"] = "found"
        }
    }

    return $result
}

function Parse-RenderUploadCallsitesLog {
    param([string[]]$Lines)

    $callsites = [ordered]@{}
    foreach ($line in $Lines) {
        $match = [regex]::Match($line, "\[client perf\] render upload top: rank=(?<rank>\d+) operation=(?<operation>\S+) label=(?<label>\S+) calls=(?<calls>\d+) bytes=(?<bytes>\d+)")
        if (-not $match.Success) {
            continue
        }
        $operation = $match.Groups["operation"].Value
        $label = $match.Groups["label"].Value
        $key = "$operation`n$label"
        if (-not $callsites.Contains($key)) {
            $callsites[$key] = [ordered]@{
                operation = $operation
                label = $label
                calls = 0
                bytes = 0
                samples = 0
            }
        }
        $entry = $callsites[$key]
        $entry.calls = [uint64]$entry.calls + [uint64]$match.Groups["calls"].Value
        $entry.bytes = [uint64]$entry.bytes + [uint64]$match.Groups["bytes"].Value
        $entry.samples = [uint64]$entry.samples + 1
    }

    $rank = 0
    return @(
        $callsites.Values |
            Sort-Object -Property @{ Expression = { [uint64]$_.bytes }; Descending = $true }, @{ Expression = { [uint64]$_.calls }; Descending = $true }, label |
            Select-Object -First 10 |
            ForEach-Object {
                $rank += 1
                [ordered]@{
                    rank = $rank
                    operation = $_.operation
                    label = $_.label
                    calls = $_.calls
                    bytes = $_.bytes
                    samples = $_.samples
                }
            }
    )
}

function Parse-ClientPerfLog {
    param([string[]]$Lines)

    $samples = New-Object "System.Collections.Generic.List[object]"
    $current = $null
    $pendingCefUiMetrics = $null

    foreach ($line in $Lines) {
        $main = [regex]::Match($line, "\[client perf\] fps=(?<fps>\S+) frame_ms=(?<frame_ms>\S+) solari_gpu_ms=(?<solari>\S+) meshlet_visibility_gpu_ms=(?<meshlet>\S+) dlss_rr_gpu_ms=(?<dlss>\S+)")
        if ($main.Success) {
            $current = [ordered]@{}
            Add-Metric -Sample $current -Name "fps" -Text $main.Groups["fps"].Value
            Add-Metric -Sample $current -Name "frame_ms" -Text $main.Groups["frame_ms"].Value -Milliseconds
            Add-Metric -Sample $current -Name "solari_gpu_ms" -Text $main.Groups["solari"].Value -Milliseconds
            Add-Metric -Sample $current -Name "meshlet_visibility_gpu_ms" -Text $main.Groups["meshlet"].Value -Milliseconds
            Add-Metric -Sample $current -Name "dlss_rr_gpu_ms" -Text $main.Groups["dlss"].Value -Milliseconds
            if ($null -ne $pendingCefUiMetrics) {
                foreach ($key in $pendingCefUiMetrics.Keys) {
                    $current[$key] = $pendingCefUiMetrics[$key]
                }
                $pendingCefUiMetrics = $null
            }
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

        $renderUploads = [regex]::Match($line, "\[client perf\] render uploads: (?<payload>.*)$")
        if ($renderUploads.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $renderUploads.Groups["payload"].Value -Prefix "render_upload_"
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

        $cefUi = [regex]::Match($line, "\[client perf\] cef_ui transport: (?<payload>.*)$")
        if ($cefUi.Success) {
            if ($null -eq $current) {
                $pendingCefUiMetrics = [ordered]@{}
                Add-KeyValueMetrics -Sample $pendingCefUiMetrics -Payload $cefUi.Groups["payload"].Value -Prefix ""
            }
            else {
                Add-KeyValueMetrics -Sample $current -Payload $cefUi.Groups["payload"].Value -Prefix ""
            }
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
            p99 = Round-Metric -Value (Get-Percentile -Values $values -Percentile 99) -MetricName $metricName
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

function Get-BenchmarkEnvValue {
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

function Get-ActivePowerScheme {
    try {
        $line = (& powercfg /getactivescheme 2>$null | Select-Object -First 1)
        if ([string]::IsNullOrWhiteSpace($line)) {
            return [ordered]@{ status = "not_found" }
        }
        $match = [regex]::Match($line, "Power Scheme GUID:\s*(?<guid>[A-Fa-f0-9-]+)\s*\((?<name>[^)]+)\)")
        if ($match.Success) {
            return [ordered]@{
                status = "found"
                guid = $match.Groups["guid"].Value
                name = $match.Groups["name"].Value
            }
        }
        return [ordered]@{
            status = "unparsed"
            raw = $line
        }
    }
    catch {
        return [ordered]@{ status = "not_collected" }
    }
}

function Get-ChassisClass {
    try {
        $chassis = @(Get-CimInstance Win32_SystemEnclosure | ForEach-Object { $_.ChassisTypes } | ForEach-Object { $_ })
        $laptopTypes = @(8, 9, 10, 14, 30, 31, 32)
        if (($chassis | Where-Object { $laptopTypes -contains [int]$_ }).Count -gt 0) {
            return "laptop_or_portable"
        }
        if ($chassis.Count -gt 0) {
            return "desktop_or_workstation"
        }
    }
    catch {
    }
    return "not_collected"
}

function Test-CommandAvailable {
    param([string]$Name)

    return $null -ne (Get-Command $Name -ErrorAction SilentlyContinue)
}

function Get-BenchmarkEnvironmentInfo {
    $operatingSystem = $null
    try {
        $operatingSystem = Get-CimInstance Win32_OperatingSystem |
            Select-Object Caption, Version, BuildNumber
    }
    catch {
        $operatingSystem = $null
    }

    $gpuCount = 0
    try {
        $gpuCount = @(Get-CimInstance Win32_VideoController).Count
    }
    catch {
        $gpuCount = 0
    }

    return [ordered]@{
        windows = [ordered]@{
            os = $operatingSystem
            build = if ($null -ne $operatingSystem) { $operatingSystem.BuildNumber } else { "not_collected" }
            hags_state = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_HAGS"
            hdr_state = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_HDR"
            vrr_state = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_VRR"
            rebar_state = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_REBAR"
        }
        machine = [ordered]@{
            chassis_class = Get-ChassisClass
            gpu_adapter_count = $gpuCount
            hybrid_graphics_state = if ($gpuCount -gt 1) { "multiple_adapters_observed" } elseif ($gpuCount -eq 1) { "single_adapter_observed" } else { "not_collected" }
            power_profile = Get-ActivePowerScheme
        }
        display = [ordered]@{
            monitor_refresh_hz = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_MONITOR_REFRESH_HZ"
            monitor_vrr_state = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_MONITOR_VRR"
        }
        tools = [ordered]@{
            presentmon_available = Test-CommandAvailable -Name "PresentMon"
            pix_attached = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_PIX_ATTACHED" -Default "false"
            renderdoc_attached = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_RENDERDOC_ATTACHED" -Default "false"
        }
        overlays = [ordered]@{
            status = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_OVERLAYS"
            steam = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_OVERLAY_STEAM"
            discord = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_OVERLAY_DISCORD"
            geforce_experience = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_OVERLAY_GFE"
            amd = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_OVERLAY_AMD"
            xbox_game_bar = Get-BenchmarkEnvValue -Name "FUN_BENCH_ENV_OVERLAY_XBOX_GAME_BAR"
        }
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

function Test-StatsMetric {
    param(
        [System.Collections.IDictionary]$Stats,
        [string]$Metric
    )

    return $null -ne $Stats -and $Stats.Contains($Metric)
}

function Get-StatsMetricValue {
    param(
        [System.Collections.IDictionary]$Stats,
        [string]$Metric,
        [string]$Field
    )

    if (-not (Test-StatsMetric -Stats $Stats -Metric $Metric)) {
        return $null
    }

    $entry = $Stats[$Metric]
    if ($entry -is [System.Collections.IDictionary]) {
        if ($entry.Contains($Field)) {
            return $entry[$Field]
        }
        return $null
    }

    $property = $entry.PSObject.Properties[$Field]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function New-Dx12DlssRrAcceptance {
    param(
        [System.Collections.IDictionary]$Stats,
        [switch]$Required,
        [switch]$EnableDx12DlssRr,
        [switch]$DisableDlssRr,
        [string]$SolariDenoiseMode,
        [int]$SampleSeconds,
        [switch]$InputLogMode,
        [int]$StressFrameTarget
    )

    $failures = [System.Collections.Generic.List[string]]::new()
    $requiredMetrics = [ordered]@{}
    foreach ($metric in @(
            "dlss_rr_gpu_ns",
            "solari_pass_dlss_rr_guide_resolve_ns",
            "frame_ns"
        )) {
        $present = Test-StatsMetric -Stats $Stats -Metric $metric
        $requiredMetrics[$metric] = [ordered]@{
            present = $present
            count = Get-StatsMetricValue -Stats $Stats -Metric $metric -Field "count"
            mean = Get-StatsMetricValue -Stats $Stats -Metric $metric -Field "mean"
            p95 = Get-StatsMetricValue -Stats $Stats -Metric $metric -Field "p95"
        }
        if ($Required -and -not $present) {
            $failures.Add("missing_metric:$metric") | Out-Null
        }
        if ($Required -and $present) {
            if ($null -eq $requiredMetrics[$metric]["mean"]) {
                $failures.Add("missing_metric:$metric.mean") | Out-Null
            }
            if ($null -eq $requiredMetrics[$metric]["p95"]) {
                $failures.Add("missing_metric:$metric.p95") | Out-Null
            }
        }
    }

    if ($Required -and -not (Test-StatsMetric -Stats $Stats -Metric "frame_ns")) {
        $failures.Add("missing_metric:frame_ns.mean") | Out-Null
        $failures.Add("missing_metric:frame_ns.p95") | Out-Null
    }

    $fpsMean = Get-StatsMetricValue -Stats $Stats -Metric "fps" -Field "mean"
    $estimatedFrames = 0.0
    if ($null -ne $fpsMean -and -not $InputLogMode -and $SampleSeconds -gt 0) {
        $estimatedFrames = [double]$fpsMean * [double]$SampleSeconds
    }

    if ($Required) {
        if (-not $EnableDx12DlssRr) {
            $failures.Add("rr_gate_not_enabled") | Out-Null
        }
        if ($DisableDlssRr) {
            $failures.Add("rr_kill_switch_enabled") | Out-Null
        }
        if ($SolariDenoiseMode -notin @("rr", "dlss", "dlss-rr", "dlss_rr", "ray-reconstruction")) {
            $failures.Add("solari_rr_denoise_mode_not_selected") | Out-Null
        }
        if ($InputLogMode) {
            $failures.Add("input_log_cannot_prove_live_500_frame_stress") | Out-Null
        }
        if ($estimatedFrames -lt $StressFrameTarget) {
            $failures.Add("stress_frame_estimate_below_target") | Out-Null
        }
    }

    return [ordered]@{
        schema_version = 1
        required = [bool]$Required
        pass = $failures.Count -eq 0
        failures = @($failures.ToArray())
        stress_frame_target = $StressFrameTarget
        estimated_frames = [Math]::Round($estimatedFrames, 0)
        required_metrics = $requiredMetrics
    }
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
    if (-not [string]::IsNullOrWhiteSpace($Summary.config.benchmark_profile)) {
        $lines.Add("- Benchmark profile: $($Summary.config.benchmark_profile)") | Out-Null
    }
    if (-not [string]::IsNullOrWhiteSpace($Summary.config.benchmark_matrix_lane)) {
        $lines.Add("- Matrix lane: $($Summary.config.benchmark_matrix_lane)") | Out-Null
    }
    $lines.Add("- Backend: $($Summary.config.render_backend)") | Out-Null
    $lines.Add("- Present mode: $($Summary.config.present_mode)") | Out-Null
    $startupLatency = if ($null -ne $Summary.render_presentation) { $Summary.render_presentation.desired_maximum_frame_latency } else { "n/a" }
    $surfacePresent = if ($null -ne $Summary.render_presentation) { $Summary.render_presentation.surface_selected_present_mode } else { "n/a" }
    $swapchainFormat = if ($null -ne $Summary.render_presentation) { $Summary.render_presentation.swapchain_format } else { "n/a" }
    $lines.Add("- Max frame latency: requested=$($Summary.config.requested_maximum_frame_latency) startup=$startupLatency") | Out-Null
    $lines.Add("- Surface present: selected=$surfacePresent swapchain_format=$swapchainFormat") | Out-Null
    $lines.Add("- CEF UI: mode=$($Summary.config.cef_ui_mode) transport=$($Summary.config.cef_paint_transport) enabled=$($Summary.config.cef_ui_enabled)") | Out-Null
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
        "cloud_history_reject_rate",
        "cloud_history_reset_count",
        "cloud_history_average_age",
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
        "cef_on_paint_fps",
        "cef_on_accelerated_paint_fps",
        "cef_cpu_upload_bytes",
        "cef_gpu_copy_bytes",
        "cef_gpu_copy_ns",
        "cef_gpu_copy_failures",
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
        "dlss_rr_gpu_ns",
        "solari_pass_dlss_rr_guide_resolve_ns",
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
    $lines.Add("| metric | mean | p50 | p95 | p99 | min | max | samples |") | Out-Null
    $lines.Add("|---|---:|---:|---:|---:|---:|---:|---:|") | Out-Null
    foreach ($metric in $primaryMetrics) {
        if ($stats.Contains($metric)) {
            $row = "| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} |" -f `
                $metric, `
                (Format-StatValue -Stats $stats -Metric $metric -Field "mean"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "p50"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "p95"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "p99"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "min"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "max"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "count")
            $lines.Add($row) | Out-Null
        }
    }
    if ($null -ne $Summary.render_upload_callsites -and $Summary.render_upload_callsites.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Render Upload Top Callsites") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | operation | label | calls | bytes | samples |") | Out-Null
        $lines.Add("|---:|---|---|---:|---:|---:|") | Out-Null
        foreach ($callsite in $Summary.render_upload_callsites) {
            $lines.Add("| $($callsite.rank) | $($callsite.operation) | $($callsite.label) | $($callsite.calls) | $($callsite.bytes) | $($callsite.samples) |") | Out-Null
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

    $rrAcceptance = $Summary.rr_acceptance
    if ($null -ne $rrAcceptance -and $rrAcceptance.required) {
        $lines.Add("") | Out-Null
        $lines.Add("## DX12 DLSS RR Acceptance") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("- Pass: $($rrAcceptance.pass)") | Out-Null
        $lines.Add("- Estimated stress frames: $($rrAcceptance.estimated_frames) / $($rrAcceptance.stress_frame_target)") | Out-Null
        if ($rrAcceptance.failures.Count -gt 0) {
            $lines.Add("- Failures: $($rrAcceptance.failures -join ', ')") | Out-Null
        }
        else {
            $lines.Add("- Failures: none") | Out-Null
        }
        $lines.Add("") | Out-Null
        $lines.Add("| required metric | present | mean | p95 | samples |") | Out-Null
        $lines.Add("|---|---|---:|---:|---:|") | Out-Null
        foreach ($metric in @("dlss_rr_gpu_ns", "solari_pass_dlss_rr_guide_resolve_ns", "frame_ns")) {
            $entry = $rrAcceptance.required_metrics[$metric]
            $lines.Add("| $metric | $($entry["present"]) | $($entry["mean"]) | $($entry["p95"]) | $($entry["count"]) |") | Out-Null
        }
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

$cefUiEnabled = $CefUiMode -ne "disabled" -and $CefUiMode -ne "hidden"
if ($CefUiMode -eq "animated_1440p_surface") {
    if ($WindowWidth -le 0) { $WindowWidth = 2560 }
    if ($WindowHeight -le 0) { $WindowHeight = 1440 }
}
elseif ($CefUiMode -eq "animated_4k_surface") {
    if ($WindowWidth -le 0) { $WindowWidth = 3840 }
    if ($WindowHeight -le 0) { $WindowHeight = 2160 }
}
$cefAcceleratedFeatureRequested = $cefUiEnabled -and ($CefPaintTransport -eq "auto" -or $CefPaintTransport -eq "d3d11on12")

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
        Set-BenchmarkProcessEnv -Name "FUN_BENCHMARK_PROFILE" -Value $BenchmarkProfile
        Set-BenchmarkProcessEnv -Name "FUN_BENCHMARK_SCENARIO" -Value $BenchmarkScenario
        Set-BenchmarkProcessEnv -Name "FUN_BENCHMARK_MATRIX_LANE" -Value $BenchmarkMatrixLane
        Set-BenchmarkProcessEnv -Name "FUN_CEF_UI_BENCHMARK_MODE" -Value $CefUiMode
        if ($CefPaintTransport -eq "default") {
            Set-BenchmarkProcessEnv -Name "FUN_CEF_UI_PAINT_TRANSPORT" -Value ""
            Set-BenchmarkProcessEnv -Name "FUN_CEF_UI_ACCELERATED_PAINT" -Value ""
        }
        else {
            Set-BenchmarkProcessEnv -Name "FUN_CEF_UI_PAINT_TRANSPORT" -Value $CefPaintTransport
            $acceleratedPaintValue = switch ($CefPaintTransport) {
                "cpu" { "0" }
                "auto" { "auto" }
                "d3d11on12" { "1" }
                default { "" }
            }
            Set-BenchmarkProcessEnv -Name "FUN_CEF_UI_ACCELERATED_PAINT" -Value $acceleratedPaintValue
        }
        if ($RequestedMaximumFrameLatency -gt 0) {
            Set-BenchmarkProcessEnv -Name "FUN_PRESENT_MAX_FRAME_LATENCY" -Value ([string]$RequestedMaximumFrameLatency)
            Set-BenchmarkProcessEnv -Name "FUN_RENDER_MAX_FRAME_LATENCY" -Value ([string]$RequestedMaximumFrameLatency)
            $runStackArgs += @("-RenderMaxFrameLatency", "$RequestedMaximumFrameLatency")
        }
        else {
            Set-BenchmarkProcessEnv -Name "FUN_PRESENT_MAX_FRAME_LATENCY" -Value ""
            Set-BenchmarkProcessEnv -Name "FUN_RENDER_MAX_FRAME_LATENCY" -Value ""
        }
        if ($Release) { $runStackArgs += "-Release" }
        if ($StaticBevy) { $runStackArgs += "-StaticBevy" }
        if ($cefUiEnabled) { $runStackArgs += "-CefUi" }
        if ($cefAcceleratedFeatureRequested) { $runStackArgs += "-CefUiDx12AcceleratedPaint" }
        if ($EnableDx12DlssRr) { $runStackArgs += "-EnableDx12DlssRr" }
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
    $renderPresentation = Parse-RenderPresentationLog -Lines $allLines
    $renderUploadCallsites = Parse-RenderUploadCallsitesLog -Lines $sampleLines
    $stats = Get-SummaryStats -Samples $samples
    $comparison = New-Comparison -CurrentStats $stats -BaselinePath $baselinePath
    $rrAcceptance = New-Dx12DlssRrAcceptance `
        -Stats $stats `
        -Required:$RequireDx12DlssRrAcceptance `
        -EnableDx12DlssRr:$EnableDx12DlssRr `
        -DisableDlssRr:$DisableDlssRr `
        -SolariDenoiseMode $SolariDenoiseMode `
        -SampleSeconds $SampleSeconds `
        -InputLogMode:([bool](-not [string]::IsNullOrWhiteSpace($InputLog))) `
        -StressFrameTarget $RrStressFrameTarget
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
        environment = Get-BenchmarkEnvironmentInfo
        config = [ordered]@{
            benchmark_profile = $BenchmarkProfile
            benchmark_scenario = $BenchmarkScenario
            benchmark_matrix_lane = $BenchmarkMatrixLane
            benchmark_lane = $BenchmarkLane
            profile = if ($Release) { "release" } else { "debug" }
            static_bevy = [bool]$StaticBevy
            render_backend = $RenderBackend
            present_mode = $PresentMode
            requested_maximum_frame_latency = $RequestedMaximumFrameLatency
            cef_ui_mode = $CefUiMode
            cef_ui_enabled = [bool]$cefUiEnabled
            cef_paint_transport = $CefPaintTransport
            cef_accelerated_feature_requested = [bool]$cefAcceleratedFeatureRequested
            enable_dx12_dlss_rr = [bool]$EnableDx12DlssRr
            require_dx12_dlss_rr_acceptance = [bool]$RequireDx12DlssRrAcceptance
            rr_stress_frame_target = $RrStressFrameTarget
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
        rr_acceptance = $rrAcceptance
        render_capabilities = $renderCapabilities
        rt_feature_gates = $rtFeatureGates
        render_presentation = $renderPresentation
        render_upload_callsites = $renderUploadCallsites
    }

    $jsonPath = Join-Path $outputRoot "summary.json"
    $markdownPath = Join-Path $outputRoot "summary.md"
    $summary | ConvertTo-Json -Depth 8 | Set-Content -Path $jsonPath -Encoding UTF8
    Write-MarkdownReport -Path $markdownPath -Summary $summary

    Write-Host "Benchmark summary: $markdownPath"
    Write-Host "Benchmark JSON: $jsonPath"
    if ($RequireDx12DlssRrAcceptance -and -not $rrAcceptance.pass) {
        throw "DX12 DLSS RR acceptance failed: $($rrAcceptance.failures -join ', ')"
    }
}
finally {
    if ($ranStack -and -not $KeepRunning) {
        Stop-StackProcesses -PidFile $pidFile
    }
}
