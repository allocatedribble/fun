use std::time::Duration;

use avian3d::prelude::*;
use bevy::{app::ScheduleRunnerPlugin, prelude::*};
use bevy_quinnet::server::QuinnetServerPlugin;
use game_shared::{BACKEND_SERVER_ADDR, DEFAULT_TICK_RATE_HZ, GAME_TITLE};

fn main() {
    App::new()
        .add_plugins((
            MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
                1.0 / DEFAULT_TICK_RATE_HZ,
            ))),
            PhysicsPlugins::default(),
            QuinnetServerPlugin::default(),
        ))
        .add_systems(Startup, startup)
        .run();
}

fn startup() {
    println!(
        "Starting {GAME_TITLE} game server at {DEFAULT_TICK_RATE_HZ:.0} Hz. Backend target: {BACKEND_SERVER_ADDR}"
    );
}
