use bevy::{
    core_pipeline::{Core3d, Core3dSystems},
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::ExtractResourcePlugin,
        render_resource::{SpecializedRenderPipelines, TextureUsages},
    },
};
use tracing::{info, warn};

use super::{
    config::FunCloudSettings,
    render::{
        node::{render_clouds_to_view, run_cloud_compute_passes},
        pipelines::{
            FunCloudViewCompositePipeline, init_cloud_pipelines, load_cloud_shader_assets,
            prepare_cloud_view_pipelines,
        },
        prepare::{FunCloudGpuTextures, prepare_cloud_textures},
    },
    weather::{FunWeatherProfileId, FunWeatherState},
};

#[derive(Debug, Clone)]
pub struct FunSkyPlugin {
    settings: FunCloudSettings,
}

impl FunSkyPlugin {
    pub const fn new(settings: FunCloudSettings) -> Self {
        Self { settings }
    }
}

impl Default for FunSkyPlugin {
    fn default() -> Self {
        Self {
            settings: FunCloudSettings::from_env(),
        }
    }
}

impl Plugin for FunSkyPlugin {
    fn build(&self, app: &mut App) {
        load_cloud_shader_assets(app);

        let profile_id = self.settings.profile_id;
        let weather_state = FunWeatherState::from_profile_id(profile_id).unwrap_or_else(|error| {
            warn!(
                target: "fun::weather",
                profile_id = profile_id.as_str(),
                ?error,
                "cloud profile is not available; falling back to scattered"
            );
            FunWeatherState::from_profile_id(FunWeatherProfileId::Scattered)
                .expect("scattered cloud profile is a validated built-in")
        });

        app.insert_resource(self.settings)
            .insert_resource(weather_state)
            .add_plugins((
                ExtractResourcePlugin::<FunCloudSettings>::default(),
                ExtractResourcePlugin::<FunWeatherState>::default(),
            ))
            .add_message::<FunCloudHistoryResetEvent>()
            .add_systems(Startup, log_cloud_startup);
    }

    fn finish(&self, app: &mut App) {
        let weather_state = app.world().get_resource::<FunWeatherState>().copied();
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };

        render_app.insert_resource(self.settings);
        if let Some(weather_state) = weather_state {
            render_app.insert_resource(weather_state);
        }

        render_app
            .init_resource::<FunCloudGpuTextures>()
            .init_resource::<SpecializedRenderPipelines<FunCloudViewCompositePipeline>>()
            .add_systems(RenderStartup, init_cloud_pipelines)
            .add_systems(
                Render,
                (
                    configure_cloud_camera_depth_usages.in_set(RenderSystems::PrepareViews),
                    prepare_cloud_view_pipelines.in_set(RenderSystems::Prepare),
                    prepare_cloud_textures.in_set(RenderSystems::PrepareResources),
                ),
            )
            .add_systems(
                Core3d,
                (
                    run_cloud_compute_passes
                        .in_set(Core3dSystems::EarlyPostProcess)
                        .after(Core3dSystems::MainPass),
                    render_clouds_to_view
                        .in_set(Core3dSystems::EarlyPostProcess)
                        .after(run_cloud_compute_passes),
                ),
            );
    }
}

#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunCloudHistoryResetEvent {
    pub reason: FunCloudHistoryResetReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FunCloudHistoryResetReason {
    CameraCut,
    LargeFovChange,
    WorldStreamReset,
    WorldRevisionChanged,
    LevelChanged,
    SceneLightingChanged,
    WeatherProfileJump,
    LightningDiscontinuity,
    OperatorRequested,
}

impl FunCloudHistoryResetReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CameraCut => "camera_cut",
            Self::LargeFovChange => "large_fov_change",
            Self::WorldStreamReset => "world_stream_reset",
            Self::WorldRevisionChanged => "world_revision_changed",
            Self::LevelChanged => "level_changed",
            Self::SceneLightingChanged => "scene_lighting_changed",
            Self::WeatherProfileJump => "weather_profile_jump",
            Self::LightningDiscontinuity => "lightning_discontinuity",
            Self::OperatorRequested => "operator_requested",
        }
    }
}

pub fn request_cloud_history_reset(
    reason: FunCloudHistoryResetReason,
    reset_events: &mut MessageWriter<FunCloudHistoryResetEvent>,
) {
    reset_events.write(FunCloudHistoryResetEvent { reason });
    info!(
        target: "fun::render::clouds",
        cloud_history_reset_requested = true,
        cloud_history_reset_reason = reason.as_str(),
        "requested cloud temporal history reset"
    );
}

fn log_cloud_startup(settings: Res<FunCloudSettings>, state: Res<FunWeatherState>) {
    info!(
        target: "fun::render::clouds",
        cloud_enabled = settings.enabled,
        cloud_quality = settings.quality.as_env_value(),
        cloud_internal_scale = settings.internal_scale.as_env_value(),
        cloud_temporal_enabled = settings.temporal_enabled,
        cloud_shadows_enabled = settings.shadows_enabled,
        cloud_profile_id = settings.profile_id.as_str(),
        cloud_debug_overlay = settings.debug_overlay.as_env_value(),
        cloud_primary_steps = settings.quality.primary_step_count(),
        cloud_light_steps = settings.quality.light_step_count(),
        cloud_coverage = state.profile.cloud_coverage,
        cloud_density = state.profile.cloud_density,
        "Fun cloud configuration"
    );
}

fn configure_cloud_camera_depth_usages(
    settings: Option<Res<FunCloudSettings>>,
    mut cameras: Query<&mut Camera3d>,
) {
    let Some(settings) = settings.as_deref() else {
        return;
    };
    if !settings.enabled || settings.quality == super::config::FunCloudQuality::Off {
        return;
    }

    for mut camera in &mut cameras {
        camera.depth_texture_usages.0 |= TextureUsages::TEXTURE_BINDING.bits();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloud_history_reset_reason_is_stable_for_diagnostics() {
        assert_eq!(
            FunCloudHistoryResetReason::WorldRevisionChanged.as_str(),
            "world_revision_changed"
        );
    }
}
