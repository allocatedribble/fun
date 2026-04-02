use std::process::{Command, ExitCode};

use game_shared::{BACKEND_SERVER_PACKAGE, GAME_CLIENT_PACKAGE, GAME_SERVER_PACKAGE, GAME_TITLE};

fn main() -> ExitCode {
    let Some(target) = std::env::args().nth(1) else {
        println!(
            "Usage: cargo run -p game_launcher -- [client|server|backend]\n{GAME_TITLE} workspace packages: {GAME_CLIENT_PACKAGE}, {GAME_SERVER_PACKAGE}, {BACKEND_SERVER_PACKAGE}"
        );
        return ExitCode::SUCCESS;
    };

    let package = match target.as_str() {
        "client" => GAME_CLIENT_PACKAGE,
        "server" => GAME_SERVER_PACKAGE,
        "backend" => BACKEND_SERVER_PACKAGE,
        _ => {
            eprintln!("Unknown target '{target}'. Expected one of: client, server, backend.");
            return ExitCode::from(2);
        }
    };

    match Command::new("cargo").args(["run", "-p", package]).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("Child process exited with status: {status}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("Failed to launch {package}: {error}");
            ExitCode::from(1)
        }
    }
}
