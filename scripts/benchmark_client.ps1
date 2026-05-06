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
    [ValidateSet("default", "disabled", "cpu", "auto", "d3d11on12")]
    [string]$CefPaintTransport = "default",
    [switch]$CefAcceleratedStrict,
    [ValidateRange(2, 5)]
    [int]$CefGpuRingDepth = 3,
    [switch]$CefCopyDirtyRects,
    [switch]$CefDebugTimings,
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
    [ValidateSet(0, 1, 2, 4, 8)]
    [int]$StreamRenderPrepBudgetMs = 0,
    [ValidateRange(0, 1000000)]
    [int]$StreamRenderPrepMaxChunksPerFrame = 0,
    [int]$WindowWidth = 0,
    [int]$WindowHeight = 0,
    [string]$RenderBackend = "dx12",
    [string]$PresentMode = "immediate",
    [int]$WarmupSeconds = 10,
    [int]$SampleSeconds = 30,
    [string]$Baseline = "",
    [string]$StackProfile = "",
    [string]$InputLog = "",
    [switch]$KeepRunning
)

$ErrorActionPreference = "Stop"
$benchmarkModuleRoot = Join-Path (Split-Path -Parent $MyInvocation.MyCommand.Path) "benchmark"
foreach ($module in @(
    "Benchmark.Metrics.ps1",
    "Benchmark.Parse.ps1",
    "Benchmark.Report.ps1"
)) {
    . (Join-Path $benchmarkModuleRoot $module)
}

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

function Get-RendererCapabilityReport {
    param([string]$RepoRoot)

    $path = [System.Environment]::GetEnvironmentVariable("FUN_RENDERER_CAPABILITY_REPORT_PATH", "Process")
    if ([string]::IsNullOrWhiteSpace($path)) {
        $path = Join-Path $RepoRoot "target\run-stack\renderer-capabilities.json"
    }
    if (-not (Test-Path -LiteralPath $path)) {
        return [ordered]@{
            status = "not_found"
            path = $path
        }
    }

    try {
        $report = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
        $report | Add-Member -NotePropertyName "status" -NotePropertyValue "found" -Force
        $report | Add-Member -NotePropertyName "path" -NotePropertyValue $path -Force
        return $report
    }
    catch {
        return [ordered]@{
            status = "parse_error"
            path = $path
            error = [string]$_
        }
    }
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

function Start-BenchmarkStack {
    param(
        [string]$ScriptRoot,
        [string]$StackProfile,
        [System.Collections.IDictionary]$Overrides,
        [System.Collections.IDictionary]$Environment,
        [string]$SessionPath
    )

    $runStackPath = Join-Path $ScriptRoot "run_stack.ps1"
    $powerShellPath = (Get-Process -Id $PID).Path
    if ([string]::IsNullOrWhiteSpace($powerShellPath)) {
        $powerShellPath = "powershell"
    }

    foreach ($name in $Environment.Keys) {
        Set-BenchmarkProcessEnv -Name $name -Value $Environment[$name]
    }

    $runStackArgs = @(
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        $runStackPath,
        "-Profile",
        $StackProfile
    )

    foreach ($name in $Overrides.Keys) {
        $value = $Overrides[$name]
        if ($null -eq $value) {
            continue
        }
        if ($value -is [bool] -or $value -is [System.Management.Automation.SwitchParameter]) {
            if ([bool]$value) {
                $runStackArgs += "-$name"
            }
            continue
        }
        if ($value -is [string] -and [string]::IsNullOrWhiteSpace($value)) {
            continue
        }
        $runStackArgs += @("-$name", [string]$value)
    }

    Write-Host "Starting benchmark stack profile=$StackProfile..."
    & $powerShellPath @runStackArgs
    if ($LASTEXITCODE -ne 0) {
        throw "run_stack.ps1 failed with exit code $LASTEXITCODE"
    }

    return [ordered]@{
        schema_version = "benchmark_stack_runner_v1"
        launched = $true
        profile = $StackProfile
        session_json = $SessionPath
        command = @($powerShellPath) + $runStackArgs
        overrides = $Overrides
        environment_keys = @($Environment.Keys)
    }
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Normalize-WorkspacePath ((Resolve-Path (Join-Path $scriptRoot "..")).Path)
$runRoot = Join-Path $repoRoot "target\run-stack"
$logRoot = Join-Path $runRoot "logs"
$pidFile = Join-Path $runRoot "processes.json"
$sessionPath = Join-Path $runRoot "session.json"
$clientLog = if ([string]::IsNullOrWhiteSpace($InputLog)) {
    Join-Path $logRoot "game_client.out.log"
}
else {
    Resolve-RepoPath -RepoRoot $repoRoot -Path $InputLog
}
$clientErrorLog = if ([string]::IsNullOrWhiteSpace($InputLog)) {
    Join-Path $logRoot "game_client.err.log"
}
else {
    ""
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
$stackRunner = [ordered]@{
    schema_version = "benchmark_stack_runner_v1"
    launched = $false
    profile = ""
    session_json = ""
    command = @()
    overrides = [ordered]@{}
    environment_keys = @()
}
$lineOffset = 0
$errorLineOffset = 0
$captureFromStart = $BenchmarkLane -eq "streaming_spike"

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
$stackProfileRoot = Join-Path $scriptRoot "stack\profiles"
$backendPresentStackProfile = "default.$RenderBackend.$PresentMode"
$resolvedStackProfile = if (-not [string]::IsNullOrWhiteSpace($StackProfile)) {
    $StackProfile
}
elseif (Test-Path (Join-Path $stackProfileRoot "$backendPresentStackProfile.json")) {
    $backendPresentStackProfile
}
else {
    "default.dx12.immediate"
}
$stackRunner.profile = $resolvedStackProfile

try {
    if ([string]::IsNullOrWhiteSpace($InputLog)) {
        $benchmarkLogFilter = "info,game_client=info,fun::perf=info,fun::render=debug,bevy_render::transient=debug"
        $acceleratedPaintValue = ""
        if ($CefPaintTransport -eq "default") {
            $acceleratedPaintValue = ""
        }
        else {
            $acceleratedPaintValue = switch ($CefPaintTransport) {
                "cpu" { "0" }
                "disabled" { "disabled" }
                "auto" { "auto" }
                "d3d11on12" { "1" }
                default { "" }
            }
        }
        $benchmarkStackEnvironment = [ordered]@{
            FUN_BENCHMARK_PROFILE = $BenchmarkProfile
            FUN_BENCHMARK_SCENARIO = $BenchmarkScenario
            FUN_BENCHMARK_MATRIX_LANE = $BenchmarkMatrixLane
            FUN_CEF_UI_BENCHMARK_MODE = $CefUiMode
            RUST_LOG = $benchmarkLogFilter
            BEVY_LOG = $benchmarkLogFilter
            FUN_CEF_UI_PAINT_TRANSPORT = if ($CefPaintTransport -eq "default") { "" } else { $CefPaintTransport }
            FUN_CEF_UI_ACCELERATED_PAINT = $acceleratedPaintValue
            FUN_CEF_UI_ACCELERATED_STRICT = if ($CefAcceleratedStrict) { "1" } else { "" }
            FUN_CEF_UI_GPU_RING_DEPTH = [string]$CefGpuRingDepth
            FUN_CEF_UI_COPY_DIRTY_RECTS = if ($CefCopyDirtyRects) { "1" } else { "" }
            FUN_CEF_UI_DEBUG_TIMINGS = if ($CefDebugTimings) { "1" } else { "" }
            FUN_PRESENT_MAX_FRAME_LATENCY = if ($RequestedMaximumFrameLatency -gt 0) { [string]$RequestedMaximumFrameLatency } else { "" }
            FUN_RENDER_MAX_FRAME_LATENCY = if ($RequestedMaximumFrameLatency -gt 0) { [string]$RequestedMaximumFrameLatency } else { "" }
            FUN_STREAM_RENDER_PREP_BUDGET_MS = if ($StreamRenderPrepBudgetMs -gt 0) { [string]$StreamRenderPrepBudgetMs } else { "" }
            FUN_STREAM_RENDER_PREP_MAX_CHUNKS_PER_FRAME = if ($StreamRenderPrepMaxChunksPerFrame -gt 0) { [string]$StreamRenderPrepMaxChunksPerFrame } else { "" }
        }
        $benchmarkStackOverrides = [ordered]@{
            RenderDiagnostics = $true
            RenderBackend = $RenderBackend
            PresentMode = $PresentMode
            RenderMaxFrameLatency = if ($RequestedMaximumFrameLatency -gt 0) { $RequestedMaximumFrameLatency } else { $null }
            Release = [bool]$Release
            StaticBevy = [bool]$StaticBevy
            CefUi = [bool]$cefUiEnabled
            CefUiDx12AcceleratedPaint = [bool]$cefAcceleratedFeatureRequested
            CefPaintTransport = if ($CefPaintTransport -ne "default" -and ($cefUiEnabled -or ($CefPaintTransport -ne "auto" -and $CefPaintTransport -ne "d3d11on12"))) { $CefPaintTransport } else { $null }
            CefAcceleratedStrict = [bool]$CefAcceleratedStrict
            CefGpuRingDepth = $CefGpuRingDepth
            CefCopyDirtyRects = [bool]$CefCopyDirtyRects
            CefDebugTimings = [bool]$CefDebugTimings
            EnableDx12DlssRr = [bool]$EnableDx12DlssRr
            DisableDlssRr = [bool]$DisableDlssRr
            DisableSolari = [bool]$DisableSolari
            DisableMeshlets = [bool]$DisableMeshlets
            DisableClouds = [bool]$DisableClouds
            DisableFpsOverlay = [bool]$DisableFpsOverlay
            RtSampleDirect = $RtSampleDirect
            RtSampleIndirect = $RtSampleIndirect
            RtSampleReflections = $RtSampleReflections
            RtSurfaceCache = $RtSurfaceCache
            RtMegaGeom = $RtMegaGeom
            RtOpacityMask = $RtOpacityMask
            RtHair = $RtHair
            RtAsyncReadback = $RtAsyncReadback
            RtValidation = $RtValidation
            RenderUnknownVendor = [bool]$RenderUnknownVendor
            RenderVendorEmulation = $RenderVendorEmulation
            CloudQuality = $CloudQuality
            CloudInternalScale = $CloudInternalScale
            CloudTemporal = $CloudTemporal
            CloudShadows = $CloudShadows
            CloudProfile = $CloudProfile
            CloudDebugOverlay = $CloudDebugOverlay
            BenchmarkLogMinimal = $true
            TraceDiagnostics = [bool]$TraceDiagnostics
            FrameTimeDiagnostics = [bool]$FrameTimeDiagnostics
            FrameTimeDiagnosticInterval = if ($FrameTimeDiagnostics) { $FrameTimeDiagnosticInterval } else { $null }
            FrameTimeDiagnosticMinNs = if ($FrameTimeDiagnostics) { $FrameTimeDiagnosticMinNs } else { $null }
            FrameTimeDiagnosticMaxDepth = if ($FrameTimeDiagnostics) { $FrameTimeDiagnosticMaxDepth } else { $null }
            FrameTimeDiagnosticTopChildren = if ($FrameTimeDiagnostics) { $FrameTimeDiagnosticTopChildren } else { $null }
            FrameTimeDiagnosticTopSpans = if ($FrameTimeDiagnostics) { $FrameTimeDiagnosticTopSpans } else { $null }
            FrameTimeDiagnosticRowEvents = [bool]$FrameTimeDiagnosticRowEvents
            SolariDenoiseMode = $SolariDenoiseMode
            SolariInternalScale = $SolariInternalScale
            SolariBlasCompactionVertices = if ($SolariBlasCompactionVertices -gt 0) { $SolariBlasCompactionVertices } else { $null }
            SolariArch = $SolariArch
            SolariTargetFps = if ($SolariTargetFps -gt 0) { $SolariTargetFps } else { $null }
            SolariFrameBudgetNs = if ($SolariFrameBudgetNs -gt 0) { $SolariFrameBudgetNs } else { $null }
            SolariGpuBudgetNs = if ($SolariGpuBudgetNs -gt 0) { $SolariGpuBudgetNs } else { $null }
            SolariVisualTarget = $SolariVisualTarget
            RenderGeometryPolicy = $RenderGeometryPolicy
            MeshletMinTriangles = if ($MeshletMinTriangles -gt 0) { $MeshletMinTriangles } else { $null }
            StreamRenderPrepBudgetMs = if ($StreamRenderPrepBudgetMs -gt 0) { $StreamRenderPrepBudgetMs } else { $null }
            StreamRenderPrepMaxChunksPerFrame = if ($StreamRenderPrepMaxChunksPerFrame -gt 0) { $StreamRenderPrepMaxChunksPerFrame } else { $null }
            WindowWidth = if ($WindowWidth -gt 0 -and $WindowHeight -gt 0) { $WindowWidth } else { $null }
            WindowHeight = if ($WindowWidth -gt 0 -and $WindowHeight -gt 0) { $WindowHeight } else { $null }
        }
        $stackRunner = Start-BenchmarkStack `
            -ScriptRoot $scriptRoot `
            -StackProfile $resolvedStackProfile `
            -Overrides $benchmarkStackOverrides `
            -Environment $benchmarkStackEnvironment `
            -SessionPath $sessionPath
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
        if ($captureFromStart) {
            $lineOffset = 0
            $errorLineOffset = 0
        }
        else {
            $lineOffset = @(Get-Content -Path $clientLog -ErrorAction SilentlyContinue).Count
            if (-not [string]::IsNullOrWhiteSpace($clientErrorLog) -and (Test-Path $clientErrorLog)) {
                $errorLineOffset = @(Get-Content -Path $clientErrorLog -ErrorAction SilentlyContinue).Count
            }
        }
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
    if (-not [string]::IsNullOrWhiteSpace($clientErrorLog) -and (Test-Path $clientErrorLog)) {
        $allErrorLines = @(Get-Content -Path $clientErrorLog -ErrorAction Stop)
        $sampleErrorLines = if ($errorLineOffset -gt 0) {
            @($allErrorLines | Select-Object -Skip $errorLineOffset)
        }
        else {
            $allErrorLines
        }
        $allLines = @($allLines + $allErrorLines)
        $sampleLines = @($sampleLines + $sampleErrorLines)
    }

    $samples = @(Parse-ClientPerfLog -Lines $sampleLines)
    if ($samples.Count -eq 0) {
        $sampleSources = if (-not [string]::IsNullOrWhiteSpace($clientErrorLog)) {
            "$clientLog or $clientErrorLog"
        }
        else {
            $clientLog
        }
        throw "No [client perf] samples were found in $sampleSources"
    }

    $renderCapabilities = Parse-RenderCapabilitiesLog -Lines $allLines
    $rendererCapabilityReport = Get-RendererCapabilityReport -RepoRoot $repoRoot
    $dx12BackendDiagnostics = Parse-Dx12BackendDiagnosticsLog -Lines $allLines
    $rtFeatureGates = Parse-RenderFeatureGatesLog -Lines $allLines
    $renderPresentation = Parse-RenderPresentationLog -Lines $allLines
    $renderUploadCallsites = Parse-RenderUploadCallsitesLog -Lines $sampleLines
    $renderChurnEvents = Parse-RenderChurnEventsLog -Lines $sampleLines
    $renderChurnCreationEvents = Parse-RenderChurnCreationEventsLog -Lines $sampleLines
    $renderCommandEvents = Parse-RenderCommandEventsLog -Lines $sampleLines
    $renderReadbackEvents = Parse-RenderReadbackEventsLog -Lines $sampleLines
    $renderShaderEvents = Parse-RenderShaderEventsLog -Lines $sampleLines
    $transientDescriptorCreates = @(Parse-TransientDescriptorCreateLog -Lines $sampleLines)
    $transientDescriptorLabelVariants = @(Parse-TransientDescriptorLabelVariantLog -Lines $sampleLines)
    $cefUiTransportSelection = Parse-CefUiTransportSelectionLog -Lines $allLines
    $cefUiTransportHealth = Parse-CefUiTransportHealthLog -Lines $sampleLines
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
        source_error_log = $clientErrorLog
        git = [ordered]@{
            commit = $gitCommit
            dirty_count = $gitDirty.Count
            dirty = $gitDirty
        }
        hardware = Get-HardwareInfo
        environment = Get-BenchmarkEnvironmentInfo
        dx12_memory = Get-Dx12MemoryBudgetInfo
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
            cef_accelerated_strict = [bool]$CefAcceleratedStrict
            cef_gpu_ring_depth = $CefGpuRingDepth
            cef_copy_dirty_rects = [bool]$CefCopyDirtyRects
            cef_debug_timings = [bool]$CefDebugTimings
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
            stream_render_prep_budget_ms = $StreamRenderPrepBudgetMs
            stream_render_prep_max_chunks_per_frame = $StreamRenderPrepMaxChunksPerFrame
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
        stack_runner = $stackRunner
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
        renderer_capability_report = $rendererCapabilityReport
        dx12_backend_diagnostics = $dx12BackendDiagnostics
        rt_feature_gates = $rtFeatureGates
        render_presentation = $renderPresentation
        render_upload_callsites = $renderUploadCallsites
        render_churn_events = $renderChurnEvents
        render_churn_creation_events = $renderChurnCreationEvents
        render_command_events = $renderCommandEvents
        render_readback_events = $renderReadbackEvents
        render_shader_events = $renderShaderEvents
        transient_descriptor_creates = $transientDescriptorCreates
        transient_descriptor_label_variants = $transientDescriptorLabelVariants
        cef_ui_transport_selection = $cefUiTransportSelection
        cef_ui_transport_health = $cefUiTransportHealth
    }

    $flameMapPath = Join-Path $repoRoot "target\dx12\render_graph_frame_0000.json"
    Write-RenderGraphFlameMap -Path $flameMapPath -Summary $summary
    $summary["render_graph_flame_map"] = $flameMapPath

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
