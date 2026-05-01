pub const GAME_TITLE: &str = "Fun";
pub mod diagnostics;
pub mod editor_protocol;
pub mod render_catalog;

pub use editor_protocol::*;
pub use render_catalog::*;

pub const DEFAULT_TICK_RATE_HZ: f64 = 60.0;
pub const DEFAULT_RENDER_TARGET_RATE_HZ: f64 = 144.0;
pub const DEFAULT_CORRECTION_HALF_LIFE_SECONDS: f32 = 0.075;
pub const BACKEND_SERVER_ADDR: &str = "127.0.0.1:8080";
pub const GAME_SERVER_BIND_ADDR: &str = "0.0.0.0:6000";
pub const GAME_SERVER_ADDR: &str = "127.0.0.1:6000";
pub const DEMO_LEVEL_ID: &str = "demo_arena";

pub const GAME_CLIENT_PACKAGE: &str = "game_client";
pub const GAME_SERVER_PACKAGE: &str = "game_server";
pub const BACKEND_SERVER_PACKAGE: &str = "backend_server";

pub const PLAYER_SPAWN: [f32; 3] = [0.0, 4.0, 0.0];
