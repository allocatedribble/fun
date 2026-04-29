use std::f32::consts::FRAC_PI_2;

use crate::ClientWorldStatus;
use avian3d::{
    math::{AdjustPrecision as _, AsF32 as _},
    prelude::{
        Collider, MoveAndSlide, MoveAndSlideConfig, MoveAndSlideHitResponse, RigidBody,
        ShapeCastConfig, SpatialQueryFilter,
    },
};
use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::view::Msaa,
    scene::{
        prelude::{CommandsSceneExt, Scene as BsnScene, bsn},
        template_value,
    },
    window::{CursorGrabMode, CursorOptions},
};
use game_shared::PLAYER_SPAWN;

pub struct FirstPersonControllerPlugin;
pub const PLAYER_RADIUS: f32 = 0.45;
pub const PLAYER_HALF_HEIGHT: f32 = 0.9;
const PLAYER_CAPSULE_LENGTH: f32 = PLAYER_HALF_HEIGHT * 2.0 - PLAYER_RADIUS * 2.0;
const MAX_SLOPE_ANGLE: f32 = 45.0_f32.to_radians();
const GROUND_PROBE_DISTANCE: f32 = 0.08;

impl Plugin for FirstPersonControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            .add_systems(PreUpdate, cache_movement_input)
            .add_systems(FixedUpdate, apply_kinematic_movement)
            .add_systems(Update, (update_cursor_grab, apply_look).chain());
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
    commands.spawn_scene(player_scene(base_camera_scene()));
}

fn player_scene(camera: impl BsnScene) -> impl BsnScene {
    bsn! {
        #Player
        Player
        Name::new("Player")
        MovementSettings::default()
        MovementInputState::default()
        CharacterVelocity::default()
        LookSettings::default()
        template_value(RigidBody::Kinematic)
        Collider::capsule(PLAYER_RADIUS, PLAYER_CAPSULE_LENGTH)
        Visibility::default()
        Transform::from_xyz(PLAYER_SPAWN[0], PLAYER_SPAWN[1], PLAYER_SPAWN[2])
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

fn base_camera_scene() -> impl BsnScene {
    bsn! {
        Camera3d
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
        }
        template_value(Msaa::Off)
        template_value(Projection::from(PerspectiveProjection {
            fov: 75.0_f32.to_radians(),
            ..default()
        }))
        Transform::default()
    }
}

fn cache_movement_input(
    keys: Res<ButtonInput<KeyCode>>,
    mut players: Query<&mut MovementInputState, With<Player>>,
) {
    let Ok(mut input_state) = players.single_mut() else {
        return;
    };

    input_state.movement = movement_input(&keys);
    input_state.jump_queued |= keys.just_pressed(KeyCode::Space);
}

fn update_cursor_grab(
    mut cursor_options: Single<&mut CursorOptions>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
) {
    if mouse_buttons.just_pressed(MouseButton::Left) {
        cursor_options.visible = false;
        cursor_options.grab_mode = CursorGrabMode::Locked;
    }

    if keys.just_pressed(KeyCode::Escape) {
        cursor_options.visible = true;
        cursor_options.grab_mode = CursorGrabMode::None;
    }
}

fn apply_look(
    accumulated_mouse_motion: Res<AccumulatedMouseMotion>,
    cursor_options: Single<&CursorOptions>,
    mut player: Single<&mut LookSettings, With<Player>>,
    mut yaw_pivot: Single<&mut Transform, (With<YawPivot>, Without<PitchPivot>)>,
    mut pitch_pivot: Single<&mut Transform, (With<PitchPivot>, Without<YawPivot>)>,
) {
    if cursor_options.grab_mode == CursorGrabMode::None {
        return;
    }

    let delta = accumulated_mouse_motion.delta;
    if delta == Vec2::ZERO {
        return;
    }

    player.yaw -= delta.x * player.sensitivity.x;
    player.pitch =
        (player.pitch - delta.y * player.sensitivity.y).clamp(-FRAC_PI_2 + 0.01, FRAC_PI_2 - 0.01);

    yaw_pivot.rotation = Quat::from_rotation_y(player.yaw);
    pitch_pivot.rotation = Quat::from_rotation_x(player.pitch);
}

fn apply_kinematic_movement(
    time: Res<Time>,
    mut commands: Commands,
    world_status: Res<ClientWorldStatus>,
    player: Single<
        (
            Entity,
            &Collider,
            &MovementSettings,
            &LookSettings,
            &mut MovementInputState,
            &mut CharacterVelocity,
            &mut Transform,
        ),
        With<Player>,
    >,
    move_and_slide: MoveAndSlide,
) {
    let (entity, collider, movement, look, mut input_state, mut velocity, mut transform) =
        player.into_inner();

    if !world_status.ready {
        let spawn = Vec3::from_array(PLAYER_SPAWN);
        if transform.translation.distance_squared(spawn) > 0.0001 {
            println!(
                "[client movement] holding player at spawn until streamed world is ready; previous_pos=({:.2},{:.2},{:.2})",
                transform.translation.x, transform.translation.y, transform.translation.z
            );
        }
        transform.translation = spawn;
        velocity.0 = Vec3::ZERO;
        input_state.jump_queued = false;
        commands.entity(entity).remove::<Grounded>();
        return;
    }

    let delta_seconds = time.delta_secs();
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let was_grounded = is_grounded(collider, &transform, &move_and_slide, &filter);

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

    let jump_requested = input_state.jump_queued;
    input_state.jump_queued = false;
    if jump_requested && was_grounded {
        velocity.0.y = movement.jump_impulse;
    }

    let mut move_config = MoveAndSlideConfig::default();
    if was_grounded {
        move_config.planes.push(Dir3::Y);
    }

    let walkable_dot = max_slope_dot();
    let mut grounded_now = false;
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

    transform.translation = output.position.f32();
    velocity.0 = output.projected_velocity.f32();

    grounded_now |= is_grounded(collider, &transform, &move_and_slide, &filter);

    if grounded_now && !jump_requested && velocity.0.y > 0.0 {
        velocity.0.y = 0.0;
    }
    if grounded_now && velocity.0.y < 0.0 {
        velocity.0.y = 0.0;
    }
    if jump_requested && velocity.0.y > 0.0 {
        grounded_now = false;
    }

    if grounded_now {
        commands.entity(entity).insert(Grounded);
    } else {
        commands.entity(entity).remove::<Grounded>();
    }
}

fn is_grounded(
    collider: &Collider,
    transform: &Transform,
    move_and_slide: &MoveAndSlide,
    filter: &SpatialQueryFilter,
) -> bool {
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
        .is_some_and(|hit| hit.normal1.f32().dot(Vec3::Y) >= max_slope_dot())
}

fn desired_planar_velocity(input: Vec2, yaw: f32, max_speed: f32) -> Vec3 {
    let yaw_rotation = Quat::from_rotation_y(yaw);
    let forward = yaw_rotation * -Vec3::Z;
    let right = yaw_rotation * Vec3::X;
    (forward * input.y + right * input.x).normalize_or_zero() * max_speed
}

fn movement_input(keys: &ButtonInput<KeyCode>) -> Vec2 {
    let right = keys.pressed(KeyCode::KeyD) as i8 - keys.pressed(KeyCode::KeyA) as i8;
    let forward = keys.pressed(KeyCode::KeyW) as i8 - keys.pressed(KeyCode::KeyS) as i8;

    Vec2::new(right as f32, forward as f32).normalize_or_zero()
}

fn max_slope_dot() -> f32 {
    MAX_SLOPE_ANGLE.cos()
}
