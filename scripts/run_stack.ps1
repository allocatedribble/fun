param(
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

function Set-OptionalEnvValue {
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

function New-LocalStackSessionToken {
    $bytes = [byte[]]::new(32)
    $rng = [System.Security.Cryptography.RandomNumberGenerator]::Create()
    try {
        $rng.GetBytes($bytes)
        return [Convert]::ToBase64String($bytes)
    }
    finally {
        $rng.Dispose()
    }
}

function Copy-CefRuntimeFiles {
    param(
        [string]$ProfileTargetDir
    )

    $buildDir = Join-Path $ProfileTargetDir "build"
    if (-not (Test-Path $buildDir)) {
        throw "CEF runtime build directory was not found: $buildDir"
    }

    $cefRuntime = Get-ChildItem -Path $buildDir -Directory -Filter "cef-dll-sys-*" |
        ForEach-Object { Join-Path $_.FullName "out\cef_windows_x86_64" } |
        Where-Object { Test-Path (Join-Path $_ "libcef.dll") } |
        Sort-Object { (Get-Item (Join-Path $_ "libcef.dll")).LastWriteTimeUtc } -Descending |
        Select-Object -First 1

    if ([string]::IsNullOrWhiteSpace($cefRuntime)) {
        throw "CEF runtime files were not found under $buildDir"
    }

    $runtimePatterns = @("*.dll", "*.pak", "*.dat", "*.bin", "*.json")
    foreach ($pattern in $runtimePatterns) {
        Get-ChildItem -Path $cefRuntime -File -Filter $pattern | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination $ProfileTargetDir -Force
        }
    }

    $localesSource = Join-Path $cefRuntime "locales"
    if (Test-Path $localesSource) {
        Copy-Item -LiteralPath $localesSource -Destination $ProfileTargetDir -Recurse -Force
    }

    Write-Host "Bundled CEF runtime files from $cefRuntime"
}

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptRoot "..")
$profile = if ($Release) { "release" } else { "debug" }
$clientRenderDiagnosticsRequested = -not $NoClient -and ($RenderDiagnostics -or $TraceDiagnostics -or $RenderProfileVerbose -or $FrameTimeDiagnostics)
$clientLogDiagnosticsRequested = -not $NoClient -and ($LogStreamVerbose -or $LogNetVerbose -or $LogRenderVerbose)
$clientDiagnosticsRequested = $clientRenderDiagnosticsRequested -or $clientLogDiagnosticsRequested
$serverDiagnosticsRequested = $TraceDiagnostics -or $LogStreamVerbose -or $LogNetVerbose
$diagnosticsRequested = $clientDiagnosticsRequested -or $serverDiagnosticsRequested
$targetRoot = Join-Path $repoRoot "target"
$runRoot = Join-Path $targetRoot "run-stack"
$logRoot = Join-Path $runRoot "logs"
$pidFile = Join-Path $runRoot "processes.json"
$rustSysroot = (& rustc --print sysroot).Trim()
$rustTargetLibDir = (& rustc --print target-libdir).Trim()
$rustToolchainBin = Join-Path $rustSysroot "bin"

New-Item -ItemType Directory -Force -Path $logRoot | Out-Null

if (($CefPaintTransport -eq "auto" -or $CefPaintTransport -eq "d3d11on12") -and -not $NoClient) {
    $CefUiDx12AcceleratedPaint = $true
}
if ($CefUiDx12AcceleratedPaint -and -not $NoClient) {
    $CefUi = $true
}

if (Test-Path $pidFile) {
    try {
        $existing = Get-Content -Path $pidFile | ConvertFrom-Json
        foreach ($entry in $existing) {
            $process = Get-Process -Id $entry.pid -ErrorAction SilentlyContinue
            if ($null -ne $process) {
                Write-Host "Stopping previous $($entry.name) pid=$($entry.pid)..."
                Stop-Process -Id $entry.pid -Force
            }
        }
    }
    catch {
        Write-Warning "Could not clean up previous stack metadata: $_"
    }
}

if (-not $env:RUST_BACKTRACE) {
    $env:RUST_BACKTRACE = "1"
}
if ($NoClient) {
    Remove-Item Env:\FUN_START_MODE -ErrorAction SilentlyContinue
}
elseif ($Launcher) {
    $env:FUN_START_MODE = "launcher"
}
else {
    $env:FUN_START_MODE = "game"
}
if ($TraceDiagnostics) {
    $traceFilter = "info,fun=debug,fun::diag=info,fun::perf=info,fun::perf::solari=info,fun::perf::clouds=info,fun::render::clouds=debug,fun::weather=debug,bevy_solari=debug,bevy_solari::realtime=debug,bevy_render::transient=debug,bevy_render::scheduler=trace,bevy_pbr::meshlet::scheduler=trace,bevy_pbr::meshlet::vram=debug"
    $env:RUST_LOG = $traceFilter
    $env:BEVY_LOG = $traceFilter
}
elseif ($RenderDiagnostics -or $FrameTimeDiagnostics) {
    $renderFilter = "info,fun::perf=info,fun::render=debug,bevy_render::transient=debug"
    if (-not $env:RUST_LOG) {
        $env:RUST_LOG = $renderFilter
    }
    if (-not $env:BEVY_LOG) {
        $env:BEVY_LOG = $renderFilter
    }
}
elseif (-not $env:BEVY_LOG) {
    $env:BEVY_LOG = "info"
}
if ($RenderDiagnostics -or $FrameTimeDiagnostics) {
    $env:FUN_RENDER_DIAGNOSTICS = "1"
    $env:FUN_RENDER_UPLOAD_COUNTERS = "1"
    $env:BEVY_RENDER_UPLOAD_COUNTERS = "1"
    $env:FUN_RENDER_CHURN_COUNTERS = "1"
    $env:BEVY_RENDER_CHURN_COUNTERS = "1"
    $env:FUN_RENDER_COMMAND_COUNTERS = "1"
    $env:BEVY_RENDER_COMMAND_COUNTERS = "1"
    $env:FUN_RENDER_READBACK_DIAGNOSTICS = "1"
    $env:BEVY_RENDER_READBACK_DIAGNOSTICS = "1"
    $env:FUN_RENDER_SHADER_DIAGNOSTICS = "1"
    $env:BEVY_RENDER_SHADER_DIAGNOSTICS = "1"
}
else {
    Remove-Item Env:\FUN_RENDER_DIAGNOSTICS -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_RENDER_UPLOAD_COUNTERS -ErrorAction SilentlyContinue
    Remove-Item Env:\BEVY_RENDER_UPLOAD_COUNTERS -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_RENDER_CHURN_COUNTERS -ErrorAction SilentlyContinue
    Remove-Item Env:\BEVY_RENDER_CHURN_COUNTERS -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_RENDER_COMMAND_COUNTERS -ErrorAction SilentlyContinue
    Remove-Item Env:\BEVY_RENDER_COMMAND_COUNTERS -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_RENDER_READBACK_DIAGNOSTICS -ErrorAction SilentlyContinue
    Remove-Item Env:\BEVY_RENDER_READBACK_DIAGNOSTICS -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_RENDER_SHADER_DIAGNOSTICS -ErrorAction SilentlyContinue
    Remove-Item Env:\BEVY_RENDER_SHADER_DIAGNOSTICS -ErrorAction SilentlyContinue
}
if ($CefPaintTransport -eq "default") {
    Remove-Item Env:\FUN_CEF_UI_PAINT_TRANSPORT -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_CEF_UI_ACCELERATED_PAINT -ErrorAction SilentlyContinue
}
else {
    $env:FUN_CEF_UI_PAINT_TRANSPORT = $CefPaintTransport
    $env:FUN_CEF_UI_ACCELERATED_PAINT = switch ($CefPaintTransport) {
        "disabled" { "disabled" }
        "cpu" { "0" }
        "auto" { "auto" }
        "d3d11on12" { "1" }
        default { "" }
    }
}
if ($CefAcceleratedStrict) {
    $env:FUN_CEF_UI_ACCELERATED_STRICT = "1"
}
else {
    Remove-Item Env:\FUN_CEF_UI_ACCELERATED_STRICT -ErrorAction SilentlyContinue
}
$env:FUN_CEF_UI_GPU_RING_DEPTH = [string]$CefGpuRingDepth
if ($CefCopyDirtyRects) {
    $env:FUN_CEF_UI_COPY_DIRTY_RECTS = "1"
}
else {
    Remove-Item Env:\FUN_CEF_UI_COPY_DIRTY_RECTS -ErrorAction SilentlyContinue
}
if ($CefDebugTimings) {
    $env:FUN_CEF_UI_DEBUG_TIMINGS = "1"
}
else {
    Remove-Item Env:\FUN_CEF_UI_DEBUG_TIMINGS -ErrorAction SilentlyContinue
}
if ($FrameTimeDiagnostics) {
    $env:FUN_FRAME_TIME_DIAGNOSTICS = "1"
    $env:FUN_FRAME_TIME_DIAGNOSTIC_INTERVAL = [string]$FrameTimeDiagnosticInterval
    $env:FUN_FRAME_TIME_DIAGNOSTIC_MAX_DEPTH = [string]$FrameTimeDiagnosticMaxDepth
    $env:FUN_FRAME_TIME_DIAGNOSTIC_TOP_CHILDREN = [string]$FrameTimeDiagnosticTopChildren
    $env:FUN_FRAME_TIME_DIAGNOSTIC_TOP_SPANS = [string]$FrameTimeDiagnosticTopSpans
    if ($FrameTimeDiagnosticMinNs -gt 0) {
        $env:FUN_FRAME_TIME_DIAGNOSTIC_MIN_NS = [string]$FrameTimeDiagnosticMinNs
    }
    else {
        Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTIC_MIN_NS -ErrorAction SilentlyContinue
    }
    if ($FrameTimeDiagnosticRowEvents) {
        $env:FUN_FRAME_TIME_DIAGNOSTIC_ROW_EVENTS = "1"
    }
    else {
        Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTIC_ROW_EVENTS -ErrorAction SilentlyContinue
    }
}
else {
    Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTICS -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTIC_INTERVAL -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTIC_MIN_NS -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTIC_MAX_DEPTH -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTIC_TOP_CHILDREN -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTIC_TOP_SPANS -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_FRAME_TIME_DIAGNOSTIC_ROW_EVENTS -ErrorAction SilentlyContinue
}
if ($RenderProfileVerbose) {
    $env:FUN_RENDER_PROFILE_VERBOSE = "1"
}
else {
    Remove-Item Env:\FUN_RENDER_PROFILE_VERBOSE -ErrorAction SilentlyContinue
}
if ($BenchmarkLogMinimal) {
    $env:FUN_BENCHMARK_LOG_MINIMAL = "1"
}
else {
    Remove-Item Env:\FUN_BENCHMARK_LOG_MINIMAL -ErrorAction SilentlyContinue
}
if ($LogStreamVerbose) {
    $env:FUN_LOG_STREAM_VERBOSE = "1"
}
else {
    Remove-Item Env:\FUN_LOG_STREAM_VERBOSE -ErrorAction SilentlyContinue
}
if ($LogNetVerbose) {
    $env:FUN_LOG_NET_VERBOSE = "1"
}
else {
    Remove-Item Env:\FUN_LOG_NET_VERBOSE -ErrorAction SilentlyContinue
}
if ($LogRenderVerbose) {
    $env:FUN_LOG_RENDER_VERBOSE = "1"
}
else {
    Remove-Item Env:\FUN_LOG_RENDER_VERBOSE -ErrorAction SilentlyContinue
}
if ($Maximized) {
    $env:FUN_WINDOW_MAXIMIZED = "1"
}
else {
    Remove-Item Env:\FUN_WINDOW_MAXIMIZED -ErrorAction SilentlyContinue
}
if ($DisableFpsOverlay -or ($CefUi -and -not $EnableFpsOverlay)) {
    $env:FUN_DISABLE_FPS_OVERLAY = "1"
    Remove-Item Env:\FUN_ENABLE_FPS_OVERLAY -ErrorAction SilentlyContinue
}
else {
    Remove-Item Env:\FUN_DISABLE_FPS_OVERLAY -ErrorAction SilentlyContinue
    if (-not $NoClient -and -not $Release) {
        $env:FUN_ENABLE_FPS_OVERLAY = "1"
    }
    else {
        Remove-Item Env:\FUN_ENABLE_FPS_OVERLAY -ErrorAction SilentlyContinue
    }
}
if ($WindowWidth -gt 0 -and $WindowHeight -gt 0) {
    $env:FUN_WINDOW_WIDTH = [string]$WindowWidth
    $env:FUN_WINDOW_HEIGHT = [string]$WindowHeight
}
else {
    Remove-Item Env:\FUN_WINDOW_WIDTH -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_WINDOW_HEIGHT -ErrorAction SilentlyContinue
}
if ($RenderGeometryPolicy) {
    $env:FUN_RENDER_GEOMETRY_POLICY = $RenderGeometryPolicy
}
else {
    Remove-Item Env:\FUN_RENDER_GEOMETRY_POLICY -ErrorAction SilentlyContinue
}
if ($MeshletMinTriangles -gt 0) {
    $env:FUN_MESHLET_MIN_TRIANGLES = [string]$MeshletMinTriangles
}
else {
    Remove-Item Env:\FUN_MESHLET_MIN_TRIANGLES -ErrorAction SilentlyContinue
}
if ($StreamRenderPrepBudgetMs -gt 0) {
    $env:FUN_STREAM_RENDER_PREP_BUDGET_MS = [string]$StreamRenderPrepBudgetMs
}
else {
    Remove-Item Env:\FUN_STREAM_RENDER_PREP_BUDGET_MS -ErrorAction SilentlyContinue
}
if ($StreamRenderPrepMaxChunksPerFrame -gt 0) {
    $env:FUN_STREAM_RENDER_PREP_MAX_CHUNKS_PER_FRAME = [string]$StreamRenderPrepMaxChunksPerFrame
}
else {
    Remove-Item Env:\FUN_STREAM_RENDER_PREP_MAX_CHUNKS_PER_FRAME -ErrorAction SilentlyContinue
}
if ($SolariDebugDirectVisibility) {
    $env:FUN_SOLARI_DEBUG_DIRECT_VISIBILITY = "1"
}
else {
    Remove-Item Env:\FUN_SOLARI_DEBUG_DIRECT_VISIBILITY -ErrorAction SilentlyContinue
}
if ($DisableDlssRr) {
    $env:FUN_DISABLE_DLSS_RR = "1"
}
else {
    Remove-Item Env:\FUN_DISABLE_DLSS_RR -ErrorAction SilentlyContinue
}
if ($EnableDx12DlssRr -and -not $DisableDlssRr) {
    $env:FUN_RENDER_DX12_DLSS_RR = "1"
}
else {
    Remove-Item Env:\FUN_RENDER_DX12_DLSS_RR -ErrorAction SilentlyContinue
}
if ($DisableSolari) {
    $env:FUN_DISABLE_SOLARI = "1"
}
else {
    Remove-Item Env:\FUN_DISABLE_SOLARI -ErrorAction SilentlyContinue
}
if ($DisableMeshlets) {
    $env:FUN_DISABLE_MESHLETS = "1"
}
else {
    Remove-Item Env:\FUN_DISABLE_MESHLETS -ErrorAction SilentlyContinue
}
if ($DisableClouds) {
    $env:FUN_DISABLE_CLOUDS = "1"
}
else {
    Remove-Item Env:\FUN_DISABLE_CLOUDS -ErrorAction SilentlyContinue
}
Set-OptionalEnvValue -Name "FUN_CLOUD_QUALITY" -Value $CloudQuality
Set-OptionalEnvValue -Name "FUN_CLOUD_INTERNAL_SCALE" -Value $CloudInternalScale
Set-OptionalEnvValue -Name "FUN_CLOUD_TEMPORAL" -Value $CloudTemporal
Set-OptionalEnvValue -Name "FUN_CLOUD_SHADOWS" -Value $CloudShadows
Set-OptionalEnvValue -Name "FUN_CLOUD_PROFILE" -Value $CloudProfile
Set-OptionalEnvValue -Name "FUN_CLOUD_DEBUG_OVERLAY" -Value $CloudDebugOverlay
Set-OptionalEnvValue -Name "FUN_RT_SAMPLE_DIRECT" -Value $RtSampleDirect
Set-OptionalEnvValue -Name "FUN_RT_SAMPLE_INDIRECT" -Value $RtSampleIndirect
Set-OptionalEnvValue -Name "FUN_RT_SAMPLE_REFLECTIONS" -Value $RtSampleReflections
Set-OptionalEnvValue -Name "FUN_RT_SURFACE_CACHE" -Value $RtSurfaceCache
Set-OptionalEnvValue -Name "FUN_RT_MEGAGEOM" -Value $RtMegaGeom
Set-OptionalEnvValue -Name "FUN_RT_OPACITY_MASK" -Value $RtOpacityMask
Set-OptionalEnvValue -Name "FUN_RT_HAIR" -Value $RtHair
Set-OptionalEnvValue -Name "FUN_RT_ASYNC_READBACK" -Value $RtAsyncReadback
Set-OptionalEnvValue -Name "FUN_RT_VALIDATION" -Value $RtValidation
if ($RenderUnknownVendor) {
    $env:FUN_RENDER_UNKNOWN_VENDOR = "1"
}
else {
    Remove-Item Env:\FUN_RENDER_UNKNOWN_VENDOR -ErrorAction SilentlyContinue
}
Set-OptionalEnvValue -Name "FUN_RENDER_VENDOR_EMULATION" -Value $RenderVendorEmulation
if ($SolariDenoiseMode) {
    $env:FUN_SOLARI_DENOISE_MODE = $SolariDenoiseMode
}
else {
    Remove-Item Env:\FUN_SOLARI_DENOISE_MODE -ErrorAction SilentlyContinue
}
if ($SolariInternalScale) {
    $env:FUN_SOLARI_INTERNAL_SCALE = $SolariInternalScale
}
else {
    Remove-Item Env:\FUN_SOLARI_INTERNAL_SCALE -ErrorAction SilentlyContinue
}
if ($SolariBlasCompactionVertices -gt 0) {
    $env:FUN_SOLARI_BLAS_COMPACTION_VERTICES = [string]$SolariBlasCompactionVertices
}
else {
    Remove-Item Env:\FUN_SOLARI_BLAS_COMPACTION_VERTICES -ErrorAction SilentlyContinue
}
if ($SolariDebugOverlay) {
    $env:FUN_SOLARI_DEBUG_OVERLAY = $SolariDebugOverlay
}
else {
    Remove-Item Env:\FUN_SOLARI_DEBUG_OVERLAY -ErrorAction SilentlyContinue
}
if ($SolariArch) {
    $env:FUN_SOLARI_ARCH = $SolariArch
}
else {
    Remove-Item Env:\FUN_SOLARI_ARCH -ErrorAction SilentlyContinue
}
if ($SolariTargetFps -gt 0) {
    $env:FUN_SOLARI_TARGET_FPS = [string]$SolariTargetFps
}
else {
    Remove-Item Env:\FUN_SOLARI_TARGET_FPS -ErrorAction SilentlyContinue
}
if ($SolariFrameBudgetNs -gt 0) {
    $env:FUN_SOLARI_FRAME_BUDGET_NS = [string]$SolariFrameBudgetNs
}
else {
    Remove-Item Env:\FUN_SOLARI_FRAME_BUDGET_NS -ErrorAction SilentlyContinue
}
if ($SolariGpuBudgetNs -gt 0) {
    $env:FUN_SOLARI_GPU_BUDGET_NS = [string]$SolariGpuBudgetNs
}
else {
    Remove-Item Env:\FUN_SOLARI_GPU_BUDGET_NS -ErrorAction SilentlyContinue
}
if ($SolariVisualTarget) {
    $env:FUN_SOLARI_VISUAL_TARGET = $SolariVisualTarget
}
else {
    Remove-Item Env:\FUN_SOLARI_VISUAL_TARGET -ErrorAction SilentlyContinue
}
if ($RenderBackend) {
    $env:FUN_RENDER_BACKEND = $RenderBackend
}
else {
    Remove-Item Env:\FUN_RENDER_BACKEND -ErrorAction SilentlyContinue
}
if ($PresentMode) {
    $env:FUN_PRESENT_MODE = $PresentMode
    $env:FUN_RENDER_PRESENT_MODE = $PresentMode
}
else {
    Remove-Item Env:\FUN_PRESENT_MODE -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_RENDER_PRESENT_MODE -ErrorAction SilentlyContinue
}
if ($RenderMaxFrameLatency -gt 0) {
    $env:FUN_RENDER_MAX_FRAME_LATENCY = [string]$RenderMaxFrameLatency
    $env:FUN_PRESENT_MAX_FRAME_LATENCY = [string]$RenderMaxFrameLatency
}
else {
    Remove-Item Env:\FUN_RENDER_MAX_FRAME_LATENCY -ErrorAction SilentlyContinue
    Remove-Item Env:\FUN_PRESENT_MAX_FRAME_LATENCY -ErrorAction SilentlyContinue
}
if (-not $env:FUN_GAME_SERVER_TLS_MODE) {
    $env:FUN_GAME_SERVER_TLS_MODE = "development"
}
if ($env:FUN_GAME_SERVER_TLS_MODE -ieq "development") {
    $serverDevToken = $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN
    $clientSessionToken = $env:FUN_GAME_CLIENT_SESSION_TOKEN

    if ([string]::IsNullOrWhiteSpace($serverDevToken) -and [string]::IsNullOrWhiteSpace($clientSessionToken)) {
        $localStackToken = New-LocalStackSessionToken
        $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN = $localStackToken
        $env:FUN_GAME_CLIENT_SESSION_TOKEN = $localStackToken
        Write-Host "Using ephemeral local development session token for stack client/server admission."
    }
    elseif ([string]::IsNullOrWhiteSpace($serverDevToken)) {
        $env:FUN_GAME_SERVER_DEV_SESSION_TOKEN = $clientSessionToken
        Write-Host "Using FUN_GAME_CLIENT_SESSION_TOKEN for local development server admission."
    }
    elseif ([string]::IsNullOrWhiteSpace($clientSessionToken)) {
        $env:FUN_GAME_CLIENT_SESSION_TOKEN = $serverDevToken
        Write-Host "Using FUN_GAME_SERVER_DEV_SESSION_TOKEN for local development client admission."
    }
    elseif ($serverDevToken -cne $clientSessionToken) {
        Write-Warning "FUN_GAME_SERVER_DEV_SESSION_TOKEN and FUN_GAME_CLIENT_SESSION_TOKEN differ; the local client may be rejected by the development server."
    }
}

$env:PATH = @(
    (Join-Path $targetRoot $profile),
    $rustToolchainBin,
    $rustTargetLibDir,
    $env:PATH
) -join [IO.Path]::PathSeparator

$packages = @("game_shared", "game_server")
if (-not $NoClient) {
    $packages += "game_client"
}

$buildArgs = @("build")
foreach ($package in $packages) {
    $buildArgs += @("-p", $package)
}
if ($Release) {
    $buildArgs += "--release"
}
if (-not $Release -and -not $StaticBevy) {
    $buildArgs += @("--features", "bevy/dynamic_linking")
}
if ($CefUiDx12AcceleratedPaint -and -not $NoClient) {
    $buildArgs += @("--features", "game_client/cef_ui_dx12_accelerated_paint")
}
elseif ($CefUi -and -not $NoClient) {
    $buildArgs += @("--features", "game_client/cef_ui")
}
if ($diagnosticsRequested -and $Release) {
    Write-Warning "Diagnostic flags are debug-build only; no diagnostic features will be compiled into this release build."
}
if ($CefUi -and $NoClient) {
    Write-Warning "Ignoring -CefUi because -NoClient was requested."
}
if (-not $Release) {
    $diagnosticFeatures = @()
    if ($clientRenderDiagnosticsRequested) {
        $diagnosticFeatures += "game_client/render_diagnostics"
    }
    elseif ($clientLogDiagnosticsRequested) {
        $diagnosticFeatures += "game_client/diagnostics"
    }
    if ($serverDiagnosticsRequested) {
        $diagnosticFeatures += "game_server/diagnostics"
    }
    if ($diagnosticFeatures.Count -gt 0) {
        $buildArgs += @("--features", ($diagnosticFeatures -join ","))
    }
}

Push-Location $repoRoot
try {
    Write-Host "Building $($packages -join ', ') in $profile profile..."
    if (-not $Release -and -not $StaticBevy) {
        Write-Host "Using Bevy dynamic linking for faster iterative stack builds."
    }
    if ($CefUi -and -not $NoClient) {
        Write-Host "Enabling game_client/cef_ui for the CEF browser UI."
    }
    if (-not $NoClient) {
        Write-Host "Starting unified Fun client in $env:FUN_START_MODE mode."
    }
    if (-not $Release -and $diagnosticsRequested) {
        if ($clientRenderDiagnosticsRequested) {
            Write-Host "Enabling game_client/render_diagnostics for diagnostic build."
        }
        elseif ($clientLogDiagnosticsRequested) {
            Write-Host "Enabling game_client/diagnostics for diagnostic build."
        }
        if ($serverDiagnosticsRequested) {
            Write-Host "Enabling game_server/diagnostics for diagnostic build."
        }
    }
    & cargo @buildArgs
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    if (-not $Release -and -not $StaticBevy) {
        Get-ChildItem -Path $rustTargetLibDir -Filter "std-*.dll" | ForEach-Object {
            Copy-Item -LiteralPath $_.FullName -Destination (Join-Path $targetRoot $profile) -Force
        }
    }
    if ($CefUi -and -not $NoClient) {
        Copy-CefRuntimeFiles -ProfileTargetDir (Join-Path $targetRoot $profile)
    }
}
finally {
    Pop-Location
}

function Get-BinaryPath {
    param([string]$Name)

    $extension = if ($IsWindows -or $env:OS -eq "Windows_NT") { ".exe" } else { "" }
    return Join-Path (Join-Path $targetRoot $profile) "$Name$extension"
}

function Start-FunProcess {
    param(
        [string]$Name,
        [switch]$Visible
    )

    $binaryPath = Get-BinaryPath $Name
    if (-not (Test-Path $binaryPath)) {
        throw "Expected binary was not built: $binaryPath"
    }

    $stdout = Join-Path $logRoot "$Name.out.log"
    $stderr = Join-Path $logRoot "$Name.err.log"

    $startArgs = @{
        FilePath = $binaryPath
        WorkingDirectory = $repoRoot
        PassThru = $true
        RedirectStandardOutput = $stdout
        RedirectStandardError = $stderr
    }

    if (-not $Visible) {
        $startArgs.WindowStyle = "Hidden"
    }

    $process = Start-Process @startArgs

    [pscustomobject]@{
        name = $Name
        pid = $process.Id
        path = $binaryPath
        stdout = $stdout
        stderr = $stderr
        visible = [bool]$Visible
    }
}

$started = @()

$started += Start-FunProcess -Name "game_server"

if (-not $NoClient) {
    Start-Sleep -Seconds $StartupDelaySeconds
    $started += Start-FunProcess -Name "game_client" -Visible
}

$started | ConvertTo-Json -Depth 3 | Set-Content -Path $pidFile

Start-Sleep -Milliseconds 750

foreach ($entry in $started) {
    $process = Get-Process -Id $entry.pid -ErrorAction SilentlyContinue
    if ($null -eq $process) {
        Write-Warning "$($entry.name) exited immediately. Check $($entry.stderr) and $($entry.stdout)."
    }
    else {
        Write-Host "Started $($entry.name) pid=$($entry.pid)"
    }
}

Write-Host "Process metadata: $pidFile"
Write-Host "Logs: $logRoot"
