function Get-StackObjectMap {
    param([object]$Object)

    $map = [ordered]@{}
    if ($null -eq $Object) {
        return $map
    }
    foreach ($property in $Object.PSObject.Properties) {
        $map[$property.Name] = $property.Value
    }
    return $map
}

function Get-StackDefaultValues {
    [ordered]@{
        Profile = "default.dx12.immediate"
        PlanOnly = $false
        Release = $false
        StaticBevy = $false
        Server = $true
        NoClient = $false
        Launcher = $false
        CefUi = $false
        CefUiDx12AcceleratedPaint = $false
        CefPaintTransport = "default"
        CefAcceleratedStrict = $false
        CefGpuRingDepth = 3
        CefCopyDirtyRects = $false
        CefDebugTimings = $false
        RenderDiagnostics = $false
        TraceDiagnostics = $false
        RenderProfileVerbose = $false
        FrameTimeDiagnostics = $false
        BenchmarkLogMinimal = $false
        LogStreamVerbose = $false
        LogNetVerbose = $false
        LogRenderVerbose = $false
        Maximized = $false
        DisableFpsOverlay = $false
        EnableFpsOverlay = $false
        SolariDebugDirectVisibility = $false
        EnableDx12DlssRr = $false
        DisableDlssRr = $false
        DisableSolari = $false
        DisableMeshlets = $false
        DisableClouds = $false
        RtSampleDirect = ""
        RtSampleIndirect = ""
        RtSampleReflections = ""
        RtSurfaceCache = ""
        RtMegaGeom = ""
        RtOpacityMask = ""
        RtHair = ""
        RtAsyncReadback = ""
        RtValidation = ""
        RenderUnknownVendor = $false
        RenderVendorEmulation = ""
        SolariArch = "budgeted"
        SolariTargetFps = 144
        SolariFrameBudgetNs = 6944444
        SolariGpuBudgetNs = 3000000
        SolariVisualTarget = "competitive"
        SolariDenoiseMode = "balanced-fast"
        SolariInternalScale = "1.0"
        SolariBlasCompactionVertices = 0
        SolariDebugOverlay = ""
        CloudQuality = ""
        CloudInternalScale = ""
        CloudTemporal = ""
        CloudShadows = ""
        CloudProfile = ""
        CloudDebugOverlay = ""
        RenderGeometryPolicy = "hybrid"
        MeshletMinTriangles = 512
        StreamRenderPrepBudgetMs = 0
        StreamRenderPrepMaxChunksPerFrame = 0
        WindowWidth = 0
        WindowHeight = 0
        RenderBackend = "dx12"
        PresentMode = "immediate"
        RenderMaxFrameLatency = 0
        FrameTimeDiagnosticInterval = 60
        FrameTimeDiagnosticMinNs = 0
        FrameTimeDiagnosticMaxDepth = 10
        FrameTimeDiagnosticTopChildren = 16
        FrameTimeDiagnosticTopSpans = 32
        FrameTimeDiagnosticRowEvents = $false
        StartupDelaySeconds = 2
        ProfileEnv = [pscustomobject][ordered]@{}
    }
}

function Get-StackProfileKnownKeys {
    @{
        top = @("schema_version", "name", "description", "build", "features", "env", "processes", "diagnostics", "render", "cef", "security")
        build = @("release", "static_bevy")
        features = @("disable_solari", "disable_meshlets", "disable_clouds", "enable_dx12_dlss_rr", "disable_dlss_rr", "solari_debug_direct_visibility", "render_unknown_vendor")
        processes = @("server", "client", "launcher")
        diagnostics = @("render", "trace", "render_profile_verbose", "frame_time", "benchmark_log_minimal", "log_stream_verbose", "log_net_verbose", "log_render_verbose")
        render = @("backend", "present_mode", "max_frame_latency", "geometry_policy", "meshlet_min_triangles", "window_width", "window_height", "maximized", "disable_fps_overlay", "enable_fps_overlay", "solari_arch", "solari_target_fps", "solari_frame_budget_ns", "solari_gpu_budget_ns", "solari_visual_target", "solari_denoise_mode", "solari_internal_scale", "solari_blas_compaction_vertices", "solari_debug_overlay", "cloud_quality", "cloud_internal_scale", "cloud_temporal", "cloud_shadows", "cloud_profile", "cloud_debug_overlay", "stream_render_prep_budget_ms", "stream_render_prep_max_chunks_per_frame")
        cef = @("enabled", "dx12_accelerated_paint", "paint_transport", "accelerated_strict", "gpu_ring_depth", "copy_dirty_rects", "debug_timings")
        security = @("tls_mode")
    }
}

function Test-StackProfileShape {
    param([pscustomobject]$Profile)

    $known = Get-StackProfileKnownKeys
    foreach ($property in $Profile.PSObject.Properties) {
        if ($known.top -notcontains $property.Name) {
            throw "Unknown stack profile key '$($property.Name)'."
        }
    }
    foreach ($sectionName in @("build", "features", "processes", "diagnostics", "render", "cef", "security")) {
        $section = $Profile.PSObject.Properties[$sectionName].Value
        if ($null -eq $section) {
            continue
        }
        foreach ($property in $section.PSObject.Properties) {
            if ($known[$sectionName] -notcontains $property.Name) {
                throw "Unknown stack profile key '$sectionName.$($property.Name)'."
            }
        }
    }
}

function Read-StackProfile {
    param(
        [string]$ProfileRoot,
        [string]$Name
    )

    if ($Name.EndsWith(".json")) {
        $Name = $Name.Substring(0, $Name.Length - 5)
    }
    $path = Join-Path $ProfileRoot "$Name.json"
    if (-not (Test-Path -LiteralPath $path)) {
        throw "Stack profile was not found: $path"
    }
    $profile = Get-Content -LiteralPath $path -Raw | ConvertFrom-Json
    if ($profile.schema_version -ne "stack_profile_v1") {
        throw "Unsupported stack profile schema_version '$($profile.schema_version)' in $path."
    }
    Test-StackProfileShape -Profile $profile
    return [pscustomobject]@{
        name = $Name
        path = $path
        data = $profile
    }
}

function Set-StackProfileSectionValues {
    param(
        [System.Collections.IDictionary]$Values,
        [object]$Section,
        [hashtable]$Map
    )

    $sectionMap = Get-StackObjectMap -Object $Section
    foreach ($key in $sectionMap.Keys) {
        if ($Map.ContainsKey($key)) {
            $Values[$Map[$key]] = $sectionMap[$key]
        }
    }
}

function Merge-StackProfileValues {
    param(
        [System.Collections.IDictionary]$Values,
        [pscustomobject]$Profile
    )

    Set-StackProfileSectionValues -Values $Values -Section $Profile.build -Map @{
        release = "Release"
        static_bevy = "StaticBevy"
    }
    Set-StackProfileSectionValues -Values $Values -Section $Profile.features -Map @{
        disable_solari = "DisableSolari"
        disable_meshlets = "DisableMeshlets"
        disable_clouds = "DisableClouds"
        enable_dx12_dlss_rr = "EnableDx12DlssRr"
        disable_dlss_rr = "DisableDlssRr"
        solari_debug_direct_visibility = "SolariDebugDirectVisibility"
        render_unknown_vendor = "RenderUnknownVendor"
    }
    Set-StackProfileSectionValues -Values $Values -Section $Profile.processes -Map @{
        server = "Server"
        launcher = "Launcher"
    }
    if ($null -ne $Profile.processes -and $null -ne $Profile.processes.client) {
        $Values.NoClient = -not [bool]$Profile.processes.client
    }
    Set-StackProfileSectionValues -Values $Values -Section $Profile.diagnostics -Map @{
        render = "RenderDiagnostics"
        trace = "TraceDiagnostics"
        render_profile_verbose = "RenderProfileVerbose"
        frame_time = "FrameTimeDiagnostics"
        benchmark_log_minimal = "BenchmarkLogMinimal"
        log_stream_verbose = "LogStreamVerbose"
        log_net_verbose = "LogNetVerbose"
        log_render_verbose = "LogRenderVerbose"
    }
    Set-StackProfileSectionValues -Values $Values -Section $Profile.render -Map @{
        backend = "RenderBackend"
        present_mode = "PresentMode"
        max_frame_latency = "RenderMaxFrameLatency"
        geometry_policy = "RenderGeometryPolicy"
        meshlet_min_triangles = "MeshletMinTriangles"
        window_width = "WindowWidth"
        window_height = "WindowHeight"
        maximized = "Maximized"
        disable_fps_overlay = "DisableFpsOverlay"
        enable_fps_overlay = "EnableFpsOverlay"
        solari_arch = "SolariArch"
        solari_target_fps = "SolariTargetFps"
        solari_frame_budget_ns = "SolariFrameBudgetNs"
        solari_gpu_budget_ns = "SolariGpuBudgetNs"
        solari_visual_target = "SolariVisualTarget"
        solari_denoise_mode = "SolariDenoiseMode"
        solari_internal_scale = "SolariInternalScale"
        solari_blas_compaction_vertices = "SolariBlasCompactionVertices"
        solari_debug_overlay = "SolariDebugOverlay"
        cloud_quality = "CloudQuality"
        cloud_internal_scale = "CloudInternalScale"
        cloud_temporal = "CloudTemporal"
        cloud_shadows = "CloudShadows"
        cloud_profile = "CloudProfile"
        cloud_debug_overlay = "CloudDebugOverlay"
        stream_render_prep_budget_ms = "StreamRenderPrepBudgetMs"
        stream_render_prep_max_chunks_per_frame = "StreamRenderPrepMaxChunksPerFrame"
    }
    Set-StackProfileSectionValues -Values $Values -Section $Profile.cef -Map @{
        enabled = "CefUi"
        dx12_accelerated_paint = "CefUiDx12AcceleratedPaint"
        paint_transport = "CefPaintTransport"
        accelerated_strict = "CefAcceleratedStrict"
        gpu_ring_depth = "CefGpuRingDepth"
        copy_dirty_rects = "CefCopyDirtyRects"
        debug_timings = "CefDebugTimings"
    }

    $envValues = [ordered]@{}
    foreach ($entry in (Get-StackObjectMap -Object $Profile.env).GetEnumerator()) {
        $envValues[$entry.Key] = $entry.Value
    }
    if ($null -ne $Profile.security -and -not [string]::IsNullOrWhiteSpace($Profile.security.tls_mode)) {
        $envValues["FUN_GAME_SERVER_TLS_MODE"] = $Profile.security.tls_mode
    }
    $Values.ProfileEnv = [pscustomobject]$envValues
}

function ConvertTo-StackBoundValue {
    param([object]$Value)

    if ($Value -is [System.Management.Automation.SwitchParameter]) {
        return [bool]$Value
    }
    return $Value
}

function New-StackRunRequest {
    param(
        [System.Collections.IDictionary]$BoundParameters,
        [string]$ProfileRoot
    )

    $values = Get-StackDefaultValues
    $profileName = if ($BoundParameters.Keys -contains "Profile") { [string]$BoundParameters.Profile } else { [string]$values.Profile }
    $profileRecord = Read-StackProfile -ProfileRoot $ProfileRoot -Name $profileName
    Merge-StackProfileValues -Values $values -Profile $profileRecord.data
    $values.Profile = $profileRecord.name

    $overrideKeys = @()
    foreach ($key in $BoundParameters.Keys) {
        if (-not $values.Contains($key)) {
            continue
        }
        $values[$key] = ConvertTo-StackBoundValue -Value $BoundParameters[$key]
        if ($key -ne "Profile") {
            $overrideKeys += $key
        }
    }

    $request = [pscustomobject]$values
    $request | Add-Member -NotePropertyName "ProfilePath" -NotePropertyValue $profileRecord.path
    $request | Add-Member -NotePropertyName "ProfileDescription" -NotePropertyValue $profileRecord.data.description
    $request | Add-Member -NotePropertyName "OverrideKeys" -NotePropertyValue @($overrideKeys | Sort-Object)
    $request | Add-Member -NotePropertyName "Warnings" -NotePropertyValue @()
    Test-StackRunRequest -Request $request
    return $request
}

function Test-StackRunRequest {
    param([pscustomobject]$Request)

    if (@("default", "disabled", "cpu", "auto", "d3d11on12") -notcontains $Request.CefPaintTransport) {
        throw "Invalid CefPaintTransport '$($Request.CefPaintTransport)'."
    }
    if ($Request.CefGpuRingDepth -lt 2 -or $Request.CefGpuRingDepth -gt 5) {
        throw "CefGpuRingDepth must be 2..5."
    }
    if (@(0, 1, 2, 4, 8) -notcontains $Request.StreamRenderPrepBudgetMs) {
        throw "StreamRenderPrepBudgetMs must be one of 0,1,2,4,8."
    }
    if ($Request.StreamRenderPrepMaxChunksPerFrame -lt 0 -or $Request.StreamRenderPrepMaxChunksPerFrame -gt 1000000) {
        throw "StreamRenderPrepMaxChunksPerFrame must be 0..1000000."
    }
}

function Write-StackJson {
    param(
        [string]$Path,
        [object]$Value
    )

    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
    $tempPath = "$Path.$PID.$([Guid]::NewGuid().ToString('N')).tmp"
    $Value | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $tempPath -Encoding utf8
    for ($attempt = 1; $attempt -le 5; $attempt++) {
        try {
            Remove-Item -LiteralPath $Path -Force -ErrorAction SilentlyContinue
            Move-Item -LiteralPath $tempPath -Destination $Path -Force
            return
        }
        catch {
            if ($attempt -eq 5) {
                throw
            }
            Start-Sleep -Milliseconds (50 * $attempt)
        }
    }
}

function Write-StackSessionMetadata {
    param(
        [pscustomobject]$Request,
        [pscustomobject]$Paths,
        [pscustomobject]$BuildPlan,
        [object[]]$Processes,
        [hashtable]$EnvSnapshot,
        [object[]]$Events,
        [string]$Status
    )

    $processRecords = @()
    foreach ($process in $Processes) {
        $processRecords += [ordered]@{
            name = $process.name
            pid = $process.pid
            path = $process.path
            stdout = $process.stdout
            stderr = $process.stderr
            visible = $process.visible
        }
    }

    $plannedProcesses = @()
    if ($Request.Server) {
        $plannedProcesses += [ordered]@{
            name = "game_server"
            path = Get-StackBinaryPath -Paths $Paths -Name "game_server"
            stdout = Join-Path $Paths.log_root "game_server.out.log"
            stderr = Join-Path $Paths.log_root "game_server.err.log"
            enabled = $true
            visible = $false
        }
    }
    if (-not $Request.NoClient) {
        $plannedProcesses += [ordered]@{
            name = "game_client"
            path = Get-StackBinaryPath -Paths $Paths -Name "game_client"
            stdout = Join-Path $Paths.log_root "game_client.out.log"
            stderr = Join-Path $Paths.log_root "game_client.err.log"
            enabled = $true
            visible = $true
        }
    }

    $metadata = [ordered]@{
        schema_version = "stack_session_v1"
        status = $Status
        profile = [ordered]@{
            name = $Request.Profile
            path = $Request.ProfilePath
            description = $Request.ProfileDescription
        }
        overrides = $Request.OverrideKeys
        env = $EnvSnapshot
        build = [ordered]@{
            profile = $BuildPlan.profile
            packages = $BuildPlan.packages
            features = $BuildPlan.features
            args = $BuildPlan.args
            dynamic_linking = $BuildPlan.dynamic_linking
        }
        processes = $processRecords
        process_paths = $plannedProcesses
        paths = [ordered]@{
            repo_root = $Paths.repo_root
            target_root = $Paths.target_root
            run_root = $Paths.run_root
            logs = $Paths.log_root
            pid_file = $Paths.pid_file
            session_file = $Paths.session_file
            cef_transport_status_file = $Paths.cef_transport_status_file
            renderer_capability_report_file = $Paths.renderer_capability_report_file
        }
        events = $Events
    }
    Write-StackJson -Path $Paths.session_file -Value $metadata
}
