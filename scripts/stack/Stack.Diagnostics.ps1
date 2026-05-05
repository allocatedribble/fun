function Get-StackDiagnosticsState {
    param([pscustomobject]$Request)

    $clientRenderDiagnosticsRequested = -not $Request.NoClient -and (
        $Request.RenderDiagnostics -or
        $Request.TraceDiagnostics -or
        $Request.RenderProfileVerbose -or
        $Request.FrameTimeDiagnostics
    )
    $clientLogDiagnosticsRequested = -not $Request.NoClient -and (
        $Request.LogStreamVerbose -or
        $Request.LogNetVerbose -or
        $Request.LogRenderVerbose
    )
    $serverDiagnosticsRequested = $Request.TraceDiagnostics -or $Request.LogStreamVerbose -or $Request.LogNetVerbose

    [pscustomobject]@{
        client_render = [bool]$clientRenderDiagnosticsRequested
        client_log = [bool]$clientLogDiagnosticsRequested
        client_any = [bool]($clientRenderDiagnosticsRequested -or $clientLogDiagnosticsRequested)
        server = [bool]$serverDiagnosticsRequested
        any = [bool]($clientRenderDiagnosticsRequested -or $clientLogDiagnosticsRequested -or $serverDiagnosticsRequested)
    }
}

function Set-StackDiagnosticsEnv {
    param(
        [pscustomobject]$Request,
        [pscustomobject]$Diagnostics
    )

    if ($Request.TraceDiagnostics) {
        $traceFilter = "info,fun=debug,fun::diag=info,fun::perf=info,fun::perf::solari=info,fun::perf::clouds=info,fun::render::clouds=debug,fun::weather=debug,bevy_solari=debug,bevy_solari::realtime=debug,bevy_render::transient=debug,bevy_render::scheduler=trace,bevy_pbr::meshlet::scheduler=trace,bevy_pbr::meshlet::vram=debug"
        $env:RUST_LOG = $traceFilter
        $env:BEVY_LOG = $traceFilter
    }
    elseif ($Request.RenderDiagnostics -or $Request.FrameTimeDiagnostics) {
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

    $renderCountersEnabled = $Request.RenderDiagnostics -or $Request.FrameTimeDiagnostics
    foreach ($name in @(
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
        "BEVY_RENDER_SHADER_DIAGNOSTICS"
    )) {
        Set-StackFlagEnv -Name $name -Enabled $renderCountersEnabled
    }

    if ($Request.FrameTimeDiagnostics) {
        $env:FUN_FRAME_TIME_DIAGNOSTICS = "1"
        $env:FUN_FRAME_TIME_DIAGNOSTIC_INTERVAL = [string]$Request.FrameTimeDiagnosticInterval
        $env:FUN_FRAME_TIME_DIAGNOSTIC_MAX_DEPTH = [string]$Request.FrameTimeDiagnosticMaxDepth
        $env:FUN_FRAME_TIME_DIAGNOSTIC_TOP_CHILDREN = [string]$Request.FrameTimeDiagnosticTopChildren
        $env:FUN_FRAME_TIME_DIAGNOSTIC_TOP_SPANS = [string]$Request.FrameTimeDiagnosticTopSpans
        Set-OptionalEnvValue -Name "FUN_FRAME_TIME_DIAGNOSTIC_MIN_NS" -Value $(if ($Request.FrameTimeDiagnosticMinNs -gt 0) { [string]$Request.FrameTimeDiagnosticMinNs } else { "" })
        Set-StackFlagEnv -Name "FUN_FRAME_TIME_DIAGNOSTIC_ROW_EVENTS" -Enabled $Request.FrameTimeDiagnosticRowEvents
    }
    else {
        foreach ($name in @(
            "FUN_FRAME_TIME_DIAGNOSTICS",
            "FUN_FRAME_TIME_DIAGNOSTIC_INTERVAL",
            "FUN_FRAME_TIME_DIAGNOSTIC_MIN_NS",
            "FUN_FRAME_TIME_DIAGNOSTIC_MAX_DEPTH",
            "FUN_FRAME_TIME_DIAGNOSTIC_TOP_CHILDREN",
            "FUN_FRAME_TIME_DIAGNOSTIC_TOP_SPANS",
            "FUN_FRAME_TIME_DIAGNOSTIC_ROW_EVENTS"
        )) {
            Clear-StackEnvValue -Name $name
        }
    }

    Set-StackFlagEnv -Name "FUN_RENDER_PROFILE_VERBOSE" -Enabled $Request.RenderProfileVerbose
    Set-StackFlagEnv -Name "FUN_BENCHMARK_LOG_MINIMAL" -Enabled $Request.BenchmarkLogMinimal
    Set-StackFlagEnv -Name "FUN_LOG_STREAM_VERBOSE" -Enabled $Request.LogStreamVerbose
    Set-StackFlagEnv -Name "FUN_LOG_NET_VERBOSE" -Enabled $Request.LogNetVerbose
    Set-StackFlagEnv -Name "FUN_LOG_RENDER_VERBOSE" -Enabled $Request.LogRenderVerbose
}
