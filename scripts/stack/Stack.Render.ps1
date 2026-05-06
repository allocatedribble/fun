function Set-StackRenderEnv {
    param(
        [pscustomobject]$Request,
        [pscustomobject]$Paths = $null
    )

    Set-StackFlagEnv -Name "FUN_WINDOW_MAXIMIZED" -Enabled $Request.Maximized
    if ($null -ne $Paths -and -not $Request.NoClient) {
        Remove-Item -LiteralPath $Paths.renderer_capability_report_file -ErrorAction SilentlyContinue
        $env:FUN_RENDERER_CAPABILITY_REPORT_PATH = $Paths.renderer_capability_report_file
    }
    else {
        Clear-StackEnvValue -Name "FUN_RENDERER_CAPABILITY_REPORT_PATH"
    }

    if ($Request.DisableFpsOverlay -or ($Request.CefUi -and -not $Request.NoClient -and -not $Request.EnableFpsOverlay)) {
        $env:FUN_DISABLE_FPS_OVERLAY = "1"
        Clear-StackEnvValue -Name "FUN_ENABLE_FPS_OVERLAY"
    }
    else {
        Clear-StackEnvValue -Name "FUN_DISABLE_FPS_OVERLAY"
        if (-not $Request.NoClient -and -not $Request.Release) {
            $env:FUN_ENABLE_FPS_OVERLAY = "1"
        }
        else {
            Clear-StackEnvValue -Name "FUN_ENABLE_FPS_OVERLAY"
        }
    }

    if ($Request.WindowWidth -gt 0 -and $Request.WindowHeight -gt 0) {
        $env:FUN_WINDOW_WIDTH = [string]$Request.WindowWidth
        $env:FUN_WINDOW_HEIGHT = [string]$Request.WindowHeight
    }
    else {
        Clear-StackEnvValue -Name "FUN_WINDOW_WIDTH"
        Clear-StackEnvValue -Name "FUN_WINDOW_HEIGHT"
    }

    Set-OptionalEnvValue -Name "FUN_RENDER_GEOMETRY_POLICY" -Value $Request.RenderGeometryPolicy
    Set-OptionalEnvValue -Name "FUN_MESHLET_MIN_TRIANGLES" -Value $(if ($Request.MeshletMinTriangles -gt 0) { [string]$Request.MeshletMinTriangles } else { "" })
    Set-OptionalEnvValue -Name "FUN_STREAM_RENDER_PREP_BUDGET_MS" -Value $(if ($Request.StreamRenderPrepBudgetMs -gt 0) { [string]$Request.StreamRenderPrepBudgetMs } else { "" })
    Set-OptionalEnvValue -Name "FUN_STREAM_RENDER_PREP_MAX_CHUNKS_PER_FRAME" -Value $(if ($Request.StreamRenderPrepMaxChunksPerFrame -gt 0) { [string]$Request.StreamRenderPrepMaxChunksPerFrame } else { "" })

    Set-StackFlagEnv -Name "FUN_SOLARI_DEBUG_DIRECT_VISIBILITY" -Enabled $Request.SolariDebugDirectVisibility
    Set-StackFlagEnv -Name "FUN_DISABLE_DLSS_RR" -Enabled $Request.DisableDlssRr
    Set-StackFlagEnv -Name "FUN_RENDER_DX12_DLSS_RR" -Enabled ($Request.EnableDx12DlssRr -and -not $Request.DisableDlssRr)
    Set-StackFlagEnv -Name "FUN_DISABLE_SOLARI" -Enabled $Request.DisableSolari
    Set-StackFlagEnv -Name "FUN_DISABLE_MESHLETS" -Enabled $Request.DisableMeshlets
    Set-StackFlagEnv -Name "FUN_DISABLE_CLOUDS" -Enabled $Request.DisableClouds

    Set-OptionalEnvValue -Name "FUN_CLOUD_QUALITY" -Value $Request.CloudQuality
    Set-OptionalEnvValue -Name "FUN_CLOUD_INTERNAL_SCALE" -Value $Request.CloudInternalScale
    Set-OptionalEnvValue -Name "FUN_CLOUD_TEMPORAL" -Value $Request.CloudTemporal
    Set-OptionalEnvValue -Name "FUN_CLOUD_SHADOWS" -Value $Request.CloudShadows
    Set-OptionalEnvValue -Name "FUN_CLOUD_PROFILE" -Value $Request.CloudProfile
    Set-OptionalEnvValue -Name "FUN_CLOUD_DEBUG_OVERLAY" -Value $Request.CloudDebugOverlay

    Set-OptionalEnvValue -Name "FUN_RT_SAMPLE_DIRECT" -Value $Request.RtSampleDirect
    Set-OptionalEnvValue -Name "FUN_RT_SAMPLE_INDIRECT" -Value $Request.RtSampleIndirect
    Set-OptionalEnvValue -Name "FUN_RT_SAMPLE_REFLECTIONS" -Value $Request.RtSampleReflections
    Set-OptionalEnvValue -Name "FUN_RT_SURFACE_CACHE" -Value $Request.RtSurfaceCache
    Set-OptionalEnvValue -Name "FUN_RT_MEGAGEOM" -Value $Request.RtMegaGeom
    Set-OptionalEnvValue -Name "FUN_RT_OPACITY_MASK" -Value $Request.RtOpacityMask
    Set-OptionalEnvValue -Name "FUN_RT_HAIR" -Value $Request.RtHair
    Set-OptionalEnvValue -Name "FUN_RT_ASYNC_READBACK" -Value $Request.RtAsyncReadback
    Set-OptionalEnvValue -Name "FUN_RT_VALIDATION" -Value $Request.RtValidation
    Set-StackFlagEnv -Name "FUN_RENDER_UNKNOWN_VENDOR" -Enabled $Request.RenderUnknownVendor
    Set-OptionalEnvValue -Name "FUN_RENDER_VENDOR_EMULATION" -Value $Request.RenderVendorEmulation

    Set-OptionalEnvValue -Name "FUN_SOLARI_DENOISE_MODE" -Value $Request.SolariDenoiseMode
    Set-OptionalEnvValue -Name "FUN_SOLARI_INTERNAL_SCALE" -Value $Request.SolariInternalScale
    Set-OptionalEnvValue -Name "FUN_SOLARI_BLAS_COMPACTION_VERTICES" -Value $(if ($Request.SolariBlasCompactionVertices -gt 0) { [string]$Request.SolariBlasCompactionVertices } else { "" })
    Set-OptionalEnvValue -Name "FUN_SOLARI_DEBUG_OVERLAY" -Value $Request.SolariDebugOverlay
    Set-OptionalEnvValue -Name "FUN_SOLARI_ARCH" -Value $Request.SolariArch
    Set-OptionalEnvValue -Name "FUN_SOLARI_TARGET_FPS" -Value $(if ($Request.SolariTargetFps -gt 0) { [string]$Request.SolariTargetFps } else { "" })
    Set-OptionalEnvValue -Name "FUN_SOLARI_FRAME_BUDGET_NS" -Value $(if ($Request.SolariFrameBudgetNs -gt 0) { [string]$Request.SolariFrameBudgetNs } else { "" })
    Set-OptionalEnvValue -Name "FUN_SOLARI_GPU_BUDGET_NS" -Value $(if ($Request.SolariGpuBudgetNs -gt 0) { [string]$Request.SolariGpuBudgetNs } else { "" })
    Set-OptionalEnvValue -Name "FUN_SOLARI_VISUAL_TARGET" -Value $Request.SolariVisualTarget

    Set-OptionalEnvValue -Name "FUN_RENDER_BACKEND" -Value $Request.RenderBackend
    if ($Request.PresentMode) {
        $env:FUN_PRESENT_MODE = $Request.PresentMode
        $env:FUN_RENDER_PRESENT_MODE = $Request.PresentMode
    }
    else {
        Clear-StackEnvValue -Name "FUN_PRESENT_MODE"
        Clear-StackEnvValue -Name "FUN_RENDER_PRESENT_MODE"
    }
    if ($Request.RenderMaxFrameLatency -gt 0) {
        $env:FUN_RENDER_MAX_FRAME_LATENCY = [string]$Request.RenderMaxFrameLatency
        $env:FUN_PRESENT_MAX_FRAME_LATENCY = [string]$Request.RenderMaxFrameLatency
    }
    else {
        Clear-StackEnvValue -Name "FUN_RENDER_MAX_FRAME_LATENCY"
        Clear-StackEnvValue -Name "FUN_PRESENT_MAX_FRAME_LATENCY"
    }
}
