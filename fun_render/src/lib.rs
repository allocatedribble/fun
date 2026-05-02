mod catalog;
mod compiled_world;
mod config;
pub mod core;
pub mod lighting;
#[cfg(feature = "offscreen")]
pub mod offscreen;
mod signature;
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
pub use config::{
    ClientOpaqueRenderer, ClientRenderConfig, ClientRenderProfile, ClientWindowConfig,
    FunRenderAppOptions, FunRenderPresentation, FunRenderRtFeatures, RenderGeometryClass,
    RenderGeometryPolicy, RtHairMode, RtMegaGeometryMode, RtOpacityMaskMode, RtVendorEmulation,
    selected_present_mode, selected_render_backend,
};
pub use core::{
    FunRenderCorePlugin, enable_solari_lighting_for_ready_world, install_fun_render_core,
    request_solari_lighting_history_reset,
};
#[cfg(feature = "offscreen")]
pub use offscreen::{EditorOffscreenRenderTarget, FunRenderOffscreenPresentationPlugin};
pub use signature::{RenderPathSignature, render_path_signature_for_options};
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
