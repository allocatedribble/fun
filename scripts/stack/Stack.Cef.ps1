function Resolve-StackCefRequest {
    param(
        [pscustomobject]$Request,
        [pscustomobject]$Paths
    )

    $events = @()
    if ($Request.CefUiDx12AcceleratedPaint -and -not $Request.NoClient -and $Request.CefPaintTransport -eq "default") {
        $Request.CefPaintTransport = "d3d11on12"
    }
    if ($Request.CefUi -and -not $Request.NoClient -and $Request.CefPaintTransport -eq "default") {
        $Request.CefPaintTransport = "d3d11on12"
        $Request.CefUiDx12AcceleratedPaint = $true
    }
    if (($Request.CefPaintTransport -eq "auto" -or $Request.CefPaintTransport -eq "d3d11on12") -and -not $Request.NoClient) {
        $Request.CefUiDx12AcceleratedPaint = $true
    }
    if ($Request.CefUiDx12AcceleratedPaint -and -not $Request.NoClient) {
        $Request.CefUi = $true
    }
    if ($Request.CefUi -and $Request.NoClient) {
        $events += [pscustomobject]@{
            id = "cef_ignored_no_client"
            severity = "warn"
            message = "Ignoring CEF UI because NoClient was requested."
        }
    }
    return $events
}

function Set-StackCefEnv {
    param(
        [pscustomobject]$Request,
        [pscustomobject]$Paths
    )

    Remove-Item -LiteralPath $Paths.cef_transport_status_file -ErrorAction SilentlyContinue
    if ($Request.CefUi -and -not $Request.NoClient) {
        $env:FUN_CEF_UI_TRANSPORT_STATUS_PATH = $Paths.cef_transport_status_file
    }
    else {
        Clear-StackEnvValue -Name "FUN_CEF_UI_TRANSPORT_STATUS_PATH"
    }

    if ($Request.CefPaintTransport -eq "default") {
        Clear-StackEnvValue -Name "FUN_CEF_UI_PAINT_TRANSPORT"
        Clear-StackEnvValue -Name "FUN_CEF_UI_ACCELERATED_PAINT"
    }
    else {
        $env:FUN_CEF_UI_PAINT_TRANSPORT = $Request.CefPaintTransport
        $env:FUN_CEF_UI_ACCELERATED_PAINT = switch ($Request.CefPaintTransport) {
            "disabled" { "disabled" }
            "cpu" { "0" }
            "auto" { "auto" }
            "d3d11on12" { "1" }
            default { "" }
        }
    }

    Set-StackFlagEnv -Name "FUN_CEF_UI_ACCELERATED_STRICT" -Enabled $Request.CefAcceleratedStrict
    $env:FUN_CEF_UI_GPU_RING_DEPTH = [string]$Request.CefGpuRingDepth
    Set-StackFlagEnv -Name "FUN_CEF_UI_COPY_DIRTY_RECTS" -Enabled $Request.CefCopyDirtyRects
    Set-StackFlagEnv -Name "FUN_CEF_UI_DEBUG_TIMINGS" -Enabled $Request.CefDebugTimings
}

function Get-StackCefRuntimePath {
    param([string]$ProfileTargetDir)

    $buildDir = Join-Path $ProfileTargetDir "build"
    if (-not (Test-Path $buildDir)) {
        throw "CEF runtime build directory was not found: $buildDir"
    }

    return Get-ChildItem -Path $buildDir -Directory -Filter "cef-dll-sys-*" |
        ForEach-Object { Join-Path $_.FullName "out\cef_windows_x86_64" } |
        Where-Object { Test-Path (Join-Path $_ "libcef.dll") } |
        Sort-Object { (Get-Item (Join-Path $_ "libcef.dll")).LastWriteTimeUtc } -Descending |
        Select-Object -First 1
}

function Copy-CefRuntimeFiles {
    param([string]$ProfileTargetDir)

    $cefRuntime = Get-StackCefRuntimePath -ProfileTargetDir $ProfileTargetDir
    if ([string]::IsNullOrWhiteSpace($cefRuntime)) {
        throw "CEF runtime files were not found under $(Join-Path $ProfileTargetDir "build")"
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
