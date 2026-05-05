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

function Clear-StackEnvValue {
    param([string]$Name)

    Remove-Item "Env:\$Name" -ErrorAction SilentlyContinue
}

function Set-StackFlagEnv {
    param(
        [string]$Name,
        [bool]$Enabled
    )

    if ($Enabled) {
        [System.Environment]::SetEnvironmentVariable($Name, "1", "Process")
    }
    else {
        Clear-StackEnvValue -Name $Name
    }
}

function Set-StackRuntimeEnv {
    param([pscustomobject]$Request)

    if (-not $env:RUST_BACKTRACE) {
        $env:RUST_BACKTRACE = "1"
    }
    if ($Request.NoClient) {
        Clear-StackEnvValue -Name "FUN_START_MODE"
    }
    elseif ($Request.Launcher) {
        $env:FUN_START_MODE = "launcher"
    }
    else {
        $env:FUN_START_MODE = "game"
    }
}

function Set-StackPathEnv {
    param([pscustomobject]$Paths)

    $env:PATH = @(
        $Paths.profile_target_dir,
        $Paths.rust_toolchain_bin,
        $Paths.rust_target_lib_dir,
        $env:PATH
    ) -join [IO.Path]::PathSeparator
}

function Set-StackProfileEnv {
    param([pscustomobject]$Request)

    if ($null -eq $Request.ProfileEnv) {
        return
    }
    foreach ($property in $Request.ProfileEnv.PSObject.Properties) {
        Set-OptionalEnvValue -Name $property.Name -Value ([string]$property.Value)
    }
}

function Get-StackKnownEnvNames {
    param([pscustomobject]$Request)

    $names = @(
        "RUST_BACKTRACE",
        "RUST_LOG",
        "BEVY_LOG",
        "FUN_START_MODE",
        "FUN_RENDER_DIAGNOSTICS",
        "FUN_RENDER_UPLOAD_COUNTERS",
        "BEVY_RENDER_UPLOAD_COUNTERS",
        "FUN_RENDER_CHURN_COUNTERS",
        "BEVY_RENDER_CHURN_COUNTERS",
        "FUN_RENDER_COMMAND_COUNTERS",
        "BEVY_RENDER_COMMAND_COUNTERS",
        "FUN_RENDER_READBACK_DIAGNOSTICS",
        "BEVY_RENDER_READBACK_DIAGNOSTICS",
        "FUN_RENDER_SHADER_DIAGNOSTICS",
        "BEVY_RENDER_SHADER_DIAGNOSTICS",
        "FUN_CEF_UI_TRANSPORT_STATUS_PATH",
        "FUN_CEF_UI_PAINT_TRANSPORT",
        "FUN_CEF_UI_ACCELERATED_PAINT",
        "FUN_CEF_UI_ACCELERATED_STRICT",
        "FUN_CEF_UI_GPU_RING_DEPTH",
        "FUN_CEF_UI_COPY_DIRTY_RECTS",
        "FUN_CEF_UI_DEBUG_TIMINGS",
        "FUN_FRAME_TIME_DIAGNOSTICS",
        "FUN_FRAME_TIME_DIAGNOSTIC_INTERVAL",
        "FUN_FRAME_TIME_DIAGNOSTIC_MIN_NS",
        "FUN_FRAME_TIME_DIAGNOSTIC_MAX_DEPTH",
        "FUN_FRAME_TIME_DIAGNOSTIC_TOP_CHILDREN",
        "FUN_FRAME_TIME_DIAGNOSTIC_TOP_SPANS",
        "FUN_FRAME_TIME_DIAGNOSTIC_ROW_EVENTS",
        "FUN_RENDER_PROFILE_VERBOSE",
        "FUN_BENCHMARK_LOG_MINIMAL",
        "FUN_LOG_STREAM_VERBOSE",
        "FUN_LOG_NET_VERBOSE",
        "FUN_LOG_RENDER_VERBOSE",
        "FUN_WINDOW_MAXIMIZED",
        "FUN_DISABLE_FPS_OVERLAY",
        "FUN_ENABLE_FPS_OVERLAY",
        "FUN_WINDOW_WIDTH",
        "FUN_WINDOW_HEIGHT",
        "FUN_RENDER_GEOMETRY_POLICY",
        "FUN_MESHLET_MIN_TRIANGLES",
        "FUN_STREAM_RENDER_PREP_BUDGET_MS",
        "FUN_STREAM_RENDER_PREP_MAX_CHUNKS_PER_FRAME",
        "FUN_SOLARI_DEBUG_DIRECT_VISIBILITY",
        "FUN_DISABLE_DLSS_RR",
        "FUN_RENDER_DX12_DLSS_RR",
        "FUN_DISABLE_SOLARI",
        "FUN_DISABLE_MESHLETS",
        "FUN_DISABLE_CLOUDS",
        "FUN_CLOUD_QUALITY",
        "FUN_CLOUD_INTERNAL_SCALE",
        "FUN_CLOUD_TEMPORAL",
        "FUN_CLOUD_SHADOWS",
        "FUN_CLOUD_PROFILE",
        "FUN_CLOUD_DEBUG_OVERLAY",
        "FUN_RT_SAMPLE_DIRECT",
        "FUN_RT_SAMPLE_INDIRECT",
        "FUN_RT_SAMPLE_REFLECTIONS",
        "FUN_RT_SURFACE_CACHE",
        "FUN_RT_MEGAGEOM",
        "FUN_RT_OPACITY_MASK",
        "FUN_RT_HAIR",
        "FUN_RT_ASYNC_READBACK",
        "FUN_RT_VALIDATION",
        "FUN_RENDER_UNKNOWN_VENDOR",
        "FUN_RENDER_VENDOR_EMULATION",
        "FUN_SOLARI_DENOISE_MODE",
        "FUN_SOLARI_INTERNAL_SCALE",
        "FUN_SOLARI_BLAS_COMPACTION_VERTICES",
        "FUN_SOLARI_DEBUG_OVERLAY",
        "FUN_SOLARI_ARCH",
        "FUN_SOLARI_TARGET_FPS",
        "FUN_SOLARI_FRAME_BUDGET_NS",
        "FUN_SOLARI_GPU_BUDGET_NS",
        "FUN_SOLARI_VISUAL_TARGET",
        "FUN_RENDER_BACKEND",
        "FUN_PRESENT_MODE",
        "FUN_RENDER_PRESENT_MODE",
        "FUN_RENDER_MAX_FRAME_LATENCY",
        "FUN_PRESENT_MAX_FRAME_LATENCY",
        "FUN_GAME_SERVER_TLS_MODE",
        "FUN_GAME_SERVER_DEV_SESSION_TOKEN",
        "FUN_GAME_CLIENT_SESSION_TOKEN"
    )
    if ($null -ne $Request.ProfileEnv) {
        $names += @($Request.ProfileEnv.PSObject.Properties | ForEach-Object { $_.Name })
    }
    return $names | Sort-Object -Unique
}

function Export-StackEnvSnapshot {
    param([pscustomobject]$Request)

    $snapshot = [ordered]@{}
    foreach ($name in Get-StackKnownEnvNames -Request $Request) {
        $value = [System.Environment]::GetEnvironmentVariable($name, "Process")
        if ($null -ne $value) {
            if ($name -match "(TOKEN|SECRET|PASSWORD|KEY|COOKIE|TICKET)") {
                $snapshot[$name] = "<redacted>"
            }
            else {
                $snapshot[$name] = $value
            }
        }
    }
    return $snapshot
}
