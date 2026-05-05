function New-StackBuildPlan {
    param(
        [pscustomobject]$Request,
        [pscustomobject]$Diagnostics
    )

    $packages = @("game_shared")
    if ($Request.Server) {
        $packages += "game_server"
    }
    if (-not $Request.NoClient) {
        $packages += "game_client"
    }

    $features = @()
    $args = @("build")
    foreach ($package in $packages) {
        $args += @("-p", $package)
    }
    if ($Request.Release) {
        $args += "--release"
    }
    if (-not $Request.Release -and -not $Request.StaticBevy) {
        $features += "bevy/dynamic_linking"
    }
    if ($Request.CefUiDx12AcceleratedPaint -and -not $Request.NoClient) {
        $features += "game_client/cef_ui_dx12_accelerated_paint"
    }
    elseif ($Request.CefUi -and -not $Request.NoClient) {
        $features += "game_client/cef_ui"
    }

    $warnings = @()
    if ($Diagnostics.any -and $Request.Release) {
        $warnings += [pscustomobject]@{
            id = "release_debug_diagnostics_ignored"
            severity = "warn"
            message = "Diagnostic flags are debug-build only; no diagnostic features will be compiled into this release build."
        }
    }
    if (-not $Request.Release) {
        if ($Diagnostics.client_render) {
            $features += "game_client/render_diagnostics"
        }
        elseif ($Diagnostics.client_log) {
            $features += "game_client/diagnostics"
        }
        if ($Diagnostics.server) {
            $features += "game_server/diagnostics"
        }
    }

    if ($features.Count -gt 0) {
        $args += @("--features", ($features -join ","))
    }

    [pscustomobject]@{
        profile = if ($Request.Release) { "release" } else { "debug" }
        packages = $packages
        features = $features
        args = $args
        dynamic_linking = (-not $Request.Release -and -not $Request.StaticBevy)
        warnings = $warnings
    }
}

function Invoke-StackBuild {
    param(
        [pscustomobject]$Paths,
        [pscustomobject]$BuildPlan
    )

    Push-Location $Paths.repo_root
    $oldPreference = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    try {
        Write-Host "Building $($BuildPlan.packages -join ', ') in $($BuildPlan.profile) profile..."
        if ($BuildPlan.dynamic_linking) {
            Write-Host "Using Bevy dynamic linking for faster iterative stack builds."
        }
        & cargo @($BuildPlan.args)
        $exitCode = $LASTEXITCODE
    }
    finally {
        $ErrorActionPreference = $oldPreference
        Pop-Location
    }
    if ($exitCode -ne 0) {
        throw "cargo build failed with exit code $exitCode"
    }
}

function Copy-StackDynamicLinkLibraries {
    param(
        [pscustomobject]$Paths,
        [pscustomobject]$BuildPlan
    )

    if (-not $BuildPlan.dynamic_linking) {
        return
    }
    Get-ChildItem -Path $Paths.rust_target_lib_dir -Filter "std-*.dll" | ForEach-Object {
        Copy-Item -LiteralPath $_.FullName -Destination $Paths.profile_target_dir -Force
    }
}
