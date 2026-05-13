#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() -> Result<(), game_client::ClientBootError> {
    game_client::run_client_engine().map(|_exit| ())
}
