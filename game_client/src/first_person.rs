use std::f32::consts::FRAC_PI_2;

use crate::avis::{
    AdjustPrecision as _, AsF32 as _, Collider, MoveAndSlide, MoveAndSlideConfig,
    MoveAndSlideHitResponse, Position, RigidBody, Rotation, ShapeCastConfig, SpatialQueryFilter,
};
use crate::{ClientHostControlState, ClientWorldStatus};
#[cfg(all(feature = "render_diagnostics", debug_assertions))]
use crate::{ClientScheduleProfiler, ClientScheduleSystem, frame_profile::DetailedFrameProfiler};
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::Msaa,
    window::{CursorGrabMode, CursorOptions},
};
use fun_host::{FunClientHostState, FunInputOwner};
use fun_render::renderer_component_api as renderer_api;
use fun_scene::prelude::*;
use game_shared::{DEFAULT_CORRECTION_HALF_LIFE_SECONDS, PLAYER_SPAWN};

pub struct FirstPersonControllerPlugin {
    simulation_enabled: bool,
}

impl FirstPersonControllerPlugin {
    #[must_use]
    pub const fn gameplay() -> Self {
        Self {
            simulation_enabled: true,
        }
    }

    #[must_use]
    pub const fn preview_camera() -> Self {
        Self {
            simulation_enabled: false,
        }
    }
}

impl Default for FirstPersonControllerPlugin {
    fn default() -> Self {
        Self::gameplay()
    }
}
pub const PLAYER_RADIUS: f32 = 0.45;
pub const PLAYER_HALF_HEIGHT: f32 = 0.9;
const PLAYER_CAPSULE_LENGTH: f32 = PLAYER_HALF_HEIGHT * 2.0 - PLAYER_RADIUS * 2.0;
const MAX_SLOPE_ANGLE: f32 = 45.0_f32.to_radians();
const GROUND_PROBE_DISTANCE: f32 = 0.08;
const CONTACT_CACHE_MAX_AGE_SECONDS: f32 = 0.12;
const COYOTE_TIME_SECONDS: f32 = 0.1;
const LEDGE_RECHECK_AGE_SECONDS: f32 = 0.05;
const HIGH_VERTICAL_DELTA_METERS: f32 = 0.04;

impl Plugin for FirstPersonControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player);
        if self.simulation_enabled {
            app.add_systems(PreUpdate, cache_movement_input)
                .add_systems(
                    FixedUpdate,
                    (restore_simulation_transform, apply_kinematic_movement).chain(),
                );
        }
        app.add_systems(
            Update,
            (
                update_cursor_grab,
                apply_look,
                interpolate_player_render_transform,
            )
                .chain(),
        );
    }
}

#[derive(Component, Clone, Copy, Default)]
struct Player;

#[derive(Component, Clone, Copy, Default)]
#[component(storage = "SparseSet")]
struct Grounded;

#[derive(Component, Clone, Copy, Default)]
struct YawPivot;

#[derive(Component, Clone, Copy, Default)]
struct PitchPivot;

#[derive(Component, Clone, Copy)]
struct MovementSettings {
    max_speed: f32,
    ground_acceleration: f32,
    air_acceleration: f32,
    jump_impulse: f32,
    ground_damping: f32,
    air_damping: f32,
    gravity: f32,
    terminal_velocity: f32,
}

impl Default for MovementSettings {
    fn default() -> Self {
        Self {
            max_speed: 7.5,
            ground_acceleration: 90.0,
            air_acceleration: 30.0,
            jump_impulse: 8.5,
            ground_damping: 10.5,
            air_damping: 1.5,
            gravity: 9.81 * 2.2,
            terminal_velocity: 55.0,
        }
    }
}

#[derive(Component, Clone, Copy, Default)]
struct MovementInputState {
    movement: Vec2,
    jump_queued: bool,
}

#[derive(Component, Clone, Copy, Default)]
struct CharacterVelocity(Vec3);

#[derive(Component, Clone)]
struct NetworkInterpolationState {
    previous_simulation: Transform,
    current_simulation: Transform,
    previous_render: Transform,
    correction_velocity: Vec3,
    correction_half_life: f32,
}

impl NetworkInterpolationState {
    fn new(transform: Transform) -> Self {
        Self {
            previous_simulation: transform,
            current_simulation: transform,
            previous_render: transform,
            correction_velocity: Vec3::ZERO,
            correction_half_life: DEFAULT_CORRECTION_HALF_LIFE_SECONDS,
        }
    }

    fn reset(&mut self, transform: Transform) {
        self.previous_simulation = transform;
        self.current_simulation = transform;
        self.previous_render = transform;
        self.correction_velocity = Vec3::ZERO;
    }

    fn commit_simulation(&mut self, transform: Transform) {
        self.previous_simulation = self.current_simulation;
        self.current_simulation = transform;
    }
}

impl Default for NetworkInterpolationState {
    fn default() -> Self {
        Self::new(Transform::default())
    }
}

#[derive(Component, Clone, Copy)]
struct MovementContactCache {
    ground_normal: Vec3,
    ground_entity: Option<Entity>,
    contact_age: f32,
    coyote_timer: f32,
    step_candidate: Option<Vec3>,
}

impl Default for MovementContactCache {
    fn default() -> Self {
        Self {
            ground_normal: Vec3::Y,
            ground_entity: None,
            contact_age: CONTACT_CACHE_MAX_AGE_SECONDS,
            coyote_timer: 0.0,
            step_candidate: None,
        }
    }
}

impl MovementContactCache {
    fn advance(&mut self, delta_seconds: f32) {
        if self.ground_entity.is_some() {
            self.contact_age += delta_seconds;
        }
        self.coyote_timer = (self.coyote_timer - delta_seconds).max(0.0);
    }

    fn refresh(&mut self, contact: GroundContact) {
        self.ground_normal = contact.normal;
        self.ground_entity = Some(contact.entity);
        self.contact_age = 0.0;
        self.coyote_timer = COYOTE_TIME_SECONDS;
        self.step_candidate = Some(contact.point);
    }

    fn mark_airborne(&mut self) {
        self.ground_entity = None;
        self.contact_age = CONTACT_CACHE_MAX_AGE_SECONDS;
        self.step_candidate = None;
    }

    fn has_valid_ground(&self) -> bool {
        self.ground_entity.is_some()
            && self.contact_age <= CONTACT_CACHE_MAX_AGE_SECONDS
            && self.ground_normal.dot(Vec3::Y) >= max_slope_dot()
    }

    fn can_jump(&self) -> bool {
        self.has_valid_ground() || self.coyote_timer > 0.0
    }

    fn should_probe_before_move(
        &self,
        velocity: Vec3,
        delta_seconds: f32,
        jump_requested: bool,
    ) -> bool {
        jump_requested
            || !self.has_valid_ground()
            || velocity.y.abs() * delta_seconds > HIGH_VERTICAL_DELTA_METERS
            || self.contact_age > LEDGE_RECHECK_AGE_SECONDS
    }

    fn should_probe_after_move(
        &self,
        had_move_hit: bool,
        was_grounded: bool,
        vertical_delta: f32,
        input: Vec2,
    ) -> bool {
        !had_move_hit
            && (!self.has_valid_ground()
                || vertical_delta.abs() > HIGH_VERTICAL_DELTA_METERS
                || (was_grounded && input != Vec2::ZERO && self.contact_age > 0.0))
    }
}

#[derive(Clone, Copy)]
struct GroundContact {
    entity: Entity,
    normal: Vec3,
    point: Vec3,
}

#[derive(Component, Clone, Copy)]
struct LookSettings {
    yaw: f32,
    pitch: f32,
    sensitivity: Vec2,
}

impl Default for LookSettings {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            sensitivity: Vec2::new(0.0025, 0.002),
        }
    }
}

fn spawn_player(mut commands: Commands) {
    commands.spawn_fun_scene(player_scene(base_camera_scene()));
}

fn player_scene(camera: impl FunScene) -> impl FunScene {
    let spawn_transform = Transform::from_xyz(PLAYER_SPAWN[0], PLAYER_SPAWN[1], PLAYER_SPAWN[2]);
    fun! {
        #Player
        Player
        Name::new("Player")
        MovementSettings::default()
        MovementInputState::default()
        CharacterVelocity::default()
        MovementContactCache::default()
        fun_value(NetworkInterpolationState::new(spawn_transform))
        LookSettings::default()
        fun_value(RigidBody::Kinematic)
        Collider::capsule(PLAYER_RADIUS, PLAYER_CAPSULE_LENGTH)
        Visibility::default()
        fun_value(Position::from_xyz(PLAYER_SPAWN[0], PLAYER_SPAWN[1], PLAYER_SPAWN[2]))
        fun_value(Rotation::IDENTITY)
        fun_value(spawn_transform)
        Children [(
            #YawPivot
            YawPivot
            Name::new("YawPivot")
            Visibility::default()
            Transform::from_xyz(0.0, 0.75, 0.0)
            Children [(
                #PitchPivot
                PitchPivot
                Name::new("PitchPivot")
                Visibility::default()
                Transform::default()
                Children [({camera})]
            )]
        )]
    }
}

fn base_camera_scene() -> impl FunScene {
    fun! {
        Camera3d
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.015, 0.025, 0.045)),
        }
        fun_value(renderer_api::RenderCamera {
            layers: renderer_api::RenderLayerMask::DEFAULT,
            order: 0,
        })
        fun_value(renderer_api::MainCamera)
        fun_value(renderer_api::CameraProjection {
            mode: renderer_api::CameraProjectionMode::Perspective,
            vertical_fov_radians: 75.0_f32.to_radians(),
            orthographic_height: 10.0,
            near: 0.05,
            far: 50_000.0,
        })
        fun_value(renderer_api::CameraRenderTarget::default())
        fun_value(renderer_api::CameraExposure {
            exposure_value: 0.0,
            auto_exposure: false,
        })
        fun_value(Msaa::Off)
        fun_value(Projection::from(PerspectiveProjection {
            fov: 75.0_f32.to_radians(),
            ..default()
        }))
        Transform::default()
    }
}

fn cache_movement_input(
    keys: Res<ButtonInput<KeyCode>>,
    host_control: Option<Res<ClientHostControlState>>,
    #[cfg(feature = "native_ui_routes")] ui_input_gate: Option<
        Res<crate::native_ui_routes::NativeUiGameplayInputGate>,
    >,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut schedule_profiler: ResMut<
        ClientScheduleProfiler,
    >,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut frame_profiler: ResMut<
        DetailedFrameProfiler,
    >,
    mut players: Query<&mut MovementInputState, With<Player>>,
) {
    crate::frame_profile_start!(started);
    crate::frame_profile_scope!(_scope, frame_profiler, "PreUpdate", "cache_movement_input");
    let Ok(mut input_state) = players.single_mut() else {
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::MovementInput, started);
        return;
    };

    let native_ui_blocks_movement = {
        #[cfg(feature = "native_ui_routes")]
        {
            ui_input_gate
                .as_deref()
                .is_some_and(|gate| gate.blocks_movement())
        }
        #[cfg(not(feature = "native_ui_routes"))]
        {
            false
        }
    };

    if host_control.as_deref().is_some_and(|control| {
        control.input_owner == game_shared::EditorInputOwner::Editor || control.visual_paused
    }) || native_ui_blocks_movement
    {
        input_state.movement = Vec2::ZERO;
        input_state.jump_queued = false;
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::MovementInput, started);
        return;
    }

    input_state.movement = movement_input(&keys);
    input_state.jump_queued |= keys.just_pressed(KeyCode::Space);
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    schedule_profiler.record_elapsed(ClientScheduleSystem::MovementInput, started);
}

fn update_cursor_grab(
    mut cursor_options: Single<&mut CursorOptions>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    host: Option<Res<FunClientHostState>>,
    #[cfg(feature = "native_ui_routes")] ui_input_gate: Option<
        Res<crate::native_ui_routes::NativeUiGameplayInputGate>,
    >,
) {
    let native_ui_requires_free_cursor = {
        #[cfg(feature = "native_ui_routes")]
        {
            native_ui_gate_requires_free_cursor(ui_input_gate.as_deref())
        }
        #[cfg(not(feature = "native_ui_routes"))]
        {
            false
        }
    };
    if host_input_owner_requires_free_cursor(host.as_deref()) || native_ui_requires_free_cursor {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
        return;
    }

    let pointer_actions_blocked = {
        #[cfg(feature = "native_ui_routes")]
        {
            ui_input_gate
                .as_deref()
                .is_some_and(|gate| gate.blocks_pointer_actions())
        }
        #[cfg(not(feature = "native_ui_routes"))]
        {
            false
        }
    };
    let keyboard_actions_blocked = {
        #[cfg(feature = "native_ui_routes")]
        {
            ui_input_gate
                .as_deref()
                .is_some_and(|gate| gate.blocks_keyboard_actions())
        }
        #[cfg(not(feature = "native_ui_routes"))]
        {
            false
        }
    };

    if !pointer_actions_blocked && mouse_buttons.just_pressed(MouseButton::Left) {
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Locked;
    }

    if !keyboard_actions_blocked && keys.just_pressed(KeyCode::Escape) {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
    }
}

fn host_input_owner_requires_free_cursor(host: Option<&FunClientHostState>) -> bool {
    host.is_some_and(|host| !matches!(host.state.input_owner, FunInputOwner::Gameplay))
}

#[cfg(feature = "native_ui_routes")]
fn native_ui_gate_requires_free_cursor(
    gate: Option<&crate::native_ui_routes::NativeUiGameplayInputGate>,
) -> bool {
    gate.is_some_and(|gate| {
        matches!(
            gate.reason,
            crate::native_ui_routes::NativeUiGameplayInputBlockReason::UiModal
                | crate::native_ui_routes::NativeUiGameplayInputBlockReason::TextEntry
        )
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "camera look input reads Bevy resources and pivots directly for scheduler clarity"
)]
fn apply_look(
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    cursor_options: Single<&CursorOptions>,
    host_control: Option<Res<ClientHostControlState>>,
    #[cfg(feature = "native_ui_routes")] ui_input_gate: Option<
        Res<crate::native_ui_routes::NativeUiGameplayInputGate>,
    >,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut schedule_profiler: ResMut<
        ClientScheduleProfiler,
    >,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut frame_profiler: ResMut<
        DetailedFrameProfiler,
    >,
    mut player: Single<&mut LookSettings, With<Player>>,
    mut yaw_pivot: Single<&mut Transform, (With<YawPivot>, Without<PitchPivot>)>,
    mut pitch_pivot: Single<&mut Transform, (With<PitchPivot>, Without<YawPivot>)>,
) {
    crate::frame_profile_start!(started);
    crate::frame_profile_scope!(_scope, frame_profiler, "Update", "apply_look");
    let native_ui_blocks_look = {
        #[cfg(feature = "native_ui_routes")]
        {
            ui_input_gate
                .as_deref()
                .is_some_and(|gate| gate.blocks_look())
        }
        #[cfg(not(feature = "native_ui_routes"))]
        {
            false
        }
    };

    if host_control.as_deref().is_some_and(|control| {
        control.input_owner == game_shared::EditorInputOwner::Editor || control.visual_paused
    }) || native_ui_blocks_look
    {
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::Look, started);
        return;
    }

    if cursor_options.grab_mode == CursorGrabMode::None {
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::Look, started);
        return;
    }

    let delta = accumulated_mouse_motion.delta;
    if delta == Vec2::ZERO {
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::Look, started);
        return;
    }

    player.yaw -= delta.x * player.sensitivity.x;
    player.pitch =
        (player.pitch - delta.y * player.sensitivity.y).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);

    yaw_pivot.rotation = Quat::from_rotation_y(player.yaw);
    pitch_pivot.rotation = Quat::from_rotation_x(player.pitch);
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    schedule_profiler.record_elapsed(ClientScheduleSystem::Look, started);
}

fn restore_simulation_transform(
    player: Single<(&mut Transform, &NetworkInterpolationState), With<Player>>,
) {
    let (mut transform, interpolation) = player.into_inner();
    *transform = interpolation.current_simulation;
}

fn interpolate_player_render_transform(
    time: Res<Time>,
    fixed_time: Res<Time<Fixed>>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut schedule_profiler: ResMut<
        ClientScheduleProfiler,
    >,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut frame_profiler: ResMut<
        DetailedFrameProfiler,
    >,
    player: Single<(&mut Transform, &mut NetworkInterpolationState), With<Player>>,
) {
    crate::frame_profile_start!(started);
    crate::frame_profile_scope!(
        _scope,
        frame_profiler,
        "Update",
        "interpolate_player_render_transform",
    );
    let (mut transform, mut interpolation) = player.into_inner();
    let alpha = fixed_time.overstep_fraction().clamp(0.0, 1.0);
    let target = interpolate_transform(
        interpolation.previous_simulation,
        interpolation.current_simulation,
        alpha,
    );
    let smoothing = half_life_alpha(interpolation.correction_half_life, time.delta_secs());
    let previous_translation = interpolation.previous_render.translation;

    transform.translation = interpolation
        .previous_render
        .translation
        .lerp(target.translation, smoothing);
    transform.rotation = interpolation
        .previous_render
        .rotation
        .slerp(target.rotation, smoothing);
    transform.scale = interpolation
        .previous_render
        .scale
        .lerp(target.scale, smoothing);

    interpolation.correction_velocity = if time.delta_secs() > 0.0 {
        (transform.translation - previous_translation) / time.delta_secs()
    } else {
        Vec3::ZERO
    };
    interpolation.previous_render = *transform;
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    schedule_profiler.record_elapsed(ClientScheduleSystem::RenderInterpolation, started);
}

#[allow(
    clippy::type_complexity,
    reason = "Bevy system query tuple keeps the player movement read/write set explicit for scheduler analysis"
)]
fn apply_kinematic_movement(
    time: Res<Time>,
    mut commands: Commands,
    world_status: Res<ClientWorldStatus>,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut schedule_profiler: ResMut<
        ClientScheduleProfiler,
    >,
    #[cfg(all(feature = "render_diagnostics", debug_assertions))] mut frame_profiler: ResMut<
        DetailedFrameProfiler,
    >,
    player: Single<
        (
            Entity,
            &Collider,
            &MovementSettings,
            &LookSettings,
            &mut MovementInputState,
            &mut CharacterVelocity,
            &mut MovementContactCache,
            &mut NetworkInterpolationState,
            &mut Transform,
        ),
        With<Player>,
    >,
    move_and_slide: MoveAndSlide,
) {
    crate::frame_profile_start!(started);
    let (
        entity,
        collider,
        movement,
        look,
        mut input_state,
        mut velocity,
        mut contact_cache,
        mut interpolation,
        mut transform,
    ) = player.into_inner();

    if !world_status.ready {
        let spawn = Vec3::from_array(PLAYER_SPAWN);
        if transform.translation.distance_squared(spawn) > 0.0001 {
            game_shared::fun_diag_info!(
                "[client movement] holding player at spawn until streamed world is ready; previous_pos=({:.2},{:.2},{:.2})",
                transform.translation.x,
                transform.translation.y,
                transform.translation.z
            );
        }
        transform.translation = spawn;
        interpolation.reset(*transform);
        velocity.0 = Vec3::ZERO;
        contact_cache.mark_airborne();
        input_state.jump_queued = false;
        commands.entity(entity).remove::<Grounded>();
        #[cfg(all(feature = "render_diagnostics", debug_assertions))]
        schedule_profiler.record_elapsed(ClientScheduleSystem::PhysicsMovement, started);
        crate::frame_profile_elapsed!(
            frame_profiler,
            started,
            "FixedUpdate",
            "apply_kinematic_movement",
        );
        return;
    }

    let delta_seconds = time.delta_secs();
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    contact_cache.advance(delta_seconds);
    let jump_requested = input_state.jump_queued;
    input_state.jump_queued = false;
    if contact_cache.should_probe_before_move(velocity.0, delta_seconds, jump_requested) {
        crate::frame_profile_start!(probe_started);
        if let Some(contact) = probe_ground(collider, &transform, &move_and_slide, &filter) {
            contact_cache.refresh(contact);
        } else if !contact_cache.has_valid_ground() {
            contact_cache.mark_airborne();
        }
        crate::frame_profile_elapsed!(
            frame_profiler,
            probe_started,
            "FixedUpdate",
            "apply_kinematic_movement",
            "probe_ground_before_move",
        );
    }
    let was_grounded = contact_cache.has_valid_ground();

    if was_grounded && velocity.0.y < 0.0 {
        velocity.0.y = 0.0;
    }
    if !was_grounded {
        velocity.0.y =
            (velocity.0.y - movement.gravity * delta_seconds).max(-movement.terminal_velocity);
    }

    let desired_velocity =
        desired_planar_velocity(input_state.movement, look.yaw, movement.max_speed);
    let current_planar = Vec3::new(velocity.0.x, 0.0, velocity.0.z);
    let acceleration = if was_grounded {
        movement.ground_acceleration
    } else {
        movement.air_acceleration
    };
    let planar_delta =
        (desired_velocity - current_planar).clamp_length_max(acceleration * delta_seconds);

    velocity.0.x += planar_delta.x;
    velocity.0.z += planar_delta.z;

    if input_state.movement == Vec2::ZERO {
        let damping = if was_grounded {
            movement.ground_damping
        } else {
            movement.air_damping
        };
        let damping_factor = 1.0 / (1.0 + delta_seconds * damping);
        velocity.0.x *= damping_factor;
        velocity.0.z *= damping_factor;
    }

    let jumped = jump_requested && contact_cache.can_jump();
    if jumped {
        velocity.0.y = movement.jump_impulse;
        contact_cache.mark_airborne();
    }

    let mut move_config = MoveAndSlideConfig::default();
    if was_grounded && !jumped {
        move_config.planes.push(Dir3::Y);
    }

    let walkable_dot = max_slope_dot();
    let mut grounded_now = false;
    let mut had_walkable_move_hit = false;
    let previous_translation = transform.translation;
    crate::frame_profile_start!(move_started);
    let output = move_and_slide.move_and_slide(
        collider,
        transform.translation.adjust_precision(),
        transform.rotation.adjust_precision(),
        velocity.0.adjust_precision(),
        time.delta(),
        &move_config,
        &filter,
        |hit| {
            let normal = hit.normal.f32();
            let up_dot = normal.dot(Vec3::Y);
            if up_dot >= walkable_dot {
                grounded_now = true;
                had_walkable_move_hit = true;
                contact_cache.refresh(GroundContact {
                    entity: hit.entity,
                    normal,
                    point: hit.point.f32(),
                });
                return MoveAndSlideHitResponse::Accept;
            }

            if up_dot > 0.0 {
                let horizontal_normal = Vec3::new(normal.x, 0.0, normal.z).normalize_or_zero();
                if let Ok(wall_normal) = Dir3::new(horizontal_normal) {
                    *hit.normal = wall_normal;
                }
            }

            MoveAndSlideHitResponse::Accept
        },
    );
    crate::frame_profile_elapsed!(
        frame_profiler,
        move_started,
        "FixedUpdate",
        "apply_kinematic_movement",
        "move_and_slide",
    );

    transform.translation = output.position.f32();
    velocity.0 = output.projected_velocity.f32();

    let vertical_delta = transform.translation.y - previous_translation.y;
    if contact_cache.should_probe_after_move(
        had_walkable_move_hit,
        was_grounded,
        vertical_delta,
        input_state.movement,
    ) {
        crate::frame_profile_start!(probe_started);
        if let Some(contact) = probe_ground(collider, &transform, &move_and_slide, &filter) {
            contact_cache.refresh(contact);
            grounded_now = true;
        } else if !had_walkable_move_hit {
            contact_cache.mark_airborne();
        }
        crate::frame_profile_elapsed!(
            frame_profiler,
            probe_started,
            "FixedUpdate",
            "apply_kinematic_movement",
            "probe_ground_after_move",
        );
    } else {
        grounded_now |= contact_cache.has_valid_ground();
    }

    if grounded_now && !jumped && velocity.0.y > 0.0 {
        velocity.0.y = 0.0;
    }
    if grounded_now && velocity.0.y < 0.0 {
        velocity.0.y = 0.0;
    }
    if jumped && velocity.0.y > 0.0 {
        grounded_now = false;
    }

    if grounded_now {
        commands.entity(entity).insert(Grounded);
    } else {
        commands.entity(entity).remove::<Grounded>();
    }

    interpolation.commit_simulation(*transform);
    #[cfg(all(feature = "render_diagnostics", debug_assertions))]
    schedule_profiler.record_elapsed(ClientScheduleSystem::PhysicsMovement, started);
    crate::frame_profile_elapsed!(
        frame_profiler,
        started,
        "FixedUpdate",
        "apply_kinematic_movement",
    );
}

fn probe_ground(
    collider: &Collider,
    transform: &Transform,
    move_and_slide: &MoveAndSlide,
    filter: &SpatialQueryFilter,
) -> Option<GroundContact> {
    let config = ShapeCastConfig::from_max_distance(GROUND_PROBE_DISTANCE);
    move_and_slide
        .spatial_query
        .cast_shape(
            collider,
            transform.translation.adjust_precision(),
            transform.rotation.adjust_precision(),
            Dir3::NEG_Y,
            &config,
            filter,
        )
        .and_then(|hit| {
            let normal = hit.normal1.f32();
            (normal.dot(Vec3::Y) >= max_slope_dot()).then_some(GroundContact {
                entity: hit.entity,
                normal,
                point: hit.point1.f32(),
            })
        })
}

fn desired_planar_velocity(input: Vec2, yaw: f32, max_speed: f32) -> Vec3 {
    let yaw_rotation = Quat::from_rotation_y(yaw);
    let forward = yaw_rotation * -Vec3::Z;
    let right = yaw_rotation * Vec3::X;
    (forward * input.y + right * input.x).normalize_or_zero() * max_speed
}

fn interpolate_transform(previous: Transform, current: Transform, alpha: f32) -> Transform {
    Transform {
        translation: previous.translation.lerp(current.translation, alpha),
        rotation: previous.rotation.slerp(current.rotation, alpha),
        scale: previous.scale.lerp(current.scale, alpha),
    }
}

fn half_life_alpha(half_life: f32, delta_seconds: f32) -> f32 {
    if half_life <= 0.0 {
        1.0
    } else {
        1.0 - 0.5_f32.powf(delta_seconds / half_life)
    }
    .clamp(0.0, 1.0)
}

#[cfg(any(test, feature = "benchmarks"))]
pub fn benchmark_desired_planar_velocity(input: Vec2, yaw: f32, max_speed: f32) -> Vec3 {
    desired_planar_velocity(input, yaw, max_speed)
}

#[cfg(any(test, feature = "benchmarks"))]
pub fn benchmark_max_slope_dot() -> f32 {
    max_slope_dot()
}

fn movement_input(keys: &ButtonInput<KeyCode>) -> Vec2 {
    let right = keys.pressed(KeyCode::KeyD) as i8 - keys.pressed(KeyCode::KeyA) as i8;
    let forward = keys.pressed(KeyCode::KeyW) as i8 - keys.pressed(KeyCode::KeyS) as i8;

    Vec2::new(right as f32, forward as f32).normalize_or_zero()
}

fn max_slope_dot() -> f32 {
    MAX_SLOPE_ANGLE.cos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_host::{FunClientHostStartConfig, FunHostMode};

    #[test]
    fn launcher_and_editor_input_owners_require_free_cursor() {
        let mut host = FunClientHostState::from_start_config(FunClientHostStartConfig {
            mode: FunHostMode::Game,
            ..Default::default()
        });
        assert!(!host_input_owner_requires_free_cursor(Some(&host)));

        host.set_input_owner(FunInputOwner::LauncherUi);
        assert!(host_input_owner_requires_free_cursor(Some(&host)));

        host.set_input_owner(FunInputOwner::EditorUi);
        assert!(host_input_owner_requires_free_cursor(Some(&host)));
    }
}
