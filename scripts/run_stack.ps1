param(
    [string]$Profile = "default.dx12.immediate",
    [switch]$PlanOnly,
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$NoClient,
    [switch]$Launcher,
    [switch]$CefUi,
    [switch]$CefUiDx12AcceleratedPaint,
    [ValidateSet("default", "disabled", "cpu", "auto", "d3d11on12")]
    [string]$CefPaintTransport = "default",
    [switch]$CefAcceleratedStrict,
    [ValidateRange(2, 5)]
    [int]$CefGpuRingDepth = 3,
    [switch]$CefCopyDirtyRects,
    [switch]$CefDebugTimings,
    [switch]$RenderDiagnostics,
    [switch]$TraceDiagnostics,
    [switch]$RenderProfileVerbose,
    [switch]$FrameTimeDiagnostics,
    [switch]$BenchmarkLogMinimal,
    [switch]$LogStreamVerbose,
    [switch]$LogNetVerbose,
    [switch]$LogRenderVerbose,
    [switch]$Maximized,
    [switch]$DisableFpsOverlay,
    [switch]$EnableFpsOverlay,
    [switch]$SolariDebugDirectVisibility,
    [switch]$EnableDx12DlssRr,
    [switch]$DisableDlssRr,
    [switch]$DisableSolari,
    [switch]$DisableMeshlets,
    [switch]$DisableClouds,
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
    [string]$SolariArch = "budgeted",
    [int]$SolariTargetFps = 144,
    [int]$SolariFrameBudgetNs = 6944444,
    [int]$SolariGpuBudgetNs = 3000000,
    [string]$SolariVisualTarget = "competitive",
    [string]$SolariDenoiseMode = "balanced-fast",
    [string]$SolariInternalScale = "1.0",
    [int]$SolariBlasCompactionVertices = 0,
    [string]$SolariDebugOverlay = "",
    [string]$CloudQuality = "",
    [string]$CloudInternalScale = "",
    [string]$CloudTemporal = "",
    [string]$CloudShadows = "",
    [string]$CloudProfile = "",
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
    [int]$RenderMaxFrameLatency = 0,
    [int]$FrameTimeDiagnosticInterval = 60,
    [int]$FrameTimeDiagnosticMinNs = 0,
    [int]$FrameTimeDiagnosticMaxDepth = 10,
    [int]$FrameTimeDiagnosticTopChildren = 16,
    [int]$FrameTimeDiagnosticTopSpans = 32,
    [switch]$FrameTimeDiagnosticRowEvents,
    [int]$StartupDelaySeconds = 2
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$stackRoot = Join-Path $scriptRoot "stack"
foreach ($module in @(
    "Stack.Paths.ps1",
    "Stack.Env.ps1",
    "Stack.Security.ps1",
    "Stack.Cef.ps1",
    "Stack.Diagnostics.ps1",
    "Stack.Render.ps1",
    "Stack.Build.ps1",
    "Stack.Process.ps1",
    "Stack.Schema.ps1"
)) {
    . (Join-Path $stackRoot $module)
}

$request = New-StackRunRequest -BoundParameters $PSBoundParameters -ProfileRoot (Join-Path $stackRoot "profiles")
$paths = New-StackPaths -ScriptRoot $scriptRoot -Release $request.Release
Initialize-StackDirectories -Paths $paths

$events = @()
$events += Resolve-StackCefRequest -Request $request -Paths $paths
$diagnostics = Get-StackDiagnosticsState -Request $request
$buildPlan = New-StackBuildPlan -Request $request -Diagnostics $diagnostics
$events += $buildPlan.warnings

Set-StackRuntimeEnv -Request $request
Set-StackProfileEnv -Request $request
Set-StackDiagnosticsEnv -Request $request -Diagnostics $diagnostics
Set-StackCefEnv -Request $request -Paths $paths
Set-StackRenderEnv -Request $request
$events += Set-StackDevelopmentTokens -Request $request
Set-StackPathEnv -Paths $paths

foreach ($event in $events) {
    if ($event.severity -eq "warn") {
        Write-Warning $event.message
    }
    elseif ($event.message) {
        Write-Host $event.message
    }
}
if ($request.CefUi -and -not $request.NoClient) {
    Write-Host "Enabling game_client/cef_ui for the CEF browser UI."
}
if ($request.CefUiDx12AcceleratedPaint -and -not $request.NoClient) {
    Write-Host "Selecting CEF D3D11On12 accelerated paint transport for the DX12 client."
}
if (-not $request.NoClient) {
    if ($request.PlanOnly) {
        Write-Host "Resolved unified Fun client in $env:FUN_START_MODE mode."
    }
    else {
        Write-Host "Starting unified Fun client in $env:FUN_START_MODE mode."
    }
}

$envSnapshot = Export-StackEnvSnapshot -Request $request
if ($request.PlanOnly) {
    Write-StackSessionMetadata -Request $request -Paths $paths -BuildPlan $buildPlan -Processes @() -EnvSnapshot $envSnapshot -Events $events -Status "planned"
    Write-Host "Planned stack session: $($paths.session_file)"
    Write-Host "Logs: $($paths.log_root)"
    return
}

Stop-PreviousStackProcesses -PidFile $paths.pid_file
Invoke-StackBuild -Paths $paths -BuildPlan $buildPlan
Copy-StackDynamicLinkLibraries -Paths $paths -BuildPlan $buildPlan
if ($request.CefUi -and -not $request.NoClient) {
    Copy-CefRuntimeFiles -ProfileTargetDir $paths.profile_target_dir
}

$started = Start-StackProcesses -Request $request -Paths $paths
Write-StackSessionMetadata -Request $request -Paths $paths -BuildPlan $buildPlan -Processes $started -EnvSnapshot (Export-StackEnvSnapshot -Request $request) -Events $events -Status "launched"
Confirm-StackProcesses -Processes $started

Write-Host "Process metadata: $($paths.pid_file)"
Write-Host "Session metadata: $($paths.session_file)"
Write-Host "Logs: $($paths.log_root)"
