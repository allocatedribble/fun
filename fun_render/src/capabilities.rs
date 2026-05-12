use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use bevy::{
    prelude::*,
    render::{
        backend_capabilities::RenderBackendCapabilities,
        renderer::{RenderAdapterInfo, RenderDevice},
        settings::Backends,
    },
};
use serde::Serialize;
use tracing::{info, warn};

use crate::{
    BridgeFeatureToggles, NativeDlssConfig, RenderBackendSelectionFacts, RendererBridgeSettings,
    dx12_dlss_sr, pipeline_warmup,
};

pub const RENDERER_CAPABILITY_REPORT_SCHEMA: &str = "fun.renderer.capability_report.v1";
pub const RENDERER_CAPABILITY_REPORT_PATH_ENV: &str = "FUN_RENDERER_CAPABILITY_REPORT_PATH";

#[derive(Debug, Clone, PartialEq, Eq, Resource, Serialize)]
pub struct RendererCapabilityReport {
    pub schema: &'static str,
    pub selected_renderer_lane: &'static str,
    pub actual_renderer_lane: &'static str,
    pub renderer_lane_selection_reason: &'static str,
    pub requested_graphics_backend: &'static str,
    pub selected_graphics_backend: &'static str,
    pub actual_graphics_backend: &'static str,
    pub fallback_graphics_backend: &'static str,
    pub graphics_backend_fallback_reason: &'static str,
    pub graphics_backend_selection_reason: &'static str,
    pub graphics_backend_truth_state: &'static str,
    pub adapter_vendor: &'static str,
    pub adapter_vendor_id: u32,
    pub adapter_device_id: u32,
    pub adapter_type: &'static str,
    pub adapter_name_hash: String,
    pub driver_hash: String,
    pub driver_info_hash: String,
    pub feature_level: &'static str,
    pub dx12_native_handle_support: bool,
    pub dx12_native_interop_support: &'static str,
    pub vulkan_backend_support: bool,
    pub d3d11on12_fallback_state: &'static str,
    pub native_ui_accelerated_shared_texture_support: &'static str,
    pub native_ui_cpu_runtime_upload_fallback_allowed: bool,
    pub bindless_resource_array_support: bool,
    pub descriptor_indexing_support: bool,
    pub sampler_feedback_support: &'static str,
    pub mesh_shader_support: bool,
    pub ray_tracing_support: bool,
    pub variable_rate_shading_support: &'static str,
    pub hdr_swapchain_support: &'static str,
    pub swapchain_format: &'static str,
    pub dlss_availability: &'static str,
    pub fsr_availability: &'static str,
    pub frame_generation_eligibility: &'static str,
    pub premium_rendering_gate: &'static str,
    pub backend_parity_scene_ids: &'static [&'static str],
    pub pipeline_warmup_status: &'static str,
    pub pipeline_warmup_mode: &'static str,
    pub ui_transport_mode: &'static str,
    pub bevy_backend_capability_hash: String,
    pub bevy_backend_capabilities: BevyBackendCapabilityReport,
    pub active_feature_flags: BridgeFeatureFlagReport,
    pub environment_overrides: BTreeMap<&'static str, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BevyBackendCapabilityReport {
    pub ray_query: &'static str,
    pub blas_tlas: &'static str,
    pub blas_compaction: &'static str,
    pub tlas_update: &'static str,
    pub async_copy_queue: &'static str,
    pub async_compute_queue: &'static str,
    pub dlss_sr: &'static str,
    pub dlss_rr: &'static str,
    pub hardware_opacity_micromap: &'static str,
    pub shader_execution_reordering: &'static str,
    pub linear_swept_sphere_rt: &'static str,
    pub native_rt_validation: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BridgeFeatureFlagReport {
    pub legacy_renderer: bool,
    pub fun_renderer_core: bool,
    pub dx12_native_interop: bool,
    pub vulkan_backend: bool,
    pub native_ui_gpu_only: bool,
    pub upscaling: bool,
    pub dlss: bool,
    pub fsr: bool,
    pub frame_generation: bool,
    pub experimental_renderer_ml: bool,
    pub many_light: bool,
    pub virtual_shadows: bool,
    pub hybrid_gi: bool,
}

pub fn emit_renderer_capability_report(
    mut commands: Commands,
    settings: Res<RendererBridgeSettings>,
    render_device: Res<RenderDevice>,
    adapter_info: Res<RenderAdapterInfo>,
    backend_capabilities: Res<RenderBackendCapabilities>,
    dlss_support: Option<Res<dx12_dlss_sr::Dx12NativeDlssSrSupport>>,
) {
    let report = RendererCapabilityReport::from_runtime(
        *settings,
        &render_device,
        &adapter_info,
        &backend_capabilities,
        dlss_support.as_deref().copied().unwrap_or_default(),
    );
    report.log_startup();
    report.write_if_requested();
    commands.insert_resource(report);
}

impl RendererCapabilityReport {
    #[must_use]
    pub fn from_runtime(
        settings: RendererBridgeSettings,
        render_device: &RenderDevice,
        adapter_info: &RenderAdapterInfo,
        backend_capabilities: &RenderBackendCapabilities,
        dlss_support: dx12_dlss_sr::Dx12NativeDlssSrSupport,
    ) -> Self {
        let features = render_device.features();
        let graphics_backend_selection = RenderBackendSelectionFacts::from_env();
        let selected_graphics_backend = graphics_backend_selection.selected_backends;
        let actual_graphics_backend = adapter_info.backend;
        let warmup = pipeline_warmup::FunPipelineWarmupConfig::from_env();
        let native_dlss_config = NativeDlssConfig::from_env();
        let native_ui_cpu_runtime_upload_fallback_allowed =
            native_ui_cpu_runtime_upload_fallback_allowed();
        let d3d11on12_fallback_state = if native_ui_cpu_runtime_upload_fallback_allowed {
            "cpu_fallback_opt_in"
        } else {
            "cpu_fallback_forbidden"
        };
        let actual_is_dx12 = actual_graphics_backend == wgpu::Backend::Dx12;
        let actual_backend_selected =
            graphics_backend_selected(selected_graphics_backend, actual_graphics_backend);
        let bindless_resource_array_support = features
            .contains(wgpu::Features::TEXTURE_BINDING_ARRAY)
            && features.contains(wgpu::Features::STORAGE_RESOURCE_BINDING_ARRAY)
            && features.contains(
                wgpu::Features::SAMPLED_TEXTURE_AND_STORAGE_BUFFER_ARRAY_NON_UNIFORM_INDEXING,
            );
        let dx12_native_handle_support =
            cfg!(all(target_os = "windows", feature = "dx12_native_interop")) && actual_is_dx12;
        let native_ui_accelerated_shared_texture_support =
            if cfg!(target_os = "windows") && actual_is_dx12 && dx12_native_handle_support {
                "renderer_prerequisites_supported"
            } else if !actual_is_dx12 {
                "fallback_reason=render_backend_not_dx12"
            } else {
                "native_handle_support_unavailable"
            };
        let dlss_availability = dlss_availability(native_dlss_config, dlss_support, actual_is_dx12);
        Self {
            schema: RENDERER_CAPABILITY_REPORT_SCHEMA,
            selected_renderer_lane: settings.backend_selection.requested.as_env_value(),
            actual_renderer_lane: settings.backend_selection.resolved.as_env_value(),
            renderer_lane_selection_reason: settings.backend_selection.reason.as_str(),
            requested_graphics_backend: graphics_backend_selection.requested_label,
            selected_graphics_backend: selected_backends_label(selected_graphics_backend),
            actual_graphics_backend: backend_label(actual_graphics_backend),
            fallback_graphics_backend: graphics_backend_fallback_backend(
                actual_backend_selected,
                actual_graphics_backend,
            ),
            graphics_backend_fallback_reason: graphics_backend_fallback_reason(
                selected_graphics_backend,
                actual_graphics_backend,
            ),
            graphics_backend_selection_reason: graphics_backend_selection.selection_reason,
            graphics_backend_truth_state: graphics_backend_truth_state(
                selected_graphics_backend,
                actual_graphics_backend,
            ),
            adapter_vendor: backend_capabilities.vendor.as_str(),
            adapter_vendor_id: adapter_info.vendor,
            adapter_device_id: adapter_info.device,
            adapter_type: device_type_label(adapter_info.device_type),
            adapter_name_hash: format!("{:016x}", stable_str_hash(&adapter_info.name)),
            driver_hash: format!("{:016x}", stable_str_hash(&adapter_info.driver)),
            driver_info_hash: format!("{:016x}", stable_str_hash(&adapter_info.driver_info)),
            feature_level: "not_exposed_by_wgpu",
            dx12_native_handle_support,
            dx12_native_interop_support: dx12_native_interop_support(actual_is_dx12),
            vulkan_backend_support: actual_graphics_backend == wgpu::Backend::Vulkan,
            d3d11on12_fallback_state,
            native_ui_accelerated_shared_texture_support,
            native_ui_cpu_runtime_upload_fallback_allowed,
            bindless_resource_array_support,
            descriptor_indexing_support: bindless_resource_array_support,
            sampler_feedback_support: "not_exposed_by_wgpu",
            mesh_shader_support: features.contains(wgpu::Features::EXPERIMENTAL_MESH_SHADER),
            ray_tracing_support: features.contains(wgpu::Features::EXPERIMENTAL_RAY_QUERY),
            variable_rate_shading_support: "not_exposed_by_wgpu",
            hdr_swapchain_support: "not_collected",
            swapchain_format: "not_collected",
            dlss_availability,
            fsr_availability: if cfg!(feature = "fsr") {
                "compiled_no_runtime_backend"
            } else {
                "not_compiled"
            },
            frame_generation_eligibility: frame_generation_eligibility(),
            premium_rendering_gate: premium_rendering_gate(
                actual_backend_selected,
                actual_is_dx12,
                dx12_native_handle_support,
            ),
            backend_parity_scene_ids: &fun_renderer::BACKEND_PARITY_SCENE_IDS,
            pipeline_warmup_status: if warmup.is_enabled() {
                "enabled"
            } else {
                "disabled"
            },
            pipeline_warmup_mode: warmup.mode.as_env_value(),
            ui_transport_mode: ui_transport_mode(),
            bevy_backend_capability_hash: format!(
                "{:016x}",
                backend_capabilities.capability_hash()
            ),
            bevy_backend_capabilities: BevyBackendCapabilityReport::from_bevy(backend_capabilities),
            active_feature_flags: BridgeFeatureFlagReport::from_bridge(settings.features),
            environment_overrides: collect_environment_overrides(),
        }
    }

    fn log_startup(&self) {
        info!(
            target: "fun::render",
            schema = self.schema,
            selected_renderer_lane = self.selected_renderer_lane,
            actual_renderer_lane = self.actual_renderer_lane,
            renderer_lane_selection_reason = self.renderer_lane_selection_reason,
            requested_backend = self.requested_graphics_backend,
            selected_backend = self.selected_graphics_backend,
            actual_backend = self.actual_graphics_backend,
            fallback_backend = self.fallback_graphics_backend,
            fallback_reason = self.graphics_backend_fallback_reason,
            backend_selection_reason = self.graphics_backend_selection_reason,
            backend_truth_state = self.graphics_backend_truth_state,
            dx12_interop_support = self.dx12_native_interop_support,
            ui_transport_mode = self.ui_transport_mode,
            d3d11on12_fallback_state = self.d3d11on12_fallback_state,
            native_ui_accelerated_shared_texture_support = self.native_ui_accelerated_shared_texture_support,
            dlss_availability = self.dlss_availability,
            fsr_availability = self.fsr_availability,
            frame_generation_eligibility = self.frame_generation_eligibility,
            premium_rendering_gate = self.premium_rendering_gate,
            pipeline_warmup_status = self.pipeline_warmup_status,
            "Renderer capability report"
        );
        info!(
            target: "fun::render",
            "[fun render] startup capabilities: selected_renderer_lane={} actual_renderer_lane={} renderer_lane_reason={} requested_backend={} selected_backend={} actual_backend={} fallback_backend={} fallback_reason={} backend_selection_reason={} backend_truth_state={} dx12_interop_support={} ui_transport_mode={} d3d11on12_fallback_state={} native_ui_shared_texture={} dlss={} fsr={} frame_generation={} premium_rendering_gate={} pipeline_warmup={} pipeline_warmup_mode={}",
            self.selected_renderer_lane,
            self.actual_renderer_lane,
            self.renderer_lane_selection_reason,
            self.requested_graphics_backend,
            self.selected_graphics_backend,
            self.actual_graphics_backend,
            self.fallback_graphics_backend,
            self.graphics_backend_fallback_reason,
            self.graphics_backend_selection_reason,
            self.graphics_backend_truth_state,
            self.dx12_native_interop_support,
            self.ui_transport_mode,
            self.d3d11on12_fallback_state,
            self.native_ui_accelerated_shared_texture_support,
            self.dlss_availability,
            self.fsr_availability,
            self.frame_generation_eligibility,
            self.premium_rendering_gate,
            self.pipeline_warmup_status,
            self.pipeline_warmup_mode,
        );
    }

    fn write_if_requested(&self) {
        let Some(path) = std::env::var_os(RENDERER_CAPABILITY_REPORT_PATH_ENV).map(PathBuf::from)
        else {
            return;
        };
        if path.as_os_str().is_empty() {
            return;
        }
        if let Err(error) = write_report_file(&path, self) {
            warn!(
                target: "fun::render",
                ?error,
                path = %path.display(),
                "could not write renderer capability report"
            );
        }
    }
}

impl BevyBackendCapabilityReport {
    const fn from_bevy(capabilities: &RenderBackendCapabilities) -> Self {
        Self {
            ray_query: capabilities.ray_query.as_str(),
            blas_tlas: capabilities.blas_tlas.as_str(),
            blas_compaction: capabilities.blas_compaction.as_str(),
            tlas_update: capabilities.tlas_update.as_str(),
            async_copy_queue: capabilities.async_copy_queue.as_str(),
            async_compute_queue: capabilities.async_compute_queue.as_str(),
            dlss_sr: capabilities.dlss_sr.as_str(),
            dlss_rr: capabilities.dlss_rr.as_str(),
            hardware_opacity_micromap: capabilities.hardware_opacity_micromap.as_str(),
            shader_execution_reordering: capabilities.shader_execution_reordering.as_str(),
            linear_swept_sphere_rt: capabilities.linear_swept_sphere_rt.as_str(),
            native_rt_validation: capabilities.native_rt_validation.as_str(),
        }
    }
}

impl BridgeFeatureFlagReport {
    const fn from_bridge(features: BridgeFeatureToggles) -> Self {
        Self {
            legacy_renderer: features.legacy,
            fun_renderer_core: features.new_core,
            dx12_native_interop: features.dx12,
            vulkan_backend: features.vulkan,
            native_ui_gpu_only: features.native_ui_gpu_only,
            upscaling: features.upscale,
            dlss: features.dlss,
            fsr: features.fsr,
            frame_generation: features.frame_generation,
            experimental_renderer_ml: features.experimental_ml,
            many_light: features.lux_many_light,
            virtual_shadows: features.lux_virtual_shadows,
            hybrid_gi: features.lux_hybrid_gi,
        }
    }
}

fn write_report_file(path: &Path, report: &RendererCapabilityReport) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let payload = serde_json::to_string_pretty(report).map_err(std::io::Error::other)?;
    std::fs::write(path, format!("{payload}\n"))
}

fn collect_environment_overrides() -> BTreeMap<&'static str, String> {
    const NAMES: [&str; 24] = [
        "FUN_RENDERER_BACKEND",
        "FUN_RENDER_BACKEND",
        "WGPU_BACKEND",
        "WGPU_BACKENDS",
        "BEVY_RENDER_BACKEND",
        "FUN_PRESENT_MODE",
        "FUN_RENDER_PRESENT_MODE",
        "FUN_RENDER_MAX_FRAME_LATENCY",
        "FUN_PRESENT_MAX_FRAME_LATENCY",
        "FUN_NATIVE_UI_PAINT_TRANSPORT",
        "FUN_NATIVE_UI_ACCELERATED_PAINT",
        "FUN_NATIVE_UI_ACCELERATED_STRICT",
        "FUN_NATIVE_UI_ALLOW_CPU_FALLBACK",
        "FUN_NATIVE_UI_GPU_RING_DEPTH",
        "FUN_NATIVE_UI_COPY_DIRTY_RECTS",
        "FUN_NATIVE_UI_DEBUG_TIMINGS",
        "FUN_RENDER_PIPELINE_WARMUP",
        "FUN_RENDER_PIPELINE_WARMUP_BUDGET_MS",
        "FUN_DISABLE_DLSS_RR",
        "FUN_RENDER_DX12_DLSS_RR",
        "FUN_DISABLE_SOLARI",
        "FUN_DISABLE_MESHLETS",
        "FUN_DISABLE_CLOUDS",
        "FUN_RENDER_VENDOR_EMULATION",
    ];
    let mut values = BTreeMap::new();
    for name in NAMES {
        if let Ok(value) = std::env::var(name)
            && !value.trim().is_empty()
        {
            values.insert(name, value);
        }
    }
    values
}

fn selected_backends_label(backends: Backends) -> &'static str {
    match (
        backends.contains(Backends::DX12),
        backends.contains(Backends::VULKAN),
    ) {
        (true, false) => "dx12",
        (false, true) => "vulkan",
        (true, true) => "auto_dx12_vulkan",
        (false, false) => "none",
    }
}

const fn backend_label(backend: wgpu::Backend) -> &'static str {
    match backend {
        wgpu::Backend::Noop => "noop",
        wgpu::Backend::Vulkan => "vulkan",
        wgpu::Backend::Metal => "metal",
        wgpu::Backend::Dx12 => "dx12",
        wgpu::Backend::Gl => "gl",
        wgpu::Backend::BrowserWebGpu => "browser_webgpu",
    }
}

const fn device_type_label(device_type: wgpu::DeviceType) -> &'static str {
    match device_type {
        wgpu::DeviceType::Other => "other",
        wgpu::DeviceType::IntegratedGpu => "integrated_gpu",
        wgpu::DeviceType::DiscreteGpu => "discrete_gpu",
        wgpu::DeviceType::VirtualGpu => "virtual_gpu",
        wgpu::DeviceType::Cpu => "cpu",
    }
}

fn graphics_backend_fallback_reason(
    selected_backends: Backends,
    actual_backend: wgpu::Backend,
) -> &'static str {
    if graphics_backend_selected(selected_backends, actual_backend) {
        "none"
    } else {
        "actual_backend_mismatch"
    }
}

fn graphics_backend_fallback_backend(
    actual_backend_selected: bool,
    actual_backend: wgpu::Backend,
) -> &'static str {
    if actual_backend_selected {
        "none"
    } else {
        backend_label(actual_backend)
    }
}

fn graphics_backend_selected(selected_backends: Backends, actual_backend: wgpu::Backend) -> bool {
    match actual_backend {
        wgpu::Backend::Dx12 => selected_backends.contains(Backends::DX12),
        wgpu::Backend::Vulkan => selected_backends.contains(Backends::VULKAN),
        _ => false,
    }
}

fn graphics_backend_truth_state(
    selected_backends: Backends,
    actual_backend: wgpu::Backend,
) -> &'static str {
    if !graphics_backend_selected(selected_backends, actual_backend) {
        return "actual_backend_mismatch";
    }
    let auto_selected =
        selected_backends.contains(Backends::DX12) && selected_backends.contains(Backends::VULKAN);
    match (auto_selected, actual_backend) {
        (true, wgpu::Backend::Dx12) => "auto_resolved_dx12",
        (true, wgpu::Backend::Vulkan) => "auto_resolved_vulkan",
        (false, wgpu::Backend::Dx12) => "trusted_dx12",
        (false, wgpu::Backend::Vulkan) => "trusted_vulkan",
        _ => "trusted_other",
    }
}

fn dx12_native_interop_support(actual_is_dx12: bool) -> &'static str {
    if !cfg!(target_os = "windows") {
        return "unsupported_non_windows";
    }
    if !cfg!(feature = "dx12_native_interop") {
        return "not_compiled";
    }
    if !actual_is_dx12 {
        return "fallback_reason=render_backend_not_dx12";
    }
    "supported"
}

fn premium_rendering_gate(
    actual_backend_selected: bool,
    actual_is_dx12: bool,
    dx12_native_handle_support: bool,
) -> &'static str {
    if !actual_backend_selected {
        return "blocked_backend_mismatch";
    }
    if !actual_is_dx12 {
        return "blocked_backend_not_dx12";
    }
    if !dx12_native_handle_support {
        return "blocked_dx12_native_interop";
    }
    "dx12_foundation_eligible"
}

fn native_ui_cpu_runtime_upload_fallback_allowed() -> bool {
    env_flag_enabled("FUN_NATIVE_UI_ALLOW_CPU_FALLBACK")
}

fn ui_transport_mode() -> &'static str {
    let Some(value) = std::env::var("FUN_NATIVE_UI_PAINT_TRANSPORT").ok() else {
        return "not_requested";
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "disabled" | "off" | "none" => "disabled",
        "cpu" | "cpu_paint" => "cpu",
        "auto" => "auto",
        "d3d11on12" | "d3d11_shared_texture_dx12_copy" | "d3d11-shared-texture-dx12-copy" => {
            "d3d11on12"
        }
        _ => "unknown",
    }
}

fn dlss_availability(
    config: NativeDlssConfig,
    support: dx12_dlss_sr::Dx12NativeDlssSrSupport,
    actual_is_dx12: bool,
) -> &'static str {
    if !cfg!(feature = "dlss") {
        return "not_compiled";
    }
    if !actual_is_dx12 {
        return "fallback_reason=render_backend_not_dx12";
    }
    if !config.enabled {
        return "compiled_runtime_disabled";
    }
    if support.super_resolution_ready(config) {
        return "available";
    }
    if support.driver_needs_update {
        return "driver_needs_update";
    }
    if !support.runtime_found {
        return "sdk_runtime_not_found";
    }
    if !support.native_handle_extraction_available {
        return "native_handle_extraction_unavailable";
    }
    "unsupported"
}

fn frame_generation_eligibility() -> &'static str {
    if cfg!(feature = "frame_generation") {
        "compiled_boundary_not_ready"
    } else {
        "not_compiled"
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "on"
        )
    })
}

fn stable_str_hash(value: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = FNV_OFFSET;
    for byte in value.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_truth_names_actual_fallback_backend() {
        assert_eq!(
            graphics_backend_fallback_reason(Backends::DX12, wgpu::Backend::Vulkan),
            "actual_backend_mismatch"
        );
        assert_eq!(
            graphics_backend_fallback_backend(false, wgpu::Backend::Vulkan),
            "vulkan"
        );
        assert_eq!(
            graphics_backend_truth_state(Backends::DX12, wgpu::Backend::Vulkan),
            "actual_backend_mismatch"
        );
    }

    #[test]
    fn backend_truth_accepts_auto_only_when_actual_is_in_requested_set() {
        let auto = Backends::DX12 | Backends::VULKAN;
        assert_eq!(
            graphics_backend_fallback_reason(auto, wgpu::Backend::Dx12),
            "none"
        );
        assert_eq!(
            graphics_backend_truth_state(auto, wgpu::Backend::Dx12),
            "auto_resolved_dx12"
        );
        assert_eq!(
            graphics_backend_truth_state(auto, wgpu::Backend::Vulkan),
            "auto_resolved_vulkan"
        );
    }

    #[test]
    fn premium_gate_requires_selected_actual_dx12_and_native_handles() {
        assert_eq!(
            premium_rendering_gate(false, true, true),
            "blocked_backend_mismatch"
        );
        assert_eq!(
            premium_rendering_gate(true, false, false),
            "blocked_backend_not_dx12"
        );
        assert_eq!(
            premium_rendering_gate(true, true, false),
            "blocked_dx12_native_interop"
        );
        assert_eq!(
            premium_rendering_gate(true, true, true),
            "dx12_foundation_eligible"
        );
    }

    #[test]
    fn capability_report_exposes_required_backend_parity_scene_catalog() {
        assert!(fun_renderer::BACKEND_PARITY_SCENE_IDS.contains(&"backend_parity.clear_present"));
        assert!(
            fun_renderer::BACKEND_PARITY_SCENE_IDS.contains(&"backend_parity.native_ui_composite")
        );
        assert!(
            fun_renderer::BACKEND_PARITY_SCENE_IDS
                .contains(&"backend_parity.post_upscale_placeholder")
        );
    }
}
