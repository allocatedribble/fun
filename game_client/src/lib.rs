pub mod first_person;
pub mod lighting;

use std::time::Duration;

use avian3d::prelude::*;
#[cfg(feature = "dlss")]
use bevy::render::{
    RenderPlugin,
    settings::{Backends, RenderCreation, WgpuSettings},
};
#[cfg(feature = "dlss")]
use bevy::{anti_alias::dlss::DlssProjectId, asset::uuid::Uuid};
use bevy::{
    dev_tools::fps_overlay::{FpsOverlayConfig, FpsOverlayPlugin},
    mesh::Mesh,
    prelude::*,
    solari::prelude::{RaytracingMesh3d, SolariPlugins},
    window::PresentMode,
};
use bevy_quinnet::client::QuinnetClientPlugin;
use first_person::FirstPersonControllerPlugin;
use game_shared::GAME_TITLE;

#[cfg(feature = "dlss")]
const DLSS_PROJECT_ID: &str = "7f2c56d9-bbd1-40e6-aeea-ad1cde733e2e";

pub struct GameClientPlugin;

impl Plugin for GameClientPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            PhysicsPlugins::default(),
            QuinnetClientPlugin::default(),
            SolariPlugins,
            FpsOverlayPlugin {
                config: FpsOverlayConfig {
                    refresh_interval: Duration::from_secs(1),
                    text_config: bevy::text::TextFont {
                        font_size: 12.0,
                        ..default()
                    },
                    ..default()
                },
            },
            FirstPersonControllerPlugin,
        ))
        .add_systems(Startup, setup_scene);
    }
}

pub fn build_client_app() -> App {
    let mut app = App::new();

    #[cfg(feature = "dlss")]
    app.insert_resource(DlssProjectId(
        Uuid::parse_str(DLSS_PROJECT_ID).expect("DLSS project ID should be a valid UUID"),
    ));

    let default_plugins = DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: format!("{GAME_TITLE} Client").into(),
            present_mode: PresentMode::AutoNoVsync,
            ..default()
        }),
        ..default()
    });
    #[cfg(feature = "dlss")]
    let default_plugins = default_plugins.set(RenderPlugin {
        render_creation: RenderCreation::Automatic(WgpuSettings {
            backends: Some(Backends::VULKAN),
            ..default()
        }),
        ..default()
    });

    app.add_plugins(default_plugins)
        .add_plugins(GameClientPlugin);

    app
}

fn setup_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let floor_mesh = meshes.add(
        Plane3d::default()
            .mesh()
            .size(60.0, 60.0)
            .build()
            .with_generated_tangents()
            .expect("floor mesh should support tangent generation"),
    );
    let wall_mesh = meshes.add(
        Cuboid::new(5.0, 3.0, 1.0)
            .mesh()
            .build()
            .with_generated_tangents()
            .expect("wall mesh should support tangent generation"),
    );
    let ramp_mesh = meshes.add(
        Cuboid::new(3.0, 0.5, 6.0)
            .mesh()
            .build()
            .with_generated_tangents()
            .expect("ramp mesh should support tangent generation"),
    );
    let cube_mesh = meshes.add(
        Cuboid::new(1.0, 1.0, 1.0)
            .mesh()
            .build()
            .with_generated_tangents()
            .expect("cube mesh should support tangent generation"),
    );

    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.0,
            shadows_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(30.0, 0.5, 30.0),
        RaytracingMesh3d(floor_mesh.clone()),
        Mesh3d(floor_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.16, 0.19, 0.16))),
    ));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(2.5, 1.5, 0.5),
        RaytracingMesh3d(wall_mesh.clone()),
        Mesh3d(wall_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.32, 0.35, 0.42))),
        Transform::from_xyz(0.0, 1.5, -8.0),
    ));

    commands.spawn((
        RigidBody::Static,
        Collider::cuboid(1.5, 0.25, 3.0),
        RaytracingMesh3d(ramp_mesh.clone()),
        Mesh3d(ramp_mesh),
        MeshMaterial3d(materials.add(Color::srgb(0.38, 0.28, 0.22))),
        Transform::from_xyz(-6.0, 0.25, -2.0)
            .with_rotation(Quat::from_rotation_z(-12.0_f32.to_radians())),
    ));

    for (x, y, z) in [(3.0, 1.0, 2.0), (5.0, 1.0, -1.5), (7.0, 2.0, 4.0)] {
        commands.spawn((
            RigidBody::Dynamic,
            Collider::cuboid(0.5, 0.5, 0.5),
            RaytracingMesh3d(cube_mesh.clone()),
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(materials.add(Color::srgb(0.8, 0.4, 0.3))),
            Transform::from_xyz(x, y, z),
        ));
    }
}
