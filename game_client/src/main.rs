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
    let cef_ui_browser = match fun_ui_cef::CefUiBrowser::create_with_bridge(
        cef_ui_config,
        cef_ui_compositor.clone(),
        cef_ui_bridge_queues.clone(),
    ) {
        Ok(browser) => browser,
        Err(error) => {
            tracing::error!(target: "fun::cef_ui", %error, "failed to load CEF UI page");
            std::process::exit(71);
        }
    };
    tracing::info!(
        target: "fun::cef_ui",
        page_url = cef_ui_browser.config().page_url_str(),
        viewport_width,
        viewport_height,
        "loaded CEF UI page"
    );

    let mut app = game_client::build_client_app();
    app.insert_non_send(game_client::cef_ui::CefUiBrowserControl::new(
        cef_ui_browser.handle(),
    ));
    app.insert_resource(game_client::cef_ui::CefUiBridge::new(cef_ui_bridge_queues));
    app.insert_resource(game_client::cef_ui::CefUiRenderCompositor::new(
        cef_ui_compositor,
    ));
    app.insert_resource(game_client::cef_ui::CefUiTransportCountersResource::new(
        cef_ui_browser.transport_counters(),
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
