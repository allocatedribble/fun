#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(feature = "cef_ui")]
fn main() {
    if let Some(exit_code) = fun_ui_cef::maybe_execute_cef_subprocess().handled_exit_code() {
        std::process::exit(exit_code);
    }

    let cef_runtime =
        match fun_ui_cef::CefRuntime::initialize(fun_ui_cef::CefRuntimeConfig::from_env()) {
            Ok(runtime) => runtime,
            Err(_error) => std::process::exit(70),
        };

    game_client::build_client_app().run();
    cef_runtime.shutdown();
}

#[cfg(not(feature = "cef_ui"))]
fn main() {
    game_client::build_client_app().run();
}
