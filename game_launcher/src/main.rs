use std::process::{Command, ExitCode};

use game_shared::{
    FUN_BACKEND_PACKAGE, FUN_BACKEND_WORKSPACE_DIR, GAME_CLIENT_PACKAGE, GAME_SERVER_PACKAGE,
    GAME_TITLE,
};

#[derive(Debug, Clone, Copy)]
struct LaunchTarget {
    package: &'static str,
    current_dir: Option<&'static str>,
}

#[allow(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "game_launcher is a developer CLI boundary; these messages are direct command output, not runtime diagnostics"
)]
fn main() -> ExitCode {
    let Some(target) = std::env::args().nth(1) else {
        println!(
            "Usage: cargo run -p game_launcher -- [client|server|backend]\n{GAME_TITLE} packages: {GAME_CLIENT_PACKAGE}, {GAME_SERVER_PACKAGE}, {FUN_BACKEND_PACKAGE}"
        );
        return ExitCode::SUCCESS;
    };

    let launch_target = match target.as_str() {
        "client" => LaunchTarget {
            package: GAME_CLIENT_PACKAGE,
            current_dir: None,
        },
        "server" => LaunchTarget {
            package: GAME_SERVER_PACKAGE,
            current_dir: None,
        },
        "backend" => LaunchTarget {
            package: FUN_BACKEND_PACKAGE,
            current_dir: Some(FUN_BACKEND_WORKSPACE_DIR),
        },
        _ => {
            eprintln!("Unknown target '{target}'. Expected one of: client, server, backend.");
            return ExitCode::from(2);
        }
    };

    let mut command = Command::new("cargo");
    if let Some(current_dir) = launch_target.current_dir {
        command.current_dir(current_dir);
    }

    match command.args(["run", "-p", launch_target.package]).status() {
        Ok(status) if status.success() => ExitCode::SUCCESS,
        Ok(status) => {
            eprintln!("Child process exited with status: {status}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("Failed to launch {}: {error}", launch_target.package);
            ExitCode::from(1)
        }
    }
}
