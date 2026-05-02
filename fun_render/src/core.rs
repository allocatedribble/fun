use bevy::{
    camera::CameraMainTextureUsages,
    pbr::experimental::meshlet::MeshletPlugin,
    prelude::*,
    render::render_resource::TextureUsages,
    solari::prelude::{SolariDenoiseMode, SolariPlugins, SolariRuntimeParams, SolariSettings},
};
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use bevy::{
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin},
    render::diagnostic::RenderDiagnosticsPlugin,
};
use tracing::info;
#[cfg(not(all(feature = "render_diagnostics", debug_assertions)))]
use tracing::warn;

use crate::{
    ClientOpaqueRenderer, ClientRenderConfig, FunRenderAppOptions, RenderPathSignature, lighting,
    prewarm_world_render_catalog, render_path_signature_for_options,
    solari::{solari_runtime_params_from_env, solari_settings_from_env},
};

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
    let opaque_renderer = selected_opaque_renderer(&render_config);
    let signature = render_path_signature_for_options(
        options,
        &render_config,
        &solari_settings,
        opaque_renderer,
    );
    emit_render_path_signature(options, &signature);

    let solari_enabled = render_config.solari_enabled;
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    let fps_overlay_enabled = render_config.fps_overlay_enabled;

    app.insert_resource(opaque_renderer.method())
        .insert_resource(signature)
        .insert_resource(render_config)
        .insert_resource(solari_settings)
        .insert_resource(solari_runtime_params)
        .add_message::<bevy::solari::prelude::SolariResetEvent>()
        .add_plugins(MeshletPlugin {
            cluster_buffer_slots: 1 << 14,
        })
        .add_systems(
            Startup,
            (lighting::setup_lighting, prewarm_world_render_catalog).chain(),
        );

    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
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
        geometry_policy = ?render_config.geometry_policy,
        meshlet_min_triangles = render_config.meshlet_min_triangles,
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
        dlss_rr_enabled = signature.dlss_rr_enabled,
        geometry_policy = ?signature.geometry_policy,
        meshlet_min_triangles = signature.meshlet_min_triangles,
        opaque_renderer = signature.opaque_renderer.as_str(),
        render_target_format = signature.render_target_format,
        internal_scale = signature.internal_scale,
        catalog_version = signature.catalog_version,
        "FunRenderPathSignature"
    );
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
