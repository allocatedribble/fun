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
                eprintln!("failed to configure CEF UI page: {error:?}");
                std::process::exit(71);
            }
        };
    let cef_ui_compositor = fun_ui_cef::SharedCefUiCompositor::default();
    let cef_ui_browser =
        match fun_ui_cef::CefUiBrowser::create(cef_ui_config, cef_ui_compositor.clone()) {
            Ok(browser) => browser,
            Err(error) => {
                eprintln!("failed to load CEF UI page: {error}");
                std::process::exit(71);
            }
        };
    eprintln!(
        "loaded CEF UI page {} at {}x{}",
        cef_ui_browser.config().page_url_str(),
        viewport_width,
        viewport_height
    );

    let mut app = game_client::build_client_app();
    app.insert_resource(game_client::cef_ui::CefUiRenderCompositor::new(
        cef_ui_compositor,
    ));
    if matches!(
        cef_message_loop_strategy,
        fun_ui_cef::CefMessageLoopStrategy::ExternalPump
    ) {
        app.insert_resource(game_client::cef_ui::CefUiMessageLoopPump::external_pump_60hz());
    }
    app.run();
    drop(cef_ui_browser);
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
