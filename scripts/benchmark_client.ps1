param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$DisableDlssRr,
    [switch]$DisableSolari,
    [switch]$DisableMeshlets,
    [switch]$TraceDiagnostics,
    [string]$SolariDenoiseMode = "balanced",
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

        $top = [regex]::Match($line, "\[client perf\] top render timings (?<payload>.*)$")
        if ($top.Success) {
            Add-KeyValueMetrics -Sample $current -Payload $top.Groups["payload"].Value -Prefix "top_render_" -Milliseconds
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
    $lines.Add("- Sample count: $($Summary.samples.count)") | Out-Null
    $lines.Add("") | Out-Null

    $primaryMetrics = @(
        "fps",
        "frame_ns",
        "frame_ms",
        "solari_gpu_ns",
        "meshlet_visibility_gpu_ns",
        "dlss_rr_gpu_ns",
        "solari_pass_direct_ns",
        "solari_pass_diffuse_ns",
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
        if ($TraceDiagnostics) { $runStackArgs += "-TraceDiagnostics" }
        if (-not [string]::IsNullOrWhiteSpace($SolariDenoiseMode)) {
            $runStackArgs += @("-SolariDenoiseMode", $SolariDenoiseMode)
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
            profile = if ($Release) { "release" } else { "debug" }
            static_bevy = [bool]$StaticBevy
            render_backend = $RenderBackend
            present_mode = $PresentMode
            disable_dlss_rr = [bool]$DisableDlssRr
            disable_solari = [bool]$DisableSolari
            disable_meshlets = [bool]$DisableMeshlets
            solari_denoise_mode = if ([string]::IsNullOrWhiteSpace($SolariDenoiseMode)) { "balanced" } else { $SolariDenoiseMode }
        }
        samples = [ordered]@{
            count = $samples.Count
            warmup_seconds = if ([string]::IsNullOrWhiteSpace($InputLog)) { $WarmupSeconds } else { 0 }
            sample_seconds = if ([string]::IsNullOrWhiteSpace($InputLog)) { $SampleSeconds } else { 0 }
            input_log_mode = -not [string]::IsNullOrWhiteSpace($InputLog)
        }
        metrics = $stats
        comparison = $comparison
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
