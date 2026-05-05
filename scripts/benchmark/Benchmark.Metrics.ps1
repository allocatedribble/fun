$ErrorActionPreference = "Stop"

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

function Get-SummaryMetricValue {
    param(
        [System.Collections.IDictionary]$Summary,
        [string]$Name,
        [string]$Field = "p95"
    )

    if ($null -eq $Summary -or -not $Summary.Contains("metrics")) {
        return $null
    }
    $metrics = $Summary["metrics"]
    $entry = if ($metrics -is [System.Collections.IDictionary] -and $metrics.Contains($Name)) {
        $metrics[$Name]
    }
    else {
        $property = $metrics.PSObject.Properties[$Name]
        if ($null -ne $property) { $property.Value } else { $null }
    }
    if ($null -eq $entry) {
        return $null
    }
    if ($entry -is [System.Collections.IDictionary] -and $entry.Contains($Field)) {
        return $entry[$Field]
    }
    $fieldProperty = $entry.PSObject.Properties[$Field]
    if ($null -eq $fieldProperty) {
        return $null
    }
    return $fieldProperty.Value
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

function Get-BenchmarkEnvUInt64 {
    param([string]$Name)

    $value = [System.Environment]::GetEnvironmentVariable($Name, "Process")
    if ([string]::IsNullOrWhiteSpace($value)) {
        return $null
    }
    $parsed = 0UL
    if ([UInt64]::TryParse($value, [ref]$parsed)) {
        return $parsed
    }
    return $null
}

function Get-AdapterRamBytes {
    try {
        $adapter = @(Get-CimInstance Win32_VideoController | Select-Object -First 1)
        if ($adapter.Count -eq 0 -or $null -eq $adapter[0].AdapterRAM) {
            return $null
        }
        return [UInt64]$adapter[0].AdapterRAM
    }
    catch {
        return $null
    }
}

function Get-Dx12MemoryBudgetInfo {
    $localBudget = Get-BenchmarkEnvUInt64 -Name "FUN_BENCH_DX12_LOCAL_BUDGET_BYTES"
    $localUsage = Get-BenchmarkEnvUInt64 -Name "FUN_BENCH_DX12_LOCAL_USAGE_BYTES"
    $availableForReservation = Get-BenchmarkEnvUInt64 -Name "FUN_BENCH_DX12_LOCAL_AVAILABLE_FOR_RESERVATION_BYTES"
    $currentReservation = Get-BenchmarkEnvUInt64 -Name "FUN_BENCH_DX12_LOCAL_CURRENT_RESERVATION_BYTES"
    $adapterRam = Get-AdapterRamBytes
    $hasBudgetSample = $null -ne $localBudget -or $null -ne $localUsage -or $null -ne $availableForReservation -or $null -ne $currentReservation
    $status = if ($hasBudgetSample) {
        "provided"
    }
    elseif ($null -ne $adapterRam) {
        "adapter_ram_only"
    }
    else {
        "not_collected"
    }
    $source = if ($hasBudgetSample) {
        "env_or_native_collector"
    }
    elseif ($null -ne $adapterRam) {
        "win32_video_controller_adapter_ram"
    }
    else {
        "none"
    }

    return [ordered]@{
        status = $status
        source = $source
        local_budget_bytes = $localBudget
        local_usage_bytes = $localUsage
        local_available_for_reservation_bytes = $availableForReservation
        local_current_reservation_bytes = $currentReservation
        adapter_ram_bytes = $adapterRam
    }
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
