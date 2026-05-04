use bevy::{
    camera::{CameraUpdateSystems, MainPassResolutionOverride},
    core_pipeline::prepass::{
        DepthPrepass, MotionVectorPrepass, PreviousViewUniformOffset, ViewPrepassTextures,
    },
    prelude::*,
    render::{
        camera::{CameraMainPassTextureFormats, MipBias, TemporalJitter},
        error_handler::{ErrorType, RenderRecoveryStatus},
        render_resource::TextureFormat,
        view::Msaa,
    },
    window::WindowEvent,
};
use tracing::{info, warn};

use crate::{
    ClientRenderConfig, Dx12NativeDlssSrRuntimeMode, Dx12NativeDlssSrSupport, NativeDlssMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component, Default, Reflect)]
#[reflect(Component, Default, Clone)]
#[require(TemporalJitter, MipBias, DepthPrepass, MotionVectorPrepass)]
pub struct Dx12NativeDlssCamera {
    pub mode: Option<NativeDlssMode>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component, Default, Reflect)]
#[reflect(Component, Default, Clone)]
pub struct Dx12NativeDlssCameraRuntimeState {
    pub runtime_mode: Dx12NativeDlssSrRuntimeMode,
    pub input_resolution: UVec2,
    pub output_resolution: UVec2,
    pub support_ready: bool,
}

#[derive(Debug, Clone, Copy, Component, Reflect)]
#[reflect(Component, Default, Clone)]
pub struct Dx12DlssPreviousViewProjection {
    pub current_view_projection: Mat4,
    pub previous_view_projection: Mat4,
    pub current_non_jittered_projection: Mat4,
    pub previous_non_jittered_projection: Mat4,
    pub current_jitter: Vec2,
    pub previous_jitter: Vec2,
    pub valid_previous: bool,
}

impl Default for Dx12DlssPreviousViewProjection {
    fn default() -> Self {
        Self {
            current_view_projection: Mat4::IDENTITY,
            previous_view_projection: Mat4::IDENTITY,
            current_non_jittered_projection: Mat4::IDENTITY,
            previous_non_jittered_projection: Mat4::IDENTITY,
            current_jitter: Vec2::ZERO,
            previous_jitter: Vec2::ZERO,
            valid_previous: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DlssCameraValidation {
    pub camera: Entity,
    pub has_depth: bool,
    pub has_motion_vectors: bool,
    pub has_jitter: bool,
    pub has_previous_view_projection: bool,
    pub msaa_disabled: bool,
    pub format_supported: bool,
    pub resolution_supported: bool,
    pub history_valid: bool,
}

impl DlssCameraValidation {
    pub const fn ready(self) -> bool {
        self.has_depth
            && self.has_motion_vectors
            && self.has_jitter
            && self.has_previous_view_projection
            && self.msaa_disabled
            && self.format_supported
            && self.resolution_supported
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]
#[reflect(Resource, Default, Clone)]
pub struct DlssHistoryReset {
    pub reset_this_frame: bool,
    pub reason: DlssResetReason,
    pub generation: u64,
}

impl Default for DlssHistoryReset {
    fn default() -> Self {
        Self {
            reset_this_frame: true,
            reason: DlssResetReason::Startup,
            generation: 0,
        }
    }
}

impl DlssHistoryReset {
    pub fn request(&mut self, reason: DlssResetReason) {
        self.reset_this_frame = true;
        self.reason = reason;
        self.generation = self.generation.saturating_add(1);
    }

    pub fn clear_after_consume(&mut self) {
        self.reset_this_frame = false;
        self.reason = DlssResetReason::None;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum DlssResetReason {
    #[default]
    None,
    Startup,
    WindowResize,
    FullscreenToggle,
    RenderScaleChange,
    DlssModeChange,
    CameraTeleport,
    SceneLoad,
    EditorPreviewSceneSwitch,
    StreamedWorldHardSwap,
    BackendRestart,
    DeviceRecreation,
    SolariHistoryReset,
    CameraCut,
    MenuWorldRenderingChange,
    Manual,
}

impl DlssResetReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Startup => "startup",
            Self::WindowResize => "window_resize",
            Self::FullscreenToggle => "fullscreen_toggle",
            Self::RenderScaleChange => "render_scale_change",
            Self::DlssModeChange => "dlss_mode_change",
            Self::CameraTeleport => "camera_teleport",
            Self::SceneLoad => "scene_load",
            Self::EditorPreviewSceneSwitch => "editor_preview_scene_switch",
            Self::StreamedWorldHardSwap => "streamed_world_hard_swap",
            Self::BackendRestart => "backend_restart",
            Self::DeviceRecreation => "device_recreation",
            Self::SolariHistoryReset => "solari_history_reset",
            Self::CameraCut => "camera_cut",
            Self::MenuWorldRenderingChange => "menu_world_rendering_change",
            Self::Manual => "manual",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DlssDepthConvention {
    ReversedZInfiniteFarNonLinear,
    Unknown,
}

impl DlssDepthConvention {
    pub const FUN_CAMERA: Self = Self::ReversedZInfiniteFarNonLinear;

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReversedZInfiniteFarNonLinear => "reversed_z_infinite_far_non_linear",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DlssMotionVectorDirection {
    CurrentMinusPrevious,
}

impl DlssMotionVectorDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CurrentMinusPrevious => "current_minus_previous",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum DlssMotionVectorUnits {
    NormalizedUvOffset,
}

impl DlssMotionVectorUnits {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NormalizedUvOffset => "normalized_uv_offset",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct DlssMotionVectorConvention {
    pub direction: DlssMotionVectorDirection,
    pub units: DlssMotionVectorUnits,
    pub low_resolution: bool,
    pub jitter_excluded: bool,
    pub camera_motion_included: bool,
    pub object_motion_included: bool,
}

impl DlssMotionVectorConvention {
    pub const BEVY_PREPASS: Self = Self {
        direction: DlssMotionVectorDirection::CurrentMinusPrevious,
        units: DlssMotionVectorUnits::NormalizedUvOffset,
        low_resolution: true,
        jitter_excluded: true,
        camera_motion_included: true,
        object_motion_included: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct DlssMipBiasState {
    pub mode: NativeDlssMode,
    pub internal_scale_factor: f32,
    pub computed_mip_bias: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Reflect)]
pub struct DlssDepthDiagnostics {
    pub convention: DlssDepthConvention,
    pub min_depth: Option<f32>,
    pub max_depth: Option<f32>,
    pub invalid_depth_count: Option<u64>,
}

impl Default for DlssDepthDiagnostics {
    fn default() -> Self {
        Self {
            convention: DlssDepthConvention::FUN_CAMERA,
            min_depth: None,
            max_depth: None,
            invalid_depth_count: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Resource, Reflect)]
#[reflect(Resource, Default, Clone)]
pub enum DlssDebugVisualization {
    #[default]
    Off,
    DepthPreview,
    DepthHistogram,
    MotionVectorHeatmap,
    ZeroVectorMask,
    OverlargeVectorMask,
    MovingObjectOverlay,
}

impl DlssDebugVisualization {
    pub fn from_env() -> Self {
        let Ok(value) = std::env::var("FUN_RENDER_DX12_DLSS_DEBUG_VIEW") else {
            return Self::Off;
        };
        Self::parse(&value).unwrap_or_else(|| {
            warn!(
                target: "fun::render::dlss",
                debug_view = value,
                "unknown FUN_RENDER_DX12_DLSS_DEBUG_VIEW; disabling DLSS debug visualization"
            );
            Self::Off
        })
    }

    pub fn parse(value: &str) -> Option<Self> {
        if value.eq_ignore_ascii_case("off") || value.eq_ignore_ascii_case("0") {
            return Some(Self::Off);
        }
        if value.eq_ignore_ascii_case("depth")
            || value.eq_ignore_ascii_case("depth_preview")
            || value.eq_ignore_ascii_case("depth-preview")
        {
            return Some(Self::DepthPreview);
        }
        if value.eq_ignore_ascii_case("depth_histogram")
            || value.eq_ignore_ascii_case("depth-histogram")
            || value.eq_ignore_ascii_case("histogram")
        {
            return Some(Self::DepthHistogram);
        }
        if value.eq_ignore_ascii_case("motion")
            || value.eq_ignore_ascii_case("motion_heatmap")
            || value.eq_ignore_ascii_case("motion-heatmap")
        {
            return Some(Self::MotionVectorHeatmap);
        }
        if value.eq_ignore_ascii_case("zero_vector_mask")
            || value.eq_ignore_ascii_case("zero-vector-mask")
            || value.eq_ignore_ascii_case("zero")
        {
            return Some(Self::ZeroVectorMask);
        }
        if value.eq_ignore_ascii_case("overlarge_vector_mask")
            || value.eq_ignore_ascii_case("overlarge-vector-mask")
            || value.eq_ignore_ascii_case("overlarge")
        {
            return Some(Self::OverlargeVectorMask);
        }
        if value.eq_ignore_ascii_case("moving_object_overlay")
            || value.eq_ignore_ascii_case("moving-object-overlay")
            || value.eq_ignore_ascii_case("moving")
        {
            return Some(Self::MovingObjectOverlay);
        }
        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Resource)]
struct LastDlssResetGeneration(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Resource)]
struct ObservedRenderRecovery {
    errors_seen: u64,
    recovery_successes: u64,
}

pub fn install_dlss_correctness(app: &mut App) {
    app.register_type::<Dx12NativeDlssCamera>()
        .register_type::<Dx12NativeDlssCameraRuntimeState>()
        .register_type::<Dx12DlssPreviousViewProjection>()
        .register_type::<DlssHistoryReset>()
        .register_type::<DlssDebugVisualization>()
        .init_resource::<DlssHistoryReset>()
        .init_resource::<LastDlssResetGeneration>()
        .init_resource::<ObservedRenderRecovery>()
        .insert_resource(DlssDebugVisualization::from_env())
        .add_systems(
            PostUpdate,
            (
                request_dlss_history_resets_from_events,
                ensure_dx12_native_dlss_camera_requirements,
                track_dx12_dlss_previous_view_projection,
                log_dlss_history_reset_debug,
            )
                .chain()
                .after(CameraUpdateSystems),
        );
}

pub fn validate_camera_for_dx12_dlss(camera: Entity, world: &World) -> DlssCameraValidation {
    let has_depth =
        world.get::<DepthPrepass>(camera).is_some() || prepass_textures_have_depth(camera, world);
    let has_motion_vectors = world.get::<MotionVectorPrepass>(camera).is_some()
        || prepass_textures_have_motion_vectors(camera, world);

    let has_jitter = world.get::<TemporalJitter>(camera).is_some_and(|jitter| {
        jitter.offset.is_finite() && jitter.offset.abs().cmple(Vec2::splat(0.5)).all()
    });

    let has_previous_view_projection = world
        .get::<Dx12DlssPreviousViewProjection>(camera)
        .is_some_and(|previous| previous.valid_previous)
        || world.get::<PreviousViewUniformOffset>(camera).is_some();

    let msaa_disabled = world
        .get::<Msaa>(camera)
        .is_some_and(|msaa| *msaa == Msaa::Off);

    let format_supported =
        known_main_pass_format(camera, world).is_none_or(is_dlss_supported_color_format);

    let resolution_supported = validate_dlss_resolution(camera, world).is_some_and(|resolution| {
        resolution.input.x < resolution.output.x
            && resolution.input.y < resolution.output.y
            && resolution.input.x > 0
            && resolution.input.y > 0
    });

    let history_valid = has_previous_view_projection
        && world
            .get_resource::<DlssHistoryReset>()
            .is_none_or(|reset| !reset.reset_this_frame);

    DlssCameraValidation {
        camera,
        has_depth,
        has_motion_vectors,
        has_jitter,
        has_previous_view_projection,
        msaa_disabled,
        format_supported,
        resolution_supported,
        history_valid,
    }
}

pub fn native_dlss_internal_scale_factor(mode: NativeDlssMode) -> f32 {
    match mode {
        NativeDlssMode::Quality => 2.0 / 3.0,
        NativeDlssMode::Balanced => 0.58,
        NativeDlssMode::Performance => 0.5,
        NativeDlssMode::UltraPerformance => 1.0 / 3.0,
    }
}

pub fn native_dlss_mip_bias(mode: NativeDlssMode) -> f32 {
    native_dlss_internal_scale_factor(mode).log2()
}

pub fn native_dlss_input_resolution(output: UVec2, mode: NativeDlssMode) -> UVec2 {
    let scale = native_dlss_internal_scale_factor(mode);
    UVec2::new(
        ((output.x as f32) * scale).round().max(1.0) as u32,
        ((output.y as f32) * scale).round().max(1.0) as u32,
    )
}

pub fn native_dlss_mip_bias_state(mode: NativeDlssMode) -> DlssMipBiasState {
    DlssMipBiasState {
        mode,
        internal_scale_factor: native_dlss_internal_scale_factor(mode),
        computed_mip_bias: native_dlss_mip_bias(mode),
    }
}

pub fn is_dlss_supported_color_format(format: TextureFormat) -> bool {
    matches!(
        format,
        TextureFormat::Rgba16Float
            | TextureFormat::Rgba8Unorm
            | TextureFormat::Rgba8UnormSrgb
            | TextureFormat::Bgra8Unorm
            | TextureFormat::Bgra8UnormSrgb
    )
}

fn request_dlss_history_resets_from_events(
    mut reset: ResMut<DlssHistoryReset>,
    mut window_events: MessageReader<WindowEvent>,
    mut solari_resets: MessageReader<bevy::solari::prelude::SolariResetEvent>,
    config: Res<ClientRenderConfig>,
    render_recovery: Option<Res<RenderRecoveryStatus>>,
    mut observed_recovery: ResMut<ObservedRenderRecovery>,
) {
    if !config.native_dlss.enabled {
        return;
    }

    if window_events
        .read()
        .any(dlss_window_event_invalidates_history)
    {
        reset.request(DlssResetReason::WindowResize);
    }
    if solari_resets.read().next().is_some() {
        reset.request(DlssResetReason::SolariHistoryReset);
    }
    if config.native_dlss.force_reset_next_frame {
        reset.request(DlssResetReason::Manual);
    }
    if let Some(render_recovery) = render_recovery.as_deref() {
        if render_recovery.recovery_successes != observed_recovery.recovery_successes {
            observed_recovery.recovery_successes = render_recovery.recovery_successes;
            reset.request(DlssResetReason::DeviceRecreation);
        }
        if render_recovery.errors_seen != observed_recovery.errors_seen {
            observed_recovery.errors_seen = render_recovery.errors_seen;
            if render_recovery.last_error_type == Some(ErrorType::DeviceLost) {
                reset.request(DlssResetReason::BackendRestart);
            }
        }
    }
}

fn dlss_window_event_invalidates_history(event: &WindowEvent) -> bool {
    matches!(
        event,
        WindowEvent::WindowBackendScaleFactorChanged(_)
            | WindowEvent::WindowResized(_)
            | WindowEvent::WindowScaleFactorChanged(_)
    )
}

#[allow(
    clippy::type_complexity,
    reason = "camera requirement gate owns one explicit query"
)]
fn ensure_dx12_native_dlss_camera_requirements(
    mut commands: Commands,
    config: Res<ClientRenderConfig>,
    support: Option<Res<Dx12NativeDlssSrSupport>>,
    mut reset: ResMut<DlssHistoryReset>,
    mut cameras: Query<(
        Entity,
        &Camera,
        &Dx12NativeDlssCamera,
        Option<&mut MipBias>,
        Option<&mut Msaa>,
        Option<&MainPassResolutionOverride>,
        Has<DepthPrepass>,
        Has<MotionVectorPrepass>,
        Has<TemporalJitter>,
        Option<&Dx12NativeDlssCameraRuntimeState>,
    )>,
) {
    for (
        entity,
        camera,
        native_dlss,
        mip_bias,
        msaa,
        resolution_override,
        has_depth,
        has_motion_vectors,
        has_jitter,
        runtime_state,
    ) in &mut cameras
    {
        let mode = native_dlss.mode.unwrap_or(config.native_dlss.mode);
        let mut entity_commands = commands.entity(entity);
        let super_resolution_ready = support
            .as_deref()
            .filter(|_| config.native_dlss.enabled)
            .is_some_and(|support| support.super_resolution_ready(config.native_dlss));
        let requested_runtime_mode = Dx12NativeDlssSrRuntimeMode::from_config(config.native_dlss);
        let runtime_mode = if super_resolution_ready {
            Dx12NativeDlssSrRuntimeMode::from_dlss_mode(mode)
        } else if config.native_dlss.enabled {
            Dx12NativeDlssSrRuntimeMode::NativeTaa
        } else {
            Dx12NativeDlssSrRuntimeMode::Disabled
        };

        let output = camera.physical_viewport_size().unwrap_or(UVec2::ZERO);
        let input = runtime_mode
            .dlss_mode()
            .map_or(output, |mode| native_dlss_input_resolution(output, mode));
        let next_runtime_state = Dx12NativeDlssCameraRuntimeState {
            runtime_mode,
            input_resolution: input,
            output_resolution: output,
            support_ready: super_resolution_ready,
        };

        if runtime_state.is_none_or(|current| *current != next_runtime_state) {
            let reason = runtime_state.map_or(DlssResetReason::Startup, |current| {
                if current.output_resolution != output {
                    DlssResetReason::WindowResize
                } else if current.runtime_mode != runtime_mode
                    || current.input_resolution != input
                    || requested_runtime_mode != runtime_mode
                {
                    DlssResetReason::DlssModeChange
                } else if current.support_ready != super_resolution_ready {
                    DlssResetReason::BackendRestart
                } else {
                    DlssResetReason::RenderScaleChange
                }
            });
            reset.request(reason);
            info!(
                target: "fun::render::dlss",
                reason = reason.as_str(),
                requested_mode = requested_runtime_mode.as_str(),
                active_mode = runtime_mode.as_str(),
                support_ready = super_resolution_ready,
                input_width = input.x,
                input_height = input.y,
                output_width = output.x,
                output_height = output.y,
                "FUN DX12 DLSS camera runtime transition"
            );
            entity_commands.insert(next_runtime_state);
        }

        if runtime_mode.dlss_mode().is_none() {
            if resolution_override.is_some() {
                entity_commands.remove::<MainPassResolutionOverride>();
            }
            if let Some(mut mip_bias) = mip_bias
                && mip_bias.0.abs() > f32::EPSILON
            {
                mip_bias.0 = 0.0;
            }
            continue;
        }

        if !has_depth {
            entity_commands.insert(DepthPrepass);
        }
        if !has_motion_vectors {
            entity_commands.insert(MotionVectorPrepass);
        }
        if !has_jitter {
            entity_commands.insert(TemporalJitter::default());
        }

        if let Some(mut msaa) = msaa
            && *msaa != Msaa::Off
        {
            *msaa = Msaa::Off;
            reset.request(DlssResetReason::RenderScaleChange);
        }

        let mip_bias_value = if super_resolution_ready {
            native_dlss_mip_bias(mode)
        } else {
            0.0
        };
        if let Some(mut mip_bias) = mip_bias {
            if (mip_bias.0 - mip_bias_value).abs() > f32::EPSILON {
                mip_bias.0 = mip_bias_value;
            }
        } else {
            entity_commands.insert(MipBias(mip_bias_value));
        }

        if output == UVec2::ZERO {
            continue;
        };

        if resolution_override.is_none_or(|current| current.0 != input) {
            entity_commands.insert(MainPassResolutionOverride(input));
            reset.request(DlssResetReason::RenderScaleChange);
        }
    }
}

#[allow(
    clippy::type_complexity,
    reason = "previous-view tracker owns one explicit camera query"
)]
fn track_dx12_dlss_previous_view_projection(
    mut commands: Commands,
    mut cameras: Query<
        (
            Entity,
            &Camera,
            &GlobalTransform,
            Option<&TemporalJitter>,
            Option<&mut Dx12DlssPreviousViewProjection>,
        ),
        With<Dx12NativeDlssCamera>,
    >,
) {
    for (entity, camera, transform, jitter, previous) in &mut cameras {
        let view_from_world = transform.affine().inverse();
        let non_jittered_projection = camera.clip_from_view();
        let mut jittered_projection = non_jittered_projection;
        let jitter_offset = jitter.map_or(Vec2::ZERO, |jitter| jitter.offset);
        if let Some(view_size) = camera.physical_viewport_size()
            && let Some(jitter) = jitter
        {
            jitter.jitter_projection(&mut jittered_projection, view_size.as_vec2());
        }
        let current_view_projection = jittered_projection * Mat4::from(view_from_world);
        let current_non_jittered_projection = non_jittered_projection * Mat4::from(view_from_world);

        if let Some(mut previous) = previous {
            previous.previous_view_projection = previous.current_view_projection;
            previous.previous_non_jittered_projection = previous.current_non_jittered_projection;
            previous.previous_jitter = previous.current_jitter;
            previous.current_view_projection = current_view_projection;
            previous.current_non_jittered_projection = current_non_jittered_projection;
            previous.current_jitter = jitter_offset;
            previous.valid_previous = true;
        } else {
            commands
                .entity(entity)
                .insert(Dx12DlssPreviousViewProjection {
                    current_view_projection,
                    previous_view_projection: current_view_projection,
                    current_non_jittered_projection,
                    previous_non_jittered_projection: current_non_jittered_projection,
                    current_jitter: jitter_offset,
                    previous_jitter: jitter_offset,
                    valid_previous: false,
                });
        }
    }
}

fn log_dlss_history_reset_debug(
    reset: Res<DlssHistoryReset>,
    config: Res<ClientRenderConfig>,
    mut last_logged: ResMut<LastDlssResetGeneration>,
) {
    if !config.native_dlss.enabled || !config.native_dlss.debug_overlay {
        return;
    }
    if reset.reset_this_frame && last_logged.0 != reset.generation {
        last_logged.0 = reset.generation;
        info!(
            target: "fun::render::dlss",
            reason = reset.reason.as_str(),
            generation = reset.generation,
            "FUN DX12 DLSS history reset requested"
        );
    }
}

fn prepass_textures_have_depth(camera: Entity, world: &World) -> bool {
    world
        .get::<ViewPrepassTextures>(camera)
        .is_some_and(|textures| textures.depth.is_some())
}

fn prepass_textures_have_motion_vectors(camera: Entity, world: &World) -> bool {
    world
        .get::<ViewPrepassTextures>(camera)
        .is_some_and(|textures| textures.motion_vectors.is_some())
}

fn known_main_pass_format(camera: Entity, world: &World) -> Option<TextureFormat> {
    world
        .get_resource::<CameraMainPassTextureFormats>()
        .and_then(|formats| formats.get(&camera).copied())
}

fn validate_dlss_resolution(camera: Entity, world: &World) -> Option<DlssResolutionState> {
    let output = world.get::<Camera>(camera)?.physical_viewport_size()?;
    let input = world
        .get::<MainPassResolutionOverride>(camera)
        .map_or(output, |override_size| override_size.0);
    Some(DlssResolutionState { input, output })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DlssResolutionState {
    input: UVec2,
    output: UVec2,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::camera::{ComputedCameraValues, RenderTargetInfo};

    #[test]
    fn native_dlss_mode_drives_resolution_and_mip_bias() {
        let output = UVec2::new(3840, 2160);
        assert_eq!(
            native_dlss_input_resolution(output, NativeDlssMode::Quality),
            UVec2::new(2560, 1440)
        );
        assert_eq!(
            native_dlss_input_resolution(output, NativeDlssMode::Performance),
            UVec2::new(1920, 1080)
        );
        assert!(native_dlss_mip_bias(NativeDlssMode::Quality) < 0.0);
        assert!(
            native_dlss_mip_bias(NativeDlssMode::UltraPerformance)
                < native_dlss_mip_bias(NativeDlssMode::Quality)
        );
    }

    #[test]
    fn window_event_stream_marks_dlss_history_unsafe_for_size_changes() {
        let window = Entity::PLACEHOLDER;

        assert!(dlss_window_event_invalidates_history(
            &WindowEvent::WindowResized(bevy::window::WindowResized {
                window,
                width: 1920.0,
                height: 1080.0,
            })
        ));
        assert!(dlss_window_event_invalidates_history(
            &WindowEvent::WindowScaleFactorChanged(bevy::window::WindowScaleFactorChanged {
                window,
                scale_factor: 2.0,
            })
        ));
        assert!(dlss_window_event_invalidates_history(
            &WindowEvent::WindowBackendScaleFactorChanged(
                bevy::window::WindowBackendScaleFactorChanged {
                    window,
                    scale_factor: 2.0,
                },
            )
        ));
        assert!(!dlss_window_event_invalidates_history(
            &WindowEvent::WindowFocused(bevy::window::WindowFocused {
                window,
                focused: true,
            })
        ));
    }

    #[test]
    fn camera_validation_rejects_missing_prerequisites() {
        let mut world = World::new();
        world.insert_resource(DlssHistoryReset {
            reset_this_frame: false,
            reason: DlssResetReason::None,
            generation: 1,
        });
        let camera = world.spawn(Camera::default()).id();

        let validation = validate_camera_for_dx12_dlss(camera, &world);
        assert!(!validation.ready());
        assert!(!validation.has_depth);
        assert!(!validation.has_motion_vectors);
        assert!(!validation.has_jitter);
        assert!(!validation.msaa_disabled);
        assert!(!validation.resolution_supported);
    }

    #[test]
    fn camera_validation_accepts_ready_camera_after_history_is_valid() {
        let mut world = World::new();
        world.insert_resource(DlssHistoryReset {
            reset_this_frame: false,
            reason: DlssResetReason::None,
            generation: 1,
        });
        let camera = world
            .spawn((
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(3840, 2160),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
                Msaa::Off,
                TemporalJitter { offset: Vec2::ZERO },
                MipBias(native_dlss_mip_bias(NativeDlssMode::Quality)),
                DepthPrepass,
                MotionVectorPrepass,
                MainPassResolutionOverride(UVec2::new(2560, 1440)),
                Dx12DlssPreviousViewProjection {
                    valid_previous: true,
                    ..default()
                },
            ))
            .id();

        let validation = validate_camera_for_dx12_dlss(camera, &world);
        assert!(validation.ready());
    }

    #[test]
    fn history_reset_marks_history_invalid_without_hiding_ready_inputs() {
        let mut world = World::new();
        world.insert_resource(DlssHistoryReset {
            reset_this_frame: true,
            reason: DlssResetReason::WindowResize,
            generation: 2,
        });
        let camera = world
            .spawn((
                Camera {
                    computed: ComputedCameraValues {
                        target_info: Some(RenderTargetInfo {
                            physical_size: UVec2::new(1920, 1080),
                            scale_factor: 1.0,
                        }),
                        ..default()
                    },
                    ..default()
                },
                Msaa::Off,
                TemporalJitter { offset: Vec2::ZERO },
                DepthPrepass,
                MotionVectorPrepass,
                MainPassResolutionOverride(UVec2::new(1280, 720)),
                Dx12DlssPreviousViewProjection {
                    valid_previous: true,
                    ..default()
                },
            ))
            .id();

        let validation = validate_camera_for_dx12_dlss(camera, &world);
        assert!(!validation.history_valid);
        assert!(validation.ready());
    }
}
