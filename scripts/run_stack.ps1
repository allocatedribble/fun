param(
    [switch]$Release,
    [switch]$StaticBevy,
    [switch]$NoClient,
    [switch]$RenderDiagnostics,
    [switch]$TraceDiagnostics,
    [switch]$RenderProfileVerbose,
    [switch]$Maximized,
    [switch]$SolariDebugDirectVisibility,
    [switch]$DisableDlssRr,
    [switch]$DisableSolari,
    [switch]$DisableMeshlets,
    [string]$SolariArch = "budgeted",
    [int]$SolariTargetFps = 144,
    [int]$SolariFrameBudgetNs = 6944444,
    [int]$SolariGpuBudgetNs = 3000000,
    [string]$SolariVisualTarget = "competitive",
    [string]$SolariDenoiseMode = "balanced-fast",
    [string]$SolariInternalScale = "1.0",
    [string]$SolariDebugOverlay = "",
    [string]$RenderBackend = "vulkan",
    [string]$PresentMode = "immediate",
    [int]$StartupDelaySeconds = 2
)

$ErrorActionPreference = "Stop"

$scriptRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptRoot "..")
$profile = if ($Release) { "release" } else { "debug" }
$targetRoot = Join-Path $repoRoot "target"
$runRoot = Join-Path $targetRoot "run-stack"
$logRoot = Join-Path $runRoot "logs"
$pidFile = Join-Path $runRoot "processes.json"
$rustSysroot = (& rustc --print sysroot).Trim()
$rustTargetLibDir = (& rustc --print target-libdir).Trim()
$rustToolchainBin = Join-Path $rustSysroot "bin"

New-Item -ItemType Directory -Force -Path $logRoot | Out-Null

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
if ($TraceDiagnostics) {
    $traceFilter = "info,fun=debug,fun::diag=info,fun::perf=info,fun::perf::solari=info,bevy_solari=debug,bevy_solari::realtime=debug"
    $env:RUST_LOG = $traceFilter
    $env:BEVY_LOG = $traceFilter
}
elseif (-not $env:BEVY_LOG) {
    $env:BEVY_LOG = "info"
}
if ($RenderDiagnostics) {
    $env:FUN_RENDER_DIAGNOSTICS = "1"
}
else {
    Remove-Item Env:\FUN_RENDER_DIAGNOSTICS -ErrorAction SilentlyContinue
}
if ($RenderProfileVerbose) {
    $env:FUN_RENDER_PROFILE_VERBOSE = "1"
}
else {
    Remove-Item Env:\FUN_RENDER_PROFILE_VERBOSE -ErrorAction SilentlyContinue
}
if ($Maximized) {
    $env:FUN_WINDOW_MAXIMIZED = "1"
}
else {
    Remove-Item Env:\FUN_WINDOW_MAXIMIZED -ErrorAction SilentlyContinue
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
}
else {
    Remove-Item Env:\FUN_PRESENT_MODE -ErrorAction SilentlyContinue
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

Push-Location $repoRoot
try {
    Write-Host "Building $($packages -join ', ') in $profile profile..."
    if (-not $Release -and -not $StaticBevy) {
        Write-Host "Using Bevy dynamic linking for faster iterative stack builds."
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
