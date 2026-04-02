use std::f32::consts::FRAC_PI_2;

use avian3d::{character_controller::move_and_slide::DepenetrationConfig, math::*, prelude::*};
#[cfg(feature = "dlss")]
use bevy::anti_alias::dlss::{
    Dlss, DlssPerfQualityMode, DlssRayReconstructionFeature, DlssRayReconstructionSupported,
};
use bevy::{
    camera::CameraMainTextureUsages,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    render::{render_resource::TextureUsages, view::Msaa},
    solari::prelude::SolariLighting,
    text::{TextColor, TextFont},
    ui::{
        BackgroundColor, GlobalZIndex, Node, PositionType, Val,
        widget::{Text, TextUiWriter},
    },
    window::{CursorGrabMode, CursorOptions},
};
use game_shared::PLAYER_SPAWN;

pub struct FirstPersonControllerPlugin;
pub const PLAYER_RADIUS: Scalar = 0.45;
pub const PLAYER_HEIGHT: Scalar = 0.9;
const SWEEP_INVARIANT_EPSILON: Scalar = 0.001;

impl Plugin for FirstPersonControllerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, (spawn_player, spawn_solari_demo_label))
            .add_systems(PreUpdate, cache_movement_input)
            .add_systems(FixedUpdate, apply_kinematic_movement)
            .add_systems(
                Update,
                (
                    toggle_solari_demo,
                    update_solari_demo_label,
                    update_cursor_grab,
                    apply_look,
                )
                    .chain(),
            );
    }
}

#[derive(Component)]
struct Player;

#[derive(Component)]
#[component(storage = "SparseSet")]
struct Grounded;

#[derive(Component)]
struct YawPivot;

#[derive(Component)]
struct PitchPivot;

#[derive(Component)]
struct SolariDemoCamera;

#[derive(Component)]
struct SolariDemoLabel;

#[derive(Component)]
struct MovementSettings {
    max_speed: Scalar,
    ground_acceleration: Scalar,
    air_acceleration: Scalar,
    jump_impulse: Scalar,
    ground_damping: Scalar,
    air_damping: Scalar,
    gravity: Scalar,
    terminal_velocity: Scalar,
    max_slope_angle: Scalar,
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
            max_slope_angle: 45.0_f32.to_radians(),
        }
    }
}

#[derive(Component)]
struct GroundProbe {
    cast_shape: Collider,
    max_distance: Scalar,
}

#[derive(Component, Clone)]
struct MovementCollisionSettings {
    move_and_slide: MoveAndSlideConfig,
    substeps: usize,
    verification_steps: usize,
}

impl Default for MovementCollisionSettings {
    fn default() -> Self {
        let mut move_and_slide = MoveAndSlideConfig::default();
        move_and_slide.move_and_slide_iterations = 8;
        move_and_slide.depenetration_iterations = 32;
        move_and_slide.penetration_rejection_threshold = 2.0;
        move_and_slide.skin_width = 0.02;

        Self {
            move_and_slide,
            substeps: 4,
            verification_steps: 12,
        }
    }
}

#[derive(Component, Default)]
struct MovementInputState {
    movement: Vec2,
    jump_queued: bool,
}

#[derive(Component)]
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

#[derive(Debug)]
struct VelocityDecomposition {
    normal_part: Vector,
    horizontal_tangent: Vector,
    vertical_tangent: Vector,
}

fn spawn_player(
    mut commands: Commands,
    #[cfg(feature = "dlss")] dlss_rr_supported: Option<Res<DlssRayReconstructionSupported>>,
) {
    let collider = Collider::capsule(PLAYER_RADIUS, PLAYER_HEIGHT);
    let mut ground_shape = collider.clone();
    ground_shape.set_scale(Vector::ONE * 0.96, 10);
    #[cfg(feature = "dlss")]
    let dlss_rr_supported = dlss_rr_supported.is_some();

    commands
        .spawn((
            Player,
            Name::new("Player"),
            RigidBody::Kinematic,
            CustomPositionIntegration,
            SpeculativeMargin(0.0),
            collider,
            GroundProbe {
                cast_shape: ground_shape,
                max_distance: 0.2,
            },
            MovementSettings::default(),
            MovementCollisionSettings::default(),
            MovementInputState::default(),
            LookSettings::default(),
            Transform::from_xyz(PLAYER_SPAWN[0], PLAYER_SPAWN[1], PLAYER_SPAWN[2]),
        ))
        .with_children(|player| {
            player
                .spawn((
                    YawPivot,
                    Name::new("YawPivot"),
                    Transform::from_xyz(0.0, 0.75, 0.0),
                ))
                .with_children(|yaw| {
                    yaw.spawn((PitchPivot, Name::new("PitchPivot"), Transform::default()))
                        .with_children(|pitch| {
                            #[cfg(not(feature = "dlss"))]
                            pitch.spawn((
                                Camera3d::default(),
                                SolariDemoCamera,
                                Camera {
                                    clear_color: ClearColorConfig::Custom(Color::BLACK),
                                    ..default()
                                },
                                SolariLighting::default(),
                                CameraMainTextureUsages::default()
                                    .with(TextureUsages::STORAGE_BINDING),
                                Msaa::Off,
                                Projection::from(PerspectiveProjection {
                                    fov: 75.0_f32.to_radians(),
                                    ..default()
                                }),
                                Transform::default(),
                            ));

                            #[cfg(feature = "dlss")]
                            {
                                let mut camera = pitch.spawn((
                                    Camera3d::default(),
                                    SolariDemoCamera,
                                    Camera {
                                        clear_color: ClearColorConfig::Custom(Color::BLACK),
                                        ..default()
                                    },
                                    SolariLighting::default(),
                                    CameraMainTextureUsages::default()
                                        .with(TextureUsages::STORAGE_BINDING),
                                    Msaa::Off,
                                    Projection::from(PerspectiveProjection {
                                        fov: 75.0_f32.to_radians(),
                                        ..default()
                                    }),
                                    Transform::default(),
                                ));

                                if dlss_rr_supported {
                                    camera.insert(Dlss::<DlssRayReconstructionFeature> {
                                        perf_quality_mode: DlssPerfQualityMode::Quality,
                                        reset: true,
                                        _phantom_data: Default::default(),
                                    });
                                }
                            }
                        });
                });
        });
}

fn spawn_solari_demo_label(mut commands: Commands) {
    commands.spawn((
        Text::new("Solari [F6]: On"),
        TextFont {
            font_size: 14.0,
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.0),
            top: Val::Px(12.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.08, 0.09, 0.11, 0.82)),
        GlobalZIndex(i32::MAX - 48),
        SolariDemoLabel,
    ));
}

fn toggle_solari_demo(
    keys: Res<ButtonInput<KeyCode>>,
    mut commands: Commands,
    camera: Single<(Entity, Option<&SolariLighting>), With<SolariDemoCamera>>,
) {
    if !keys.just_pressed(KeyCode::F6) {
        return;
    }

    let (camera_entity, solari_lighting) = camera.into_inner();
    if solari_lighting.is_some() {
        commands.entity(camera_entity).remove::<SolariLighting>();
    } else {
        commands
            .entity(camera_entity)
            .insert(SolariLighting::default());
    }
}

fn update_solari_demo_label(
    camera: Single<Option<&SolariLighting>, With<SolariDemoCamera>>,
    label: Single<Entity, With<SolariDemoLabel>>,
    mut writer: TextUiWriter,
) {
    let state = if camera.is_some() { "On" } else { "Off" };
    *writer.text(*label, 0) = format!("Solari [F6]: {state}");
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
    move_and_slide: MoveAndSlide,
    mut commands: Commands,
    player: Single<
        (
            Entity,
            &MovementSettings,
            &MovementCollisionSettings,
            &GroundProbe,
            &LookSettings,
            &Collider,
            &mut MovementInputState,
            &mut Transform,
            &mut LinearVelocity,
        ),
        With<Player>,
    >,
) {
    let (
        entity,
        movement,
        collision,
        ground_probe,
        look,
        collider,
        mut input_state,
        mut transform,
        mut velocity,
    ) = player.into_inner();

    let position = transform.translation.adjust_precision();
    let rotation = transform.rotation.adjust_precision();
    let was_grounded = is_grounded(
        entity,
        ground_probe,
        position,
        rotation,
        movement.max_slope_angle,
        &move_and_slide,
    );
    let delta_seconds = time.delta_secs_f64().adjust_precision();

    if was_grounded {
        velocity.y = velocity.y.max(0.0);
    } else {
        velocity.y =
            (velocity.y - movement.gravity * delta_seconds).max(-movement.terminal_velocity);
    }

    let desired_velocity =
        desired_planar_velocity(input_state.movement, look.yaw, movement.max_speed);
    let current_planar = Vector::new(velocity.x, 0.0, velocity.z);
    let acceleration = if was_grounded {
        movement.ground_acceleration
    } else {
        movement.air_acceleration
    };
    let planar_delta =
        (desired_velocity - current_planar).clamp_length_max(acceleration * delta_seconds);

    velocity.x += planar_delta.x;
    velocity.z += planar_delta.z;

    if input_state.movement == Vec2::ZERO {
        let damping = if was_grounded {
            movement.ground_damping
        } else {
            movement.air_damping
        };
        let damping_factor = 1.0 / (1.0 + delta_seconds * damping);
        velocity.x *= damping_factor;
        velocity.z *= damping_factor;
    }

    let jump_requested = input_state.jump_queued;
    input_state.jump_queued = false;
    if jump_requested && was_grounded {
        velocity.y = movement.jump_impulse;
    }

    let max_slope_angle = movement.max_slope_angle;
    let filter = SpatialQueryFilter::from_excluded_entities([entity]);
    let substeps = collision.substeps.max(1);
    let step_delta_seconds = delta_seconds / substeps as Scalar;
    let mut current_position = position;
    let mut is_grounded_now = was_grounded && !jump_requested;

    for _ in 0..substeps {
        let step_result = sweep_character_step(
            &move_and_slide,
            collider,
            current_position,
            rotation,
            velocity.0,
            step_delta_seconds,
            &collision.move_and_slide,
            collision.verification_steps,
            is_grounded_now,
            max_slope_angle,
            &filter,
        );

        current_position = step_result.position;
        velocity.0 = step_result.projected_velocity;

        if step_result.hit_walkable_surface && velocity.y < 0.0 {
            velocity.y = 0.0;
        }
        if step_result.hit_ceiling && velocity.y > 0.0 {
            velocity.y = 0.0;
        }

        is_grounded_now = if jump_requested && was_grounded {
            false
        } else {
            step_result.hit_walkable_surface
                || is_grounded(
                    entity,
                    ground_probe,
                    current_position,
                    rotation,
                    movement.max_slope_angle,
                    &move_and_slide,
                )
        };
    }

    transform.translation = current_position.f32();

    if is_grounded_now {
        commands.entity(entity).insert(Grounded);
    } else {
        commands.entity(entity).remove::<Grounded>();
    }
}

fn is_grounded(
    entity: Entity,
    ground_probe: &GroundProbe,
    translation: Vector,
    rotation: Quaternion,
    max_slope_angle: Scalar,
    move_and_slide: &MoveAndSlide,
) -> bool {
    move_and_slide
        .cast_move(
            &ground_probe.cast_shape,
            translation,
            rotation,
            Vector::NEG_Y * ground_probe.max_distance,
            0.0,
            &SpatialQueryFilter::from_excluded_entities([entity]),
        )
        .is_some_and(|hit| {
            let ground_normal = rotation * hit.normal1;
            ground_normal.angle_between(Vector::Y) <= max_slope_angle
        })
}

fn desired_planar_velocity(input: Vec2, yaw: f32, max_speed: Scalar) -> Vector {
    let yaw_rotation = Quat::from_rotation_y(yaw);
    let forward = yaw_rotation * -Vec3::Z;
    let right = yaw_rotation * Vec3::X;
    ((forward * input.y + right * input.x).normalize_or_zero() * max_speed).adjust_precision()
}

#[derive(Debug)]
struct CharacterStepResult {
    position: Vector,
    projected_velocity: Vector,
    hit_walkable_surface: bool,
    hit_ceiling: bool,
}

fn sweep_character_step(
    move_and_slide: &MoveAndSlide,
    collider: &Collider,
    start_position: Vector,
    rotation: Quaternion,
    mut velocity: Vector,
    delta_seconds: Scalar,
    config: &MoveAndSlideConfig,
    verification_steps: usize,
    was_grounded: bool,
    max_slope_angle: Scalar,
    filter: &SpatialQueryFilter,
) -> CharacterStepResult {
    let up = Vector::Y;
    let mut position = start_position;
    let mut hit_walkable_surface = false;
    let mut hit_ceiling = false;
    let mut time_left = delta_seconds;
    let depenetration_config = DepenetrationConfig::from(config);
    let mut clipping_planes = Vec::with_capacity(config.max_planes.min(8));

    if was_grounded {
        clipping_planes.push(Dir::Y);
    }

    position += move_and_slide.depenetrate(
        collider,
        position,
        rotation,
        &depenetration_config,
        filter,
    );
    let swept_start = position;

    for _ in 0..config.move_and_slide_iterations {
        let sweep = velocity * time_left;
        let Ok((direction, distance)) = Dir::new_and_length(sweep.f32()) else {
            break;
        };
        let distance = distance.adjust_precision();
        if distance < 1e-4 {
            break;
        }

        let hit = move_and_slide.cast_move(collider, position, rotation, sweep, config.skin_width, filter);
        let Some(hit) = hit else {
            position += sweep;
            break;
        };

        position += direction.adjust_precision() * hit.distance;
        time_left -= time_left * (hit.distance / distance);

        let normal = Dir::new_unchecked(hit.normal1.f32());
        let normal_vec = normal.adjust_precision();
        let is_walkable = up.angle_between(normal_vec) <= max_slope_angle;

        if is_walkable {
            hit_walkable_surface = true;

            let decomposition = decompose_velocity(velocity, normal_vec, up);
            if up.dot(decomposition.vertical_tangent) < -0.001 {
                velocity = decomposition.horizontal_tangent + decomposition.normal_part;
            }
        } else if up.dot(normal_vec) < 0.0 {
            hit_ceiling = true;
        }

        clipping_planes.push(normal);
        velocity = MoveAndSlide::project_velocity(velocity, &clipping_planes);

        position += move_and_slide.depenetrate(
            collider,
            position,
            rotation,
            &depenetration_config,
            filter,
        );

        if time_left <= 0.0 {
            break;
        }
    }

    verify_swept_segment(
        move_and_slide,
        collider,
        swept_start,
        &mut position,
        rotation,
        &mut velocity,
        &depenetration_config,
        verification_steps,
        max_slope_angle,
        filter,
        &mut clipping_planes,
        &mut hit_walkable_surface,
        &mut hit_ceiling,
    );

    CharacterStepResult {
        position,
        projected_velocity: velocity,
        hit_walkable_surface,
        hit_ceiling,
    }
}

fn verify_swept_segment(
    move_and_slide: &MoveAndSlide,
    collider: &Collider,
    start_position: Vector,
    position: &mut Vector,
    rotation: Quaternion,
    velocity: &mut Vector,
    depenetration_config: &DepenetrationConfig,
    verification_steps: usize,
    max_slope_angle: Scalar,
    filter: &SpatialQueryFilter,
    clipping_planes: &mut Vec<Dir>,
    hit_walkable_surface: &mut bool,
    hit_ceiling: &mut bool,
) {
    let total_motion = *position - start_position;
    let steps = verification_steps.max(1);
    let step_motion = total_motion / steps as Scalar;
    let up = Vector::Y;
    let mut verified_position = start_position;

    for _ in 0..steps {
        let Ok((direction, distance)) = Dir::new_and_length(step_motion.f32()) else {
            break;
        };
        let distance = distance.adjust_precision();
        if distance < 1e-4 {
            break;
        }

        let Some(hit) =
            move_and_slide.cast_move(collider, verified_position, rotation, step_motion, 0.0, filter)
        else {
            verified_position += step_motion;
            verified_position += move_and_slide.depenetrate(
                collider,
                verified_position,
                rotation,
                depenetration_config,
                filter,
            );
            continue;
        };

        if hit.distance + SWEEP_INVARIANT_EPSILON >= distance {
            verified_position += step_motion;
            verified_position += move_and_slide.depenetrate(
                collider,
                verified_position,
                rotation,
                depenetration_config,
                filter,
            );
            continue;
        }

        verified_position += direction.adjust_precision()
            * (hit.distance - SWEEP_INVARIANT_EPSILON).max(0.0);

        let normal = Dir::new_unchecked(hit.normal1.f32());
        let normal_vec = normal.adjust_precision();
        let is_walkable = up.angle_between(normal_vec) <= max_slope_angle;
        if is_walkable {
            *hit_walkable_surface = true;
        } else if up.dot(normal_vec) < 0.0 {
            *hit_ceiling = true;
        }

        push_clipping_plane(clipping_planes, normal);
        *velocity = MoveAndSlide::project_velocity(*velocity, clipping_planes);
        verified_position += move_and_slide.depenetrate(
            collider,
            verified_position,
            rotation,
            depenetration_config,
            filter,
        );
        *position = verified_position;
        return;
    }

    *position = verified_position;
}

fn push_clipping_plane(planes: &mut Vec<Dir>, normal: Dir) {
    if planes.iter().any(|existing| existing.dot(*normal) >= 0.999) {
        return;
    }
    planes.push(normal);
}

fn decompose_velocity(velocity: Vector, normal: Vector, up: Vector) -> VelocityDecomposition {
    let normal_part = normal * normal.dot(velocity);
    let tangent_part = velocity - normal_part;

    let horizontal_tangent_dir = normal.cross(up).normalize_or_zero();
    let horizontal_tangent = tangent_part.dot(horizontal_tangent_dir) * horizontal_tangent_dir;
    let vertical_tangent = tangent_part - horizontal_tangent;

    VelocityDecomposition {
        normal_part,
        horizontal_tangent,
        vertical_tangent,
    }
}

fn movement_input(keys: &ButtonInput<KeyCode>) -> Vec2 {
    let right = keys.pressed(KeyCode::KeyD) as i8 - keys.pressed(KeyCode::KeyA) as i8;
    let forward = keys.pressed(KeyCode::KeyW) as i8 - keys.pressed(KeyCode::KeyS) as i8;

    Vec2::new(right as f32, forward as f32).normalize_or_zero()
}
