use std::collections::BTreeMap;

#[cfg(debug_assertions)]
use bevy::dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin};
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use bevy::render::diagnostic::RenderDiagnosticsPlugin;
use bevy::{
    camera::CameraMainTextureUsages,
    pbr::experimental::meshlet::MeshletPlugin,
    prelude::*,
    render::{
        RenderApp, RenderStartup, backend_capabilities::RenderBackendCapabilities,
        extract_resource::ExtractResourcePlugin, init_gpu_resource, render_resource::TextureUsages,
    },
    solari::prelude::{
        SolariDenoiseMode, SolariFeaturePolicy, SolariPlugins, SolariRuntimeParams, SolariSettings,
    },
};
use tracing::info;
#[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
use tracing::warn;

use crate::{
    ClientOpaqueRenderer, ClientRenderConfig, DynamicInstanceTable,
    FunEntityRenderStrategyRegistry, FunFrameGraph, FunGeometryClass, FunGpuSceneDatabase,
    FunHiZOcclusionAdaptiveState, FunMaterialClass, FunPipelineRegistry, FunRenderAppOptions,
    FunRenderCapabilityMatrix, FunRenderPath, FunRenderRtFeatures, FunRendererConfig,
    FunRendererEcsEvent, FunRendererEcsSchedulePolicy, FunRendererPageAllocator,
    FunRendererUploadArena, FunSceneManifestRegistry, FunSkyPlugin, FunViewportRegistry,
    GeometryResidencyManager, MaterialResidencyManager, RenderPathSignature, StaticInstanceTable,
    TextureResidencyManager, VirtualGeometryResidency, dlss_correctness, dx12_dlss_rr,
    dx12_dlss_sr, lighting, pipeline_warmup, prewarm_primitive_render_cache,
    prewarm_world_render_catalog, render_path_signature_for_options,
    solari::{solari_runtime_params_from_env, solari_settings_from_env},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FunRenderPhaseKind {
    DepthPrepass,
    Shadow,
    MainOpaque,
    DeferredGBuffer,
    Transparent,
    MeshletVisibility,
    MeshletResolve,
    Clouds,
    Solari,
    PostProcess,
    CefUi,
    DebugOverlay,
}

impl FunRenderPhaseKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DepthPrepass => "depth_prepass",
            Self::Shadow => "shadow",
            Self::MainOpaque => "main_opaque",
            Self::DeferredGBuffer => "deferred_gbuffer",
            Self::Transparent => "transparent",
            Self::MeshletVisibility => "meshlet_visibility",
            Self::MeshletResolve => "meshlet_resolve",
            Self::Clouds => "clouds",
            Self::Solari => "solari",
            Self::PostProcess => "post_process",
            Self::CefUi => "cef_ui",
            Self::DebugOverlay => "debug_overlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunDrawSubmissionKind {
    Direct,
    Indirect,
    MultiDraw,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunDrawCallRecord {
    pub phase: FunRenderPhaseKind,
    pub material_class: FunMaterialClass,
    pub geometry_class: FunGeometryClass,
    pub render_path: FunRenderPath,
    pub submission_kind: FunDrawSubmissionKind,
    pub draws: u64,
    pub submitted_instances: u64,
    pub visible_instances: u64,
}

impl FunDrawCallRecord {
    pub const fn new(
        phase: FunRenderPhaseKind,
        material_class: FunMaterialClass,
        geometry_class: FunGeometryClass,
        render_path: FunRenderPath,
        submission_kind: FunDrawSubmissionKind,
        draws: u64,
    ) -> Self {
        Self {
            phase,
            material_class,
            geometry_class,
            render_path,
            submission_kind,
            draws,
            submitted_instances: 0,
            visible_instances: 0,
        }
    }

    pub const fn with_instances(
        mut self,
        submitted_instances: u64,
        visible_instances: u64,
    ) -> Self {
        self.submitted_instances = submitted_instances;
        self.visible_instances = visible_instances;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, bevy::prelude::Resource)]
pub struct FunDrawCallCounters {
    pub frame_index: u64,
    pub total_draws: u64,
    pub direct_draws: u64,
    pub indirect_draws: u64,
    pub multi_draws: u64,
    pub draws_by_phase: BTreeMap<FunRenderPhaseKind, u64>,
    pub draws_by_material_class: BTreeMap<FunMaterialClass, u64>,
    pub draws_by_geometry_class: BTreeMap<FunGeometryClass, u64>,
    pub draws_by_render_path: BTreeMap<FunRenderPath, u64>,
    pub submitted_instances: u64,
    pub visible_instances: u64,
    pub culled_instances: u64,
}

impl Default for FunDrawCallCounters {
    fn default() -> Self {
        Self::new(0)
    }
}

impl FunDrawCallCounters {
    pub fn new(frame_index: u64) -> Self {
        Self {
            frame_index,
            total_draws: 0,
            direct_draws: 0,
            indirect_draws: 0,
            multi_draws: 0,
            draws_by_phase: BTreeMap::new(),
            draws_by_material_class: BTreeMap::new(),
            draws_by_geometry_class: BTreeMap::new(),
            draws_by_render_path: BTreeMap::new(),
            submitted_instances: 0,
            visible_instances: 0,
            culled_instances: 0,
        }
    }

    pub fn begin_frame(&mut self, frame_index: u64) {
        *self = Self::new(frame_index);
    }

    pub fn record_draw(&mut self, record: FunDrawCallRecord) {
        self.total_draws = self.total_draws.saturating_add(record.draws);
        match record.submission_kind {
            FunDrawSubmissionKind::Direct => {
                self.direct_draws = self.direct_draws.saturating_add(record.draws);
            }
            FunDrawSubmissionKind::Indirect => {
                self.indirect_draws = self.indirect_draws.saturating_add(record.draws);
            }
            FunDrawSubmissionKind::MultiDraw => {
                self.multi_draws = self.multi_draws.saturating_add(record.draws);
            }
        }
        add_counter(&mut self.draws_by_phase, record.phase, record.draws);
        add_counter(
            &mut self.draws_by_material_class,
            record.material_class,
            record.draws,
        );
        add_counter(
            &mut self.draws_by_geometry_class,
            record.geometry_class,
            record.draws,
        );
        add_counter(
            &mut self.draws_by_render_path,
            record.render_path,
            record.draws,
        );
        self.record_instances(record.submitted_instances, record.visible_instances);
    }

    pub fn record_instances(&mut self, submitted_instances: u64, visible_instances: u64) {
        self.submitted_instances = self.submitted_instances.saturating_add(submitted_instances);
        self.visible_instances = self.visible_instances.saturating_add(visible_instances);
        self.culled_instances = self
            .culled_instances
            .saturating_add(submitted_instances.saturating_sub(visible_instances));
    }

    pub fn worst_phase(&self) -> Option<(FunRenderPhaseKind, u64)> {
        self.draws_by_phase
            .iter()
            .max_by_key(|(phase, draws)| (**draws, **phase))
            .map(|(phase, draws)| (*phase, *draws))
    }
}

fn add_counter<K>(counters: &mut BTreeMap<K, u64>, key: K, value: u64)
where
    K: Ord,
{
    let counter = counters.entry(key).or_default();
    *counter = counter.saturating_add(value);
}

#[derive(Debug, Clone)]
pub struct FunRenderCorePlugin {
    options: FunRenderAppOptions,
}

impl FunRenderCorePlugin {
    pub const fn new(options: FunRenderAppOptions) -> Self {
        Self { options }
    }
}

impl Plugin for FunRenderCorePlugin {
    fn build(&self, app: &mut App) {
        install_fun_render_core(app, &self.options);
    }
}

pub fn install_fun_render_core(app: &mut App, options: &FunRenderAppOptions) {
    let (render_config, solari_settings, solari_runtime_params) = render_path_config_from_env();
    log_fun_render_path(&render_config, &solari_settings, &solari_runtime_params);
    render_config.rt_features.log_config();
    let opaque_renderer = selected_opaque_renderer(&render_config);
    let signature = render_path_signature_for_options(
        options,
        &render_config,
        &solari_settings,
        opaque_renderer,
    );
    emit_render_path_signature(options, &signature);

    let solari_enabled = render_config.solari_enabled;
    let solari_feature_policy: SolariFeaturePolicy =
        render_config.rt_features.solari_feature_policy();
    let rt_features = render_config.rt_features;
    #[cfg(debug_assertions)]
    let fps_overlay_enabled = render_config.fps_overlay_enabled;

    app.insert_resource(opaque_renderer.method())
        .init_resource::<FunDrawCallCounters>()
        .init_resource::<FunRendererEcsSchedulePolicy>()
        .init_resource::<FunRendererConfig>()
        .init_resource::<FunGpuSceneDatabase>()
        .init_resource::<FunFrameGraph>()
        .init_resource::<FunRendererPageAllocator>()
        .init_resource::<FunRendererUploadArena>()
        .init_resource::<FunRenderCapabilityMatrix>()
        .init_resource::<FunPipelineRegistry>()
        .init_resource::<crate::fun_lux::FunLuxLightDatabase>()
        .init_resource::<FunSceneManifestRegistry>()
        .init_resource::<FunViewportRegistry>()
        .init_resource::<GeometryResidencyManager>()
        .init_resource::<TextureResidencyManager>()
        .init_resource::<MaterialResidencyManager>()
        .init_resource::<StaticInstanceTable>()
        .init_resource::<DynamicInstanceTable>()
        .init_resource::<crate::gpu_visibility::StaticOpaqueGpuVisibilityConfig>()
        .init_resource::<FunHiZOcclusionAdaptiveState>()
        .init_resource::<VirtualGeometryResidency>()
        .init_resource::<FunEntityRenderStrategyRegistry>()
        .insert_resource(signature)
        .insert_resource(rt_features)
        .insert_resource(render_config)
        .insert_resource(solari_settings)
        .insert_resource(solari_runtime_params)
        .insert_resource(solari_feature_policy)
        .add_message::<FunRendererEcsEvent>()
        .add_message::<crate::fun_lux::FunLuxLightEvent>()
        .add_message::<bevy::solari::prelude::SolariResetEvent>()
        .add_plugins(ExtractResourcePlugin::<FunRenderRtFeatures>::default())
        .add_plugins(MeshletPlugin {
            cluster_buffer_slots: 1 << 14,
        })
        .add_systems(
            Startup,
            (
                lighting::setup_lighting,
                prewarm_world_render_catalog,
                prewarm_primitive_render_cache,
            )
                .chain(),
        );

    #[cfg(debug_assertions)]
    if fps_overlay_enabled && !options.is_editor_preview() {
        app.add_plugins(FpsOverlayPlugin {
            config: FpsOverlayConfig {
                refresh_interval: std::time::Duration::from_secs(1),
                text_config: bevy::text::TextFont {
                    font_size: bevy::text::FontSize::Px(12.0),
                    ..default()
                },
                ..default()
            },
        });
    }

    if solari_enabled {
        app.add_plugins(SolariPlugins);
    }
    if render_config.clouds_enabled {
        app.add_plugins(FunSkyPlugin::new(render_config.cloud_settings()));
    }
    dlss_correctness::install_dlss_correctness(app);
    let dx12_native_dlss_sr_support = dx12_dlss_sr::query_dx12_native_dlss_sr_support();
    dx12_dlss_sr::install_dx12_native_dlss_sr(app);
    app.insert_resource(dx12_native_dlss_sr_support);
    dx12_dlss_rr::install_dx12_native_dlss_rr(app);
    pipeline_warmup::install_fun_pipeline_warmup(app);
    crate::compute_culling::install_fun_compute_culling(app);
    dx12_dlss_sr::log_dx12_native_dlss_sr_support_once(
        render_config.native_dlss,
        dx12_native_dlss_sr_support,
    );

    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    {
        if std::env::var_os("FUN_RENDER_DIAGNOSTICS").is_some() {
            game_shared::fun_diag_info!("[fun render] GPU render diagnostics enabled");
            app.add_plugins((
                RenderDiagnosticsPlugin,
                bevy::diagnostic::SystemInformationDiagnosticsPlugin,
            ));
        }
    }
    #[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
    {
        if std::env::var_os("FUN_RENDER_DIAGNOSTICS").is_some()
            || std::env::var_os("FUN_FRAME_TIME_DIAGNOSTICS").is_some()
            || std::env::var_os("FUN_RENDER_PROFILE_VERBOSE").is_some()
        {
            warn!(
                target: "fun::render",
                "render diagnostics requested but fun_render/render_diagnostics is not enabled"
            );
        }
    }

    if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
        render_app.init_resource::<FunDrawCallCounters>();
        render_app.init_resource::<FunRendererEcsSchedulePolicy>();
        render_app.init_resource::<FunRendererConfig>();
        render_app.init_resource::<FunGpuSceneDatabase>();
        render_app.init_resource::<FunFrameGraph>();
        render_app.init_resource::<FunRendererPageAllocator>();
        render_app.init_resource::<FunRendererUploadArena>();
        render_app.init_resource::<FunRenderCapabilityMatrix>();
        render_app.init_resource::<FunPipelineRegistry>();
        render_app.init_resource::<crate::fun_lux::FunLuxLightDatabase>();
        render_app.init_resource::<FunSceneManifestRegistry>();
        render_app.init_resource::<FunViewportRegistry>();
        render_app.add_message::<FunRendererEcsEvent>();
        render_app.add_message::<crate::fun_lux::FunLuxLightEvent>();
        render_app.init_resource::<FunEntityRenderStrategyRegistry>();
        render_app.insert_resource(rt_features);
        render_app.add_systems(
            RenderStartup,
            log_rt_backend_fallbacks
                .after(init_gpu_resource::<RenderBackendCapabilities>)
                .ambiguous_with_all(),
        );
    }
}

fn render_path_config_from_env() -> (ClientRenderConfig, SolariSettings, SolariRuntimeParams) {
    let mut render_config = ClientRenderConfig::from_env();
    let solari_settings = solari_settings_from_env();
    let solari_runtime_params = solari_runtime_params_from_env(&solari_settings);
    if solari_settings.denoise_mode != SolariDenoiseMode::DlssRayReconstruction {
        render_config.dlss_rr_disabled_by_denoise_mode = render_config.dlss_rr_enabled;
        render_config.dlss_rr_enabled = false;
    }
    (render_config, solari_settings, solari_runtime_params)
}

fn log_fun_render_path(
    render_config: &ClientRenderConfig,
    solari_settings: &SolariSettings,
    solari_runtime_params: &SolariRuntimeParams,
) {
    if render_config.solari_enabled {
        info!("[fun render] Solari lighting will start after the streamed world is ready");
    } else {
        info!("[fun render] Solari lighting disabled by FUN_DISABLE_SOLARI");
    }
    if render_config.meshlets_enabled {
        info!("[fun render] streamed world will use meshlet meshes");
    } else {
        info!("[fun render] streamed world meshlets disabled by FUN_DISABLE_MESHLETS");
    }
    if render_config.clouds_enabled {
        info!(
            "[fun render] volumetric clouds enabled: profile={} quality={} scale={} temporal={} shadows={}",
            render_config.cloud_profile_id.as_str(),
            render_config.cloud_quality.as_env_value(),
            render_config.cloud_internal_scale.as_env_value(),
            render_config.cloud_temporal_enabled,
            render_config.cloud_shadows_enabled
        );
    } else {
        info!("[fun render] volumetric clouds disabled");
    }
    info!(
        "[fun render] Solari denoise mode: {:?}",
        solari_settings.denoise_mode
    );
    info!(
        "[fun render] Solari internal GI scale: {:?}",
        solari_settings.internal_scale
    );
    info!(
        "[fun render] Solari world-cache: {} entries, {} updates/frame soft cap, {} frame slices, camera tiers {}m/{}m/{}m",
        solari_settings.world_cache_size,
        solari_settings.world_cache_cell_updates_soft_cap,
        solari_settings.world_cache_frame_slice_count,
        solari_settings.world_cache_near_camera_distance_meters,
        solari_settings.world_cache_mid_camera_distance_meters,
        solari_settings.world_cache_far_camera_distance_meters
    );
    info!(
        "[fun render] Solari architecture: {:?}, visual target: {:?}, target_fps={}, frame_budget_ns={}, gpu_budget_ns={}",
        solari_runtime_params.architecture,
        solari_runtime_params.visual_target,
        solari_runtime_params.target_fps,
        solari_runtime_params.frame_budget_ns,
        solari_runtime_params.gpu_budget_ns
    );
    info!(
        target: "fun::render",
        solari_enabled = render_config.solari_enabled,
        meshlets_enabled = render_config.meshlets_enabled,
        dlss_rr_enabled = render_config.dlss_rr_enabled,
        denoise_mode = ?solari_settings.denoise_mode,
        internal_scale = ?solari_settings.internal_scale,
        world_cache_size = solari_settings.world_cache_size,
        world_cache_updates_soft_cap = solari_settings.world_cache_cell_updates_soft_cap,
        world_cache_frame_slice_count = solari_settings.world_cache_frame_slice_count,
        world_cache_near_meters = solari_settings.world_cache_near_camera_distance_meters,
        world_cache_mid_meters = solari_settings.world_cache_mid_camera_distance_meters,
        world_cache_far_meters = solari_settings.world_cache_far_camera_distance_meters,
        solari_architecture = ?solari_runtime_params.architecture,
        solari_visual_target = ?solari_runtime_params.visual_target,
        solari_target_fps = solari_runtime_params.target_fps,
        solari_frame_budget_ns = solari_runtime_params.frame_budget_ns,
        solari_gpu_budget_ns = solari_runtime_params.gpu_budget_ns,
        solari_quality_level = solari_runtime_params.quality_level,
        solari_cache_update_budget = solari_runtime_params.cache_update_budget,
        solari_specular_refresh_budget = solari_runtime_params.specular_refresh_budget,
        solari_debug_overlay = ?solari_runtime_params.debug_overlay,
        cloud_enabled = render_config.clouds_enabled,
        cloud_quality = render_config.cloud_quality.as_env_value(),
        cloud_internal_scale = render_config.cloud_internal_scale.as_env_value(),
        cloud_temporal_enabled = render_config.cloud_temporal_enabled,
        cloud_shadows_enabled = render_config.cloud_shadows_enabled,
        cloud_profile_id = render_config.cloud_profile_id.as_str(),
        cloud_debug_overlay = render_config.cloud_debug_overlay.as_env_value(),
        geometry_policy = ?render_config.geometry_policy,
        meshlet_min_triangles = render_config.meshlet_min_triangles,
        rt_feature_hash = %format_args!("{:016x}", render_config.rt_features.rt_feature_hash()),
        rt_sample_direct = render_config.rt_features.sample_direct,
        rt_sample_indirect = render_config.rt_features.sample_indirect,
        rt_sample_reflections = render_config.rt_features.sample_reflections,
        rt_surface_cache = render_config.rt_features.surface_cache,
        rt_megageom = render_config.rt_features.megageom.as_env_value(),
        rt_opacity_mask = render_config.rt_features.opacity_mask.as_env_value(),
        rt_hair = render_config.rt_features.hair.as_env_value(),
        rt_async_readback = render_config.rt_features.async_readback,
        rt_validation = render_config.rt_features.validation,
        rt_vendor_emulation = render_config.rt_features.vendor_emulation.as_env_value(),
        "Fun render configuration"
    );
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    game_shared::fun_diag_info!(
        target: "fun::render",
        render_profile_verbose = render_config.render_profile_verbose,
        fps_overlay_enabled = render_config.fps_overlay_enabled,
        "Fun render diagnostic configuration"
    );
}

fn selected_opaque_renderer(render_config: &ClientRenderConfig) -> ClientOpaqueRenderer {
    if render_config.solari_enabled {
        info!("[fun render] default opaque renderer: deferred");
        ClientOpaqueRenderer::Deferred
    } else {
        info!("[fun render] default opaque renderer: forward");
        ClientOpaqueRenderer::Forward
    }
}

fn emit_render_path_signature(options: &FunRenderAppOptions, signature: &RenderPathSignature) {
    info!(
        target: "fun::render",
        signature_id = signature.id,
        renderer_id = signature.renderer_id,
        runtime_mode = options.runtime_mode,
        render_profile = signature.render_profile.as_env_value(),
        hosted_by_editor = options.hosted_by_editor,
        solari_enabled = signature.solari_enabled,
        meshlets_enabled = signature.meshlets_enabled,
        clouds_enabled = signature.clouds_enabled,
        cloud_quality = signature.cloud_quality,
        cloud_internal_scale = signature.cloud_internal_scale,
        cloud_profile_id = signature.cloud_profile_id,
        cloud_temporal_enabled = signature.cloud_temporal_enabled,
        cloud_shadows_enabled = signature.cloud_shadows_enabled,
        cloud_debug_overlay = signature.cloud_debug_overlay,
        cloud_shader_profile_flags = signature.cloud_shader_profile_flags,
        dlss_rr_enabled = signature.dlss_rr_enabled,
        geometry_policy = ?signature.geometry_policy,
        meshlet_min_triangles = signature.meshlet_min_triangles,
        rt_feature_hash = %format_args!("{:016x}", signature.rt_feature_hash),
        opaque_renderer = signature.opaque_renderer.as_str(),
        render_target_format = signature.render_target_format,
        internal_scale = signature.internal_scale,
        catalog_version = signature.catalog_version,
        "FunRenderPathSignature"
    );
}

fn log_rt_backend_fallbacks(
    rt_features: Res<FunRenderRtFeatures>,
    capabilities: Res<RenderBackendCapabilities>,
) {
    rt_features.log_backend_fallbacks(&capabilities);
}

pub fn enable_solari_lighting_for_ready_world(
    commands: &mut Commands,
    render_config: &ClientRenderConfig,
    solari_cameras: &Query<
        Entity,
        (
            With<Camera3d>,
            Without<bevy::solari::prelude::SolariLighting>,
        ),
    >,
    solari_lighting: &mut Query<&mut bevy::solari::prelude::SolariLighting>,
    solari_reset_events: &mut MessageWriter<bevy::solari::prelude::SolariResetEvent>,
) {
    if !render_config.solari_enabled {
        info!("[fun render] streamed world ready; Solari remains disabled");
        info!(target: "fun::solari", "streamed world ready; Solari remains disabled");
        return;
    }

    let mut enabled_count = 0usize;
    for camera_entity in solari_cameras.iter() {
        commands.entity(camera_entity).insert((
            CameraMainTextureUsages::default().with(TextureUsages::STORAGE_BINDING),
            bevy::solari::prelude::SolariLighting::default(),
        ));
        enabled_count += 1;
    }

    if enabled_count > 0 {
        info!("[fun render] enabled Solari lighting for {enabled_count} ready camera view(s)");
        info!(
            target: "fun::solari",
            enabled_views = enabled_count,
            "enabled Solari lighting for ready world"
        );
    }

    request_solari_lighting_history_reset(
        "streamed world became ready",
        solari_reset_events,
        solari_lighting,
    );
}

pub fn request_solari_lighting_history_reset(
    reason: &str,
    solari_reset_events: &mut MessageWriter<bevy::solari::prelude::SolariResetEvent>,
    solari_lighting: &mut Query<&mut bevy::solari::prelude::SolariLighting>,
) {
    solari_reset_events.write(bevy::solari::prelude::SolariResetEvent {
        reason: std::borrow::Cow::Owned(reason.to_owned()),
    });
    info!("[fun render] requested Solari temporal history reset: {reason}");
    let reset_count = reset_solari_lighting_history(solari_lighting);
    info!(
        target: "fun::solari",
        render_solari_reset_requested = true,
        render_solari_reset_reason = reason,
        render_solari_reset_view_count = reset_count,
        render_solari_reset_resource_generation_before = tracing::field::Empty,
        render_solari_reset_resource_generation_after = tracing::field::Empty,
        "requested Solari temporal history reset"
    );
}

fn reset_solari_lighting_history(
    solari_lighting: &mut Query<&mut bevy::solari::prelude::SolariLighting>,
) -> usize {
    let mut reset_count = 0usize;
    for mut lighting in solari_lighting.iter_mut() {
        lighting.reset = true;
        reset_count += 1;
    }

    if reset_count > 0 {
        info!("[fun render] reset Solari temporal history for {reset_count} view(s)");
        info!(
            target: "fun::solari",
            render_solari_reset_applied = true,
            render_solari_reset_reason = "direct_component_reset",
            render_solari_reset_view_count = reset_count,
            render_solari_reset_resource_generation_before = tracing::field::Empty,
            render_solari_reset_resource_generation_after = tracing::field::Empty,
            reset_views = reset_count,
            "reset Solari temporal history"
        );
    }
    reset_count
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn draw_counters_split_phase_path_geometry_and_material() {
        let mut counters = FunDrawCallCounters::new(42);
        counters.record_draw(
            FunDrawCallRecord::new(
                FunRenderPhaseKind::MainOpaque,
                FunMaterialClass::OpaqueComplex,
                FunGeometryClass::StaticOpaqueDense,
                FunRenderPath::MeshletStaticDense,
                FunDrawSubmissionKind::Indirect,
                4,
            )
            .with_instances(10, 6),
        );
        counters.record_draw(
            FunDrawCallRecord::new(
                FunRenderPhaseKind::Transparent,
                FunMaterialClass::Transparent,
                FunGeometryClass::Decal,
                FunRenderPath::StandardRaster,
                FunDrawSubmissionKind::Direct,
                2,
            )
            .with_instances(3, 3),
        );

        assert_eq!(counters.frame_index, 42);
        assert_eq!(counters.total_draws, 6);
        assert_eq!(counters.direct_draws, 2);
        assert_eq!(counters.indirect_draws, 4);
        assert_eq!(counters.draws_by_phase[&FunRenderPhaseKind::MainOpaque], 4);
        assert_eq!(
            counters.draws_by_render_path[&FunRenderPath::MeshletStaticDense],
            4
        );
        assert_eq!(
            counters.draws_by_geometry_class[&FunGeometryClass::StaticOpaqueDense],
            4
        );
        assert_eq!(
            counters.draws_by_material_class[&FunMaterialClass::Transparent],
            2
        );
        assert_eq!(counters.submitted_instances, 13);
        assert_eq!(counters.visible_instances, 9);
        assert_eq!(counters.culled_instances, 4);
        assert_eq!(
            counters.worst_phase(),
            Some((FunRenderPhaseKind::MainOpaque, 4))
        );
    }
}
