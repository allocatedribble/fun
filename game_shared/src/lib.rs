pub const GAME_TITLE: &str = "Fun";
pub mod diagnostics;
pub mod editor_inspector;
pub mod editor_protocol;
pub mod editor_schema;
pub mod render_catalog;

pub use diagnostics::*;
pub use editor_inspector::*;
pub use editor_protocol::*;
pub use editor_schema::*;
pub use render_catalog::*;

pub const DEFAULT_TICK_RATE_HZ: f64 = 60.0;
pub const DEFAULT_RENDER_TARGET_RATE_HZ: f64 = 144.0;
pub const DEFAULT_CORRECTION_HALF_LIFE_SECONDS: f32 = 0.075;
pub const FUN_BACKEND_ADDR: &str = "127.0.0.1:8787";
pub const FUN_BACKEND_WORKSPACE_DIR: &str = "../fun-backend";
pub const GAME_SERVER_BIND_ADDR: &str = "0.0.0.0:6000";
pub const GAME_SERVER_ADDR: &str = "127.0.0.1:6000";
pub const GAME_SERVER_CERT_FILE: &str = "target/quinnet/game_server_cert.pem";
pub const GAME_SERVER_KEY_FILE: &str = "target/quinnet/game_server_key.pem";
pub const GAME_SERVER_KNOWN_HOSTS_FILE: &str = "target/quinnet/game_server_known_hosts";
pub const DEMO_LEVEL_ID: &str = "demo_arena";

pub const GAME_CLIENT_PACKAGE: &str = "game_client";
pub const GAME_SERVER_PACKAGE: &str = "game_server";
pub const FUN_BACKEND_PACKAGE: &str = "fun-backend";

pub const PLAYER_SPAWN: [f32; 3] = [0.0, 4.0, 0.0];
