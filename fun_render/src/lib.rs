#[cfg(all(feature = "dx12_dlss_native", not(target_os = "windows")))]
compile_error!("fun_render/dx12_dlss_native is a Windows-only experimental feature");

mod catalog;
mod compiled_world;
mod composition;
mod config;
pub mod core;
mod dlss_correctness;
mod dx12_dlss_rr;
mod dx12_dlss_sr;
#[cfg(all(target_os = "windows", feature = "dx12_dlss_native"))]
pub mod dx12_native;
pub mod lighting;
#[cfg(feature = "offscreen")]
pub mod offscreen;
mod pipeline_warmup;
mod signature;
pub mod sky;
pub mod solari;
#[cfg(feature = "winit_presentation")]
pub mod winit;
pub mod world_stream;

#[cfg(all(feature = "diagnostics", debug_assertions))]
pub use catalog::catalog_ref_summary;
pub use catalog::{
    CompiledRenderAsset, WorldRenderCatalog, prewarm_world_render_catalog, warn_missing_catalog_ref,
};
pub use compiled_world::{CompiledStaticAsset, CompiledWorldPackage, CompiledWorldPackageId};
pub use composition::{
    FUN_RENDER_COMPOSITION_ORDER, FUN_RENDER_DEBUG_OVERLAY_STAGE, FUN_RENDER_DEBUG_OVERLAY_Z_INDEX,
    FUN_RENDER_HUD_UI_STAGE, FUN_RENDER_HUD_UI_Z_INDEX, FunRenderCompositionStage,
};
pub use config::{
    ClientOpaqueRenderer, ClientRenderConfig, ClientRenderProfile, ClientWindowConfig,
    FunRenderAppOptions, FunRenderPresentation, FunRenderRtFeatures, NativeDlssConfig,
    NativeDlssMode, RenderGeometryClass, RenderGeometryPolicy, RtHairMode, RtMegaGeometryMode,
    RtOpacityMaskMode, RtVendorEmulation, log_native_dlss_startup_diagnostics,
    selected_max_frame_latency, selected_present_mode, selected_render_backend,
};
pub use core::{
    FunRenderCorePlugin, enable_solari_lighting_for_ready_world, install_fun_render_core,
    request_solari_lighting_history_reset,
};
pub use dlss_correctness::{
    DlssCameraValidation, DlssDebugVisualization, DlssDepthConvention, DlssDepthDiagnostics,
    DlssHistoryReset, DlssMipBiasState, DlssMotionVectorConvention, DlssMotionVectorDirection,
    DlssMotionVectorUnits, DlssResetReason, Dx12DlssPreviousViewProjection, Dx12NativeDlssCamera,
    Dx12NativeDlssCameraRuntimeState, is_dlss_supported_color_format, native_dlss_input_resolution,
    native_dlss_internal_scale_factor, native_dlss_mip_bias, native_dlss_mip_bias_state,
    validate_camera_for_dx12_dlss,
};
pub use dx12_dlss_rr::{
    Dx12NativeDlssRrGateRejection, Dx12NativeDlssRrGuideSurfaceSpec,
    Dx12NativeDlssRrGuideSurfaceStatus, Dx12NativeDlssRrStatus, Dx12NativeDlssRrSupport,
    install_dx12_native_dlss_rr, solari_rr_guide_surface_audit,
};
pub use dx12_dlss_sr::{
    Dx12NativeDlssSrFailure, Dx12NativeDlssSrNode, Dx12NativeDlssSrOutputDescriptor,
    Dx12NativeDlssSrRuntimeMode, Dx12NativeDlssSrState, Dx12NativeDlssSrStatus,
    Dx12NativeDlssSrSupport, Dx12NativeDlssSrTransition, Dx12NativeDlssSrView,
    install_dx12_native_dlss_sr, log_dx12_native_dlss_sr_support_once,
};
#[cfg(all(target_os = "windows", feature = "dx12_dlss_native"))]
pub use dx12_native::{
    Dx12DlssResourceStatePlan, Dx12DlssResourceStateRules, Dx12NativeHandles,
    Dx12NativeInteropError, Dx12NativeInteropFailure, Dx12NativeResourceState, Dx12TextureHandle,
    DxgiFormatLike, extract_dx12_native_handles, extract_dx12_texture_handle,
    log_dx12_dlss_resource_state_plan_once, validate_dx12_backend, validate_dx12_device_queue,
    with_dx12_command_list, with_dx12_command_list_checked,
};
#[cfg(feature = "offscreen")]
pub use offscreen::{EditorOffscreenRenderTarget, FunRenderOffscreenPresentationPlugin};
pub use pipeline_warmup::{FunPipelineWarmupConfig, FunPipelineWarmupMode};
pub use signature::{RenderPathSignature, render_path_signature_for_options};
pub use sky::{
    FunCloudDebugOverlay, FunCloudHistoryResetEvent, FunCloudHistoryResetReason,
    FunCloudHistoryState, FunCloudInternalScale, FunCloudQuality, FunCloudSettings,
    FunCloudTypeMix, FunSkyPlugin, FunWeatherPattern, FunWeatherPatternPhase, FunWeatherProfile,
    FunWeatherProfileId, FunWeatherState, FunWeatherTransition, FunWeatherTransitionCurve,
    FunWeatherValidationError, request_cloud_history_reset,
};
pub use solari::benchmark_parse_solari_denoise_mode;
pub use solari::{
    parse_solari_denoise_mode, solari_runtime_params_from_env, solari_settings_from_env,
};
#[cfg(feature = "winit_presentation")]
pub use winit::FunRenderWinitPresentationPlugin;
pub use world_stream::{
    RenderWorldApplyOptions, RenderWorldChunkOutcome, RenderWorldContext, RenderWorldStatus,
    apply_render_world_chunk, despawn_render_context, spawn_render_entity_from_spec,
    update_render_context_visibility,
};

pub const WINIT_PRESENTATION_ENABLED: bool = cfg!(feature = "winit_presentation");
