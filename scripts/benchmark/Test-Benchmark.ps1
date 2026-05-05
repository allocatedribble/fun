param()

$ErrorActionPreference = "Stop"

function Normalize-BenchmarkTestPath {
    param([string]$Path)

    if ($Path.StartsWith("\\?\")) {
        return $Path.Substring(4)
    }
    if ($Path.StartsWith("\?\")) {
        return $Path.Substring(3)
    }
    return $Path
}

function Assert-BenchmarkCondition {
    param(
        [bool]$Condition,
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

$moduleRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$scriptRoot = Split-Path -Parent $moduleRoot
$repoRoot = Normalize-BenchmarkTestPath ((Resolve-Path (Join-Path $scriptRoot "..")).Path)
$inputLog = Join-Path $repoRoot "target\run-stack\logs\game_client.out.log"
$inputLogParent = Split-Path -Parent $inputLog
$backupLog = Join-Path $repoRoot "target\run-stack\logs\game_client.out.log.benchmark-test-backup"
$hadInputLog = Test-Path $inputLog

$fixtureLines = @(
    "[client perf] cef_ui transport selected: requested=auto selected=d3d11on12 backend=dx12 bridge_ready=true cpu_fallback_enabled=true ring_depth=3 copy_mode=full_frame strict=false debug_timings=true fallback_reason=none",
    "[bevy render] dx12 backend: schema_version=bevy_dx12_backend_diagnostics_snapshot_v1 requested_backends=dx12 backend=dx12 selected_backend=dx12 adapter=hash:0123456789abcdef adapter_name_hash_or_redacted_name=hash:0123456789abcdef vendor_id=4318 device_id=9860 device_type=discrete_gpu driver=hash:1111111111111111 driver_info=hash:2222222222222222 compiler=fxc shader_compiler=fxc presentation_system=dxgi_from_hwnd latency=wait latency_waitable_object=wait timestamp_query_supported=true pipeline_statistics_supported=false pipeline_cache_feature_supported=true pipeline_cache_policy=memory_only surface_format=Bgra8UnormSrgb present_mode_requested=immediate present_mode_selected=immediate desired_maximum_frame_latency=2",
    "[client perf] cef_ui transport: cef_on_paint_fps=0 cef_on_accelerated_paint_fps=60 cef_cpu_upload_bytes=0 cef_gpu_copy_bytes=33177600 cef_gpu_copy_ns=120000 cef_gpu_copy_failures=0 cef_gpu_frame_ready_count=12 cef_gpu_frame_not_ready_count=1 cef_gpu_frame_reused_count=1 cef_gpu_frame_blocking_wait_count=0 cef_transport_fallback_count=0 cef_published_generation=12 cef_sampled_generation=11 cef_stale_frame_count=0",
    "[client perf] fps=144 frame_ms=6.9444 solari_gpu_ms=1.0 meshlet_visibility_gpu_ms=0.5 dlss_rr_gpu_ms=0",
    "[client perf] non_solari gpu_ms: present_wait_ms=0.1 post_process_gpu_ms=0.2",
    "[client perf] render uploads: write_texture_calls=1 write_texture_bytes=4096 write_buffer_calls=2 write_buffer_bytes=2048 write_buffer_with_calls=0 write_buffer_with_bytes=0 callsite_count=1",
    "[client perf] render churn: bind_group_creations=0 bind_group_layout_creations=0 bind_group_layout_cache_hits=1 bind_group_layout_cache_misses=0 pipeline_layout_creations=0 render_pipeline_queued=0 compute_pipeline_queued=0 render_pipeline_creations=0 compute_pipeline_creations=0 pipeline_cache_hits=1 pipeline_cache_misses=0",
    "[client perf] render commands: command_encoder_creations=1 render_passes=1 compute_passes=1 command_buffers_submitted=1 queue_submits=1 copy_commands=0 native_interop_command_insertions=0 event_count=0",
    "[client perf] render readbacks: readback_requested_count=0 readback_completed_count=0 readback_dropped_count=0 readback_blocking_wait_count=0 readback_latency_frame_sum=0 readback_latency_frame_max=0 map_async_count=0 poll_count=0 event_count=0",
    "[client perf] render shaders: shader_module_creations=0 shader_module_create_ns=0 shader_variant_requests=0 shader_def_count=0 material_specializations=0 render_pipeline_create_count=0 render_pipeline_create_ns=0 compute_pipeline_create_count=0 compute_pipeline_create_ns=0 pipeline_create_count=0 pipeline_create_ns=0 pipeline_specialization_count=0 event_count=0"
)

try {
    New-Item -ItemType Directory -Force -Path $inputLogParent | Out-Null
    if ($hadInputLog) {
        Copy-Item -LiteralPath $inputLog -Destination $backupLog -Force
    }

    Set-Content -Path $inputLog -Value $fixtureLines -Encoding UTF8
    $output = @(powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $scriptRoot "benchmark_client.ps1") -InputLog "target\run-stack\logs\game_client.out.log" 2>&1)
    if ($LASTEXITCODE -ne 0) {
        $output | ForEach-Object { [string]$_ }
        throw "benchmark_client.ps1 parser-only smoke failed with exit code $LASTEXITCODE"
    }

    $jsonLine = @($output | ForEach-Object { [string]$_ } | Select-String -Pattern "^Benchmark JSON:\s*(?<path>.+)$" | Select-Object -Last 1)
    Assert-BenchmarkCondition -Condition ($jsonLine.Count -gt 0) -Message "benchmark_client.ps1 did not print a Benchmark JSON path."
    $jsonPath = $jsonLine[0].Matches[0].Groups["path"].Value.Trim()
    Assert-BenchmarkCondition -Condition (Test-Path $jsonPath) -Message "Benchmark JSON was not written: $jsonPath"

    $summary = Get-Content -Path $jsonPath -Raw | ConvertFrom-Json
    $summaryKeys = @($summary.PSObject.Properties.Name)
    $requiredKeys = @(
        "schema_version",
        "created_at",
        "repo_root",
        "source_log",
        "source_error_log",
        "git",
        "hardware",
        "environment",
        "dx12_memory",
        "config",
        "samples",
        "metrics",
        "comparison",
        "rr_acceptance",
        "render_capabilities",
        "dx12_backend_diagnostics",
        "rt_feature_gates",
        "render_presentation",
        "render_upload_callsites",
        "render_churn_events",
        "render_churn_creation_events",
        "render_command_events",
        "render_readback_events",
        "render_shader_events",
        "transient_descriptor_creates",
        "transient_descriptor_label_variants",
        "cef_ui_transport_selection",
        "cef_ui_transport_health",
        "render_graph_flame_map"
    )
    $missingKeys = @($requiredKeys | Where-Object { $summaryKeys -notcontains $_ })
    Assert-BenchmarkCondition -Condition ($missingKeys.Count -eq 0) -Message "Parser-only summary removed key(s): $($missingKeys -join ', ')"
    Assert-BenchmarkCondition -Condition ($summary.stack_runner.schema_version -eq "benchmark_stack_runner_v1") -Message "stack_runner schema marker was missing."
    Assert-BenchmarkCondition -Condition ($summary.dx12_backend_diagnostics.schema_version -eq "bevy_dx12_backend_diagnostics_snapshot_v1") -Message "DX12 backend diagnostic snapshot was not parsed."
    Assert-BenchmarkCondition -Condition ($summary.samples.input_log_mode -eq $true) -Message "Parser-only summary did not record input_log_mode=true."
    Assert-BenchmarkCondition -Condition ([int]$summary.samples.count -gt 0) -Message "Parser-only summary recorded no samples."

    [ordered]@{
        schema_version = "benchmark_self_test_v1"
        status = "pass"
        parser_only_command = "scripts/benchmark_client.ps1 -InputLog target/run-stack/logs/game_client.out.log"
        summary_json = $jsonPath
        required_key_count = $requiredKeys.Count
    } | ConvertTo-Json -Depth 4
}
finally {
    if ($hadInputLog) {
        Copy-Item -LiteralPath $backupLog -Destination $inputLog -Force
        Remove-Item -LiteralPath $backupLog -Force -ErrorAction SilentlyContinue
    }
    else {
        Remove-Item -LiteralPath $inputLog -Force -ErrorAction SilentlyContinue
    }
}
