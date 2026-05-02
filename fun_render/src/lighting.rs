use bevy::prelude::*;
use tracing::info;

pub fn setup_lighting(mut commands: Commands) {
    info!("[fun render] spawning directional light");
    commands.spawn((
        DirectionalLight {
            illuminance: 15_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}
