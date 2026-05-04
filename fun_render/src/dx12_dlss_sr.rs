use bevy::{
    app::App,
    camera::{Camera3d, CameraMainTextureUsages, MainPassResolutionOverride},
    core_pipeline::{
        prepass::ViewPrepassTextures,
        schedule::{Core3d, Core3dSystems},
    },
    image::ToExtents,
    prelude::*,
    render::{
        ExtractSchedule, MainWorld, Render, RenderApp, RenderSystems,
        camera::TemporalJitter,
        error_handler::{ErrorType, RenderRecoveryStatus},
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_resource::{TextureDescriptor, TextureDimension, TextureFormat, TextureUsages},
        renderer::{RenderContext, RenderDevice, ViewQuery},
        sync_world::RenderEntity,
        texture::{CachedTexture, TextureCache},
        view::{ExtractedView, ViewTarget},
    },
};
use tracing::{info, warn};

use crate::{
    ClientRenderConfig, DlssHistoryReset, DlssResetReason, Dx12DlssPreviousViewProjection,
    Dx12NativeDlssCamera, NativeDlssConfig, NativeDlssMode, is_dlss_supported_color_format,
    native_dlss_input_resolution,
};

const DEFAULT_FAILURE_BUDGET: u32 = 3;

pub struct Dx12NativeDlssSrNode;

impl Dx12NativeDlssSrNode {
    pub const LABEL: &'static str = "dx12_native_dlss_sr";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum Dx12NativeDlssSrRuntimeMode {
    #[default]
    Disabled,
    NativeTaa,
    Quality,
    Balanced,
    Performance,
    UltraPerformance,
}

impl Dx12NativeDlssSrRuntimeMode {
    pub const fn from_config(config: NativeDlssConfig) -> Self {
        if !config.enabled {
            return Self::Disabled;
        }
        Self::from_dlss_mode(config.mode)
    }

    pub const fn from_dlss_mode(mode: NativeDlssMode) -> Self {
        match mode {
            NativeDlssMode::Quality => Self::Quality,
            NativeDlssMode::Balanced => Self::Balanced,
            NativeDlssMode::Performance => Self::Performance,
            NativeDlssMode::UltraPerformance => Self::UltraPerformance,
        }
    }

    pub const fn dlss_mode(self) -> Option<NativeDlssMode> {
        match self {
            Self::Disabled | Self::NativeTaa => None,
            Self::Quality => Some(NativeDlssMode::Quality),
            Self::Balanced => Some(NativeDlssMode::Balanced),
            Self::Performance => Some(NativeDlssMode::Performance),
            Self::UltraPerformance => Some(NativeDlssMode::UltraPerformance),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::NativeTaa => "native_taa",
            Self::Quality => "dlss_quality",
            Self::Balanced => "dlss_balanced",
            Self::Performance => "dlss_performance",
            Self::UltraPerformance => "dlss_ultra_performance",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, ExtractResource, Reflect)]
#[reflect(Resource, Default, Clone)]
pub struct Dx12NativeDlssSrSupport {
    pub runtime_found: bool,
    pub native_handle_extraction_available: bool,
    pub super_resolution_supported: bool,
    pub ray_reconstruction_supported: bool,
    pub driver_needs_update: bool,
}

impl Default for Dx12NativeDlssSrSupport {
    fn default() -> Self {
        Self::unsupported()
    }
}

impl Dx12NativeDlssSrSupport {
    pub const fn unsupported() -> Self {
        Self {
            runtime_found: false,
            native_handle_extraction_available: false,
            super_resolution_supported: false,
            ray_reconstruction_supported: false,
            driver_needs_update: false,
        }
    }

    pub const fn super_resolution_ready(self, config: NativeDlssConfig) -> bool {
        config.enabled
            && self.runtime_found
            && self.native_handle_extraction_available
            && self.super_resolution_supported
            && !self.driver_needs_update
    }
}

pub fn query_dx12_native_dlss_sr_support() -> Dx12NativeDlssSrSupport {
    #[cfg(all(target_os = "windows", feature = "dx12_dlss_native"))]
    {
        let support = fun_dx12_dlss::query_support_from_env();
        Dx12NativeDlssSrSupport {
            runtime_found: support.runtime_found,
            native_handle_extraction_available: true,
            super_resolution_supported: support.sr_supported,
            ray_reconstruction_supported: support.rr_supported,
            driver_needs_update: support.needs_updated_driver,
        }
    }

    #[cfg(not(all(target_os = "windows", feature = "dx12_dlss_native")))]
    {
        Dx12NativeDlssSrSupport::unsupported()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum Dx12NativeDlssSrState {
    PendingSupport,
    Available,
    DisabledUntilDeviceReset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Reflect)]
pub enum Dx12NativeDlssSrTransition {
    #[default]
    None,
    Startup,
    Resize,
    ModeSwitch,
    OutputRecreated,
    DeviceLost,
    DeviceRecreated,
    SupportChanged,
    NativeResizePending,
}

impl Dx12NativeDlssSrTransition {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Startup => "startup",
            Self::Resize => "resize",
            Self::ModeSwitch => "mode_switch",
            Self::OutputRecreated => "output_recreated",
            Self::DeviceLost => "device_lost",
            Self::DeviceRecreated => "device_recreated",
            Self::SupportChanged => "support_changed",
            Self::NativeResizePending => "native_resize_pending",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Reflect)]
#[repr(i32)]
pub enum Dx12NativeDlssSrFailure {
    Unsupported = 1,
    InvalidDevice = 2,
    InvalidCommandList = 3,
    InvalidResource = 4,
    InvalidDimensions = 5,
    InvalidState = 6,
    SdkInitFailed = 7,
    SdkEvaluateFailed = 8,
    MissingDll = 9,
    DriverUnsupported = 10,
    NativeShimUnavailable = 11,
    WrongBackend = 12,
    CommandListUnavailable = 13,
    MissingInputs = 14,
}

impl Dx12NativeDlssSrFailure {
    pub const fn code(self) -> i32 {
        self as i32
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::InvalidDevice => "invalid_device",
            Self::InvalidCommandList => "invalid_command_list",
            Self::InvalidResource => "invalid_resource",
            Self::InvalidDimensions => "invalid_dimensions",
            Self::InvalidState => "invalid_state",
            Self::SdkInitFailed => "sdk_init_failed",
            Self::SdkEvaluateFailed => "sdk_evaluate_failed",
            Self::MissingDll => "missing_dll",
            Self::DriverUnsupported => "driver_unsupported",
            Self::NativeShimUnavailable => "native_shim_unavailable",
            Self::WrongBackend => "wrong_backend",
            Self::CommandListUnavailable => "command_list_unavailable",
            Self::MissingInputs => "missing_inputs",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource, Reflect)]
#[reflect(Resource, Default, Clone)]
pub struct Dx12NativeDlssSrStatus {
    pub state: Dx12NativeDlssSrState,
    pub consecutive_failures: u32,
    pub failure_budget: u32,
    pub last_failure: Option<Dx12NativeDlssSrFailure>,
    pub active_mode: Dx12NativeDlssSrRuntimeMode,
    pub active_input_resolution: UVec2,
    pub active_output_resolution: UVec2,
    pub device_generation: u64,
    pub observed_recovery_successes: u64,
    pub observed_errors_seen: u64,
    pub device_observed: bool,
    pub native_resize_pending: bool,
    pub last_transition: Dx12NativeDlssSrTransition,
}

impl Default for Dx12NativeDlssSrStatus {
    fn default() -> Self {
        Self {
            state: Dx12NativeDlssSrState::PendingSupport,
            consecutive_failures: 0,
            failure_budget: DEFAULT_FAILURE_BUDGET,
            last_failure: None,
            active_mode: Dx12NativeDlssSrRuntimeMode::Disabled,
            active_input_resolution: UVec2::ZERO,
            active_output_resolution: UVec2::ZERO,
            device_generation: 0,
            observed_recovery_successes: 0,
            observed_errors_seen: 0,
            device_observed: false,
            native_resize_pending: false,
            last_transition: Dx12NativeDlssSrTransition::Startup,
        }
    }
}

impl Dx12NativeDlssSrStatus {
    pub fn mark_available(&mut self) {
        self.state = Dx12NativeDlssSrState::Available;
        self.consecutive_failures = 0;
        self.last_failure = None;
        self.native_resize_pending = false;
    }

    pub fn record_failure(&mut self, failure: Dx12NativeDlssSrFailure) {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        self.last_failure = Some(failure);
        if self.consecutive_failures >= self.failure_budget {
            self.state = Dx12NativeDlssSrState::DisabledUntilDeviceReset;
        }
        warn!(
            target: "fun::render::dlss",
            failure = failure.as_str(),
            code = failure.code(),
            consecutive_failures = self.consecutive_failures,
            failure_budget = self.failure_budget,
            state = ?self.state,
            "FUN DX12 DLSS SR evaluation failed"
        );
    }

    pub fn record_transition(
        &mut self,
        transition: Dx12NativeDlssSrTransition,
        mode: Dx12NativeDlssSrRuntimeMode,
        input_resolution: UVec2,
        output_resolution: UVec2,
    ) {
        self.active_mode = mode;
        self.active_input_resolution = input_resolution;
        self.active_output_resolution = output_resolution;
        self.last_transition = transition;
        if matches!(
            transition,
            Dx12NativeDlssSrTransition::Resize
                | Dx12NativeDlssSrTransition::ModeSwitch
                | Dx12NativeDlssSrTransition::OutputRecreated
                | Dx12NativeDlssSrTransition::DeviceRecreated
        ) {
            self.native_resize_pending = true;
            self.consecutive_failures = 0;
            self.last_failure = None;
            if !matches!(self.state, Dx12NativeDlssSrState::PendingSupport) {
                self.state = Dx12NativeDlssSrState::Available;
            }
        }
        info!(
            target: "fun::render::dlss",
            transition = transition.as_str(),
            mode = mode.as_str(),
            input_width = input_resolution.x,
            input_height = input_resolution.y,
            output_width = output_resolution.x,
            output_height = output_resolution.y,
            device_generation = self.device_generation,
            "FUN DX12 DLSS SR runtime transition"
        );
    }

    pub fn record_device_recreated(&mut self, recovery_successes: u64) {
        self.device_generation = self.device_generation.saturating_add(1).max(1);
        self.observed_recovery_successes = recovery_successes;
        self.device_observed = true;
        self.state = Dx12NativeDlssSrState::PendingSupport;
        self.consecutive_failures = 0;
        self.last_failure = None;
        self.native_resize_pending = true;
        self.last_transition = Dx12NativeDlssSrTransition::DeviceRecreated;
        info!(
            target: "fun::render::dlss",
            device_generation = self.device_generation,
            recovery_successes,
            "FUN DX12 DLSS SR released native resources after renderer recovery"
        );
    }

    pub fn record_device_lost(&mut self, errors_seen: u64) {
        self.observed_errors_seen = errors_seen;
        self.state = Dx12NativeDlssSrState::PendingSupport;
        self.consecutive_failures = 0;
        self.last_failure = None;
        self.native_resize_pending = true;
        self.last_transition = Dx12NativeDlssSrTransition::DeviceLost;
        warn!(
            target: "fun::render::dlss",
            errors_seen,
            device_generation = self.device_generation,
            "FUN DX12 DLSS SR invalidated native context after device loss"
        );
    }

    pub const fn disabled_until_device_reset(self) -> bool {
        matches!(self.state, Dx12NativeDlssSrState::DisabledUntilDeviceReset)
    }
}

#[derive(Debug, Clone, Copy, Component)]
pub struct Dx12NativeDlssSrView {
    pub runtime_mode: Dx12NativeDlssSrRuntimeMode,
    pub mode: NativeDlssMode,
    pub input_resolution: UVec2,
    pub output_resolution: UVec2,
    pub sharpness: f32,
    pub reset_this_frame: bool,
    pub reset_reason: DlssResetReason,
    pub previous_view_projection: Dx12DlssPreviousViewProjection,
}

#[derive(Component)]
pub struct Dx12NativeDlssSrOutput {
    pub texture: CachedTexture,
    pub input_size: UVec2,
    pub size: UVec2,
    pub format: TextureFormat,
    pub mode: NativeDlssMode,
    pub runtime_mode: Dx12NativeDlssSrRuntimeMode,
    pub device_generation: u64,
    pub skip_evaluation_frames: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12NativeDlssSrOutputDescriptor {
    pub size: UVec2,
    pub format: TextureFormat,
    pub usage: TextureUsages,
}

impl Dx12NativeDlssSrOutputDescriptor {
    pub const USAGE: TextureUsages = TextureUsages::RENDER_ATTACHMENT
        .union(TextureUsages::TEXTURE_BINDING)
        .union(TextureUsages::STORAGE_BINDING)
        .union(TextureUsages::COPY_SRC)
        .union(TextureUsages::COPY_DST);

    pub const fn new(size: UVec2, format: TextureFormat) -> Self {
        Self {
            size,
            format,
            usage: Self::USAGE,
        }
    }

    pub fn texture_descriptor(self) -> TextureDescriptor<'static> {
        TextureDescriptor {
            label: Some("dx12_native_dlss_sr_output"),
            size: self.size.to_extents(),
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.format,
            usage: self.usage,
            view_formats: &[],
        }
    }
}

pub fn install_dx12_native_dlss_sr(app: &mut App) {
    app.register_type::<Dx12NativeDlssSrSupport>()
        .register_type::<Dx12NativeDlssSrStatus>()
        .register_type::<Dx12NativeDlssSrState>()
        .register_type::<Dx12NativeDlssSrRuntimeMode>()
        .register_type::<Dx12NativeDlssSrTransition>()
        .register_type::<Dx12NativeDlssSrFailure>()
        .init_resource::<Dx12NativeDlssSrSupport>()
        .add_plugins(ExtractResourcePlugin::<Dx12NativeDlssSrSupport>::default());

    let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
        return;
    };

    render_app
        .init_resource::<Dx12NativeDlssSrStatus>()
        .add_systems(ExtractSchedule, extract_dx12_native_dlss_sr_views)
        .add_systems(
            Render,
            (
                track_dx12_native_dlss_sr_device_recovery.in_set(RenderSystems::PrepareResources),
                (
                    cleanup_dx12_native_dlss_sr_outputs,
                    prepare_dx12_native_dlss_sr_outputs,
                )
                    .chain()
                    .in_set(RenderSystems::PrepareResources),
            )
                .chain(),
        )
        .add_systems(
            Render,
            prepare_dx12_native_dlss_sr_camera_usages
                .in_set(RenderSystems::PrepareViews)
                .before(bevy::render::view::prepare_view_targets)
                .ambiguous_with_all(),
        )
        .add_systems(
            Core3d,
            dx12_native_dlss_sr_node.in_set(Core3dSystems::EarlyPostProcess),
        );
}

fn extract_dx12_native_dlss_sr_views(
    mut commands: Commands,
    mut main_world: ResMut<MainWorld>,
    cleanup_query: Query<Has<Dx12NativeDlssSrView>>,
) {
    let Some(render_config) = main_world.get_resource::<ClientRenderConfig>().copied() else {
        return;
    };
    let support = main_world
        .get_resource::<Dx12NativeDlssSrSupport>()
        .copied()
        .unwrap_or_default();
    let reset_state = main_world
        .get_resource::<DlssHistoryReset>()
        .copied()
        .unwrap_or_default();

    let mut cameras = main_world.query::<(
        RenderEntity,
        &Camera,
        &Projection,
        Option<&Dx12NativeDlssCamera>,
        Option<&Dx12DlssPreviousViewProjection>,
    )>();

    let mut extracted_any = false;
    for (render_entity, camera, projection, native_dlss, previous_view_projection) in
        cameras.iter(&main_world)
    {
        let Ok(mut entity_commands) = commands.get_entity(render_entity) else {
            continue;
        };

        let Some(native_dlss) = native_dlss else {
            if cleanup_query.get(render_entity) == Ok(true) {
                entity_commands.remove::<Dx12NativeDlssSrView>();
            }
            continue;
        };

        if !camera.is_active
            || !projection.is_perspective()
            || !support.super_resolution_ready(render_config.native_dlss)
        {
            if cleanup_query.get(render_entity) == Ok(true) {
                entity_commands.remove::<Dx12NativeDlssSrView>();
            }
            continue;
        }

        let Some(output_resolution) = camera.physical_viewport_size() else {
            if cleanup_query.get(render_entity) == Ok(true) {
                entity_commands.remove::<Dx12NativeDlssSrView>();
            }
            continue;
        };

        let mode = native_dlss.mode.unwrap_or(render_config.native_dlss.mode);
        let runtime_mode = Dx12NativeDlssSrRuntimeMode::from_dlss_mode(mode);
        let previous_view_projection = previous_view_projection
            .copied()
            .unwrap_or_else(Dx12DlssPreviousViewProjection::default);
        let reset_this_frame =
            reset_state.reset_this_frame || !previous_view_projection.valid_previous;
        entity_commands.insert(Dx12NativeDlssSrView {
            runtime_mode,
            mode,
            input_resolution: native_dlss_input_resolution(output_resolution, mode),
            output_resolution,
            sharpness: render_config.native_dlss.sharpness,
            reset_this_frame,
            reset_reason: reset_state.reason,
            previous_view_projection,
        });
        extracted_any = true;
    }

    if extracted_any && let Some(mut reset) = main_world.get_resource_mut::<DlssHistoryReset>() {
        reset.clear_after_consume();
    }
}

fn prepare_dx12_native_dlss_sr_camera_usages(
    mut cameras: Query<
        (
            &mut Camera3d,
            &mut CameraMainTextureUsages,
            &Dx12NativeDlssSrView,
        ),
        With<MainPassResolutionOverride>,
    >,
) {
    for (mut camera_3d, mut main_texture_usages, _) in &mut cameras {
        main_texture_usages.0 |= Dx12NativeDlssSrOutputDescriptor::USAGE;
        camera_3d.depth_texture_usages.0 |= TextureUsages::TEXTURE_BINDING.bits();
    }
}

fn track_dx12_native_dlss_sr_device_recovery(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    recovery: Option<Res<RenderRecoveryStatus>>,
    mut status: ResMut<Dx12NativeDlssSrStatus>,
    outputs: Query<Entity, With<Dx12NativeDlssSrOutput>>,
) {
    let recovery_successes = recovery
        .as_deref()
        .map_or(0, |status| status.recovery_successes);
    let errors_seen = recovery.as_deref().map_or(0, |status| status.errors_seen);

    if !status.device_observed {
        status.device_observed = true;
        status.device_generation = 1;
        status.observed_recovery_successes = recovery_successes;
        status.observed_errors_seen = errors_seen;
        return;
    }

    let device_recreated =
        recovery_successes != status.observed_recovery_successes || render_device.is_changed();
    if device_recreated {
        status.record_device_recreated(recovery_successes);
        for entity in &outputs {
            commands.entity(entity).remove::<Dx12NativeDlssSrOutput>();
        }
        return;
    }

    let device_lost = recovery.as_deref().is_some_and(|recovery| {
        recovery.errors_seen != status.observed_errors_seen
            && recovery.last_error_type == Some(ErrorType::DeviceLost)
    });
    if device_lost {
        status.record_device_lost(errors_seen);
        for entity in &outputs {
            commands.entity(entity).remove::<Dx12NativeDlssSrOutput>();
        }
    } else {
        status.observed_errors_seen = errors_seen;
    }
}

fn cleanup_dx12_native_dlss_sr_outputs(
    mut commands: Commands,
    views: Query<(Entity, Has<Dx12NativeDlssSrView>), With<Dx12NativeDlssSrOutput>>,
) {
    for (entity, has_view) in &views {
        if !has_view {
            commands.entity(entity).remove::<Dx12NativeDlssSrOutput>();
        }
    }
}

fn prepare_dx12_native_dlss_sr_outputs(
    mut commands: Commands,
    render_device: Res<RenderDevice>,
    mut texture_cache: ResMut<TextureCache>,
    mut status: ResMut<Dx12NativeDlssSrStatus>,
    views: Query<(
        Entity,
        &Dx12NativeDlssSrView,
        &ViewTarget,
        Option<&Dx12NativeDlssSrOutput>,
    )>,
) {
    for (entity, sr_view, view_target, output) in &views {
        let format = view_target.main_texture_format();
        if !is_dlss_supported_color_format(format) {
            commands
                .entity(entity)
                .try_remove::<Dx12NativeDlssSrOutput>();
            continue;
        }

        if output.is_some_and(|output| {
            output.input_size == sr_view.input_resolution
                && output.size == sr_view.output_resolution
                && output.format == format
                && output.mode == sr_view.mode
                && output.runtime_mode == sr_view.runtime_mode
                && output.device_generation == status.device_generation
        }) {
            continue;
        }

        let transition = output.map_or(Dx12NativeDlssSrTransition::OutputRecreated, |output| {
            if output.device_generation != status.device_generation {
                Dx12NativeDlssSrTransition::DeviceRecreated
            } else if output.mode != sr_view.mode || output.runtime_mode != sr_view.runtime_mode {
                Dx12NativeDlssSrTransition::ModeSwitch
            } else if output.input_size != sr_view.input_resolution
                || output.size != sr_view.output_resolution
            {
                Dx12NativeDlssSrTransition::Resize
            } else {
                Dx12NativeDlssSrTransition::OutputRecreated
            }
        });

        let descriptor = Dx12NativeDlssSrOutputDescriptor::new(sr_view.output_resolution, format);
        let texture = texture_cache.get(&render_device, descriptor.texture_descriptor());
        status.record_transition(
            transition,
            sr_view.runtime_mode,
            sr_view.input_resolution,
            sr_view.output_resolution,
        );
        commands.entity(entity).insert(Dx12NativeDlssSrOutput {
            texture,
            input_size: sr_view.input_resolution,
            size: sr_view.output_resolution,
            format,
            mode: sr_view.mode,
            runtime_mode: sr_view.runtime_mode,
            device_generation: status.device_generation,
            skip_evaluation_frames: 1,
        });
    }
}

fn dx12_native_dlss_sr_node(
    view: ViewQuery<(
        &Dx12NativeDlssSrView,
        &ExtractedView,
        &ViewTarget,
        &ViewPrepassTextures,
        &TemporalJitter,
        &MainPassResolutionOverride,
        &mut Dx12NativeDlssSrOutput,
    )>,
    mut status: ResMut<Dx12NativeDlssSrStatus>,
    mut ctx: RenderContext,
) {
    let (
        sr_view,
        extracted_view,
        view_target,
        prepass_textures,
        temporal_jitter,
        resolution_override,
        mut output,
    ) = view.into_inner();

    if status.disabled_until_device_reset() {
        let post_process = view_target.post_process_write();
        ctx.command_encoder().copy_texture_to_texture(
            post_process.source_texture.as_image_copy(),
            post_process.destination_texture.as_image_copy(),
            sr_view.output_resolution.to_extents(),
        );
        return;
    }

    if prepass_textures.depth.is_none() || prepass_textures.motion_vectors.is_none() {
        let post_process = view_target.post_process_write();
        ctx.command_encoder().copy_texture_to_texture(
            post_process.source_texture.as_image_copy(),
            post_process.destination_texture.as_image_copy(),
            sr_view.output_resolution.to_extents(),
        );
        status.record_failure(Dx12NativeDlssSrFailure::MissingInputs);
        return;
    }

    let post_process = view_target.post_process_write();
    if output.skip_evaluation_frames > 0 {
        output.skip_evaluation_frames = output.skip_evaluation_frames.saturating_sub(1);
        status.record_transition(
            Dx12NativeDlssSrTransition::NativeResizePending,
            sr_view.runtime_mode,
            sr_view.input_resolution,
            sr_view.output_resolution,
        );
        status.native_resize_pending = false;
        ctx.command_encoder().copy_texture_to_texture(
            post_process.source_texture.as_image_copy(),
            post_process.destination_texture.as_image_copy(),
            sr_view.output_resolution.to_extents(),
        );
        return;
    }

    let failure = evaluate_dx12_native_dlss_sr(
        sr_view,
        temporal_jitter,
        resolution_override,
        &output,
        post_process.source_texture,
        prepass_textures,
        ctx.command_encoder(),
    );

    match failure {
        None => {
            ctx.command_encoder().copy_texture_to_texture(
                output.texture.texture.as_image_copy(),
                post_process.destination_texture.as_image_copy(),
                sr_view.output_resolution.to_extents(),
            );
            status.mark_available();
        }
        Some(failure) => {
            status.record_failure(failure);
            ctx.command_encoder().copy_texture_to_texture(
                post_process.source_texture.as_image_copy(),
                post_process.destination_texture.as_image_copy(),
                sr_view.output_resolution.to_extents(),
            );
            warn!(
                target: "fun::render::dlss",
                node = Dx12NativeDlssSrNode::LABEL,
                view = ?extracted_view.retained_view_entity,
                mode = sr_view.mode.as_env_value(),
                input_width = sr_view.input_resolution.x,
                input_height = sr_view.input_resolution.y,
                output_width = sr_view.output_resolution.x,
                output_height = sr_view.output_resolution.y,
                reset = sr_view.reset_this_frame,
                reset_reason = sr_view.reset_reason.as_str(),
                "FUN DX12 DLSS SR fell back to debug copy for this frame"
            );
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "native evaluation descriptor mirrors the future C ABI"
)]
fn evaluate_dx12_native_dlss_sr(
    sr_view: &Dx12NativeDlssSrView,
    temporal_jitter: &TemporalJitter,
    resolution_override: &MainPassResolutionOverride,
    output: &Dx12NativeDlssSrOutput,
    input_color: &bevy::render::render_resource::Texture,
    prepass_textures: &ViewPrepassTextures,
    encoder: &mut bevy::render::render_resource::CommandEncoder,
) -> Option<Dx12NativeDlssSrFailure> {
    let Some(depth) = prepass_textures.depth.as_ref() else {
        return Some(Dx12NativeDlssSrFailure::MissingInputs);
    };
    let Some(motion_vectors) = prepass_textures.motion_vectors.as_ref() else {
        return Some(Dx12NativeDlssSrFailure::MissingInputs);
    };

    if resolution_override.0 != sr_view.input_resolution || output.size != sr_view.output_resolution
    {
        return Some(Dx12NativeDlssSrFailure::InvalidDimensions);
    }

    #[cfg(all(target_os = "windows", feature = "dx12_dlss_native"))]
    {
        use crate::dx12_native::{
            Dx12DlssResourceStatePlan, Dx12NativeInteropFailure, extract_dx12_texture_handle,
            log_dx12_dlss_resource_state_plan_once, with_dx12_command_list_checked,
        };

        log_dx12_dlss_resource_state_plan_once(
            Dx12DlssResourceStatePlan::SUPER_RESOLUTION_FIRST_PASS,
        );

        for texture in [
            input_color,
            &output.texture.texture,
            &depth.texture.texture,
            &motion_vectors.texture.texture,
        ] {
            if let Err(error) = unsafe { extract_dx12_texture_handle(texture) } {
                return Some(match error.failure {
                    Dx12NativeInteropFailure::WrongBackend => Dx12NativeDlssSrFailure::WrongBackend,
                    Dx12NativeInteropFailure::InvalidTextureDimensions => {
                        Dx12NativeDlssSrFailure::InvalidDimensions
                    }
                    Dx12NativeInteropFailure::UnsupportedTextureFormat
                    | Dx12NativeInteropFailure::UnsupportedTextureShape
                    | Dx12NativeInteropFailure::TextureHalUnavailable => {
                        Dx12NativeDlssSrFailure::InvalidResource
                    }
                    Dx12NativeInteropFailure::DeviceHalUnavailable
                    | Dx12NativeInteropFailure::QueueHalUnavailable => {
                        Dx12NativeDlssSrFailure::InvalidDevice
                    }
                    Dx12NativeInteropFailure::CommandEncoderHalUnavailable => {
                        Dx12NativeDlssSrFailure::InvalidCommandList
                    }
                    Dx12NativeInteropFailure::CommandListUnavailable => {
                        Dx12NativeDlssSrFailure::CommandListUnavailable
                    }
                    Dx12NativeInteropFailure::ObjectNameUnavailable
                    | Dx12NativeInteropFailure::ObjectNameFailed => {
                        Dx12NativeDlssSrFailure::NativeShimUnavailable
                    }
                });
            }
        }

        let command_list =
            unsafe { with_dx12_command_list_checked(encoder, |handle| handle.command_list) };
        if let Err(error) = command_list {
            return Some(match error.failure {
                Dx12NativeInteropFailure::WrongBackend => Dx12NativeDlssSrFailure::WrongBackend,
                Dx12NativeInteropFailure::CommandEncoderHalUnavailable => {
                    Dx12NativeDlssSrFailure::InvalidCommandList
                }
                Dx12NativeInteropFailure::CommandListUnavailable => {
                    Dx12NativeDlssSrFailure::CommandListUnavailable
                }
                Dx12NativeInteropFailure::DeviceHalUnavailable
                | Dx12NativeInteropFailure::QueueHalUnavailable => {
                    Dx12NativeDlssSrFailure::InvalidDevice
                }
                Dx12NativeInteropFailure::InvalidTextureDimensions => {
                    Dx12NativeDlssSrFailure::InvalidDimensions
                }
                Dx12NativeInteropFailure::TextureHalUnavailable
                | Dx12NativeInteropFailure::UnsupportedTextureFormat
                | Dx12NativeInteropFailure::UnsupportedTextureShape => {
                    Dx12NativeDlssSrFailure::InvalidResource
                }
                Dx12NativeInteropFailure::ObjectNameUnavailable
                | Dx12NativeInteropFailure::ObjectNameFailed => {
                    Dx12NativeDlssSrFailure::NativeShimUnavailable
                }
            });
        }

        info!(
            target: "fun::render::dlss",
            node = Dx12NativeDlssSrNode::LABEL,
            mode = sr_view.mode.as_env_value(),
            input_width = sr_view.input_resolution.x,
            input_height = sr_view.input_resolution.y,
            output_width = sr_view.output_resolution.x,
            output_height = sr_view.output_resolution.y,
            jitter_x = -temporal_jitter.offset.x,
            jitter_y = -temporal_jitter.offset.y,
            sharpness = sr_view.sharpness,
            reset = sr_view.reset_this_frame,
            previous_valid = sr_view.previous_view_projection.valid_previous,
            "FUN DX12 DLSS SR native evaluate descriptor is ready"
        );

        Some(Dx12NativeDlssSrFailure::NativeShimUnavailable)
    }

    #[cfg(not(all(target_os = "windows", feature = "dx12_dlss_native")))]
    {
        let _ = (
            temporal_jitter,
            output,
            input_color,
            depth,
            motion_vectors,
            encoder,
        );
        Some(Dx12NativeDlssSrFailure::NativeShimUnavailable)
    }
}

pub fn log_dx12_native_dlss_sr_support_once(
    config: NativeDlssConfig,
    support: Dx12NativeDlssSrSupport,
) {
    if !config.enabled {
        return;
    }
    info!(
        target: "fun::render::dlss",
        runtime_found = support.runtime_found,
        native_handle_extraction_available = support.native_handle_extraction_available,
        sr_supported = support.super_resolution_supported,
        rr_supported = support.ray_reconstruction_supported,
        driver_needs_update = support.driver_needs_update,
        "FUN DX12 DLSS SR support state"
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_support_does_not_activate_super_resolution() {
        let config = NativeDlssConfig {
            enabled: true,
            ..default()
        };
        assert!(!Dx12NativeDlssSrSupport::default().super_resolution_ready(config));
    }

    #[test]
    fn native_support_query_fails_closed_until_bridge_reports_sr() {
        let support = query_dx12_native_dlss_sr_support();

        assert!(!support.super_resolution_supported);
        assert!(!support.ray_reconstruction_supported);
        assert!(!support.driver_needs_update);
    }

    #[test]
    fn runtime_modes_map_config_and_dlss_modes() {
        assert_eq!(
            Dx12NativeDlssSrRuntimeMode::from_config(NativeDlssConfig::default()),
            Dx12NativeDlssSrRuntimeMode::Disabled
        );
        let config = NativeDlssConfig {
            enabled: true,
            mode: NativeDlssMode::Balanced,
            ..default()
        };
        assert_eq!(
            Dx12NativeDlssSrRuntimeMode::from_config(config),
            Dx12NativeDlssSrRuntimeMode::Balanced
        );
        assert_eq!(
            Dx12NativeDlssSrRuntimeMode::UltraPerformance.dlss_mode(),
            Some(NativeDlssMode::UltraPerformance)
        );
        assert_eq!(Dx12NativeDlssSrRuntimeMode::NativeTaa.dlss_mode(), None);
    }

    #[test]
    fn output_descriptor_uses_sdk_compatible_intermediate_usages() {
        let descriptor = Dx12NativeDlssSrOutputDescriptor::new(
            UVec2::new(3840, 2160),
            TextureFormat::Rgba16Float,
        );
        assert!(descriptor.usage.contains(TextureUsages::RENDER_ATTACHMENT));
        assert!(descriptor.usage.contains(TextureUsages::TEXTURE_BINDING));
        assert!(descriptor.usage.contains(TextureUsages::STORAGE_BINDING));
        assert!(descriptor.usage.contains(TextureUsages::COPY_SRC));
        assert!(descriptor.usage.contains(TextureUsages::COPY_DST));
        assert_eq!(
            descriptor.texture_descriptor().size,
            UVec2::new(3840, 2160).to_extents()
        );
    }

    #[test]
    fn failure_budget_disables_until_device_reset() {
        let mut status = Dx12NativeDlssSrStatus {
            failure_budget: 2,
            ..default()
        };
        status.record_failure(Dx12NativeDlssSrFailure::CommandListUnavailable);
        assert!(!status.disabled_until_device_reset());
        status.record_failure(Dx12NativeDlssSrFailure::CommandListUnavailable);
        assert!(status.disabled_until_device_reset());
    }

    #[test]
    fn resize_transition_marks_native_resize_pending_and_clears_failures() {
        let mut status = Dx12NativeDlssSrStatus {
            consecutive_failures: 2,
            last_failure: Some(Dx12NativeDlssSrFailure::SdkEvaluateFailed),
            device_generation: 4,
            ..default()
        };
        status.record_transition(
            Dx12NativeDlssSrTransition::Resize,
            Dx12NativeDlssSrRuntimeMode::Quality,
            UVec2::new(1280, 720),
            UVec2::new(1920, 1080),
        );
        assert!(status.native_resize_pending);
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.last_failure, None);
        assert_eq!(status.active_mode, Dx12NativeDlssSrRuntimeMode::Quality);
        assert_eq!(status.active_input_resolution, UVec2::new(1280, 720));
        assert_eq!(status.active_output_resolution, UVec2::new(1920, 1080));
    }

    #[test]
    fn device_recreation_drops_disabled_failure_state() {
        let mut status = Dx12NativeDlssSrStatus {
            state: Dx12NativeDlssSrState::DisabledUntilDeviceReset,
            consecutive_failures: 3,
            last_failure: Some(Dx12NativeDlssSrFailure::CommandListUnavailable),
            device_observed: true,
            device_generation: 7,
            ..default()
        };
        status.record_device_recreated(2);
        assert_eq!(status.state, Dx12NativeDlssSrState::PendingSupport);
        assert_eq!(status.device_generation, 8);
        assert_eq!(status.consecutive_failures, 0);
        assert_eq!(status.last_failure, None);
        assert!(status.native_resize_pending);
    }
}
