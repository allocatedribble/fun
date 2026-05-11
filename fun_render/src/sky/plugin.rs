use hashbrown::HashMap;

use bevy::{
    core_pipeline::{Core3d, Core3dSystems},
    prelude::*,
    render::{
        Render, RenderApp, RenderStartup, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::{SpecializedRenderPipelines, TextureUsages},
    },
};
use tracing::{info, warn};

use crate::world_stream::RenderWorldContext;

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
            .init_resource::<FunCloudHistoryState>()
            .init_resource::<FunCloudCameraResetTracker>()
            .init_resource::<FunCloudWeatherResetTracker>()
            .init_resource::<FunCloudWorldResetTracker>()
            .init_resource::<FunCloudSceneLightState>()
            .add_plugins((
                ExtractResourcePlugin::<FunCloudSettings>::default(),
                ExtractResourcePlugin::<FunWeatherState>::default(),
                ExtractResourcePlugin::<FunCloudHistoryState>::default(),
                ExtractResourcePlugin::<FunCloudSceneLightState>::default(),
            ))
            .add_message::<FunCloudHistoryResetEvent>()
            .add_systems(Startup, log_cloud_startup)
            .add_systems(
                Update,
                (
                    advance_cloud_history_age,
                    sync_cloud_scene_light_state,
                    detect_cloud_world_context_resets,
                    detect_cloud_weather_profile_jumps,
                    detect_cloud_camera_history_resets,
                    consume_cloud_history_reset_events,
                )
                    .chain(),
            );
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
        render_app.init_resource::<FunCloudHistoryState>();
        render_app.init_resource::<FunCloudSceneLightState>();

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct FunCloudHistoryState {
    generation: u32,
    reset_count: u64,
    frames_since_reset: u32,
    last_reason: Option<FunCloudHistoryResetReason>,
}

impl Default for FunCloudHistoryState {
    fn default() -> Self {
        Self {
            generation: 1,
            reset_count: 0,
            frames_since_reset: 0,
            last_reason: None,
        }
    }
}

impl FunCloudHistoryState {
    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub const fn reset_count(self) -> u64 {
        self.reset_count
    }

    pub const fn frames_since_reset(self) -> u32 {
        self.frames_since_reset
    }

    pub const fn last_reason(self) -> Option<FunCloudHistoryResetReason> {
        self.last_reason
    }

    fn record_reset(&mut self, reason: FunCloudHistoryResetReason) {
        self.generation = self.generation.wrapping_add(1).max(1);
        self.reset_count = self.reset_count.saturating_add(1);
        self.frames_since_reset = 0;
        self.last_reason = Some(reason);
    }
}

impl ExtractResource for FunCloudHistoryState {
    type Source = FunCloudHistoryState;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Resource)]
pub struct FunCloudSceneLightState {
    pub direction_to_light: Vec3,
    pub illuminance_lux: f32,
}

impl Default for FunCloudSceneLightState {
    fn default() -> Self {
        Self {
            direction_to_light: Vec3::new(0.408_248_28, 0.816_496_55, 0.408_248_28),
            illuminance_lux: 15_000.0,
        }
    }
}

impl ExtractResource for FunCloudSceneLightState {
    type Source = FunCloudSceneLightState;

    fn extract_resource(source: &Self::Source) -> Self {
        *source
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

fn consume_cloud_history_reset_events(
    mut reset_events: MessageReader<FunCloudHistoryResetEvent>,
    mut history: ResMut<FunCloudHistoryState>,
) {
    for event in reset_events.read() {
        let previous_generation = history.generation();
        history.record_reset(event.reason);
        info!(
            target: "fun::render::clouds",
            cloud_history_reset_applied = true,
            cloud_history_reset_reason = event.reason.as_str(),
            cloud_history_reset_generation_before = previous_generation,
            cloud_history_reset_generation_after = history.generation(),
            cloud_history_reset_count = history.reset_count(),
            "applied cloud temporal history reset"
        );
    }
}

fn advance_cloud_history_age(
    settings: Option<Res<FunCloudSettings>>,
    mut history: ResMut<FunCloudHistoryState>,
) {
    let Some(settings) = settings.as_deref() else {
        return;
    };
    if !settings.enabled || settings.quality == super::config::FunCloudQuality::Off {
        return;
    }
    history.frames_since_reset = history.frames_since_reset.saturating_add(1);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FunCloudWorldResetSnapshot {
    level_hash: u64,
    revision: Option<u64>,
    manifest_signature: u64,
}

#[derive(Resource, Default)]
struct FunCloudWorldResetTracker {
    previous: Option<FunCloudWorldResetSnapshot>,
}

fn detect_cloud_world_context_resets(
    settings: Option<Res<FunCloudSettings>>,
    render_world: Option<Res<RenderWorldContext>>,
    mut tracker: ResMut<FunCloudWorldResetTracker>,
    mut reset_events: MessageWriter<FunCloudHistoryResetEvent>,
) {
    let Some(settings) = settings.as_deref() else {
        return;
    };
    if !settings.enabled || settings.quality == super::config::FunCloudQuality::Off {
        tracker.previous = None;
        return;
    }

    let Some(render_world) = render_world.as_deref() else {
        tracker.previous = None;
        return;
    };
    let current = FunCloudWorldResetSnapshot {
        level_hash: stable_optional_level_hash(render_world.level_id.as_deref()),
        revision: render_world.revision.map(|revision| revision.0),
        manifest_signature: render_world.manifest_signature,
    };

    if let Some(previous) = tracker.previous {
        if previous.revision.is_some() && current.revision.is_none() {
            request_cloud_history_reset(
                FunCloudHistoryResetReason::WorldStreamReset,
                &mut reset_events,
            );
        } else if previous.level_hash != current.level_hash {
            request_cloud_history_reset(
                FunCloudHistoryResetReason::LevelChanged,
                &mut reset_events,
            );
        } else if previous.revision != current.revision
            || previous.manifest_signature != current.manifest_signature
        {
            request_cloud_history_reset(
                FunCloudHistoryResetReason::WorldRevisionChanged,
                &mut reset_events,
            );
        }
    }

    tracker.previous = Some(current);
}

fn stable_optional_level_hash(level_id: Option<&str>) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325u64;
    for byte in level_id.unwrap_or_default().as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

#[derive(Debug, Clone, Copy)]
struct FunCloudWeatherResetSnapshot {
    to_profile_id: FunWeatherProfileId,
    weather_seed: u64,
    blend_alpha: f32,
}

#[derive(Resource, Default)]
struct FunCloudWeatherResetTracker {
    previous: Option<FunCloudWeatherResetSnapshot>,
}

fn detect_cloud_weather_profile_jumps(
    settings: Option<Res<FunCloudSettings>>,
    weather_state: Option<Res<FunWeatherState>>,
    mut tracker: ResMut<FunCloudWeatherResetTracker>,
    mut reset_events: MessageWriter<FunCloudHistoryResetEvent>,
) {
    let Some(settings) = settings.as_deref() else {
        return;
    };
    if !settings.enabled || settings.quality == super::config::FunCloudQuality::Off {
        tracker.previous = None;
        return;
    }
    let Some(weather_state) = weather_state.as_deref() else {
        tracker.previous = None;
        return;
    };

    let current = FunCloudWeatherResetSnapshot {
        to_profile_id: weather_state.to_profile_id,
        weather_seed: weather_state.weather_seed,
        blend_alpha: weather_state.blend_alpha,
    };

    if let Some(previous) = tracker.previous {
        let settled_profile_changed = previous.to_profile_id != current.to_profile_id
            && previous.blend_alpha >= 0.999
            && current.blend_alpha >= 0.999;
        let seed_jump =
            previous.weather_seed != current.weather_seed && current.blend_alpha >= 0.999;
        if settled_profile_changed || seed_jump {
            request_cloud_history_reset(
                FunCloudHistoryResetReason::WeatherProfileJump,
                &mut reset_events,
            );
        }
    }

    tracker.previous = Some(current);
}

#[derive(Debug, Clone, Copy)]
struct FunCloudCameraResetSnapshot {
    translation: Vec3,
    rotation: Quat,
    projection_signature: [f32; 4],
    fov: Option<f32>,
}

#[derive(Resource, Default)]
struct FunCloudCameraResetTracker {
    cameras: HashMap<Entity, FunCloudCameraResetSnapshot>,
}

fn detect_cloud_camera_history_resets(
    settings: Option<Res<FunCloudSettings>>,
    cameras: Query<(Entity, &Projection, &GlobalTransform), With<Camera3d>>,
    mut tracker: ResMut<FunCloudCameraResetTracker>,
    mut reset_events: MessageWriter<FunCloudHistoryResetEvent>,
) {
    let Some(settings) = settings.as_deref() else {
        return;
    };
    if !settings.enabled || settings.quality == super::config::FunCloudQuality::Off {
        tracker.cameras.clear();
        return;
    }

    for (entity, projection, transform) in &cameras {
        let current = camera_reset_snapshot(projection, transform);
        if let Some(previous) = tracker.cameras.get(&entity).copied() {
            if is_large_projection_change(previous, current) {
                request_cloud_history_reset(
                    FunCloudHistoryResetReason::LargeFovChange,
                    &mut reset_events,
                );
            } else if is_camera_cut(previous, current) {
                request_cloud_history_reset(
                    FunCloudHistoryResetReason::CameraCut,
                    &mut reset_events,
                );
            }
        }
        tracker.cameras.insert(entity, current);
    }
}

fn camera_reset_snapshot(
    projection: &Projection,
    transform: &GlobalTransform,
) -> FunCloudCameraResetSnapshot {
    let transform = transform.compute_transform();
    let clip = projection.get_clip_from_view();
    FunCloudCameraResetSnapshot {
        translation: transform.translation,
        rotation: transform.rotation,
        projection_signature: [clip.x_axis.x, clip.y_axis.y, clip.z_axis.z, clip.w_axis.z],
        fov: match projection {
            Projection::Perspective(perspective) => Some(perspective.fov),
            Projection::Orthographic(_) | Projection::Custom(_) => None,
        },
    }
}

fn is_large_projection_change(
    previous: FunCloudCameraResetSnapshot,
    current: FunCloudCameraResetSnapshot,
) -> bool {
    if let (Some(previous_fov), Some(current_fov)) = (previous.fov, current.fov) {
        return (previous_fov - current_fov).abs() > 0.08;
    }

    previous
        .projection_signature
        .into_iter()
        .zip(current.projection_signature)
        .any(|(previous, current)| (previous - current).abs() > 0.02)
}

fn is_camera_cut(
    previous: FunCloudCameraResetSnapshot,
    current: FunCloudCameraResetSnapshot,
) -> bool {
    let moved_far = previous.translation.distance_squared(current.translation) > 250.0 * 250.0;
    let rotated_far = previous.rotation.angle_between(current.rotation) > 1.65;
    moved_far || rotated_far
}

fn sync_cloud_scene_light_state(
    settings: Option<Res<FunCloudSettings>>,
    lights: Query<(&DirectionalLight, &GlobalTransform)>,
    mut state: ResMut<FunCloudSceneLightState>,
    mut reset_events: MessageWriter<FunCloudHistoryResetEvent>,
) {
    let Some(settings) = settings.as_deref() else {
        return;
    };
    if !settings.enabled || settings.quality == super::config::FunCloudQuality::Off {
        return;
    }

    let Some((light, transform)) = lights
        .iter()
        .max_by(|(left, _), (right, _)| left.illuminance.total_cmp(&right.illuminance))
    else {
        return;
    };
    let direction_to_light: Vec3 = transform.back().into();
    let next = FunCloudSceneLightState {
        direction_to_light: direction_to_light.normalize_or_zero(),
        illuminance_lux: light.illuminance,
    };
    let direction_changed = state.direction_to_light.dot(next.direction_to_light) < 0.94;
    let illuminance_delta = (state.illuminance_lux - next.illuminance_lux).abs();
    let illuminance_changed = illuminance_delta > state.illuminance_lux.max(1.0) * 0.25;

    if direction_changed || illuminance_changed {
        request_cloud_history_reset(
            FunCloudHistoryResetReason::SceneLightingChanged,
            &mut reset_events,
        );
    }
    *state = next;
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

    #[test]
    fn cloud_history_state_records_monotonic_generation_and_count() {
        let mut state = FunCloudHistoryState::default();

        state.record_reset(FunCloudHistoryResetReason::OperatorRequested);
        state.record_reset(FunCloudHistoryResetReason::WeatherProfileJump);

        assert_eq!(state.generation(), 3);
        assert_eq!(state.reset_count(), 2);
        assert_eq!(state.frames_since_reset(), 0);
        assert_eq!(
            state.last_reason(),
            Some(FunCloudHistoryResetReason::WeatherProfileJump)
        );
    }

    #[test]
    fn cloud_camera_reset_detection_separates_fov_change_from_normal_motion() {
        let previous = FunCloudCameraResetSnapshot {
            translation: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            projection_signature: [1.0, 1.0, 0.0, 0.1],
            fov: Some(1.0),
        };
        let normal_motion = FunCloudCameraResetSnapshot {
            translation: Vec3::new(1.0, 0.0, 0.0),
            rotation: Quat::from_rotation_y(0.05),
            projection_signature: [1.0, 1.0, 0.0, 0.1],
            fov: Some(1.02),
        };
        let fov_jump = FunCloudCameraResetSnapshot {
            fov: Some(1.2),
            ..normal_motion
        };

        assert!(!is_camera_cut(previous, normal_motion));
        assert!(!is_large_projection_change(previous, normal_motion));
        assert!(is_large_projection_change(previous, fov_jump));
    }

    #[test]
    fn cloud_world_level_hash_is_stable_and_distinguishes_levels() {
        assert_eq!(
            stable_optional_level_hash(Some("arena")),
            stable_optional_level_hash(Some("arena"))
        );
        assert_ne!(
            stable_optional_level_hash(Some("arena")),
            stable_optional_level_hash(Some("editor-preview"))
        );
    }
}
