$ErrorActionPreference = "Stop"

function New-RenderGraphFlameNode {
    param(
        [System.Collections.IDictionary]$Summary,
        [string]$Id,
        [string]$Label,
        [string]$Metric,
        [string[]]$Reads = @(),
        [string[]]$Writes = @(),
        [string]$Interop = "none"
    )

    return [ordered]@{
        id = $Id
        label = $Label
        metric = $Metric
        gpu_timing_ns = Get-SummaryMetricValue -Summary $Summary -Name $Metric -Field "p95"
        cpu_encode_ns = $null
        reads = @($Reads)
        writes = @($Writes)
        barriers_known = $false
        transient_resource_ids = @()
        native_interop = $Interop
    }
}

function Write-RenderGraphFlameMap {
    param(
        [string]$Path,
        [System.Collections.IDictionary]$Summary
    )

    $parent = Split-Path -Parent $Path
    New-Item -ItemType Directory -Force -Path $parent | Out-Null

    $nodes = @(
        New-RenderGraphFlameNode -Summary $Summary -Id "world_render" -Label "World Render" -Metric "standard_raster_gpu_ns" -Writes @("main_hdr_color", "depth", "motion_vectors")
        New-RenderGraphFlameNode -Summary $Summary -Id "meshlet_visibility" -Label "Meshlet Visibility" -Metric "meshlet_visibility_gpu_ns" -Reads @("depth") -Writes @("visible_meshlets")
        New-RenderGraphFlameNode -Summary $Summary -Id "solari_lighting" -Label "Solari Lighting" -Metric "solari_gpu_ns" -Reads @("main_hdr_color", "depth", "motion_vectors") -Writes @("main_hdr_color")
        New-RenderGraphFlameNode -Summary $Summary -Id "clouds" -Label "Clouds" -Metric "cloud_total_gpu_ns" -Reads @("depth", "weather_state") -Writes @("main_hdr_color", "cloud_history")
        New-RenderGraphFlameNode -Summary $Summary -Id "post_process" -Label "Post Process" -Metric "post_process_gpu_ns" -Reads @("main_hdr_color") -Writes @("post_processed_color")
        New-RenderGraphFlameNode -Summary $Summary -Id "cef_ui_composition" -Label "CEF UI Composition" -Metric "cef_gpu_copy_ns" -Reads @("cef_ui_texture", "post_processed_color") -Writes @("present_color") -Interop "cef"
        New-RenderGraphFlameNode -Summary $Summary -Id "debug_overlays" -Label "Debug Overlays" -Metric "ui_overlay_cpu_ns" -Reads @("present_color") -Writes @("present_color")
        New-RenderGraphFlameNode -Summary $Summary -Id "readback_capture" -Label "Readback/Capture" -Metric "render_readback_readback_requested_count" -Reads @("query_buffers", "screenshot_targets") -Writes @("cpu_readback_buffers") -Interop "readback"
        New-RenderGraphFlameNode -Summary $Summary -Id "present" -Label "Present" -Metric "present_wait_ns" -Reads @("present_color") -Writes @("swapchain")
    )

    $edges = @(
        [ordered]@{ from = "world_render"; to = "meshlet_visibility"; kind = "depth_dependency" }
        [ordered]@{ from = "world_render"; to = "solari_lighting"; kind = "lighting_input" }
        [ordered]@{ from = "solari_lighting"; to = "clouds"; kind = "hdr_color" }
        [ordered]@{ from = "clouds"; to = "post_process"; kind = "hdr_color" }
        [ordered]@{ from = "post_process"; to = "cef_ui_composition"; kind = "ui_after_post" }
        [ordered]@{ from = "cef_ui_composition"; to = "debug_overlays"; kind = "overlay_order" }
        [ordered]@{ from = "debug_overlays"; to = "present"; kind = "present_color" }
        [ordered]@{ from = "world_render"; to = "readback_capture"; kind = "diagnostic_copy" }
    )

    $nativeInterop = @()
    if ($null -ne $Summary.render_command_events) {
        $nativeInterop = @(
            $Summary.render_command_events |
                Where-Object { $_.category -eq "cef_copy" -or $_.operation -eq "native_interop_command_insertion" } |
                ForEach-Object {
                    [ordered]@{
                        operation = $_.operation
                        category = $_.category
                        label = $_.label
                        calls = $_.calls
                    }
                }
        )
    }

    $payload = [ordered]@{
        schema_version = 1
        frame_index = 0
        source = "benchmark_client_summary"
        generated_at = (Get-Date).ToString("o")
        nodes = $nodes
        edges = $edges
        native_interop_points = $nativeInterop
        ordering_policy = "world->lighting->post->cef_ui->debug->present"
        notes = @(
            "barriers_known=false until PIX barrier summaries are imported",
            "CEF UI is modeled after post-processing and before debug/present so it does not feed DLSS or world temporal inputs",
            "readback_capture is diagnostic/capture-only and should be empty in performance lanes unless explicitly enabled"
        )
    }

    $payload | ConvertTo-Json -Depth 12 | Set-Content -Path $Path -Encoding UTF8
}

function Format-StatValue {
    param(
        [System.Collections.IDictionary]$Stats,
        [string]$Metric,
        [string]$Field
    )

    if (-not $Stats.Contains($Metric)) {
        return "n/a"
    }
    return "$($Stats[$Metric][$Field])"
}

function Write-MarkdownReport {
    param(
        [string]$Path,
        [System.Collections.IDictionary]$Summary
    )

    $stats = $Summary.metrics
    $comparison = $Summary.comparison
    $lines = New-Object "System.Collections.Generic.List[string]"
    $lines.Add("# Client Benchmark") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("- Created: $($Summary.created_at)") | Out-Null
    $lines.Add("- Git: $($Summary.git.commit)") | Out-Null
    $lines.Add("- Dirty files: $($Summary.git.dirty_count)") | Out-Null
    if (-not [string]::IsNullOrWhiteSpace($Summary.config.benchmark_profile)) {
        $lines.Add("- Benchmark profile: $($Summary.config.benchmark_profile)") | Out-Null
    }
    if (-not [string]::IsNullOrWhiteSpace($Summary.config.benchmark_matrix_lane)) {
        $lines.Add("- Matrix lane: $($Summary.config.benchmark_matrix_lane)") | Out-Null
    }
    $lines.Add("- Backend: $($Summary.config.render_backend)") | Out-Null
    if ($null -ne $Summary.renderer_capability_report -and $Summary.renderer_capability_report.status -eq "found") {
        $lines.Add("- Backend truth: requested=$($Summary.renderer_capability_report.requested_graphics_backend) selected=$($Summary.renderer_capability_report.selected_graphics_backend) actual=$($Summary.renderer_capability_report.actual_graphics_backend) fallback=$($Summary.renderer_capability_report.fallback_graphics_backend) reason=$($Summary.renderer_capability_report.graphics_backend_fallback_reason)") | Out-Null
    }
    $lines.Add("- Present mode: $($Summary.config.present_mode)") | Out-Null
    $startupLatency = if ($null -ne $Summary.render_presentation) { $Summary.render_presentation.desired_maximum_frame_latency } else { "n/a" }
    $surfacePresent = if ($null -ne $Summary.render_presentation) { $Summary.render_presentation.surface_selected_present_mode } else { "n/a" }
    $swapchainFormat = if ($null -ne $Summary.render_presentation) { $Summary.render_presentation.swapchain_format } else { "n/a" }
    $lines.Add("- Max frame latency: requested=$($Summary.config.requested_maximum_frame_latency) startup=$startupLatency") | Out-Null
    $lines.Add("- Surface present: selected=$surfacePresent swapchain_format=$swapchainFormat") | Out-Null
    $selectedTransport = if ($Summary.cef_ui_transport_selection.status -eq "found") { $Summary.cef_ui_transport_selection.selected } else { "not_found" }
    $health = $Summary.cef_ui_transport_health
    $healthStatus = if ($null -ne $health -and $health.record_status -eq "found") { $health.status } else { "not_found" }
    $healthRingDepth = if ($null -ne $health -and -not [string]::IsNullOrWhiteSpace([string]$health.ring_depth)) { $health.ring_depth } else { $Summary.config.cef_gpu_ring_depth }
    $lines.Add("- CEF UI: mode=$($Summary.config.cef_ui_mode) requested_transport=$($Summary.config.cef_paint_transport) selected_transport=$selectedTransport enabled=$($Summary.config.cef_ui_enabled) ring_depth=$healthRingDepth health=$healthStatus") | Out-Null
    if ($null -ne $health -and $health.record_status -eq "found") {
        $lines.Add("- CEF transport health: $($health.transport) | accel paint $($health.accel_paint_fps) fps | gpu copy $($health.gpu_copy_ms) ms | CPU upload $($health.cpu_upload_bytes_per_frame) B/frame | reused $($health.reused_frames) frames | fallback $($health.fallback_count)") | Out-Null
    }
    $lines.Add("- Solari denoise mode: $($Summary.config.solari_denoise_mode)") | Out-Null
    $lines.Add("- Solari internal scale: $($Summary.config.solari_internal_scale)") | Out-Null
    $lines.Add("- Clouds: disabled=$($Summary.config.disable_clouds) profile=$($Summary.config.cloud_profile) quality=$($Summary.config.cloud_quality) internal_scale=$($Summary.config.cloud_internal_scale) temporal=$($Summary.config.cloud_temporal) shadows=$($Summary.config.cloud_shadows)") | Out-Null
    $lines.Add("- RT feature hash: $($Summary.rt_feature_gates.rt_feature_hash)") | Out-Null
    $lines.Add("- Backend capability hash: $($Summary.render_capabilities.backend_capability_hash)") | Out-Null
    $lines.Add("- RT gates: direct=$($Summary.config.rt_sample_direct) indirect=$($Summary.config.rt_sample_indirect) reflections=$($Summary.config.rt_sample_reflections) surface_cache=$($Summary.config.rt_surface_cache) megageom=$($Summary.config.rt_megageom) opacity_mask=$($Summary.config.rt_opacity_mask) hair=$($Summary.config.rt_hair) async_readback=$($Summary.config.rt_async_readback) validation=$($Summary.config.rt_validation)") | Out-Null
    if ($null -ne $Summary.dx12_memory) {
        $lines.Add("- DX12 memory: status=$($Summary.dx12_memory.status) source=$($Summary.dx12_memory.source) local_budget_bytes=$($Summary.dx12_memory.local_budget_bytes) local_usage_bytes=$($Summary.dx12_memory.local_usage_bytes) adapter_ram_bytes=$($Summary.dx12_memory.adapter_ram_bytes)") | Out-Null
    }
    $lines.Add("- Sample count: $($Summary.samples.count)") | Out-Null
    $lines.Add("") | Out-Null

    $primaryMetrics = @(
        "fps",
        "frame_ns",
        "frame_ms",
        "solari_gpu_ns",
        "meshlet_visibility_gpu_ns",
        "cloud_total_gpu_ns",
        "cloud_weather_update_gpu_ns",
        "cloud_shape_noise_gpu_ns",
        "cloud_raymarch_gpu_ns",
        "cloud_temporal_gpu_ns",
        "cloud_resolve_gpu_ns",
        "cloud_composite_gpu_ns",
        "cloud_weather_update_cpu_ns",
        "cloud_internal_width",
        "cloud_internal_height",
        "cloud_primary_steps",
        "cloud_light_steps",
        "cloud_history_accept_rate",
        "cloud_history_reject_rate",
        "cloud_history_reset_count",
        "cloud_history_average_age",
        "cloud_vram_bytes",
        "meshlet_path_instance_count",
        "raster_path_instance_count",
        "ray_proxy_only_count",
        "meshlet_first_pass_gpu_ns",
        "meshlet_depth_pyramid_first_gpu_ns",
        "meshlet_second_pass_gpu_ns",
        "meshlet_depth_resolve_gpu_ns",
        "meshlet_material_depth_gpu_ns",
        "meshlet_depth_pyramid_second_gpu_ns",
        "meshlet_extract_cpu_ns",
        "meshlet_prepare_cpu_ns",
        "meshlet_bind_group_prepare_cpu_ns",
        "meshlet_material_queue_cpu_ns",
        "meshlet_material_queue_dirty_instance_count",
        "meshlet_instance_full_buffer_writes",
        "meshlet_instance_range_buffer_writes",
        "meshlet_material_full_buffer_writes",
        "meshlet_material_range_buffer_writes",
        "meshlet_view_visibility_buffer_writes",
        "meshlet_view_reset_cpu_queue_writes",
        "meshlet_view_reset_cpu_queue_writes_per_view",
        "meshlet_view_count",
        "meshlet_instance_buffer_upload_bytes",
        "meshlet_material_buffer_upload_bytes",
        "meshlet_view_visibility_buffer_upload_bytes",
        "meshlet_buffer_reallocations",
        "meshlet_buffer_capacity_high_water_bytes",
        "meshlet_view_resource_cache_rebuilds",
        "meshlet_culling_output_buffer_bytes",
        "meshlet_indirect_draw_buffer_writes",
        "meshlet_compaction_buffer_writes",
        "meshlet_asset_buffer_upload_bytes",
        "meshlet_asset_buffer_grow_copies",
        "meshlet_asset_buffer_capacity_bytes",
        "transient_texture_requests",
        "transient_texture_creates",
        "transient_texture_reuses",
        "transient_texture_aliases",
        "transient_buffer_requests",
        "transient_buffer_creates",
        "transient_buffer_reuses",
        "transient_buffer_aliases",
        "transient_cached_texture_slots",
        "transient_cached_buffer_slots",
        "transient_texture_descriptor_miss_creates",
        "transient_texture_lifetime_conflict_creates",
        "transient_buffer_descriptor_miss_creates",
        "transient_buffer_lifetime_conflict_creates",
        "transient_texture_near_miss_size",
        "transient_texture_near_miss_format",
        "transient_texture_near_miss_usage",
        "transient_texture_near_miss_view_formats",
        "transient_texture_near_miss_other",
        "transient_buffer_near_miss_size",
        "transient_buffer_near_miss_usage",
        "transient_buffer_near_miss_other",
        "transient_texture_label_variant_descriptors",
        "transient_buffer_label_variant_descriptors",
        "transient_texture_every_frame_create_descriptors",
        "transient_buffer_every_frame_create_descriptors",
        "transient_texture_resize_like_create_descriptors",
        "transient_buffer_resize_like_create_descriptors",
        "render_scheduler_pressure",
        "schedule_networking_receive_ns",
        "schedule_world_stream_apply_ns",
        "schedule_movement_input_ns",
        "schedule_look_ns",
        "schedule_physics_movement_ns",
        "schedule_diagnostics_logging_ns",
        "schedule_render_config_window_ns",
        "schedule_solari_runtime_params_update_ns",
        "schedule_meshlet_extraction_ns",
        "schedule_render_interpolation_ns",
        "standard_raster_gpu_ns",
        "physics_fixed_update_cpu_ns",
        "network_receive_cpu_ns",
        "world_stream_apply_cpu_ns",
        "catalog_lookup_cpu_ns",
        "world_stream_render_prep_budget_ns",
        "world_stream_render_prep_max_chunks_per_frame",
        "world_stream_render_prep_limit_reason_code",
        "world_stream_render_prep_queue_depth",
        "world_stream_render_prep_deferred_chunks",
        "world_stream_render_prep_applied_chunks",
        "world_stream_render_prep_dynamic_mesh_assets",
        "post_process_gpu_ns",
        "ui_overlay_cpu_ns",
        "present_wait_ns",
        "cef_on_paint_fps",
        "cef_on_accelerated_paint_fps",
        "cef_cpu_upload_bytes",
        "cef_gpu_copy_bytes",
        "cef_gpu_copy_ns",
        "cef_gpu_copy_failures",
        "cef_gpu_frame_ready_count",
        "cef_gpu_frame_not_ready_count",
        "cef_gpu_frame_reused_count",
        "cef_gpu_frame_blocking_wait_count",
        "cef_health_accel_paint_fps",
        "cef_health_paint_fps",
        "cef_health_gpu_copy_ms",
        "cef_health_gpu_copy_ns_per_copy",
        "cef_health_cpu_upload_bytes_per_frame",
        "cef_health_reused_frames",
        "cef_health_not_ready_frames",
        "cef_health_blocking_waits",
        "cef_health_fallback_count",
        "cef_health_ring_depth",
        "cef_transport_fallback_count",
        "cef_published_generation",
        "cef_sampled_generation",
        "cef_stale_frame_count",
        "render_upload_write_texture_calls",
        "render_upload_write_texture_bytes",
        "render_upload_write_buffer_calls",
        "render_upload_write_buffer_bytes",
        "render_upload_write_buffer_with_calls",
        "render_upload_write_buffer_with_bytes",
        "render_upload_callsite_count",
        "render_churn_bind_group_creations",
        "render_churn_bind_group_layout_creations",
        "render_churn_bind_group_layout_cache_hits",
        "render_churn_bind_group_layout_cache_misses",
        "render_churn_pipeline_layout_creations",
        "render_churn_render_pipeline_queued",
        "render_churn_compute_pipeline_queued",
        "render_churn_render_pipeline_creations",
        "render_churn_compute_pipeline_creations",
        "render_churn_render_pipeline_ready",
        "render_churn_compute_pipeline_ready",
        "render_churn_render_pipeline_errors",
        "render_churn_compute_pipeline_errors",
        "render_churn_pipeline_cache_hits",
        "render_churn_pipeline_cache_misses",
        "render_churn_material_pipeline_key_count",
        "render_churn_post_process_pipeline_key_count",
        "render_churn_cloud_pipeline_key_count",
        "render_churn_solari_pipeline_key_count",
        "render_churn_meshlet_pipeline_key_count",
        "render_churn_ui_pipeline_key_count",
        "render_churn_debug_overlay_pipeline_key_count",
        "render_churn_event_count",
        "render_command_command_encoder_creations",
        "render_command_render_passes",
        "render_command_compute_passes",
        "render_command_command_buffers_submitted",
        "render_command_queue_submits",
        "render_command_copy_commands",
        "render_command_native_interop_command_insertions",
        "render_command_event_count",
        "render_readback_readback_requested_count",
        "render_readback_readback_completed_count",
        "render_readback_readback_dropped_count",
        "render_readback_readback_blocking_wait_count",
        "render_readback_readback_latency_frame_sum",
        "render_readback_readback_latency_frame_max",
        "render_readback_map_async_count",
        "render_readback_poll_count",
        "render_readback_event_count",
        "render_shader_shader_module_creations",
        "render_shader_shader_module_create_ns",
        "render_shader_shader_variant_requests",
        "render_shader_shader_def_count",
        "render_shader_material_specializations",
        "render_shader_render_pipeline_create_count",
        "render_shader_render_pipeline_create_ns",
        "render_shader_compute_pipeline_create_count",
        "render_shader_compute_pipeline_create_ns",
        "render_shader_pipeline_create_count",
        "render_shader_pipeline_create_ns",
        "render_shader_pipeline_specialization_count",
        "render_shader_event_count",
        "dlss_rr_gpu_ns",
        "solari_pass_dlss_rr_guide_resolve_ns",
        "solari_pass_direct_ns",
        "solari_pass_diffuse_ns",
        "solari_pass_diffuse_initial_ns",
        "solari_pass_diffuse_spatial_ns",
        "solari_pass_specular_regular_ns",
        "solari_pass_specular_psr_ns",
        "solari_pass_denoise_cheap_ns",
        "solari_pass_denoise_atrous_1_ns",
        "solari_pass_denoise_atrous_2_ns",
        "solari_pass_denoise_atrous_3_ns",
        "solari_pass_denoise_composite_ns"
    )

    $lines.Add("## Primary Metrics") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("| metric | mean | p50 | p95 | p99 | min | max | samples |") | Out-Null
    $lines.Add("|---|---:|---:|---:|---:|---:|---:|---:|") | Out-Null
    foreach ($metric in $primaryMetrics) {
        if ($stats.Contains($metric)) {
            $row = "| {0} | {1} | {2} | {3} | {4} | {5} | {6} | {7} |" -f `
                $metric, `
                (Format-StatValue -Stats $stats -Metric $metric -Field "mean"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "p50"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "p95"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "p99"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "min"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "max"), `
                (Format-StatValue -Stats $stats -Metric $metric -Field "count")
            $lines.Add($row) | Out-Null
        }
    }
    if ($null -ne $Summary.render_upload_callsites -and $Summary.render_upload_callsites.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Render Upload Top Callsites") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | operation | label | calls | bytes | samples |") | Out-Null
        $lines.Add("|---:|---|---|---:|---:|---:|") | Out-Null
        foreach ($callsite in $Summary.render_upload_callsites) {
            $lines.Add("| $($callsite.rank) | $($callsite.operation) | $($callsite.label) | $($callsite.calls) | $($callsite.bytes) | $($callsite.samples) |") | Out-Null
        }
    }
    if ($null -ne $Summary.render_churn_events -and $Summary.render_churn_events.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Render Resource Churn Top Events") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | operation | category | label | calls | samples |") | Out-Null
        $lines.Add("|---:|---|---|---|---:|---:|") | Out-Null
        foreach ($event in $Summary.render_churn_events) {
            $lines.Add("| $($event.rank) | $($event.operation) | $($event.category) | $($event.label) | $($event.calls) | $($event.samples) |") | Out-Null
        }
    }
    if ($null -ne $Summary.render_churn_creation_events -and $Summary.render_churn_creation_events.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Render Resource Creation Churn Top Events") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | operation | category | label | calls | samples |") | Out-Null
        $lines.Add("|---:|---|---|---|---:|---:|") | Out-Null
        foreach ($event in $Summary.render_churn_creation_events) {
            $lines.Add("| $($event.rank) | $($event.operation) | $($event.category) | $($event.label) | $($event.calls) | $($event.samples) |") | Out-Null
        }
    }
    if ($null -ne $Summary.render_command_events -and $Summary.render_command_events.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Render Command Top Events") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | operation | category | label | calls | samples |") | Out-Null
        $lines.Add("|---:|---|---|---|---:|---:|") | Out-Null
        foreach ($event in $Summary.render_command_events) {
            $lines.Add("| $($event.rank) | $($event.operation) | $($event.category) | $($event.label) | $($event.calls) | $($event.samples) |") | Out-Null
        }
    }
    if ($null -ne $Summary.render_readback_events -and $Summary.render_readback_events.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Render Readback Top Events") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | operation | category | label | calls | latency frame sum | latency frame max | samples |") | Out-Null
        $lines.Add("|---:|---|---|---|---:|---:|---:|---:|") | Out-Null
        foreach ($event in $Summary.render_readback_events) {
            $lines.Add("| $($event.rank) | $($event.operation) | $($event.category) | $($event.label) | $($event.calls) | $($event.latency_frame_sum) | $($event.latency_frame_max) | $($event.samples) |") | Out-Null
        }
    }
    if ($null -ne $Summary.render_shader_events -and $Summary.render_shader_events.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Render Shader Top Events") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | operation | category | label | calls | elapsed ns | shader defs | samples |") | Out-Null
        $lines.Add("|---:|---|---|---|---:|---:|---:|---:|") | Out-Null
        foreach ($event in $Summary.render_shader_events) {
            $lines.Add("| $($event.rank) | $($event.operation) | $($event.category) | $($event.label) | $($event.calls) | $($event.elapsed_ns) | $($event.shader_defs) | $($event.samples) |") | Out-Null
        }
    }
    if ($null -ne $Summary.transient_descriptor_creates -and $Summary.transient_descriptor_creates.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Transient Descriptor Create Top Events") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | resource | label | reason | near miss | pattern | format | width | height | size | usage bits | creates | bytes | samples |") | Out-Null
        $lines.Add("|---:|---|---|---|---|---|---|---:|---:|---:|---:|---:|---:|---:|") | Out-Null
        foreach ($event in $Summary.transient_descriptor_creates) {
            $lines.Add("| $($event.rank) | $($event.resource) | $($event.label) | $($event.reason) | $($event.near_miss) | $($event.create_pattern) | $($event.format) | $($event.width) | $($event.height) | $($event.size) | $($event.usage_bits) | $($event.create_count) | $($event.estimated_bytes) | $($event.samples) |") | Out-Null
        }
    }
    if ($null -ne $Summary.transient_descriptor_label_variants -and $Summary.transient_descriptor_label_variants.Count -gt 0) {
        $lines.Add("") | Out-Null
        $lines.Add("## Transient Descriptor Label Variants") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| rank | resource | labels | format | width | height | size | usage bits | label count | samples |") | Out-Null
        $lines.Add("|---:|---|---|---|---:|---:|---:|---:|---:|---:|") | Out-Null
        foreach ($event in $Summary.transient_descriptor_label_variants) {
            $lines.Add("| $($event.rank) | $($event.resource) | $($event.labels) | $($event.format) | $($event.width) | $($event.height) | $($event.size) | $($event.usage_bits) | $($event.label_count) | $($event.samples) |") | Out-Null
        }
    }

    $budgetLedger = [ordered]@{
        "frame_ns" = 6944444
        "cloud_total_gpu_ns" = 1200000
        "meshlet_visibility_gpu_ns" = 1200000
        "standard_raster_gpu_ns" = 600000
        "physics_fixed_update_cpu_ns" = 350000
        "network_receive_cpu_ns" = 150000
        "world_stream_apply_cpu_ns" = 150000
        "post_process_gpu_ns" = 250000
        "ui_overlay_cpu_ns" = 50000
    }
    $lines.Add("") | Out-Null
    $lines.Add("## 144 FPS Budget Ledger") | Out-Null
    $lines.Add("") | Out-Null
    $lines.Add("| bucket | target p95 ns | actual p95 ns | pass |") | Out-Null
    $lines.Add("|---|---:|---:|---|") | Out-Null
    foreach ($metric in $budgetLedger.Keys) {
        $target = [double]$budgetLedger[$metric]
        $actual = if ($stats.Contains($metric)) { [double]$stats[$metric].p95 } else { $null }
        $pass = if ($null -eq $actual) { "n/a" } elseif ($actual -le $target) { "true" } else { "false" }
        $actualText = if ($null -eq $actual) { "n/a" } else { [Math]::Round($actual, 0) }
        $lines.Add("| $metric | $target | $actualText | $pass |") | Out-Null
    }

    $rrAcceptance = $Summary.rr_acceptance
    if ($null -ne $rrAcceptance -and $rrAcceptance.required) {
        $lines.Add("") | Out-Null
        $lines.Add("## DX12 DLSS RR Acceptance") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("- Pass: $($rrAcceptance.pass)") | Out-Null
        $lines.Add("- Estimated stress frames: $($rrAcceptance.estimated_frames) / $($rrAcceptance.stress_frame_target)") | Out-Null
        if ($rrAcceptance.failures.Count -gt 0) {
            $lines.Add("- Failures: $($rrAcceptance.failures -join ', ')") | Out-Null
        }
        else {
            $lines.Add("- Failures: none") | Out-Null
        }
        $lines.Add("") | Out-Null
        $lines.Add("| required metric | present | mean | p95 | samples |") | Out-Null
        $lines.Add("|---|---|---:|---:|---:|") | Out-Null
        foreach ($metric in @("dlss_rr_gpu_ns", "solari_pass_dlss_rr_guide_resolve_ns", "frame_ns")) {
            $entry = $rrAcceptance.required_metrics[$metric]
            $lines.Add("| $metric | $($entry["present"]) | $($entry["mean"]) | $($entry["p95"]) | $($entry["count"]) |") | Out-Null
        }
    }

    if ($null -ne $comparison) {
        $lines.Add("") | Out-Null
        $lines.Add("## Baseline Comparison") | Out-Null
        $lines.Add("") | Out-Null
        $lines.Add("| metric | baseline mean | current mean | delta | delta percent | better |") | Out-Null
        $lines.Add("|---|---:|---:|---:|---:|---|") | Out-Null
        foreach ($metric in $primaryMetrics) {
            if ($comparison.Contains($metric)) {
                $entry = $comparison[$metric]
                $row = "| {0} | {1} | {2} | {3} | {4} | {5} |" -f `
                    $metric, `
                    $entry.baseline_mean, `
                    $entry.current_mean, `
                    $entry.delta, `
                    $entry.delta_percent, `
                    $entry.better
                $lines.Add($row) | Out-Null
            }
        }
    }

    $lines.Add("") | Out-Null
    $lines.Add("Full machine-readable output: summary.json") | Out-Null

    Set-Content -Path $Path -Value $lines -Encoding UTF8
}
