#!/usr/bin/env python3
"""Generate a DX12 parity dashboard from benchmark JSON files."""

from __future__ import annotations

import argparse
import csv
import json
import math
import sys
import tempfile
from pathlib import Path
from typing import Any


DELTA_METRICS: tuple[tuple[str, str, str], ...] = (
    ("fps", "higher", "FPS"),
    ("frame_ns", "lower", "frame time"),
    ("present_wait_ns", "lower", "present wait"),
    ("post_process_gpu_ns", "lower", "post process GPU"),
    ("standard_raster_gpu_ns", "lower", "standard raster GPU"),
    ("solari_gpu_ns", "lower", "Solari GPU"),
    ("meshlet_visibility_gpu_ns", "lower", "meshlet visibility GPU"),
    ("cloud_total_gpu_ns", "lower", "cloud total GPU"),
    ("ui_overlay_cpu_ns", "lower", "UI overlay CPU"),
    ("meshlet_extract_cpu_ns", "lower", "meshlet extract CPU"),
    ("meshlet_prepare_cpu_ns", "lower", "meshlet prepare CPU"),
    ("meshlet_instance_buffer_upload_bytes", "lower", "meshlet instance upload bytes"),
    ("meshlet_material_buffer_upload_bytes", "lower", "meshlet material upload bytes"),
    ("meshlet_view_visibility_buffer_upload_bytes", "lower", "meshlet view visibility upload bytes"),
    ("meshlet_buffer_reallocations", "lower", "meshlet buffer reallocations"),
    ("meshlet_buffer_capacity_high_water_bytes", "lower", "meshlet buffer capacity high water"),
    ("meshlet_view_resource_cache_rebuilds", "lower", "meshlet view cache rebuilds"),
    ("meshlet_culling_output_buffer_bytes", "lower", "meshlet culling output buffer bytes"),
    ("meshlet_indirect_draw_buffer_writes", "lower", "meshlet indirect buffer writes"),
    ("meshlet_asset_buffer_upload_bytes", "lower", "meshlet asset upload bytes"),
    ("meshlet_asset_buffer_grow_copies", "lower", "meshlet asset grow copies"),
    ("meshlet_asset_buffer_capacity_bytes", "lower", "meshlet asset buffer capacity"),
    ("world_stream_apply_cpu_ns", "lower", "world stream CPU"),
    ("world_stream_render_prep_queue_depth", "lower", "world stream render-prep queue"),
    ("world_stream_render_prep_deferred_chunks", "lower", "world stream deferred chunks"),
    ("world_stream_render_prep_dynamic_mesh_assets", "lower", "world stream dynamic mesh assets"),
    ("physics_fixed_update_cpu_ns", "lower", "physics CPU"),
    ("network_receive_cpu_ns", "lower", "network receive CPU"),
    ("cef_on_paint_fps", "higher", "CEF OnPaint FPS"),
    ("cef_on_accelerated_paint_fps", "higher", "CEF accelerated paint FPS"),
    ("cef_cpu_upload_bytes", "lower", "CEF CPU upload bytes"),
    ("cef_gpu_copy_bytes", "lower", "CEF GPU copy bytes"),
    ("cef_gpu_copy_ns", "lower", "CEF GPU copy"),
    ("cef_gpu_frame_ready_count", "higher", "CEF GPU ready frames"),
    ("cef_gpu_frame_not_ready_count", "lower", "CEF GPU not-ready frames"),
    ("cef_gpu_frame_reused_count", "lower", "CEF GPU reused frames"),
    ("cef_gpu_frame_blocking_wait_count", "lower", "CEF GPU blocking waits"),
    ("cef_transport_fallback_count", "lower", "CEF fallback count"),
    ("render_upload_write_texture_calls", "lower", "render texture upload calls"),
    ("render_upload_write_texture_bytes", "lower", "render texture upload bytes"),
    ("render_upload_write_buffer_calls", "lower", "render buffer upload calls"),
    ("render_upload_write_buffer_bytes", "lower", "render buffer upload bytes"),
    ("render_upload_write_buffer_with_calls", "lower", "render buffer-with upload calls"),
    ("render_upload_write_buffer_with_bytes", "lower", "render buffer-with upload bytes"),
    ("render_upload_callsite_count", "lower", "render upload callsite count"),
    ("render_churn_bind_group_creations", "lower", "bind group creations"),
    ("render_churn_bind_group_layout_creations", "lower", "bind group layout creations"),
    ("render_churn_bind_group_layout_cache_misses", "lower", "bind group layout cache misses"),
    ("render_churn_pipeline_layout_creations", "lower", "pipeline layout creations"),
    ("render_churn_render_pipeline_queued", "lower", "render pipelines queued"),
    ("render_churn_compute_pipeline_queued", "lower", "compute pipelines queued"),
    ("render_churn_render_pipeline_creations", "lower", "render pipeline creations"),
    ("render_churn_compute_pipeline_creations", "lower", "compute pipeline creations"),
    ("render_churn_pipeline_cache_misses", "lower", "pipeline cache misses"),
    ("render_churn_material_pipeline_key_count", "lower", "material pipeline keys"),
    ("render_churn_post_process_pipeline_key_count", "lower", "post-process pipeline keys"),
    ("render_churn_cloud_pipeline_key_count", "lower", "cloud pipeline keys"),
    ("render_churn_solari_pipeline_key_count", "lower", "Solari pipeline keys"),
    ("render_churn_meshlet_pipeline_key_count", "lower", "meshlet pipeline keys"),
    ("render_churn_ui_pipeline_key_count", "lower", "UI pipeline keys"),
    ("render_churn_debug_overlay_pipeline_key_count", "lower", "debug overlay pipeline keys"),
    ("render_churn_event_count", "lower", "render churn event count"),
    ("render_command_command_encoder_creations", "lower", "command encoder creations"),
    ("render_command_render_passes", "lower", "render passes"),
    ("render_command_compute_passes", "lower", "compute passes"),
    ("render_command_command_buffers_submitted", "lower", "command buffers submitted"),
    ("render_command_queue_submits", "lower", "queue submits"),
    ("render_command_copy_commands", "lower", "copy commands"),
    ("render_command_native_interop_command_insertions", "lower", "native interop command insertions"),
    ("render_command_event_count", "lower", "render command event count"),
    ("render_readback_readback_requested_count", "lower", "readback requests"),
    ("render_readback_readback_completed_count", "lower", "readback completions"),
    ("render_readback_readback_dropped_count", "lower", "readback drops"),
    ("render_readback_readback_blocking_wait_count", "lower", "readback blocking waits"),
    ("render_readback_readback_latency_frame_sum", "lower", "readback latency frame sum"),
    ("render_readback_readback_latency_frame_max", "lower", "readback latency frame max"),
    ("render_readback_map_async_count", "lower", "readback map_async count"),
    ("render_readback_poll_count", "lower", "readback device polls"),
    ("render_readback_event_count", "lower", "readback event count"),
    ("render_shader_shader_module_creations", "lower", "shader module creations"),
    ("render_shader_shader_module_create_ns", "lower", "shader module creation time"),
    ("render_shader_shader_variant_requests", "lower", "shader variant requests"),
    ("render_shader_shader_def_count", "lower", "shader definition count"),
    ("render_shader_material_specializations", "lower", "material specializations"),
    ("render_shader_render_pipeline_create_count", "lower", "render pipeline create count"),
    ("render_shader_render_pipeline_create_ns", "lower", "render pipeline create time"),
    ("render_shader_compute_pipeline_create_count", "lower", "compute pipeline create count"),
    ("render_shader_compute_pipeline_create_ns", "lower", "compute pipeline create time"),
    ("render_shader_pipeline_create_count", "lower", "pipeline create count"),
    ("render_shader_pipeline_create_ns", "lower", "pipeline create time"),
    ("render_shader_pipeline_specialization_count", "lower", "pipeline specialization count"),
    ("render_shader_event_count", "lower", "render shader event count"),
    ("transient_texture_creates", "lower", "transient texture creates"),
    ("transient_buffer_creates", "lower", "transient buffer creates"),
    ("transient_texture_descriptor_miss_creates", "lower", "transient texture descriptor misses"),
    ("transient_texture_lifetime_conflict_creates", "lower", "transient texture lifetime conflicts"),
    ("transient_buffer_descriptor_miss_creates", "lower", "transient buffer descriptor misses"),
    ("transient_buffer_lifetime_conflict_creates", "lower", "transient buffer lifetime conflicts"),
    ("transient_texture_near_miss_usage", "lower", "transient texture usage near-misses"),
    ("transient_texture_near_miss_format", "lower", "transient texture format near-misses"),
    ("transient_texture_near_miss_size", "lower", "transient texture size near-misses"),
    ("transient_texture_label_variant_descriptors", "lower", "transient texture label variants"),
    ("transient_texture_every_frame_create_descriptors", "lower", "transient texture every-frame creates"),
    ("transient_texture_resize_like_create_descriptors", "lower", "transient texture resize-like creates"),
    ("transient_buffer_near_miss_usage", "lower", "transient buffer usage near-misses"),
    ("transient_buffer_near_miss_size", "lower", "transient buffer size near-misses"),
    ("transient_buffer_label_variant_descriptors", "lower", "transient buffer label variants"),
    ("transient_buffer_every_frame_create_descriptors", "lower", "transient buffer every-frame creates"),
    ("transient_buffer_resize_like_create_descriptors", "lower", "transient buffer resize-like creates"),
    ("render_scheduler_pressure", "lower", "render scheduler pressure"),
)

GPU_PASS_METRICS = {
    "post_process_gpu_ns",
    "standard_raster_gpu_ns",
    "solari_gpu_ns",
    "meshlet_visibility_gpu_ns",
    "cloud_total_gpu_ns",
}

CPU_WORK_METRICS = {
    "ui_overlay_cpu_ns",
    "meshlet_extract_cpu_ns",
    "meshlet_prepare_cpu_ns",
    "world_stream_apply_cpu_ns",
    "physics_fixed_update_cpu_ns",
    "network_receive_cpu_ns",
}

RECOMMENDATION_CLASSIFICATIONS: tuple[str, ...] = (
    "present_bound",
    "cpu_upload_bound",
    "cef_transport_bound",
    "barrier_or_state_bound",
    "descriptor_or_pso_churn_bound",
    "runtime_pipeline_creation_bound",
    "readback_or_fence_bound",
    "meshlet_or_stream_upload_bound",
    "shader_or_pass_gpu_bound",
    "inconclusive_needs_pix",
    "inconclusive_needs_presentmon",
    "inconclusive_needs_upload_callsite_table",
)

UPLOAD_CALLSITE_METRICS: tuple[str, ...] = (
    "render_upload_write_texture_calls",
    "render_upload_write_texture_bytes",
    "render_upload_write_buffer_calls",
    "render_upload_write_buffer_bytes",
    "render_upload_write_buffer_with_calls",
    "render_upload_write_buffer_with_bytes",
    "render_upload_callsite_count",
)

RUNTIME_PIPELINE_CREATION_METRICS: tuple[str, ...] = (
    "render_churn_render_pipeline_creations",
    "render_churn_compute_pipeline_creations",
    "render_shader_render_pipeline_create_count",
    "render_shader_compute_pipeline_create_count",
    "render_shader_pipeline_create_count",
)

DESCRIPTOR_OR_PSO_CHURN_METRICS: tuple[str, ...] = (
    "render_churn_bind_group_creations",
    "render_churn_bind_group_layout_creations",
    "render_churn_bind_group_layout_cache_misses",
    "render_churn_pipeline_layout_creations",
    "render_churn_pipeline_cache_misses",
)

READBACK_OR_FENCE_METRICS: tuple[str, ...] = (
    "render_readback_readback_requested_count",
    "render_readback_readback_completed_count",
    "render_readback_readback_dropped_count",
    "render_readback_readback_blocking_wait_count",
    "render_readback_readback_latency_frame_sum",
    "render_readback_readback_latency_frame_max",
    "render_readback_map_async_count",
    "render_readback_poll_count",
)

MESHLET_OR_STREAM_UPLOAD_METRICS: tuple[str, ...] = (
    "meshlet_instance_buffer_upload_bytes",
    "meshlet_material_buffer_upload_bytes",
    "meshlet_view_visibility_buffer_upload_bytes",
    "meshlet_buffer_reallocations",
    "meshlet_culling_output_buffer_bytes",
    "meshlet_indirect_draw_buffer_writes",
    "meshlet_asset_buffer_upload_bytes",
    "meshlet_asset_buffer_grow_copies",
    "world_stream_render_prep_queue_depth",
    "world_stream_render_prep_deferred_chunks",
    "world_stream_render_prep_dynamic_mesh_assets",
)

CPU_UPLOAD_BYTE_THRESHOLD = 1_048_576.0
MATERIAL_TIME_REGRESSION_NS = 200_000.0
PRESENT_WAIT_REGRESSION_NS = 500_000.0
PRESENT_WAIT_DOMINANCE_RATIO = 0.25

NVIDIA_VENDOR_ACTIONS: dict[str, tuple[str, ...]] = {
    "descriptor churn": (
        "evaluate a bindless-like material table where Bevy/wgpu permits it",
        "reduce per-draw bind group changes before changing shader layout",
        "inspect descriptor heap rollover and root signature switches in PIX",
    ),
    "pipeline churn": (
        "increase pipeline warmup coverage before first visible gameplay",
        "experiment with a persistent PSO cache keyed by adapter, driver, wgpu, shader, and pipeline descriptor hashes",
        "reduce material pipeline-key fragmentation by moving non-structural toggles to uniforms",
    ),
    "shader compilation": (
        "move remaining runtime shader and pipeline creation into controlled warmup",
        "compare NVIDIA driver pipeline-cache behavior before adding native cache hooks",
        "reduce variant pressure only when benchmark top events identify the hot family",
    ),
    "shader variant pressure": (
        "inspect material and shader-definition cardinality before rewriting shaders",
        "prefer uniform toggles for non-layout-changing quality/debug options",
        "keep structural specialization for format, sample-count, topology, and binding layout changes",
    ),
    "upload-bound": (
        "use GPU-native or persistent staging upload paths only for measured hot callsites",
        "keep CEF accelerated transport and meshlet/world-stream uploads separately attributable",
        "prototype native upload heaps only behind a DX12/NVIDIA experiment flag",
    ),
    "barrier/state-bound": (
        "use PIX to collapse redundant transitions around the identified pass",
        "avoid native interop state changes outside the shared dx12_native boundary",
        "prefer pass ordering/resource-state fixes before async queue experiments",
    ),
    "submission fragmentation": (
        "reduce command buffer and queue-submit count before adding async scheduling",
        "use GPUView or PIX queue lanes to prove idle overlap exists",
        "keep async copy/compute experiments opt-in and reject them if p95 worsens",
    ),
    "CEF/native interop sync": (
        "consume completed CEF GPU frames without blocking waits",
        "reuse the last ready frame instead of waiting on a hot fence",
        "keep CPU paint as a measured fallback lane, not a hidden path",
    ),
    "shader/pass GPU bound": (
        "inspect hot NVIDIA shader variants with Nsight or driver shader tools",
        "compare register pressure, occupancy, memory loads, and branch divergence",
        "land shader rewrites only with DX12/Vulkan before-after timings and visual evidence",
    ),
}

VENDOR_REQUIRED_EVIDENCE: dict[str, str] = {
    "descriptor churn": "render_churn bind-group/layout deltas or PIX descriptor heap switch counters",
    "pipeline churn": "render_churn pipeline creation/cache counters or PIX PSO creation counters",
    "shader compilation": "render_shader module/pipeline creation counters during the sample window",
    "shader variant pressure": "render_shader variant/material specialization deltas and top event labels",
    "upload-bound": "render_upload, meshlet/world-stream upload, CEF upload/copy counters, or PIX copy evidence",
    "barrier/state-bound": "PIX barrier/resource-state summary for the losing pass",
    "submission fragmentation": "render_command submit/command-buffer deltas plus PIX or GPUView queue evidence",
    "CEF/native interop sync": "CEF GPU frame readiness/reuse/blocking-wait counters plus PIX fence/queue evidence",
    "shader/pass GPU bound": "pass-level GPU timing regression plus vendor shader analysis for the hot variant",
}

MOONSHOT_EXPERIMENTS: tuple[dict[str, str], ...] = (
    {
        "id": "gpu_driven_visibility_pipeline_refinement",
        "title": "GPU-driven visibility pipeline refinement",
        "gate": "meshlet CPU prep or meshlet upload pressure is the confirmed bottleneck",
        "payoff": "stronger dense-scene and meshlet-heavy DX12 scaling",
    },
    {
        "id": "bindless_style_material_resource_table",
        "title": "Bindless-style material resource table",
        "gate": "descriptor churn is confirmed by render_churn counters or PIX descriptor heap switches",
        "payoff": "fewer per-draw bind group/layout changes in material-heavy scenes",
    },
    {
        "id": "dx12_specific_render_graph_compiler",
        "title": "DX12-specific render graph compiler",
        "gate": "barrier/state churn, submission fragmentation, or tiny pass overhead is confirmed",
        "payoff": "fewer barriers, fewer passes, and better queue utilization",
    },
    {
        "id": "native_dx12_residency_memory_budget_diagnostics",
        "title": "Native DX12 residency and memory budget diagnostics",
        "gate": "p95 spikes, transient allocation churn, or VRAM pressure remain unexplained",
        "payoff": "budget/usage telemetry for large worlds, CEF, clouds, Solari, and future DLSS",
    },
)


def load_json(path: Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8-sig") as handle:
        value = json.load(handle)
    if not isinstance(value, dict):
        raise ValueError(f"{path} did not contain a JSON object")
    return value


def get_path(value: Any, *path: str, default: Any = None) -> Any:
    current = value
    for key in path:
        if not isinstance(current, dict) or key not in current:
            return default
        current = current[key]
    return current


def number(value: Any) -> float | None:
    if value is None:
        return None
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        return None
    if not math.isfinite(parsed):
        return None
    return parsed


def metric_value(summary: dict[str, Any], metric: str, field: str) -> float | None:
    return number(get_path(summary, "metrics", metric, field))


def first_metric_value(summary: dict[str, Any], names: tuple[str, ...], field: str = "mean") -> float | None:
    for name in names:
        value = metric_value(summary, name, field)
        if value is not None:
            return value
    return None


def percent_delta(current: float | None, baseline: float | None) -> float | None:
    if current is None or baseline is None or baseline == 0:
        return None
    return ((current - baseline) / abs(baseline)) * 100.0


def classify_delta(metric: str, direction: str, mean_pct: float | None, p95_pct: float | None) -> tuple[str, str]:
    observed = p95_pct if p95_pct is not None else mean_pct
    if observed is None:
        return "unknown", "missing metric"
    regression = -observed if direction == "higher" else observed
    if regression > 5.0:
        return "red", "regression over 5 percent"
    if regression > 1.0:
        return "yellow", "regression over 1 percent"
    if metric == "frame_ns" and regression > 0.0:
        return "yellow", "minor p95 regression"
    return "green", "within threshold"


def build_delta_rows(vulkan: dict[str, Any], dx12: dict[str, Any]) -> list[dict[str, Any]]:
    rows: list[dict[str, Any]] = []
    for metric, direction, label in DELTA_METRICS:
        vk_mean = metric_value(vulkan, metric, "mean")
        dx_mean = metric_value(dx12, metric, "mean")
        vk_p95 = metric_value(vulkan, metric, "p95")
        dx_p95 = metric_value(dx12, metric, "p95")
        mean_delta = None if vk_mean is None or dx_mean is None else dx_mean - vk_mean
        p95_delta = None if vk_p95 is None or dx_p95 is None else dx_p95 - vk_p95
        mean_pct = percent_delta(dx_mean, vk_mean)
        p95_pct = percent_delta(dx_p95, vk_p95)
        status, reason = classify_delta(metric, direction, mean_pct, p95_pct)
        rows.append(
            {
                "metric": metric,
                "label": label,
                "direction": direction,
                "vulkan_mean": vk_mean,
                "dx12_mean": dx_mean,
                "delta": mean_delta,
                "delta_pct": mean_pct,
                "vulkan_p95": vk_p95,
                "dx12_p95": dx_p95,
                "p95_delta": p95_delta,
                "p95_delta_pct": p95_pct,
                "status": status,
                "reason": reason,
            }
        )
    return rows


def is_accelerated_cef_lane(summary: dict[str, Any]) -> bool:
    config = summary.get("config", {})
    selection = summary.get("cef_ui_transport_selection", {})
    transport = str(
        selection.get("selected") or config.get("cef_paint_transport", "")
    ).lower()
    return bool(config.get("cef_accelerated_feature_requested")) or transport in {
        "auto",
        "d3d11on12",
        "d3d11_shared_texture_dx12_copy",
    }


def summarize_gpu(summary: dict[str, Any]) -> str:
    gpu = get_path(summary, "hardware", "gpu", default=[])
    if isinstance(gpu, list) and gpu:
        first = gpu[0]
        if isinstance(first, dict):
            name = first.get("Name") or first.get("name") or "unknown"
            driver = first.get("DriverVersion") or first.get("driver_version") or "unknown"
            return f"{name} / driver {driver}"
    return "unknown"


def first_gpu_adapter_ram(summary: dict[str, Any]) -> float | None:
    gpu = get_path(summary, "hardware", "gpu", default=[])
    if isinstance(gpu, list) and gpu:
        first = gpu[0]
        if isinstance(first, dict):
            return number(first.get("AdapterRAM") or first.get("adapter_ram") or first.get("adapter_ram_bytes"))
    return None


def external_number(external: dict[str, float], *names: str) -> float | None:
    for name in names:
        value = external.get(normalize_key(name))
        if value is not None:
            return value
    return None


def memory_snapshot(summary: dict[str, Any], external: dict[str, float] | None = None) -> dict[str, Any]:
    external = external or {}
    dx12_memory = summary.get("dx12_memory")
    if not isinstance(dx12_memory, dict):
        dx12_memory = get_path(summary, "environment", "dx12_memory", default={})
    if not isinstance(dx12_memory, dict):
        dx12_memory = {}

    local_budget = number(dx12_memory.get("local_budget_bytes"))
    if local_budget is None:
        local_budget = first_metric_value(
            summary,
            (
                "dx12_memory_local_budget_bytes",
                "d3d12_local_budget_bytes",
                "gpu_local_budget_bytes",
            ),
        )
    if local_budget is None:
        local_budget = external_number(
            external,
            "dx12_memory_local_budget_bytes",
            "d3d12_local_budget_bytes",
            "local_budget_bytes",
            "budget_bytes",
        )

    local_usage = number(dx12_memory.get("local_usage_bytes"))
    if local_usage is None:
        local_usage = first_metric_value(
            summary,
            (
                "dx12_memory_local_usage_bytes",
                "d3d12_local_usage_bytes",
                "gpu_local_usage_bytes",
            ),
        )
    if local_usage is None:
        local_usage = external_number(
            external,
            "dx12_memory_local_usage_bytes",
            "d3d12_local_usage_bytes",
            "local_usage_bytes",
            "usage_bytes",
        )

    available = number(dx12_memory.get("local_available_for_reservation_bytes"))
    if available is None:
        available = external_number(external, "local_available_for_reservation_bytes", "available_for_reservation_bytes")
    reservation = number(dx12_memory.get("local_current_reservation_bytes"))
    if reservation is None:
        reservation = external_number(external, "local_current_reservation_bytes", "current_reservation_bytes")
    adapter_ram = number(dx12_memory.get("adapter_ram_bytes"))
    if adapter_ram is None:
        adapter_ram = first_gpu_adapter_ram(summary)

    usage_pct = None
    if local_budget is not None and local_usage is not None and local_budget > 0:
        usage_pct = (local_usage / local_budget) * 100.0

    has_budget = any(value is not None for value in (local_budget, local_usage, available, reservation))
    status = str(dx12_memory.get("status") or ("provided" if has_budget else "adapter_ram_only" if adapter_ram is not None else "not_collected"))
    source = str(dx12_memory.get("source") or ("summary_or_external_trace" if has_budget else "win32_video_controller_adapter_ram" if adapter_ram is not None else "none"))
    return {
        "status": status,
        "source": source,
        "local_budget_bytes": local_budget,
        "local_usage_bytes": local_usage,
        "local_available_for_reservation_bytes": available,
        "local_current_reservation_bytes": reservation,
        "adapter_ram_bytes": adapter_ram,
        "local_usage_pct": usage_pct,
    }


def infer_adapter_vendor(summary: dict[str, Any]) -> str:
    gpu_text = summarize_gpu(summary).lower()
    presentation = summary.get("render_presentation", {})
    config = summary.get("config", {})
    extra = " ".join(
        str(value).lower()
        for value in (
            presentation.get("adapter"),
            presentation.get("adapter_name"),
            presentation.get("vendor"),
            config.get("adapter"),
            config.get("gpu_vendor"),
        )
        if value is not None
    )
    text = f"{gpu_text} {extra}"
    if any(token in text for token in ("nvidia", "geforce", "rtx", "gtx")):
        return "nvidia"
    if any(token in text for token in ("amd", "radeon")):
        return "amd"
    if any(token in text for token in ("intel", "arc graphics", "iris xe")):
        return "intel"
    return "unknown"


def backend_summary(summary: dict[str, Any]) -> dict[str, str]:
    config = summary.get("config", {})
    presentation = summary.get("render_presentation", {})
    cef_selection = summary.get("cef_ui_transport_selection", {})
    width = config.get("window_width") or presentation.get("resolution_width") or "unknown"
    height = config.get("window_height") or presentation.get("resolution_height") or "unknown"
    return {
        "backend": str(config.get("render_backend") or presentation.get("backend") or "unknown"),
        "adapter": summarize_gpu(summary),
        "driver": str(presentation.get("driver") or "see adapter"),
        "present_mode": str(config.get("present_mode") or presentation.get("present_mode") or "unknown"),
        "surface_present_mode": str(presentation.get("surface_selected_present_mode") or "unknown"),
        "max_frame_latency": str(
            config.get("requested_maximum_frame_latency")
            or presentation.get("desired_maximum_frame_latency")
            or "unknown"
        ),
        "resolution": f"{width}x{height}",
        "cef_transport": str(
            cef_selection.get("selected") or config.get("cef_paint_transport") or "unknown"
        ),
        "cef_mode": str(config.get("cef_ui_mode") or "unknown"),
    }


def summary_sample_seconds(summary: dict[str, Any]) -> float:
    seconds = number(get_path(summary, "samples", "sample_seconds"))
    if seconds is not None and seconds > 0.0:
        return seconds
    count = number(get_path(summary, "samples", "count"))
    if count is not None and count > 0.0:
        return count
    fps_count = metric_value(summary, "fps", "count")
    if fps_count is not None and fps_count > 0.0:
        return fps_count
    return 1.0


def upload_callsite_fix(operation: str, label: str) -> str:
    normalized = normalize_key(f"{operation} {label}")
    if "cef_ui_cpu_paint" in normalized:
        return "accelerated CEF or persistent texture upload ring"
    if operation == "write_texture":
        if "texture_gpu_image" in normalized:
            return "prove startup-only or move dynamic image uploads to persistent texture ring"
        return "persistent texture upload ring with dirty-rect batching"
    if "meshlet" in normalized or "visibility" in normalized:
        return "move measured meshlet/visibility writes to staging belt or range batching"
    if "uniform_buffer" in normalized:
        return "move owning dynamic uniform writer to FunUploadArena after semantic label split"
    if "buffer_vec" in normalized:
        return "batch owning BufferVec writes or move proven hot owner to FunUploadArena"
    if "solari" in normalized:
        return "batch Solari constants or stage them through FunUploadArena"
    if "debug" in normalized or "overlay" in normalized:
        return "batch debug overlay writes through staging belt"
    return "add semantic label, then choose staging belt, batching, or texture ring"


def upload_callsite_impact(
    operation: str,
    calls_per_frame: float | None,
    bytes_per_frame: float | None,
    bytes_per_second: float,
) -> str:
    if operation == "write_texture" and (
        bytes_per_second >= 16.0 * 1024.0 * 1024.0
        or (bytes_per_frame is not None and bytes_per_frame >= 256.0 * 1024.0)
    ):
        return "high"
    if calls_per_frame is not None and calls_per_frame >= 4.0:
        return "high"
    if bytes_per_frame is not None and bytes_per_frame >= 64.0 * 1024.0:
        return "high"
    if bytes_per_second >= 1.0 * 1024.0 * 1024.0:
        return "medium"
    if calls_per_frame is not None and calls_per_frame >= 1.0:
        return "medium"
    if bytes_per_frame is not None and bytes_per_frame >= 4.0 * 1024.0:
        return "medium"
    return "low"


def build_upload_callsite_kill_list(summary: dict[str, Any], limit: int = 10) -> list[dict[str, Any]]:
    raw_callsites = summary.get("render_upload_callsites")
    if not isinstance(raw_callsites, list):
        return []
    backend = backend_summary(summary)["backend"]
    sample_seconds = summary_sample_seconds(summary)
    fps = metric_value(summary, "fps", "mean")
    frame_count = fps * sample_seconds if fps is not None and fps > 0.0 else None
    merged: dict[tuple[str, str], dict[str, Any]] = {}
    for callsite in raw_callsites:
        if not isinstance(callsite, dict):
            continue
        operation = str(callsite.get("operation") or "unknown")
        label = str(callsite.get("label") or "unknown")
        calls = number(callsite.get("calls")) or 0.0
        bytes_uploaded = number(callsite.get("bytes")) or 0.0
        samples = number(callsite.get("samples")) or 0.0
        key = (operation, label)
        entry = merged.setdefault(
            key,
            {
                "operation": operation,
                "label": label,
                "calls": 0.0,
                "bytes": 0.0,
                "samples": 0.0,
            },
        )
        entry["calls"] += calls
        entry["bytes"] += bytes_uploaded
        entry["samples"] += samples

    rows: list[dict[str, Any]] = []
    for entry in merged.values():
        calls = float(entry["calls"])
        bytes_uploaded = float(entry["bytes"])
        calls_per_second = calls / sample_seconds
        bytes_per_second = bytes_uploaded / sample_seconds
        calls_per_frame = calls / frame_count if frame_count and frame_count > 0.0 else None
        bytes_per_frame = bytes_uploaded / frame_count if frame_count and frame_count > 0.0 else None
        operation = str(entry["operation"])
        label = str(entry["label"])
        rows.append(
            {
                "rank": 0,
                "callsite": f"{operation}:{label}",
                "backend": backend,
                "operation": operation,
                "label": label,
                "calls": int(calls),
                "bytes": int(bytes_uploaded),
                "samples": int(entry["samples"]),
                "calls_per_frame": calls_per_frame,
                "bytes_per_frame": bytes_per_frame,
                "calls_per_second": calls_per_second,
                "bytes_per_second": bytes_per_second,
                "p95_impact_guess": upload_callsite_impact(
                    operation,
                    calls_per_frame,
                    bytes_per_frame,
                    bytes_per_second,
                ),
                "fix": upload_callsite_fix(operation, label),
            }
        )
    rows.sort(
        key=lambda row: (
            -float(row["bytes_per_second"]),
            -float(row["calls_per_second"]),
            str(row["callsite"]),
        )
    )
    for index, row in enumerate(rows[:limit], start=1):
        row["rank"] = index
    return rows[:limit]


def vendor_follow_up(category: str, dx12: dict[str, Any]) -> dict[str, Any]:
    vendor = infer_adapter_vendor(dx12)
    actions: tuple[str, ...] = ()
    if vendor == "nvidia":
        actions = NVIDIA_VENDOR_ACTIONS.get(category, ())
    eligible = bool(actions)
    if category in {"unknown, requires PIX", "no clear bottleneck", "p95 regression", "present-bound"}:
        status = "blocked_until_specific_bottleneck"
    elif vendor != "nvidia":
        status = "not_nvidia_path"
    elif eligible:
        status = "eligible_optional_nvidia_experiment"
    else:
        status = "no_vendor_specific_action"
    return {
        "vendor": vendor,
        "status": status,
        "required_evidence": VENDOR_REQUIRED_EVIDENCE.get(
            category,
            "specific PIX/GPUView/Nsight evidence naming the losing subsystem",
        ),
        "actions": actions,
        "guardrail": "optional DX12/NVIDIA experiment only; must not regress AMD, Intel, or Vulkan lanes",
    }


def row_has_dx12_pressure(row: dict[str, Any] | None) -> bool:
    if row is None:
        return False
    for key in ("p95_delta", "delta"):
        value = row.get(key)
        if value is not None and value > 0:
            return True
    dx_value = row.get("dx12_p95") if row.get("dx12_p95") is not None else row.get("dx12_mean")
    vk_value = row.get("vulkan_p95") if row.get("vulkan_p95") is not None else row.get("vulkan_mean")
    return dx_value is not None and dx_value > 0 and (vk_value is None or vk_value <= 0)
    return False


def moonshot_follow_up(
    category: str,
    rows: list[dict[str, Any]],
    dx12: dict[str, Any],
    pix: dict[str, float],
    memory: dict[str, Any],
) -> list[dict[str, str]]:
    meshlet_pressure = any(
        row_has_dx12_pressure(row_by_metric(rows, metric))
        for metric in (
            "meshlet_prepare_cpu_ns",
            "meshlet_instance_buffer_upload_bytes",
            "meshlet_material_buffer_upload_bytes",
            "meshlet_view_visibility_buffer_upload_bytes",
            "meshlet_asset_buffer_upload_bytes",
        )
    )
    descriptor_pressure = category == "descriptor churn" or pix.get("descriptor_heap_switch_count", 0.0) > 0
    render_graph_pressure = category in {"barrier/state-bound", "submission fragmentation"} or any(
        pix.get(key, 0.0) > 0
        for key in ("barrier_count", "resource_barrier_count", "command_list_count", "queue_idle_ns")
    )
    memory_observed = memory["status"] != "not_collected"
    memory_pressure = memory.get("local_usage_pct") is not None and memory["local_usage_pct"] >= 85.0
    transient_pressure = category == "transient allocation churn"

    rows_out: list[dict[str, str]] = []
    for experiment in MOONSHOT_EXPERIMENTS:
        experiment_id = experiment["id"]
        if experiment_id == "gpu_driven_visibility_pipeline_refinement":
            status = "eligible_after_confirmed_meshlet_pressure" if category == "upload-bound" and meshlet_pressure else "blocked_until_meshlet_prep_or_upload_bottleneck"
            evidence = "meshlet CPU/upload metrics" if meshlet_pressure else "none"
        elif experiment_id == "bindless_style_material_resource_table":
            status = "eligible_after_descriptor_bottleneck" if descriptor_pressure else "blocked_until_descriptor_churn_bottleneck"
            evidence = "descriptor churn category or PIX descriptor heap switches" if descriptor_pressure else "none"
        elif experiment_id == "dx12_specific_render_graph_compiler":
            status = "eligible_after_barrier_or_submission_bottleneck" if render_graph_pressure else "blocked_until_barrier_or_submission_bottleneck"
            evidence = "PIX barriers/queue counters or submission fragmentation category" if render_graph_pressure else "none"
        else:
            status = "diagnostics_present" if memory_observed else "instrumentation_needed"
            if memory_pressure:
                status = "eligible_after_memory_pressure"
            elif transient_pressure:
                status = "eligible_after_transient_allocation_churn"
            evidence = "DX12 memory budget snapshot" if memory_observed else "none"
        rows_out.append(
            {
                "id": experiment_id,
                "title": experiment["title"],
                "status": status,
                "gate": experiment["gate"],
                "evidence": evidence,
                "payoff": experiment["payoff"],
            }
        )
    return rows_out


def parse_external_csv(path: Path | None) -> dict[str, float]:
    if path is None:
        return {}
    if not path.exists():
        raise FileNotFoundError(path)
    totals: dict[str, float] = {}
    counts: dict[str, int] = {}
    with path.open("r", encoding="utf-8-sig", newline="") as handle:
        reader = csv.DictReader(handle)
        for row in reader:
            for raw_key, raw_value in row.items():
                key = normalize_key(raw_key)
                value = number(raw_value)
                if value is None:
                    continue
                totals[key] = totals.get(key, 0.0) + value
                counts[key] = counts.get(key, 0) + 1
    return {key: totals[key] / counts[key] for key in totals if counts.get(key, 0) > 0}


def normalize_key(key: str | None) -> str:
    if not key:
        return ""
    out = []
    prev_sep = False
    for char in key.lower():
        if char.isalnum():
            out.append(char)
            prev_sep = False
        elif not prev_sep:
            out.append("_")
            prev_sep = True
    return "".join(out).strip("_")


def row_by_metric(rows: list[dict[str, Any]], metric: str) -> dict[str, Any] | None:
    for row in rows:
        if row["metric"] == metric:
            return row
    return None


def likely_bottleneck(
    rows: list[dict[str, Any]],
    vulkan: dict[str, Any],
    dx12: dict[str, Any],
    pix: dict[str, float],
    presentmon: dict[str, float],
) -> tuple[str, str, list[str]]:
    reasons: list[str] = []
    dx_fps = metric_value(dx12, "fps", "mean")
    vk_fps = metric_value(vulkan, "fps", "mean")
    dx_frame_p95 = metric_value(dx12, "frame_ns", "p95")
    vk_frame_p95 = metric_value(vulkan, "frame_ns", "p95")
    dx_loses_avg = dx_fps is not None and vk_fps is not None and dx_fps < vk_fps
    p95_regression = (
        dx_frame_p95 is not None
        and vk_frame_p95 is not None
        and dx_frame_p95 > vk_frame_p95 * 1.01
    )

    cef_cpu_upload = metric_value(dx12, "cef_cpu_upload_bytes", "mean") or 0.0
    if is_accelerated_cef_lane(dx12) and cef_cpu_upload > 0.0:
        reasons.append("accelerated CEF lane still reports CPU upload bytes")
        return "CEF sync", "PIX plus CEF transport counters", reasons

    readback_blocking = metric_value(dx12, "render_readback_readback_blocking_wait_count", "p95") or 0.0
    if readback_blocking > 0.0:
        reasons.append("DX12 render readback diagnostics observed blocking waits")
        return "readback sync", "PIX plus readback event table", reasons
    readback_requests = row_by_metric(rows, "render_readback_readback_requested_count")
    readback_maps = row_by_metric(rows, "render_readback_map_async_count")
    if readback_requests and readback_requests["p95_delta"] is not None and readback_requests["p95_delta"] > 0:
        reasons.append("DX12 readback requests exceeded the Vulkan lane")
        return "readback/capture overhead", "benchmark readback top events", reasons
    if readback_maps and readback_maps["p95_delta"] is not None and readback_maps["p95_delta"] > 0:
        reasons.append("DX12 map_async activity exceeded the Vulkan lane")
        return "readback/capture overhead", "benchmark readback top events", reasons

    present = row_by_metric(rows, "present_wait_ns")
    if present and present["p95_delta"] is not None and present["p95_delta"] > 500_000:
        reasons.append("DX12 present_wait_ns p95 is materially higher")
        return "present-bound", "PresentMon", reasons
    if presentmon:
        wait_keys = [key for key in presentmon if "present" in key and "wait" in key]
        if wait_keys:
            reasons.append("PresentMon CSV includes present wait fields; inspect pacing there")
            return "present-bound", "PresentMon", reasons

    cef_gpu = row_by_metric(rows, "cef_gpu_copy_ns")
    if cef_cpu_upload > 0.0 or (cef_gpu and cef_gpu["p95_delta"] is not None and cef_gpu["p95_delta"] > 200_000):
        reasons.append("CEF upload/copy metric regressed")
        return "upload-bound", "PIX", reasons

    texture_upload = row_by_metric(rows, "render_upload_write_texture_bytes")
    buffer_upload = row_by_metric(rows, "render_upload_write_buffer_bytes")
    if texture_upload and texture_upload["p95_delta"] is not None and texture_upload["p95_delta"] > 1_000_000:
        reasons.append("DX12 texture upload bytes are materially higher")
        return "upload-bound", "PIX", reasons
    if buffer_upload and buffer_upload["p95_delta"] is not None and buffer_upload["p95_delta"] > 1_000_000:
        reasons.append("DX12 buffer upload bytes are materially higher")
        return "upload-bound", "PIX", reasons

    render_pipeline_creates = row_by_metric(rows, "render_churn_render_pipeline_creations")
    compute_pipeline_creates = row_by_metric(rows, "render_churn_compute_pipeline_creations")
    bind_group_creates = row_by_metric(rows, "render_churn_bind_group_creations")
    bind_group_layout_creates = row_by_metric(rows, "render_churn_bind_group_layout_creations")
    bind_group_layout_misses = row_by_metric(rows, "render_churn_bind_group_layout_cache_misses")
    pipeline_misses = row_by_metric(rows, "render_churn_pipeline_cache_misses")
    if bind_group_creates and bind_group_creates["p95_delta"] is not None and bind_group_creates["p95_delta"] > 0:
        reasons.append("DX12 bind group creation exceeded the Vulkan lane")
        return "descriptor churn", "PIX", reasons
    if bind_group_layout_creates and bind_group_layout_creates["p95_delta"] is not None and bind_group_layout_creates["p95_delta"] > 0:
        reasons.append("DX12 bind group layout creation exceeded the Vulkan lane")
        return "descriptor churn", "PIX", reasons
    if bind_group_layout_misses and bind_group_layout_misses["p95_delta"] is not None and bind_group_layout_misses["p95_delta"] > 0:
        reasons.append("DX12 bind group layout cache misses exceeded the Vulkan lane")
        return "descriptor churn", "PIX", reasons
    if render_pipeline_creates and render_pipeline_creates["dx12_p95"] and render_pipeline_creates["dx12_p95"] > 0:
        reasons.append("DX12 runtime render pipeline creation was observed")
        return "pipeline churn", "PIX", reasons
    if compute_pipeline_creates and compute_pipeline_creates["dx12_p95"] and compute_pipeline_creates["dx12_p95"] > 0:
        reasons.append("DX12 runtime compute pipeline creation was observed")
        return "pipeline churn", "PIX", reasons
    if pipeline_misses and pipeline_misses["p95_delta"] is not None and pipeline_misses["p95_delta"] > 0:
        reasons.append("DX12 pipeline cache misses exceeded the Vulkan lane")
        return "pipeline churn", "PIX", reasons

    queue_submits = row_by_metric(rows, "render_command_queue_submits")
    command_buffers = row_by_metric(rows, "render_command_command_buffers_submitted")
    encoders = row_by_metric(rows, "render_command_command_encoder_creations")
    native_interop = row_by_metric(rows, "render_command_native_interop_command_insertions")
    if queue_submits and queue_submits["p95_delta"] is not None and queue_submits["p95_delta"] > 0:
        reasons.append("DX12 queue submits exceeded the Vulkan lane")
        return "submission fragmentation", "PIX or GPUView", reasons
    if command_buffers and command_buffers["p95_delta"] is not None and command_buffers["p95_delta"] > 0:
        reasons.append("DX12 submitted more command buffers than the Vulkan lane")
        return "submission fragmentation", "PIX or GPUView", reasons
    if encoders and encoders["p95_delta"] is not None and encoders["p95_delta"] > 0:
        reasons.append("DX12 created more command encoders than the Vulkan lane")
        return "submission fragmentation", "PIX", reasons
    if native_interop and native_interop["dx12_p95"] and native_interop["dx12_p95"] > 0:
        reasons.append("DX12 native interop command insertion was observed; inspect fence and queue overlap")
        return "CEF/native interop sync", "PIX plus CEF counters", reasons

    shader_module_creates = row_by_metric(rows, "render_shader_shader_module_creations")
    shader_module_ns = row_by_metric(rows, "render_shader_shader_module_create_ns")
    pipeline_create_count = row_by_metric(rows, "render_shader_pipeline_create_count")
    pipeline_create_ns = row_by_metric(rows, "render_shader_pipeline_create_ns")
    shader_variants = row_by_metric(rows, "render_shader_shader_variant_requests")
    material_specializations = row_by_metric(rows, "render_shader_material_specializations")
    if shader_module_creates and shader_module_creates["dx12_p95"] and shader_module_creates["dx12_p95"] > 0:
        reasons.append("DX12 shader module creation was observed in the sample window")
        return "shader compilation", "PIX plus vendor shader tools", reasons
    if shader_module_ns and shader_module_ns["dx12_p95"] and shader_module_ns["dx12_p95"] > 0:
        reasons.append("DX12 shader module creation time was observed in the sample window")
        return "shader compilation", "PIX plus vendor shader tools", reasons
    if pipeline_create_count and pipeline_create_count["dx12_p95"] and pipeline_create_count["dx12_p95"] > 0:
        reasons.append("DX12 pipeline creation was observed in the sample window")
        return "shader compilation", "PIX plus vendor shader tools", reasons
    if pipeline_create_ns and pipeline_create_ns["dx12_p95"] and pipeline_create_ns["dx12_p95"] > 0:
        reasons.append("DX12 pipeline creation time was observed in the sample window")
        return "shader compilation", "PIX plus vendor shader tools", reasons
    if shader_variants and shader_variants["p95_delta"] is not None and shader_variants["p95_delta"] > 0:
        reasons.append("DX12 shader variant requests exceeded the Vulkan lane")
        return "shader variant pressure", "benchmark shader top events", reasons
    if material_specializations and material_specializations["p95_delta"] is not None and material_specializations["p95_delta"] > 0:
        reasons.append("DX12 material specialization count exceeded the Vulkan lane")
        return "shader variant pressure", "benchmark shader top events", reasons

    transient_texture_creates = row_by_metric(rows, "transient_texture_creates")
    transient_buffer_creates = row_by_metric(rows, "transient_buffer_creates")
    transient_texture_misses = row_by_metric(rows, "transient_texture_descriptor_miss_creates")
    transient_texture_every_frame = row_by_metric(rows, "transient_texture_every_frame_create_descriptors")
    if transient_texture_creates and transient_texture_creates["p95_delta"] is not None and transient_texture_creates["p95_delta"] > 0:
        reasons.append("DX12 transient texture creation exceeded the Vulkan lane")
        return "transient allocation churn", "PIX", reasons
    if transient_buffer_creates and transient_buffer_creates["p95_delta"] is not None and transient_buffer_creates["p95_delta"] > 0:
        reasons.append("DX12 transient buffer creation exceeded the Vulkan lane")
        return "transient allocation churn", "PIX", reasons
    if transient_texture_misses and transient_texture_misses["dx12_p95"] and transient_texture_misses["dx12_p95"] > 0:
        reasons.append("DX12 transient texture descriptor misses were observed")
        return "transient allocation churn", "PIX", reasons
    if (
        transient_texture_every_frame
        and transient_texture_every_frame["dx12_p95"]
        and transient_texture_every_frame["dx12_p95"] > 0
    ):
        reasons.append("DX12 transient texture descriptors are being created every frame")
        return "transient allocation churn", "PIX", reasons

    if pix.get("barrier_count", 0.0) > 0 or pix.get("resource_barrier_count", 0.0) > 0:
        reasons.append("PIX summary includes barrier counters")
        return "barrier/state-bound", "PIX", reasons
    if pix.get("pipeline_creation_count", 0.0) > 0 or pix.get("pso_creation_count", 0.0) > 0:
        reasons.append("PIX summary includes pipeline creation counters")
        return "pipeline churn", "PIX", reasons
    if pix.get("descriptor_heap_switch_count", 0.0) > 0:
        reasons.append("PIX summary includes descriptor heap switch counters")
        return "descriptor churn", "PIX", reasons

    cpu_regressions = [
        row
        for row in rows
        if row["metric"] in CPU_WORK_METRICS and row["p95_delta"] is not None and row["p95_delta"] > 200_000
    ]
    if cpu_regressions:
        reasons.append(f"CPU-side metric regressed: {cpu_regressions[0]['metric']}")
        return "upload-bound", "PIX", reasons

    gpu_regressions = [
        row
        for row in rows
        if row["metric"] in GPU_PASS_METRICS and row["p95_delta"] is not None and row["p95_delta"] > 200_000
    ]
    if gpu_regressions:
        reasons.append(f"GPU pass metric regressed: {gpu_regressions[0]['metric']}")
        return "shader/pass GPU bound", "PIX or RenderDoc", reasons

    if not dx_loses_avg and p95_regression:
        reasons.append("DX12 wins average FPS but loses frame p95")
        return "p95 regression", "PresentMon plus PIX", reasons
    if dx_loses_avg:
        reasons.append("DX12 loses average FPS without a dominant parsed metric")
        return "unknown, requires PIX", "PIX", reasons

    reasons.append("no obvious DX12 loss in parsed metrics")
    return "no clear bottleneck", "PresentMon smoke trace", reasons


def metric_has_any_stat(summary: dict[str, Any], metric: str) -> bool:
    metric_data = get_path(summary, "metrics", metric)
    if not isinstance(metric_data, dict):
        return False
    return any(number(metric_data.get(field)) is not None for field in ("p95", "mean"))


def metric_best_stat(summary: dict[str, Any], metric: str) -> float | None:
    for field in ("p95", "mean"):
        value = metric_value(summary, metric, field)
        if value is not None:
            return value
    return None


def first_metric_evidence(summary: dict[str, Any], metrics: tuple[str, ...]) -> tuple[str, float] | None:
    for metric in metrics:
        value = metric_best_stat(summary, metric)
        if value is not None and value > 0.0:
            return metric, value
    return None


def first_regressed_row(
    rows: list[dict[str, Any]],
    metrics: tuple[str, ...],
    minimum_delta: float = 0.0,
) -> tuple[str, float] | None:
    for metric in metrics:
        row = row_by_metric(rows, metric)
        if not row:
            continue
        p95_delta = number(row.get("p95_delta"))
        mean_delta = number(row.get("delta"))
        if p95_delta is not None and p95_delta > minimum_delta:
            return metric, p95_delta
        if mean_delta is not None and mean_delta > minimum_delta:
            return metric, mean_delta
    return None


def make_recommendation(
    classification: str,
    next_action: str,
    required_trace: str,
    confidence: str,
    evidence: list[str],
    missing_evidence: list[str] | None = None,
) -> dict[str, Any]:
    if classification not in RECOMMENDATION_CLASSIFICATIONS:
        raise ValueError(f"unknown DX12 recommendation classification: {classification}")
    return {
        "dx12_classification": classification,
        "next_action": next_action,
        "required_trace": required_trace,
        "confidence": confidence,
        "evidence": evidence,
        "missing_evidence": missing_evidence or [],
    }


def build_next_action_recommendation(
    rows: list[dict[str, Any]],
    vulkan: dict[str, Any],
    dx12: dict[str, Any],
    pix: dict[str, float],
    presentmon: dict[str, float],
    failures: list[str],
) -> dict[str, Any]:
    missing_evidence: list[str] = []
    dx_fps = metric_value(dx12, "fps", "mean")
    vk_fps = metric_value(vulkan, "fps", "mean")
    dx_frame_p95 = metric_value(dx12, "frame_ns", "p95")
    vk_frame_p95 = metric_value(vulkan, "frame_ns", "p95")
    dx_loses_avg = dx_fps is not None and vk_fps is not None and dx_fps < vk_fps
    p95_regression = (
        dx_frame_p95 is not None
        and vk_frame_p95 is not None
        and dx_frame_p95 > vk_frame_p95 * 1.01
    )

    cef_cpu_upload = metric_value(dx12, "cef_cpu_upload_bytes", "mean") or 0.0
    cef_cpu_upload_p95 = metric_value(dx12, "cef_cpu_upload_bytes", "p95") or cef_cpu_upload
    cef_fallback = metric_best_stat(dx12, "cef_transport_fallback_count") or 0.0
    if failures or (is_accelerated_cef_lane(dx12) and (cef_cpu_upload > 0.0 or cef_fallback > 0.0)):
        evidence = failures.copy()
        if cef_cpu_upload > 0.0:
            evidence.append(f"cef_cpu_upload_bytes.mean={cef_cpu_upload:g} in accelerated CEF lane")
        if cef_fallback > 0.0:
            evidence.append(f"cef_transport_fallback_count={cef_fallback:g}")
        return make_recommendation(
            "cef_transport_bound",
            "fix CEF accelerated D3D11On12 transport or convert the CEF texture upload path before treating this lane as GPU accelerated",
            "cef_transport_health",
            "high",
            evidence,
        )
    if max(cef_cpu_upload, cef_cpu_upload_p95) >= CPU_UPLOAD_BYTE_THRESHOLD:
        return make_recommendation(
            "cef_transport_bound",
            "fix CEF transport or convert the CEF texture upload path to remove full-frame CPU uploads",
            "cef_transport_health",
            "medium",
            [f"cef_cpu_upload_bytes={max(cef_cpu_upload, cef_cpu_upload_p95):g}"],
        )

    present_wait_p95 = metric_value(dx12, "present_wait_ns", "p95")
    present_row = row_by_metric(rows, "present_wait_ns")
    present_delta = number(present_row.get("p95_delta")) if present_row else None
    if present_wait_p95 is not None:
        frame_bound = (
            dx_frame_p95 is not None
            and present_wait_p95 >= max(PRESENT_WAIT_REGRESSION_NS, dx_frame_p95 * PRESENT_WAIT_DOMINANCE_RATIO)
        )
        delta_bound = present_delta is not None and present_delta > PRESENT_WAIT_REGRESSION_NS
        if frame_bound or delta_bound:
            evidence = [f"present_wait_ns.p95={present_wait_p95:g}"]
            if dx_frame_p95 is not None:
                evidence.append(f"frame_ns.p95={dx_frame_p95:g}")
            if present_delta is not None:
                evidence.append(f"present_wait_ns.p95_delta={present_delta:g}")
            return make_recommendation(
                "present_bound",
                "run the present matrix and tune present mode plus maximum frame latency before changing renderer internals",
                "present_matrix_presentmon",
                "high" if frame_bound else "medium",
                evidence,
            )
    elif not presentmon:
        missing_evidence.append("present_wait_ns or PresentMon CSV")

    runtime_pipeline = first_metric_evidence(dx12, RUNTIME_PIPELINE_CREATION_METRICS)
    if runtime_pipeline:
        metric, value = runtime_pipeline
        return make_recommendation(
            "runtime_pipeline_creation_bound",
            "add pipeline warmup and reduce pipeline-key fragmentation for runtime-created pipeline families",
            "pipeline_churn_top_events",
            "high",
            [f"{metric}={value:g}"],
        )

    readback_blocking = metric_best_stat(dx12, "render_readback_readback_blocking_wait_count") or 0.0
    if readback_blocking > 0.0:
        return make_recommendation(
            "readback_or_fence_bound",
            "defer readbacks or remove blocking waits from the sampled lane",
            "readback_fence_trace",
            "high",
            [f"render_readback_readback_blocking_wait_count={readback_blocking:g}"],
        )
    readback_regression = first_regressed_row(rows, READBACK_OR_FENCE_METRICS, 0.0)
    if readback_regression:
        metric, delta = readback_regression
        return make_recommendation(
            "readback_or_fence_bound",
            "defer readbacks, widen readback rings, or remove fence waits from the hot frame",
            "readback_fence_trace",
            "medium",
            [f"{metric}.delta={delta:g}"],
        )

    texture_upload = max(
        metric_best_stat(dx12, "render_upload_write_texture_bytes") or 0.0,
        metric_value(dx12, "render_upload_write_texture_bytes", "mean") or 0.0,
    )
    upload_regression = first_regressed_row(rows, UPLOAD_CALLSITE_METRICS, CPU_UPLOAD_BYTE_THRESHOLD)
    if texture_upload >= CPU_UPLOAD_BYTE_THRESHOLD or upload_regression:
        evidence = []
        if texture_upload >= CPU_UPLOAD_BYTE_THRESHOLD:
            evidence.append(f"render_upload_write_texture_bytes={texture_upload:g}")
        if upload_regression:
            evidence.append(f"{upload_regression[0]}.delta={upload_regression[1]:g}")
        return make_recommendation(
            "cpu_upload_bound",
            "convert top write_texture callsite or force CEF accelerated lane",
            "upload_callsite_table",
            "medium",
            evidence,
        )

    meshlet_or_stream_metric = first_regressed_row(rows, MESHLET_OR_STREAM_UPLOAD_METRICS, 0.0)
    if not meshlet_or_stream_metric:
        meshlet_or_stream_metric = first_metric_evidence(dx12, MESHLET_OR_STREAM_UPLOAD_METRICS)
    if meshlet_or_stream_metric:
        metric, value = meshlet_or_stream_metric
        return make_recommendation(
            "meshlet_or_stream_upload_bound",
            "move meshlet and world-stream uploads to pooled staging or off the hot frame",
            "upload_callsite_table",
            "medium",
            [f"{metric}={value:g}"],
        )

    descriptor_or_pso_metric = first_regressed_row(rows, DESCRIPTOR_OR_PSO_CHURN_METRICS, 0.0)
    if not descriptor_or_pso_metric:
        descriptor_or_pso_metric = first_metric_evidence(dx12, DESCRIPTOR_OR_PSO_CHURN_METRICS)
    if descriptor_or_pso_metric:
        metric, value = descriptor_or_pso_metric
        return make_recommendation(
            "descriptor_or_pso_churn_bound",
            "inspect bind group layout and PSO churn, then canonicalize layouts or prewarm pipelines",
            "pipeline_churn_top_events",
            "medium",
            [f"{metric}={value:g}"],
        )

    if pix.get("barrier_count", 0.0) > 0 or pix.get("resource_barrier_count", 0.0) > 0:
        return make_recommendation(
            "barrier_or_state_bound",
            "inspect PIX barrier and resource-state rows, then collapse redundant transitions in the named pass",
            "pix_barrier_summary",
            "medium",
            [
                f"barrier_count={pix.get('barrier_count', 0.0):g}",
                f"resource_barrier_count={pix.get('resource_barrier_count', 0.0):g}",
            ],
        )

    gpu_pass_regression = first_regressed_row(rows, tuple(GPU_PASS_METRICS), MATERIAL_TIME_REGRESSION_NS)
    if gpu_pass_regression:
        metric, delta = gpu_pass_regression
        return make_recommendation(
            "shader_or_pass_gpu_bound",
            "profile the regressed shader or pass with PIX or RenderDoc and compare DX12 shader variants",
            "pix_or_renderdoc_pass_trace",
            "medium",
            [f"{metric}.p95_delta={delta:g}"],
        )

    if not any(metric_has_any_stat(dx12, metric) for metric in UPLOAD_CALLSITE_METRICS):
        missing_evidence.append("upload_callsite_table")

    if dx_loses_avg or p95_regression:
        if present_wait_p95 is None and not presentmon:
            return make_recommendation(
                "inconclusive_needs_presentmon",
                "capture PresentMon or present_wait_ns before attributing the DX12 loss to renderer work",
                "presentmon_csv",
                "low",
                ["DX12 frame metrics regress without present wait evidence"],
                missing_evidence,
            )
        if not any(metric_has_any_stat(dx12, metric) for metric in UPLOAD_CALLSITE_METRICS):
            return make_recommendation(
                "inconclusive_needs_upload_callsite_table",
                "capture upload callsite counters so the report can separate CEF, meshlet, streaming, and generic uploads",
                "upload_callsite_table",
                "low",
                ["DX12 regresses without upload callsite evidence"],
                missing_evidence,
            )
        return make_recommendation(
            "inconclusive_needs_pix",
            "capture PIX with pass markers because parsed metrics do not isolate the DX12 loss",
            "pix_capture",
            "low",
            ["DX12 regresses without a dominant parsed bottleneck"],
            missing_evidence,
        )

    if pix.get("pipeline_creation_count", 0.0) > 0 or pix.get("pso_creation_count", 0.0) > 0:
        return make_recommendation(
            "runtime_pipeline_creation_bound",
            "add pipeline warmup and reduce pipeline-key fragmentation for runtime-created pipeline families",
            "pipeline_churn_top_events",
            "medium",
            [
                f"pipeline_creation_count={pix.get('pipeline_creation_count', 0.0):g}",
                f"pso_creation_count={pix.get('pso_creation_count', 0.0):g}",
            ],
        )
    if pix.get("descriptor_heap_switch_count", 0.0) > 0:
        return make_recommendation(
            "descriptor_or_pso_churn_bound",
            "inspect descriptor heap switches and canonicalize hot bind group or pipeline layouts",
            "pix_descriptor_summary",
            "medium",
            [f"descriptor_heap_switch_count={pix.get('descriptor_heap_switch_count', 0.0):g}"],
        )

    missing = missing_evidence or ["PIX capture with pass, barrier, descriptor, and queue markers"]
    return make_recommendation(
        "inconclusive_needs_pix",
        "capture PIX with FUN render markers before choosing an optimization lane",
        "pix_capture",
        "low",
        ["parsed metrics do not show a DX12-specific loss"],
        missing,
    )


def lane_failure_messages(dx12: dict[str, Any]) -> list[str]:
    failures: list[str] = []
    cef_cpu_upload = metric_value(dx12, "cef_cpu_upload_bytes", "mean") or 0.0
    if is_accelerated_cef_lane(dx12) and cef_cpu_upload > 0.0:
        failures.append(
            "CEF accelerated lane failed: cef_cpu_upload_bytes.mean is nonzero "
            f"({cef_cpu_upload:g})"
        )
    return failures


def format_number(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    if abs(parsed) >= 1000:
        return f"{parsed:.0f}"
    return f"{parsed:.3f}".rstrip("0").rstrip(".")


def format_bytes(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    units = ("B", "KiB", "MiB", "GiB")
    unit_index = 0
    while abs(parsed) >= 1024.0 and unit_index < len(units) - 1:
        parsed /= 1024.0
        unit_index += 1
    return f"{parsed:.2f} {units[unit_index]}"


def format_pct(value: Any) -> str:
    parsed = number(value)
    if parsed is None:
        return "n/a"
    return f"{parsed:+.2f}%"


def write_csv(path: Path, rows: list[dict[str, Any]]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fields = [
        "metric",
        "label",
        "status",
        "reason",
        "vulkan_mean",
        "dx12_mean",
        "delta",
        "delta_pct",
        "vulkan_p95",
        "dx12_p95",
        "p95_delta",
        "p95_delta_pct",
    ]
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fields)
        writer.writeheader()
        for row in rows:
            writer.writerow({field: row.get(field) for field in fields})


def write_json_report(
    path: Path,
    recommendation: dict[str, Any],
    bottleneck: tuple[str, str, list[str]],
    failures: list[str],
    rows: list[dict[str, Any]],
    top_upload_callsites: list[dict[str, Any]],
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    category, trace, reasons = bottleneck
    payload = {
        "schema": "fun.dx12_parity_recommendation.v1",
        "dx12_classification": recommendation["dx12_classification"],
        "next_action": recommendation["next_action"],
        "required_trace": recommendation["required_trace"],
        "confidence": recommendation["confidence"],
        "evidence": recommendation.get("evidence", []),
        "missing_evidence": recommendation.get("missing_evidence", []),
        "legacy_bottleneck_category": category,
        "legacy_required_trace": trace,
        "legacy_evidence": reasons,
        "lane_failures": failures,
        "top_upload_callsites": top_upload_callsites,
        "red_metrics": [
            row["metric"]
            for row in rows
            if row.get("status") == "red" and (row.get("vulkan_mean") is not None or row.get("dx12_mean") is not None)
        ],
    }
    path.write_text(json.dumps(payload, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def write_markdown(
    path: Path,
    vulkan: dict[str, Any],
    dx12: dict[str, Any],
    rows: list[dict[str, Any]],
    bottleneck: tuple[str, str, list[str]],
    recommendation: dict[str, Any],
    top_upload_callsites: list[dict[str, Any]],
    failures: list[str],
    pix: dict[str, float],
    presentmon: dict[str, float],
) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    vk_summary = backend_summary(vulkan)
    dx_summary = backend_summary(dx12)
    vk_memory = memory_snapshot(vulkan)
    dx_memory = memory_snapshot(dx12, pix)
    category, trace, reasons = bottleneck
    p95_row = row_by_metric(rows, "frame_ns")
    fps_row = row_by_metric(rows, "fps")
    p95_regression = (
        fps_row is not None
        and p95_row is not None
        and (fps_row["delta"] or 0) > 0
        and (p95_row["p95_delta"] or 0) > 0
    )

    lines = [
        "# DX12 Parity Dashboard",
        "",
        "## Backend Summary",
        "",
        "| field | Vulkan | DX12 |",
        "|---|---|---|",
    ]
    for key in (
        "backend",
        "adapter",
        "driver",
        "present_mode",
        "surface_present_mode",
        "max_frame_latency",
        "resolution",
        "cef_transport",
        "cef_mode",
    ):
        lines.append(f"| {key} | {vk_summary[key]} | {dx_summary[key]} |")

    lines.extend(
        [
            "",
            "## DX12 vs Vulkan Deltas",
            "",
            "| status | metric | mean delta | mean delta % | p95 delta | p95 delta % | reason |",
            "|---|---|---:|---:|---:|---:|---|",
        ]
    )
    for row in rows:
        if row["vulkan_mean"] is None and row["dx12_mean"] is None:
            continue
        lines.append(
            "| {status} | {metric} | {delta} | {delta_pct} | {p95_delta} | {p95_delta_pct} | {reason} |".format(
                status=row["status"],
                metric=row["metric"],
                delta=format_number(row["delta"]),
                delta_pct=format_pct(row["delta_pct"]),
                p95_delta=format_number(row["p95_delta"]),
                p95_delta_pct=format_pct(row["p95_delta_pct"]),
                reason=row["reason"],
            )
        )

    lines.extend(
        [
            "",
            "## Next Action Recommendation",
            "",
            f"- dx12_classification: `{recommendation['dx12_classification']}`",
            f"- next_action: {recommendation['next_action']}",
            f"- required_trace: `{recommendation['required_trace']}`",
            f"- confidence: `{recommendation['confidence']}`",
        ]
    )
    for evidence in recommendation.get("evidence", []):
        lines.append(f"- evidence: {evidence}")
    missing_evidence = recommendation.get("missing_evidence", [])
    if missing_evidence:
        for missing in missing_evidence:
            lines.append(f"- missing_evidence: {missing}")

    lines.extend(["", "## Top Upload Callsites", ""])
    if top_upload_callsites:
        lines.extend(
            [
                "| rank | callsite | backend | calls/frame | bytes/frame | calls/sec | bytes/sec | p95 impact guess | fix |",
                "|---:|---|---|---:|---:|---:|---:|---|---|",
            ]
        )
        for row in top_upload_callsites:
            lines.append(
                "| {rank} | {callsite} | {backend} | {calls_per_frame} | {bytes_per_frame} | {calls_per_second} | {bytes_per_second} | {impact} | {fix} |".format(
                    rank=row["rank"],
                    callsite=row["callsite"],
                    backend=row["backend"],
                    calls_per_frame=format_number(row["calls_per_frame"]),
                    bytes_per_frame=format_bytes(row["bytes_per_frame"]),
                    calls_per_second=format_number(row["calls_per_second"]),
                    bytes_per_second=format_bytes(row["bytes_per_second"]),
                    impact=row["p95_impact_guess"],
                    fix=row["fix"],
                )
            )
    else:
        lines.append("- No `render_upload_callsites` table was present; rerun with render upload counters enabled.")

    lines.extend(
        [
            "",
            "## Bottleneck Guess",
            "",
            f"- Category: {category}",
            f"- Required next trace: {trace}",
        ]
    )
    if p95_regression:
        lines.append("- P95 regression: DX12 wins average FPS but loses frame p95.")
    if failures:
        lines.append(f"- Lane failure: {'; '.join(failures)}")
    for reason in reasons:
        lines.append(f"- Evidence: {reason}")

    follow_up = vendor_follow_up(category, dx12)
    lines.extend(
        [
            "",
            "## Vendor-Specific Follow-Up",
            "",
            f"- Adapter vendor: {follow_up['vendor']}",
            f"- Status: {follow_up['status']}",
            f"- Required evidence: {follow_up['required_evidence']}",
            f"- Guardrail: {follow_up['guardrail']}",
        ]
    )
    actions = follow_up["actions"]
    if actions:
        lines.append("- NVIDIA experiments:")
        for action in actions:
            lines.append(f"  - {action}")
    else:
        lines.append("- NVIDIA experiments: none until the adapter and bottleneck evidence match the gate.")

    lines.extend(
        [
            "",
            "## DX12 Memory Budget",
            "",
            "| field | Vulkan | DX12 |",
            "|---|---:|---:|",
            f"| status | {vk_memory['status']} | {dx_memory['status']} |",
            f"| source | {vk_memory['source']} | {dx_memory['source']} |",
            f"| local_budget_bytes | {format_bytes(vk_memory['local_budget_bytes'])} | {format_bytes(dx_memory['local_budget_bytes'])} |",
            f"| local_usage_bytes | {format_bytes(vk_memory['local_usage_bytes'])} | {format_bytes(dx_memory['local_usage_bytes'])} |",
            f"| local_usage_pct | {format_pct(vk_memory['local_usage_pct'])} | {format_pct(dx_memory['local_usage_pct'])} |",
            f"| local_available_for_reservation_bytes | {format_bytes(vk_memory['local_available_for_reservation_bytes'])} | {format_bytes(dx_memory['local_available_for_reservation_bytes'])} |",
            f"| local_current_reservation_bytes | {format_bytes(vk_memory['local_current_reservation_bytes'])} | {format_bytes(dx_memory['local_current_reservation_bytes'])} |",
            f"| adapter_ram_bytes | {format_bytes(vk_memory['adapter_ram_bytes'])} | {format_bytes(dx_memory['adapter_ram_bytes'])} |",
        ]
    )

    moonshots = moonshot_follow_up(category, rows, dx12, pix, dx_memory)
    lines.extend(
        [
            "",
            "## Moonshot Experiments",
            "",
            "| experiment | status | gate | evidence | payoff |",
            "|---|---|---|---|---|",
        ]
    )
    for experiment in moonshots:
        lines.append(
            "| {title} | {status} | {gate} | {evidence} | {payoff} |".format(
                title=experiment["title"],
                status=experiment["status"],
                gate=experiment["gate"],
                evidence=experiment["evidence"],
                payoff=experiment["payoff"],
            )
        )

    lines.extend(["", "## External Trace Summary", ""])
    if pix:
        lines.append(f"- PIX fields parsed: {', '.join(sorted(pix)[:12])}")
    else:
        lines.append("- PIX fields parsed: none")
    if presentmon:
        lines.append(f"- PresentMon fields parsed: {', '.join(sorted(presentmon)[:12])}")
    else:
        lines.append("- PresentMon fields parsed: none")

    lines.extend(
        [
            "",
            "## Verdict",
            "",
            f"- Lane status: {'fail' if failures else 'pass'}",
            "- Red/yellow/green thresholds: red >5 percent regression, yellow >1 percent regression, green otherwise.",
        ]
    )

    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def run_report(args: argparse.Namespace) -> int:
    vulkan = load_json(Path(args.vulkan_json))
    dx12 = load_json(Path(args.dx12_json))
    rows = build_delta_rows(vulkan, dx12)
    pix = parse_external_csv(Path(args.pix_csv)) if args.pix_csv else {}
    presentmon = parse_external_csv(Path(args.presentmon_csv)) if args.presentmon_csv else {}
    failures = lane_failure_messages(dx12)
    bottleneck = likely_bottleneck(rows, vulkan, dx12, pix, presentmon)
    recommendation = build_next_action_recommendation(rows, vulkan, dx12, pix, presentmon, failures)
    top_upload_callsites = build_upload_callsite_kill_list(dx12)
    write_csv(Path(args.csv_summary), rows)
    json_report = getattr(args, "json_report", "")
    if json_report:
        write_json_report(Path(json_report), recommendation, bottleneck, failures, rows, top_upload_callsites)
    write_markdown(
        Path(args.markdown_report),
        vulkan,
        dx12,
        rows,
        bottleneck,
        recommendation,
        top_upload_callsites,
        failures,
        pix,
        presentmon,
    )
    return 2 if failures else 0


def write_sample_summary(
    path: Path,
    backend: str,
    fps: float,
    frame_p95: float,
    cef_cpu_bytes: float,
    gpu_name: str = "Synthetic GPU",
    extra_metrics: dict[str, dict[str, float]] | None = None,
    render_upload_callsites: list[dict[str, Any]] | None = None,
    memory_budget: float | None = None,
    memory_usage: float | None = None,
) -> None:
    payload = {
        "hardware": {"gpu": [{"Name": gpu_name, "DriverVersion": "0.0", "AdapterRAM": 8_589_934_592}]},
        "config": {
            "render_backend": backend,
            "present_mode": "immediate",
            "requested_maximum_frame_latency": 2,
            "window_width": 1280,
            "window_height": 720,
            "cef_ui_mode": "animated",
            "cef_paint_transport": "d3d11on12" if backend == "dx12" else "cpu",
            "cef_accelerated_feature_requested": backend == "dx12",
        },
        "metrics": {
            "fps": {"mean": fps, "p95": fps},
            "frame_ns": {"mean": 1_000_000_000.0 / fps, "p95": frame_p95},
            "present_wait_ns": {"mean": 100_000.0, "p95": 100_000.0},
            "cef_cpu_upload_bytes": {"mean": cef_cpu_bytes, "p95": cef_cpu_bytes},
        },
        "samples": {
            "count": 4,
            "warmup_seconds": 0,
            "sample_seconds": 8,
            "input_log_mode": False,
        },
    }
    if memory_budget is not None or memory_usage is not None:
        payload["dx12_memory"] = {
            "status": "provided",
            "source": "self_test",
            "local_budget_bytes": memory_budget,
            "local_usage_bytes": memory_usage,
            "local_available_for_reservation_bytes": None,
            "local_current_reservation_bytes": None,
            "adapter_ram_bytes": 8_589_934_592,
        }
    if extra_metrics:
        payload["metrics"].update(extra_metrics)
    if render_upload_callsites is not None:
        payload["render_upload_callsites"] = render_upload_callsites
    path.write_text(json.dumps(payload), encoding="utf-8")


def self_test() -> int:
    with tempfile.TemporaryDirectory() as temp_dir:
        root = Path(temp_dir)
        vulkan = root / "vulkan.json"
        dx12 = root / "dx12.json"
        markdown = root / "report.md"
        summary_csv = root / "summary.csv"
        recommendation_json = root / "recommendation.json"
        write_sample_summary(vulkan, "vulkan", fps=100.0, frame_p95=10_000_000.0, cef_cpu_bytes=0.0)
        write_sample_summary(
            dx12,
            "dx12",
            fps=105.0,
            frame_p95=15_000_000.0,
            cef_cpu_bytes=4096.0,
            memory_budget=8_589_934_592.0,
            memory_usage=4_294_967_296.0,
        )
        code = run_report(
            argparse.Namespace(
                vulkan_json=str(vulkan),
                dx12_json=str(dx12),
                markdown_report=str(markdown),
                csv_summary=str(summary_csv),
                json_report=str(recommendation_json),
                pix_csv="",
                presentmon_csv="",
            )
        )
        text = markdown.read_text(encoding="utf-8")
        recommendation = json.loads(recommendation_json.read_text(encoding="utf-8"))
        if code != 2:
            raise AssertionError("accelerated CEF CPU upload failure did not produce exit code 2")
        if "P95 regression" not in text:
            raise AssertionError("p95 regression was not reported")
        if "CEF accelerated lane failed" not in text:
            raise AssertionError("CEF accelerated lane failure was not reported")
        if recommendation["dx12_classification"] != "cef_transport_bound":
            raise AssertionError("CEF CPU upload did not produce a CEF transport recommendation")
        if "CEF" not in recommendation["next_action"]:
            raise AssertionError("CEF CPU upload recommendation did not mention CEF transport work")
        if "Vendor-Specific Follow-Up" not in text:
            raise AssertionError("vendor follow-up section was not rendered")
        if "DX12 Memory Budget" not in text or "4.00 GiB" not in text:
            raise AssertionError("DX12 memory budget section was not rendered")
        if "Moonshot Experiments" not in text:
            raise AssertionError("moonshot section was not rendered")

        nvidia_dx12 = root / "dx12_nvidia_pipeline.json"
        nvidia_markdown = root / "nvidia_report.md"
        nvidia_json = root / "nvidia_report.json"
        write_sample_summary(
            nvidia_dx12,
            "dx12",
            fps=90.0,
            frame_p95=16_000_000.0,
            cef_cpu_bytes=0.0,
            gpu_name="NVIDIA GeForce RTX Synthetic",
            extra_metrics={
                "render_churn_render_pipeline_creations": {"mean": 1.0, "p95": 1.0},
            },
        )
        code = run_report(
            argparse.Namespace(
                vulkan_json=str(vulkan),
                dx12_json=str(nvidia_dx12),
                markdown_report=str(nvidia_markdown),
                csv_summary=str(summary_csv),
                json_report=str(nvidia_json),
                pix_csv="",
                presentmon_csv="",
            )
        )
        text = nvidia_markdown.read_text(encoding="utf-8")
        recommendation = json.loads(nvidia_json.read_text(encoding="utf-8"))
        if code != 0:
            raise AssertionError("NVIDIA pipeline follow-up scenario should not fail the lane")
        if recommendation["dx12_classification"] != "runtime_pipeline_creation_bound":
            raise AssertionError("runtime pipeline creation did not produce the pipeline recommendation")
        if "pipeline warmup" not in recommendation["next_action"]:
            raise AssertionError("pipeline recommendation did not mention warmup")
        if "Status: eligible_optional_nvidia_experiment" not in text:
            raise AssertionError("NVIDIA follow-up gate was not marked eligible")
        if "persistent PSO cache" not in text:
            raise AssertionError("NVIDIA PSO follow-up action was not reported")

        present_dx12 = root / "dx12_present.json"
        present_json = root / "present_report.json"
        write_sample_summary(
            present_dx12,
            "dx12",
            fps=90.0,
            frame_p95=16_000_000.0,
            cef_cpu_bytes=0.0,
            extra_metrics={
                "present_wait_ns": {"mean": 6_000_000.0, "p95": 8_000_000.0},
                "render_upload_write_texture_bytes": {"mean": 0.0, "p95": 0.0},
            },
        )
        code = run_report(
            argparse.Namespace(
                vulkan_json=str(vulkan),
                dx12_json=str(present_dx12),
                markdown_report=str(root / "present_report.md"),
                csv_summary=str(summary_csv),
                json_report=str(present_json),
                pix_csv="",
                presentmon_csv="",
            )
        )
        recommendation = json.loads(present_json.read_text(encoding="utf-8"))
        if code != 0:
            raise AssertionError("present-bound scenario should not fail the lane")
        if recommendation["dx12_classification"] != "present_bound":
            raise AssertionError("dominant present_wait_ns did not produce a present-bound recommendation")
        if "present matrix" not in recommendation["next_action"]:
            raise AssertionError("present-bound recommendation did not mention the present matrix")

        upload_dx12 = root / "dx12_upload.json"
        upload_json = root / "upload_report.json"
        write_sample_summary(
            upload_dx12,
            "dx12",
            fps=92.0,
            frame_p95=13_000_000.0,
            cef_cpu_bytes=0.0,
            extra_metrics={
                "render_upload_write_texture_bytes": {"mean": 4_194_304.0, "p95": 4_194_304.0},
            },
            render_upload_callsites=[
                {
                    "rank": 1,
                    "operation": "write_texture",
                    "label": "cef_ui.cpu_paint.full_frame@game_client\\src\\cef_ui.rs:3539",
                    "calls": 16,
                    "bytes": 67_108_864,
                    "samples": 4,
                },
                {
                    "rank": 2,
                    "operation": "write_buffer",
                    "label": "bevy\\crates\\bevy_render\\src\\render_resource\\buffer_vec.rs:183",
                    "calls": 400,
                    "bytes": 262_144,
                    "samples": 4,
                },
                {
                    "rank": 3,
                    "operation": "write_buffer",
                    "label": "bevy\\crates\\bevy_render\\src\\render_resource\\buffer_vec.rs:183",
                    "calls": 100,
                    "bytes": 131_072,
                    "samples": 1,
                },
            ],
        )
        code = run_report(
            argparse.Namespace(
                vulkan_json=str(vulkan),
                dx12_json=str(upload_dx12),
                markdown_report=str(root / "upload_report.md"),
                csv_summary=str(summary_csv),
                json_report=str(upload_json),
                pix_csv="",
                presentmon_csv="",
            )
        )
        upload_markdown = (root / "upload_report.md").read_text(encoding="utf-8")
        recommendation = json.loads(upload_json.read_text(encoding="utf-8"))
        if code != 0:
            raise AssertionError("generic CPU upload scenario should not fail the lane")
        if recommendation["dx12_classification"] != "cpu_upload_bound":
            raise AssertionError("high write_texture bytes did not produce the CPU upload recommendation")
        if recommendation["required_trace"] != "upload_callsite_table":
            raise AssertionError("CPU upload recommendation did not require the upload callsite table")
        if "## Top Upload Callsites" not in upload_markdown:
            raise AssertionError("top upload callsite table was not rendered")
        if not recommendation["top_upload_callsites"]:
            raise AssertionError("top upload callsites were not written to JSON")
        if recommendation["top_upload_callsites"][0]["fix"] != "accelerated CEF or persistent texture upload ring":
            raise AssertionError("CEF upload callsite did not get the CEF transport fix")
        if recommendation["top_upload_callsites"][1]["calls"] != 500:
            raise AssertionError("duplicate upload callsite labels were not merged")

        inconclusive_dx12 = root / "dx12_missing_present.json"
        inconclusive_json = root / "missing_present_report.json"
        write_sample_summary(
            inconclusive_dx12,
            "dx12",
            fps=80.0,
            frame_p95=14_000_000.0,
            cef_cpu_bytes=0.0,
        )
        inconclusive_payload = json.loads(inconclusive_dx12.read_text(encoding="utf-8"))
        inconclusive_payload["metrics"].pop("present_wait_ns", None)
        inconclusive_payload["metrics"].pop("cef_cpu_upload_bytes", None)
        inconclusive_dx12.write_text(json.dumps(inconclusive_payload), encoding="utf-8")
        code = run_report(
            argparse.Namespace(
                vulkan_json=str(vulkan),
                dx12_json=str(inconclusive_dx12),
                markdown_report=str(root / "missing_present_report.md"),
                csv_summary=str(summary_csv),
                json_report=str(inconclusive_json),
                pix_csv="",
                presentmon_csv="",
            )
        )
        recommendation = json.loads(inconclusive_json.read_text(encoding="utf-8"))
        if code != 0:
            raise AssertionError("missing-present inconclusive scenario should not fail the lane")
        if recommendation["dx12_classification"] != "inconclusive_needs_presentmon":
            raise AssertionError("missing present data did not ask for PresentMon evidence")
        if not recommendation["missing_evidence"]:
            raise AssertionError("inconclusive recommendation did not list missing evidence")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--vulkan", "--vulkan-json", dest="vulkan_json", help="Vulkan baseline benchmark summary.json")
    parser.add_argument("--dx12", "--dx12-json", dest="dx12_json", help="DX12 benchmark summary.json")
    parser.add_argument("--pix", "--pix-csv", dest="pix_csv", default="", help="Optional PIX summary CSV")
    parser.add_argument("--presentmon", "--presentmon-csv", dest="presentmon_csv", default="", help="Optional PresentMon CSV")
    parser.add_argument("--markdown", "--markdown-report", dest="markdown_report", default="dx12_parity_report.md")
    parser.add_argument("--csv", "--csv-summary", dest="csv_summary", default="dx12_parity_summary.csv")
    parser.add_argument("--json", "--json-report", dest="json_report", default="", help="Optional machine-readable recommendation JSON")
    parser.add_argument("--self-test", action="store_true", help="Run the built-in parser/report smoke test")
    return parser


def main(argv: list[str]) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    if args.self_test:
        return self_test()
    if not args.vulkan_json or not args.dx12_json:
        parser.error("--vulkan-json and --dx12-json are required unless --self-test is used")
    return run_report(args)


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
