param(
    [string]$Baseline = "",
    [string]$Current = "",
    [string]$EnvelopePath = "",
    [string]$Lane = "",
    [string]$ReportPath = "",
    [string]$JsonOut = "",
    [switch]$SelfTest,
    [switch]$FailOnWarning
)

$ErrorActionPreference = "Stop"

function Normalize-WorkspacePath {
    param([string]$Path)

    if ([string]::IsNullOrWhiteSpace($Path)) {
        return $Path
    }
    if ($Path.StartsWith("\\?\")) {
        return $Path.Substring(4)
    }
    if ($Path.StartsWith("\?\")) {
        return $Path.Substring(3)
    }
    return $Path
}

function Read-JsonFile {
    param([string]$Path)

    $resolved = Normalize-WorkspacePath ((Resolve-Path $Path).Path)
    return Get-Content -Path $resolved -Raw | ConvertFrom-Json
}

function Get-JsonProperty {
    param(
        [object]$Object,
        [string]$Name
    )

    if ($null -eq $Object) {
        return $null
    }
    $property = $Object.PSObject.Properties[$Name]
    if ($null -eq $property) {
        return $null
    }
    return $property.Value
}

function Get-MetricValue {
    param(
        [object]$Summary,
        [string]$Metric,
        [string]$Field
    )

    $metrics = Get-JsonProperty -Object $Summary -Name "metrics"
    $entry = Get-JsonProperty -Object $metrics -Name $Metric
    $value = Get-JsonProperty -Object $entry -Name $Field
    if ($null -eq $value) {
        return $null
    }
    return [double]$value
}

function Test-AcceleratedCefLane {
    param([object]$Summary)

    $config = Get-JsonProperty -Object $Summary -Name "config"
    $selection = Get-JsonProperty -Object $Summary -Name "cef_ui_transport_selection"
    $selected = Get-JsonProperty -Object $selection -Name "selected"
    $transport = if ($null -ne $selected) {
        [string]$selected
    }
    else {
        [string](Get-JsonProperty -Object $config -Name "cef_paint_transport")
    }
    $requested = Get-JsonProperty -Object $config -Name "cef_accelerated_feature_requested"
    if ($requested -is [bool] -and $requested) {
        return $true
    }
    return @(
        "auto",
        "d3d11on12",
        "d3d11_shared_texture_dx12_copy"
    ) -contains $transport.ToLowerInvariant()
}

function Get-RegressionAmount {
    param(
        [double]$BaselineValue,
        [double]$CurrentValue,
        [string]$Direction
    )

    if ($Direction -eq "higher") {
        return $BaselineValue - $CurrentValue
    }
    return $CurrentValue - $BaselineValue
}

function Get-RegressionPercent {
    param(
        [double]$BaselineValue,
        [double]$CurrentValue,
        [string]$Direction
    )

    $amount = Get-RegressionAmount -BaselineValue $BaselineValue -CurrentValue $CurrentValue -Direction $Direction
    if ([Math]::Abs($BaselineValue) -le 0.000001) {
        if ($amount -le 0.0) {
            return 0.0
        }
        return [double]::PositiveInfinity
    }
    return ($amount / [Math]::Abs($BaselineValue)) * 100.0
}

function New-RuleResult {
    param(
        [string]$Severity,
        [object]$Rule,
        [string]$Status,
        [string]$Detail,
        [nullable[double]]$BaselineValue = $null,
        [nullable[double]]$CurrentValue = $null,
        [nullable[double]]$Observed = $null
    )

    return [pscustomobject][ordered]@{
        severity = $Severity
        id = [string]$Rule.id
        metric = [string]$Rule.metric
        field = [string]$Rule.field
        status = $Status
        baseline = $BaselineValue
        current = $CurrentValue
        observed = $Observed
        limit = [double]$Rule.limit
        detail = $Detail
    }
}

function Invoke-PerfRule {
    param(
        [object]$Rule,
        [string]$Severity,
        [object]$BaselineSummary,
        [object]$CurrentSummary
    )

    if ([string]$Rule.when -eq "accelerated_cef" -and -not (Test-AcceleratedCefLane -Summary $CurrentSummary)) {
        return New-RuleResult -Severity $Severity -Rule $Rule -Status "skipped" -Detail "rule applies only to accelerated CEF lanes"
    }

    $currentValue = Get-MetricValue -Summary $CurrentSummary -Metric $Rule.metric -Field $Rule.field
    if ($null -eq $currentValue) {
        $status = if ($Severity -eq "hard") { "fail" } else { "warn" }
        return New-RuleResult -Severity $Severity -Rule $Rule -Status $status -Detail "current metric is missing"
    }

    if ([string]$Rule.comparison -eq "max_absolute") {
        if ($currentValue -gt [double]$Rule.limit) {
            $status = if ($Severity -eq "hard") { "fail" } else { "warn" }
            return New-RuleResult -Severity $Severity -Rule $Rule -Status $status -Detail ([string]$Rule.message) -CurrentValue $currentValue -Observed $currentValue
        }
        return New-RuleResult -Severity $Severity -Rule $Rule -Status "pass" -Detail "within absolute limit" -CurrentValue $currentValue -Observed $currentValue
    }

    $baselineValue = Get-MetricValue -Summary $BaselineSummary -Metric $Rule.metric -Field $Rule.field
    if ($null -eq $baselineValue) {
        $status = if ($Severity -eq "hard") { "fail" } else { "warn" }
        return New-RuleResult -Severity $Severity -Rule $Rule -Status $status -Detail "baseline metric is missing" -CurrentValue $currentValue
    }

    $direction = if ([string]::IsNullOrWhiteSpace([string]$Rule.direction)) { "lower" } else { [string]$Rule.direction }
    if ([string]$Rule.comparison -eq "max_regression_percent") {
        $observed = Get-RegressionPercent -BaselineValue $baselineValue -CurrentValue $currentValue -Direction $direction
        if ($observed -gt [double]$Rule.limit) {
            $status = if ($Severity -eq "hard") { "fail" } else { "warn" }
            return New-RuleResult -Severity $Severity -Rule $Rule -Status $status -Detail ([string]$Rule.message) -BaselineValue $baselineValue -CurrentValue $currentValue -Observed $observed
        }
        return New-RuleResult -Severity $Severity -Rule $Rule -Status "pass" -Detail "within percent envelope" -BaselineValue $baselineValue -CurrentValue $currentValue -Observed $observed
    }

    if ([string]$Rule.comparison -eq "max_regression_absolute") {
        $observed = Get-RegressionAmount -BaselineValue $baselineValue -CurrentValue $currentValue -Direction $direction
        if ($observed -gt [double]$Rule.limit) {
            $status = if ($Severity -eq "hard") { "fail" } else { "warn" }
            return New-RuleResult -Severity $Severity -Rule $Rule -Status $status -Detail ([string]$Rule.message) -BaselineValue $baselineValue -CurrentValue $currentValue -Observed $observed
        }
        return New-RuleResult -Severity $Severity -Rule $Rule -Status "pass" -Detail "within absolute regression envelope" -BaselineValue $baselineValue -CurrentValue $currentValue -Observed $observed
    }

    return New-RuleResult -Severity $Severity -Rule $Rule -Status "fail" -Detail "unknown comparison '$($Rule.comparison)'" -CurrentValue $currentValue
}

function Get-LaneEnvelope {
    param(
        [object]$Envelope,
        [string]$LaneName
    )

    $lanes = Get-JsonProperty -Object $Envelope -Name "lanes"
    $laneProperty = $lanes.PSObject.Properties[$LaneName]
    if ($null -eq $laneProperty) {
        throw "Unknown DX12 perf envelope lane '$LaneName'"
    }
    return $laneProperty.Value
}

function Invoke-PerfGate {
    param(
        [object]$BaselineSummary,
        [object]$CurrentSummary,
        [object]$Envelope,
        [string]$LaneName
    )

    $laneEnvelope = Get-LaneEnvelope -Envelope $Envelope -LaneName $LaneName
    $results = New-Object "System.Collections.Generic.List[object]"
    foreach ($rule in @($laneEnvelope.hard)) {
        $results.Add((Invoke-PerfRule -Rule $rule -Severity "hard" -BaselineSummary $BaselineSummary -CurrentSummary $CurrentSummary)) | Out-Null
    }
    foreach ($rule in @($laneEnvelope.warn)) {
        $results.Add((Invoke-PerfRule -Rule $rule -Severity "warn" -BaselineSummary $BaselineSummary -CurrentSummary $CurrentSummary)) | Out-Null
    }

    $hardFailures = @($results | Where-Object { $_.severity -eq "hard" -and $_.status -eq "fail" })
    $warnings = @($results | Where-Object { $_.status -eq "warn" })
    $status = if ($hardFailures.Count -gt 0) {
        "fail"
    }
    elseif ($warnings.Count -gt 0) {
        "warn"
    }
    else {
        "pass"
    }

    return [pscustomobject][ordered]@{
        schema = "fun.dx12_perf_regression.report.v1"
        lane = $LaneName
        status = $status
        hard_failures = $hardFailures.Count
        warnings = $warnings.Count
        results = @($results.ToArray())
    }
}

function Format-NullableNumber {
    param([object]$Value)

    if ($null -eq $Value) {
        return "n/a"
    }
    if ($Value -is [double] -and [double]::IsPositiveInfinity($Value)) {
        return "+inf"
    }
    return "{0:0.###}" -f [double]$Value
}

function New-ReportLines {
    param([object]$Report)

    $lines = New-Object "System.Collections.Generic.List[string]"
    $lines.Add("DX12 perf regression gate: lane=$($Report.lane) status=$($Report.status) hard_failures=$($Report.hard_failures) warnings=$($Report.warnings)") | Out-Null
    $interesting = @($Report.results | Where-Object { $_.status -ne "pass" -and $_.status -ne "skipped" })
    if ($interesting.Count -eq 0) {
        $lines.Add("All checked rules are inside the envelope.") | Out-Null
    }
    else {
        foreach ($result in $interesting) {
            $lines.Add(("- {0}: {1}.{2} status={3} baseline={4} current={5} observed={6} limit={7} detail={8}" -f `
                $result.severity.ToUpperInvariant(), `
                $result.metric, `
                $result.field, `
                $result.status, `
                (Format-NullableNumber -Value $result.baseline), `
                (Format-NullableNumber -Value $result.current), `
                (Format-NullableNumber -Value $result.observed), `
                (Format-NullableNumber -Value $result.limit), `
                $result.detail)) | Out-Null
        }
    }
    return @($lines.ToArray())
}

function New-SyntheticSummary {
    param(
        [double]$FpsMean,
        [double]$FrameP95,
        [double]$PresentP95,
        [double]$CefCpuUploadBytes,
        [double]$RenderPipelineCreations,
        [double]$ComputePipelineCreations,
        [double]$ShaderPipelineCreations,
        [double]$TextureUploadBytes,
        [double]$BufferUploadBytes,
        [double]$TransientTextureCreates,
        [double]$TransientBufferCreates,
        [bool]$AcceleratedCef
    )

    $payload = [ordered]@{
        config = [ordered]@{
            render_backend = "dx12"
            cef_paint_transport = if ($AcceleratedCef) { "d3d11on12" } else { "cpu" }
            cef_accelerated_feature_requested = $AcceleratedCef
        }
        metrics = [ordered]@{
            fps = [ordered]@{ mean = $FpsMean; p95 = $FpsMean }
            frame_ns = [ordered]@{ mean = $FrameP95; p95 = $FrameP95 }
            present_wait_ns = [ordered]@{ mean = $PresentP95; p95 = $PresentP95 }
            cef_cpu_upload_bytes = [ordered]@{ mean = $CefCpuUploadBytes; p95 = $CefCpuUploadBytes }
            render_churn_render_pipeline_creations = [ordered]@{ mean = $RenderPipelineCreations; p95 = $RenderPipelineCreations }
            render_churn_compute_pipeline_creations = [ordered]@{ mean = $ComputePipelineCreations; p95 = $ComputePipelineCreations }
            render_shader_pipeline_create_count = [ordered]@{ mean = $ShaderPipelineCreations; p95 = $ShaderPipelineCreations }
            render_upload_write_texture_bytes = [ordered]@{ mean = $TextureUploadBytes; p95 = $TextureUploadBytes }
            render_upload_write_buffer_bytes = [ordered]@{ mean = $BufferUploadBytes; p95 = $BufferUploadBytes }
            transient_texture_creates = [ordered]@{ mean = $TransientTextureCreates; p95 = $TransientTextureCreates }
            transient_buffer_creates = [ordered]@{ mean = $TransientBufferCreates; p95 = $TransientBufferCreates }
        }
    }
    $json = $payload | ConvertTo-Json -Depth 8
    return $json | ConvertFrom-Json
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
if ([string]::IsNullOrWhiteSpace($EnvelopePath)) {
    $EnvelopePath = Join-Path $scriptRoot "dx12_perf_baseline_envelopes.json"
}
$envelope = Read-JsonFile -Path $EnvelopePath
if ([string]::IsNullOrWhiteSpace($Lane)) {
    $Lane = [string]$envelope.default_lane
}

if ($SelfTest) {
    $baselineSummaryForSelfTest = New-SyntheticSummary -FpsMean 100.0 -FrameP95 10000000.0 -PresentP95 100000.0 -CefCpuUploadBytes 0.0 -RenderPipelineCreations 0.0 -ComputePipelineCreations 0.0 -ShaderPipelineCreations 0.0 -TextureUploadBytes 0.0 -BufferUploadBytes 0.0 -TransientTextureCreates 1.0 -TransientBufferCreates 1.0 -AcceleratedCef $true
    $warning = New-SyntheticSummary -FpsMean 96.0 -FrameP95 10000000.0 -PresentP95 110000.0 -CefCpuUploadBytes 0.0 -RenderPipelineCreations 0.0 -ComputePipelineCreations 0.0 -ShaderPipelineCreations 0.0 -TextureUploadBytes 0.0 -BufferUploadBytes 0.0 -TransientTextureCreates 2.0 -TransientBufferCreates 1.0 -AcceleratedCef $true
    $failure = New-SyntheticSummary -FpsMean 96.0 -FrameP95 11500000.0 -PresentP95 110000.0 -CefCpuUploadBytes 4096.0 -RenderPipelineCreations 1.0 -ComputePipelineCreations 0.0 -ShaderPipelineCreations 1.0 -TextureUploadBytes 0.0 -BufferUploadBytes 0.0 -TransientTextureCreates 1.0 -TransientBufferCreates 1.0 -AcceleratedCef $true

    $warningReport = Invoke-PerfGate -BaselineSummary $baselineSummaryForSelfTest -CurrentSummary $warning -Envelope $envelope -LaneName $Lane
    if ($warningReport.status -ne "warn" -or $warningReport.hard_failures -ne 0 -or $warningReport.warnings -lt 1) {
        throw "Self-test warning scenario failed: status=$($warningReport.status) hard=$($warningReport.hard_failures) warnings=$($warningReport.warnings)"
    }
    $failureReport = Invoke-PerfGate -BaselineSummary $baselineSummaryForSelfTest -CurrentSummary $failure -Envelope $envelope -LaneName $Lane
    if ($failureReport.status -ne "fail" -or $failureReport.hard_failures -lt 3) {
        throw "Self-test failure scenario failed: status=$($failureReport.status) hard=$($failureReport.hard_failures)"
    }
    Write-Output "DX12 perf regression gate self-test passed."
    exit 0
}

if ([string]::IsNullOrWhiteSpace($Baseline) -or [string]::IsNullOrWhiteSpace($Current)) {
    throw "-Baseline and -Current summary.json paths are required unless -SelfTest is used."
}

$baselineSummary = Read-JsonFile -Path $Baseline
$currentSummary = Read-JsonFile -Path $Current
$report = Invoke-PerfGate -BaselineSummary $baselineSummary -CurrentSummary $currentSummary -Envelope $envelope -LaneName $Lane
$lines = New-ReportLines -Report $report
$lines | ForEach-Object { Write-Output $_ }

if (-not [string]::IsNullOrWhiteSpace($ReportPath)) {
    $resolvedReportPath = Normalize-WorkspacePath $ReportPath
    $reportDir = Split-Path -Parent $resolvedReportPath
    if (-not [string]::IsNullOrWhiteSpace($reportDir)) {
        New-Item -ItemType Directory -Force -Path $reportDir | Out-Null
    }
    Set-Content -Path $resolvedReportPath -Value $lines -Encoding UTF8
}
if (-not [string]::IsNullOrWhiteSpace($JsonOut)) {
    $resolvedJsonOut = Normalize-WorkspacePath $JsonOut
    $jsonDir = Split-Path -Parent $resolvedJsonOut
    if (-not [string]::IsNullOrWhiteSpace($jsonDir)) {
        New-Item -ItemType Directory -Force -Path $jsonDir | Out-Null
    }
    $report | ConvertTo-Json -Depth 8 | Set-Content -Path $resolvedJsonOut -Encoding UTF8
}

if ($report.status -eq "fail") {
    exit 2
}
if ($FailOnWarning -and $report.status -eq "warn") {
    exit 1
}
exit 0
