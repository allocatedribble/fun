#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(feature = "cef_ui")]
fn main() {
    if let Some(exit_code) = fun_ui_cef::maybe_execute_cef_subprocess().handled_exit_code() {
        std::process::exit(exit_code);
    }

    let cef_runtime_config = fun_ui_cef::CefRuntimeConfig::from_env();
    let cef_message_loop_strategy = cef_runtime_config.message_loop_strategy;
    let cef_runtime = match fun_ui_cef::CefRuntime::initialize(cef_runtime_config) {
        Ok(runtime) => runtime,
        Err(_error) => std::process::exit(70),
    };
    let (viewport_width, viewport_height) = cef_ui_viewport_from_env();
    let cef_ui_config =
        match fun_ui_cef::BrowserUiConfig::main_window_from_env(viewport_width, viewport_height) {
            Ok(config) => config,
            Err(error) => {
                tracing::error!(target: "fun::cef_ui", ?error, "failed to configure CEF UI page");
                std::process::exit(71);
            }
        };
    let cef_ui_compositor = fun_ui_cef::SharedCefUiCompositor::default();
    let cef_ui_bridge_queues = fun_ui_cef::SharedBrowserBridgeQueues::default();
    let cef_ui_startup_config = game_client::cef_ui::CefUiStartupConfig::new(
        cef_ui_config,
        cef_ui_compositor,
        cef_ui_bridge_queues,
        cef_message_loop_strategy,
    );
    let requested_paint_transport = game_client::cef_ui::CefUiRequestedPaintTransportResource::new(
        cef_ui_startup_config
            .browser_config()
            .requested_paint_transport,
    );

    {
        let mut app = game_client::build_client_app();
        app.insert_resource(cef_ui_startup_config);
        app.insert_resource(requested_paint_transport);
        app.run();
    }
    cef_runtime.shutdown();
}

#[cfg(feature = "cef_ui")]
fn cef_ui_viewport_from_env() -> (u32, u32) {
    let width = nonzero_env_u32("FUN_WINDOW_WIDTH").unwrap_or(1280);
    let height = nonzero_env_u32("FUN_WINDOW_HEIGHT").unwrap_or(720);
    (width, height)
}

#[cfg(feature = "cef_ui")]
fn nonzero_env_u32(name: &str) -> Option<u32> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
}

#[cfg(not(feature = "cef_ui"))]
fn main() {
    game_client::build_client_app().run();
}
