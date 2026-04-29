#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    game_client::build_client_app().run();
}
