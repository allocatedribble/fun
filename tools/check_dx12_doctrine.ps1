param(
    [string]$RepoRoot = "",
    [string]$CategoryPath = ".dx12_change_category",
    [string[]]$SummaryPath = @(),
    [switch]$SelfTest
)

$ErrorActionPreference = "Stop"

$DoctrineScriptRoot = if (-not [string]::IsNullOrWhiteSpace($PSScriptRoot)) {
    $PSScriptRoot
}
else {
    Split-Path -Parent $MyInvocation.MyCommand.Path
}

$AllowedCategories = @(
    "measurement-only",
    "upload-cleanup",
    "cef-transport",
    "barrier-state",
    "descriptor-pso-churn",
    "present-pacing",
    "native-interop",
    "dlss-sr",
    "dlss-rr",
    "moonshot"
)

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

function Resolve-RepoRoot {
    param([string]$Path)

    if (-not [string]::IsNullOrWhiteSpace($Path)) {
        return Normalize-WorkspacePath ((Resolve-Path $Path).Path)
    }
    return Normalize-WorkspacePath ((Resolve-Path (Join-Path $script:DoctrineScriptRoot "..")).Path)
}

function Get-RelativePath {
    param(
        [string]$Root,
        [string]$Path
    )

    $rootFull = [System.IO.Path]::GetFullPath($Root).TrimEnd([char[]]@('\', '/'))
    $pathFull = [System.IO.Path]::GetFullPath($Path)
    if ($pathFull.StartsWith($rootFull, [System.StringComparison]::OrdinalIgnoreCase)) {
        return $pathFull.Substring($rootFull.Length).TrimStart([char[]]@('\', '/')).Replace('\', '/')
    }
    return $pathFull.Replace('\', '/')
}

function New-CheckResult {
    param(
        [string]$Id,
        [string]$Status,
        [string]$Detail
    )

    return [pscustomobject][ordered]@{
        id = $Id
        status = $Status
        detail = $Detail
    }
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

function Read-JsonFile {
    param([string]$Path)

    $resolved = Normalize-WorkspacePath ((Resolve-Path $Path).Path)
    return Get-Content -Path $resolved -Raw | ConvertFrom-Json
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

function Read-ChangeCategory {
    param([string]$Path)

    if (-not (Test-Path $Path)) {
        return $null
    }
    foreach ($line in @(Get-Content -Path $Path)) {
        $trimmed = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($trimmed) -or $trimmed.StartsWith("#")) {
            continue
        }
        return $trimmed.ToLowerInvariant()
    }
    return $null
}

function Test-DlssSrGateReady {
    param([string]$RepoRoot)

    $gatePath = Join-Path $RepoRoot "docs\dx12_dlss_boundary_gate.md"
    if (-not (Test-Path $gatePath)) {
        return $false
    }
    $text = Get-Content -Path $gatePath -Raw
    return $text -match "(?im)^DX12 baseline ready for DLSS SR bring-up:\s*yes\s*$"
}

function Test-AcceleratedCefLane {
    param([object]$Summary)

    $selection = Get-JsonProperty -Object $Summary -Name "cef_ui_transport_selection"
    $selected = [string](Get-JsonProperty -Object $selection -Name "selected")
    $config = Get-JsonProperty -Object $Summary -Name "config"
    $configured = [string](Get-JsonProperty -Object $config -Name "cef_paint_transport")
    $transport = if (-not [string]::IsNullOrWhiteSpace($selected)) { $selected } else { $configured }
    $normalized = $transport.ToLowerInvariant()
    if (@("d3d11on12", "d3d11_shared_texture_dx12_copy") -contains $normalized) {
        return $true
    }
    $requested = Get-JsonProperty -Object $config -Name "cef_accelerated_feature_requested"
    return ($requested -is [bool] -and $requested -and $normalized -ne "cpu" -and $normalized -ne "cpu_paint")
}

function Test-RequiredFiles {
    param([string]$RepoRoot)

    $required = @(
        "docs\dx12_implementation_doctrine.md",
        "docs\dx12_dlss_boundary_gate.md",
        "tools\dx12_parity_report.py",
        "tools\check_dx12_perf_regression.ps1",
        "tools\dx12_perf_baseline_envelopes.json"
    )
    $missing = @()
    foreach ($path in $required) {
        if (-not (Test-Path (Join-Path $RepoRoot $path))) {
            $missing += $path
        }
    }
    if ($missing.Count -gt 0) {
        return New-CheckResult -Id "required_files" -Status "fail" -Detail ("missing=" + ($missing -join ","))
    }
    return New-CheckResult -Id "required_files" -Status "pass" -Detail "all required doctrine/perf files exist"
}

function Test-ChangeCategory {
    param(
        [string]$RepoRoot,
        [string]$CategoryPath
    )

    $path = if ([System.IO.Path]::IsPathRooted($CategoryPath)) { $CategoryPath } else { Join-Path $RepoRoot $CategoryPath }
    $category = Read-ChangeCategory -Path $path
    if ([string]::IsNullOrWhiteSpace($category)) {
        return New-CheckResult -Id "change_category" -Status "fail" -Detail "missing .dx12_change_category first non-comment line"
    }
    if (-not ($AllowedCategories -contains $category)) {
        return New-CheckResult -Id "change_category" -Status "fail" -Detail "unknown category '$category'"
    }
    return New-CheckResult -Id "change_category" -Status "pass" -Detail "category=$category"
}

function Test-DlssCategoryGate {
    param(
        [string]$RepoRoot,
        [string]$CategoryPath
    )

    $path = if ([System.IO.Path]::IsPathRooted($CategoryPath)) { $CategoryPath } else { Join-Path $RepoRoot $CategoryPath }
    $category = Read-ChangeCategory -Path $path
    if ($category -ne "dlss-sr") {
        return New-CheckResult -Id "dlss_sr_gate" -Status "pass" -Detail "category=$category"
    }
    if (Test-DlssSrGateReady -RepoRoot $RepoRoot) {
        return New-CheckResult -Id "dlss_sr_gate" -Status "pass" -Detail "DLSS SR category allowed because boundary gate is ready"
    }
    return New-CheckResult -Id "dlss_sr_gate" -Status "fail" -Detail "category=dlss-sr but dx12_dlss_boundary_gate.md does not say baseline ready yes"
}

function Test-Dx12HalBoundary {
    param([string]$RepoRoot)

    $roots = @(
        "fun_render\src",
        "game_client\src",
        "fun_ui_cef\src"
    )
    $pattern = "as_hal::<.*Dx12|as_hal_mut::<.*Dx12|wgpu::hal::api::Dx12"
    $violations = New-Object "System.Collections.Generic.List[string]"
    foreach ($root in $roots) {
        $scanRoot = Join-Path $RepoRoot $root
        if (-not (Test-Path $scanRoot)) {
            continue
        }
        foreach ($file in @(Get-ChildItem -Path $scanRoot -Recurse -File -Filter "*.rs")) {
            $relative = Get-RelativePath -Root $RepoRoot -Path $file.FullName
            if ($relative.StartsWith("fun_render/src/dx12_native/", [System.StringComparison]::OrdinalIgnoreCase)) {
                continue
            }
            foreach ($match in @(Select-String -Path $file.FullName -Pattern $pattern)) {
                $violations.Add("${relative}:$($match.LineNumber)") | Out-Null
            }
        }
    }
    if ($violations.Count -gt 0) {
        return New-CheckResult -Id "dx12_hal_boundary" -Status "fail" -Detail ("stray_hal=" + (($violations.ToArray()) -join ","))
    }
    return New-CheckResult -Id "dx12_hal_boundary" -Status "pass" -Detail "no raw DX12 HAL extraction outside fun_render/src/dx12_native"
}

function Test-AcceleratedCefSummaries {
    param(
        [string]$RepoRoot,
        [string[]]$SummaryPath
    )

    if ($SummaryPath.Count -eq 0) {
        return New-CheckResult -Id "accelerated_cef_cpu_upload" -Status "pass" -Detail "no benchmark summaries supplied"
    }
    $failures = New-Object "System.Collections.Generic.List[string]"
    foreach ($path in $SummaryPath) {
        $resolved = if ([System.IO.Path]::IsPathRooted($path)) { $path } else { Join-Path $RepoRoot $path }
        if (-not (Test-Path $resolved)) {
            $failures.Add("${path}:missing") | Out-Null
            continue
        }
        $summary = Read-JsonFile -Path $resolved
        if (-not (Test-AcceleratedCefLane -Summary $summary)) {
            continue
        }
        $mean = Get-MetricValue -Summary $summary -Metric "cef_cpu_upload_bytes" -Field "mean"
        $p95 = Get-MetricValue -Summary $summary -Metric "cef_cpu_upload_bytes" -Field "p95"
        $observed = 0.0
        if ($null -ne $mean) {
            $observed = [Math]::Max($observed, [double]$mean)
        }
        if ($null -ne $p95) {
            $observed = [Math]::Max($observed, [double]$p95)
        }
        if ($observed -gt 0.0) {
            $failures.Add("${path}:cef_cpu_upload_bytes=$observed") | Out-Null
        }
    }
    if ($failures.Count -gt 0) {
        return New-CheckResult -Id "accelerated_cef_cpu_upload" -Status "fail" -Detail (($failures.ToArray()) -join ",")
    }
    return New-CheckResult -Id "accelerated_cef_cpu_upload" -Status "pass" -Detail "accelerated summaries have zero CPU upload bytes"
}

function Invoke-DoctrineCheck {
    param(
        [string]$RepoRoot,
        [string]$CategoryPath,
        [string[]]$SummaryPath
    )

    $results = New-Object "System.Collections.Generic.List[object]"
    $results.Add((Test-RequiredFiles -RepoRoot $RepoRoot)) | Out-Null
    $results.Add((Test-ChangeCategory -RepoRoot $RepoRoot -CategoryPath $CategoryPath)) | Out-Null
    $results.Add((Test-DlssCategoryGate -RepoRoot $RepoRoot -CategoryPath $CategoryPath)) | Out-Null
    $results.Add((Test-Dx12HalBoundary -RepoRoot $RepoRoot)) | Out-Null
    $results.Add((Test-AcceleratedCefSummaries -RepoRoot $RepoRoot -SummaryPath $SummaryPath)) | Out-Null

    $failures = @($results | Where-Object { $_.status -eq "fail" })
    $resolvedCategoryPath = if ([System.IO.Path]::IsPathRooted($CategoryPath)) { $CategoryPath } else { Join-Path $RepoRoot $CategoryPath }
    $category = Read-ChangeCategory -Path $resolvedCategoryPath
    return [pscustomobject][ordered]@{
        schema = "fun.dx12_doctrine.report.v1"
        status = if ($failures.Count -gt 0) { "fail" } else { "pass" }
        category = $category
        failures = $failures.Count
        checks = $results.Count
        results = @($results.ToArray())
    }
}

function New-SyntheticSummaryFile {
    param(
        [string]$Path,
        [string]$Transport,
        [double]$CefCpuUploadBytes
    )

    $payload = [ordered]@{
        config = [ordered]@{
            cef_paint_transport = $Transport
            cef_accelerated_feature_requested = ($Transport -eq "d3d11on12")
        }
        cef_ui_transport_selection = [ordered]@{
            selected = $Transport
        }
        metrics = [ordered]@{
            cef_cpu_upload_bytes = [ordered]@{
                mean = $CefCpuUploadBytes
                p95 = $CefCpuUploadBytes
            }
        }
    }
    $payload | ConvertTo-Json -Depth 8 | Set-Content -Path $Path -Encoding UTF8
}

function New-SelfTestRoot {
    param([string]$Root)

    foreach ($dir in @(
        "docs",
        "tools",
        "fun_render\src\dx12_native",
        "game_client\src",
        "fun_ui_cef\src"
    )) {
        New-Item -ItemType Directory -Force -Path (Join-Path $Root $dir) | Out-Null
    }
    Set-Content -Path (Join-Path $Root ".dx12_change_category") -Value "measurement-only" -Encoding UTF8
    Set-Content -Path (Join-Path $Root "docs\dx12_implementation_doctrine.md") -Value "# doctrine" -Encoding UTF8
    Set-Content -Path (Join-Path $Root "docs\dx12_dlss_boundary_gate.md") -Value "DX12 baseline ready for DLSS SR bring-up: no" -Encoding UTF8
    Set-Content -Path (Join-Path $Root "tools\dx12_parity_report.py") -Value "# report" -Encoding UTF8
    Set-Content -Path (Join-Path $Root "tools\check_dx12_perf_regression.ps1") -Value "# perf gate" -Encoding UTF8
    Set-Content -Path (Join-Path $Root "tools\dx12_perf_baseline_envelopes.json") -Value "{}" -Encoding UTF8
    Set-Content -Path (Join-Path $Root "fun_render\src\dx12_native\handles.rs") -Value "fn ok() { device.as_hal::<wgpu::hal::api::Dx12>(); }" -Encoding UTF8
    Set-Content -Path (Join-Path $Root "game_client\src\lib.rs") -Value "pub fn ok() {}" -Encoding UTF8
}

if ($SelfTest) {
    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("fun_dx12_doctrine_" + [System.Guid]::NewGuid().ToString("N"))
    New-Item -ItemType Directory -Force -Path $tempRoot | Out-Null
    try {
        New-SelfTestRoot -Root $tempRoot
        $passReport = Invoke-DoctrineCheck -RepoRoot $tempRoot -CategoryPath ".dx12_change_category" -SummaryPath @()
        if ($passReport.status -ne "pass") {
            throw "Self-test expected clean synthetic tree to pass, got $($passReport.status)"
        }

        Set-Content -Path (Join-Path $tempRoot ".dx12_change_category") -Value "bad-category" -Encoding UTF8
        $badCategoryReport = Invoke-DoctrineCheck -RepoRoot $tempRoot -CategoryPath ".dx12_change_category" -SummaryPath @()
        if ($badCategoryReport.status -ne "fail") {
            throw "Self-test expected bad category to fail"
        }

        Set-Content -Path (Join-Path $tempRoot ".dx12_change_category") -Value "dlss-sr" -Encoding UTF8
        $dlssReport = Invoke-DoctrineCheck -RepoRoot $tempRoot -CategoryPath ".dx12_change_category" -SummaryPath @()
        if ($dlssReport.status -ne "fail") {
            throw "Self-test expected dlss-sr to fail while boundary gate says no"
        }

        Set-Content -Path (Join-Path $tempRoot ".dx12_change_category") -Value "measurement-only" -Encoding UTF8
        Set-Content -Path (Join-Path $tempRoot "game_client\src\bad.rs") -Value "fn bad() { device.as_hal::<wgpu::hal::api::Dx12>(); }" -Encoding UTF8
        $halReport = Invoke-DoctrineCheck -RepoRoot $tempRoot -CategoryPath ".dx12_change_category" -SummaryPath @()
        if ($halReport.status -ne "fail") {
            throw "Self-test expected stray HAL call to fail"
        }
        Remove-Item -LiteralPath (Join-Path $tempRoot "game_client\src\bad.rs") -Force

        $summaryPath = Join-Path $tempRoot "summary.json"
        New-SyntheticSummaryFile -Path $summaryPath -Transport "d3d11on12" -CefCpuUploadBytes 1.0
        $cefReport = Invoke-DoctrineCheck -RepoRoot $tempRoot -CategoryPath ".dx12_change_category" -SummaryPath @($summaryPath)
        if ($cefReport.status -ne "fail") {
            throw "Self-test expected accelerated CEF CPU upload summary to fail"
        }

        New-SyntheticSummaryFile -Path $summaryPath -Transport "d3d11on12" -CefCpuUploadBytes 0.0
        $cefPassReport = Invoke-DoctrineCheck -RepoRoot $tempRoot -CategoryPath ".dx12_change_category" -SummaryPath @($summaryPath)
        if ($cefPassReport.status -ne "pass") {
            throw "Self-test expected zero CPU upload accelerated CEF summary to pass"
        }

        Write-Output "DX12 doctrine gate self-test passed."
        exit 0
    }
    finally {
        if (Test-Path $tempRoot) {
            Remove-Item -LiteralPath $tempRoot -Recurse -Force
        }
    }
}

$resolvedRepoRoot = Resolve-RepoRoot -Path $RepoRoot
$report = Invoke-DoctrineCheck -RepoRoot $resolvedRepoRoot -CategoryPath $CategoryPath -SummaryPath $SummaryPath
Write-Output "DX12 doctrine gate: status=$($report.status) category=$($report.category) failures=$($report.failures) checks=$($report.checks)"
foreach ($result in @($report.results)) {
    if ($result.status -ne "pass") {
        Write-Output "- $($result.id): status=$($result.status) detail=$($result.detail)"
    }
}
if ($report.status -eq "fail") {
    exit 2
}
exit 0
