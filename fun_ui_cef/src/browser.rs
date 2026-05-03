use std::sync::{Arc, Mutex};

use cef::rc::Rc as _;
use cef::{
    Browser, BrowserSettings, CefString, Client, ImplBrowser, ImplBrowserHost, ImplClient,
    ImplLifeSpanHandler, LifeSpanHandler, PaintElementType, Rect, RenderHandler, WindowInfo,
    WrapClient, WrapLifeSpanHandler, browser_host_create_browser, wrap_client,
    wrap_life_span_handler,
};

use crate::diagnostics::FUN_UI_DIAGNOSTICS_TARGET;
use crate::render_handler::CefUiScaleFactor;
use crate::scheme::{
    FUN_UI_MAIN_URL, FunUiDevServerError, FunUiDevServerUrl, fun_ui_dev_server_from_env,
    register_fun_ui_scheme_handler_factory,
};
use crate::{
    compositor::SharedCefUiCompositor,
    render_handler::{CefPaintSink, new_fun_cef_render_handler_for_viewport},
};

pub const MAIN_BROWSER_PAGE: BrowserUiPage = BrowserUiPage {
    url: FUN_UI_MAIN_URL,
    transparent_background: true,
};
pub const CEF_UI_WINDOWLESS_FRAME_RATE_HZ: i32 = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BrowserUiPage {
    pub url: &'static str,
    pub transparent_background: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserUiConfig {
    pub page: BrowserUiPage,
    pub dev_server_url: Option<FunUiDevServerUrl>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub windowless_frame_rate: i32,
}

impl BrowserUiConfig {
    #[must_use]
    pub const fn main_window(viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            page: MAIN_BROWSER_PAGE,
            dev_server_url: None,
            viewport_width,
            viewport_height,
            windowless_frame_rate: CEF_UI_WINDOWLESS_FRAME_RATE_HZ,
        }
    }

    pub fn main_window_from_env(
        viewport_width: u32,
        viewport_height: u32,
    ) -> Result<Self, FunUiDevServerError> {
        Ok(Self {
            dev_server_url: fun_ui_dev_server_from_env()?,
            ..Self::main_window(viewport_width, viewport_height)
        })
    }

    #[must_use]
    pub fn page_url(&self) -> CefString {
        CefString::from(
            self.dev_server_url
                .as_ref()
                .map_or(self.page.url, FunUiDevServerUrl::as_str),
        )
    }

    #[must_use]
    pub fn page_url_str(&self) -> &str {
        self.dev_server_url
            .as_ref()
            .map_or(self.page.url, FunUiDevServerUrl::as_str)
    }

    #[must_use]
    pub const fn transparent_background(&self) -> bool {
        self.page.transparent_background
    }

    #[must_use]
    pub fn browser_settings(&self) -> BrowserSettings {
        BrowserSettings {
            background_color: if self.page.transparent_background {
                0x0000_0000
            } else {
                0xff00_0000
            },
            windowless_frame_rate: self.windowless_frame_rate,
            ..BrowserSettings::default()
        }
    }
}

impl Default for BrowserUiConfig {
    fn default() -> Self {
        Self::main_window(1280, 720)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CefBrowserId(pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserLifecycleState {
    NotCreated,
    Creating,
    Ready { browser_id: CefBrowserId },
    Closing,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserLifecycle {
    state: BrowserLifecycleState,
}

impl BrowserLifecycle {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: BrowserLifecycleState::NotCreated,
        }
    }

    #[must_use]
    pub const fn state(&self) -> BrowserLifecycleState {
        self.state
    }

    pub fn mark_creating(&mut self) {
        if matches!(self.state, BrowserLifecycleState::NotCreated) {
            self.state = BrowserLifecycleState::Creating;
        }
    }

    pub fn mark_ready(&mut self, browser_id: CefBrowserId) {
        self.state = BrowserLifecycleState::Ready { browser_id };
    }

    pub fn begin_close(&mut self) {
        if !matches!(self.state, BrowserLifecycleState::Closed) {
            self.state = BrowserLifecycleState::Closing;
        }
    }

    pub fn mark_closed(&mut self) {
        self.state = BrowserLifecycleState::Closed;
    }
}

impl Default for BrowserLifecycle {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
struct BrowserState {
    lifecycle: BrowserLifecycle,
    browser: Option<Browser>,
}

impl BrowserState {
    const fn new() -> Self {
        Self {
            lifecycle: BrowserLifecycle::new(),
            browser: None,
        }
    }
}

type SharedBrowserState = Arc<Mutex<BrowserState>>;

fn with_browser_state(state: &SharedBrowserState, update: impl FnOnce(&mut BrowserState)) {
    if let Ok(mut state) = state.lock() {
        update(&mut state);
    }
}

#[derive(Debug)]
pub enum CefUiBrowserError {
    DevServer(FunUiDevServerError),
    SchemeFactoryRejected,
    CreationRejected,
}

impl std::fmt::Display for CefUiBrowserError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::DevServer(error) => {
                write!(formatter, "CEF UI dev server URL rejected: {error:?}")
            }
            Self::SchemeFactoryRejected => {
                formatter.write_str("CEF rejected the fun-ui scheme handler factory")
            }
            Self::CreationRejected => formatter.write_str("CEF rejected browser creation"),
        }
    }
}

impl std::error::Error for CefUiBrowserError {}

impl From<FunUiDevServerError> for CefUiBrowserError {
    fn from(value: FunUiDevServerError) -> Self {
        Self::DevServer(value)
    }
}

wrap_life_span_handler! {
    struct FunCefLifeSpanHandler {
        state: SharedBrowserState,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            let browser_id = CefBrowserId(browser.identifier());
            with_browser_state(&self.state, |state| {
                state.lifecycle.mark_ready(browser_id);
                state.browser = Some(browser.clone());
            });
            if let Some(host) = browser.host() {
                host.set_windowless_frame_rate(CEF_UI_WINDOWLESS_FRAME_RATE_HZ);
                host.was_hidden(0);
                host.set_focus(1);
                host.notify_screen_info_changed();
                host.was_resized();
                host.invalidate(PaintElementType::VIEW);
            }
            tracing::info!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                browser_id = browser_id.0,
                render_rate_hz = CEF_UI_WINDOWLESS_FRAME_RATE_HZ,
                "CEF UI browser created"
            );
        }

        fn do_close(&self, _browser: Option<&mut Browser>) -> std::os::raw::c_int {
            with_browser_state(&self.state, |state| state.lifecycle.begin_close());
            0
        }

        fn on_before_close(&self, browser: Option<&mut Browser>) {
            let browser_id = browser.as_ref().map(|browser| CefBrowserId(browser.identifier()));
            with_browser_state(&self.state, |state| {
                state.browser = None;
                state.lifecycle.mark_closed();
            });
            tracing::info!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                browser_id = browser_id.map(|id| id.0),
                "CEF UI browser closed"
            );
        }
    }
}

#[must_use]
fn new_fun_cef_life_span_handler(state: SharedBrowserState) -> LifeSpanHandler {
    FunCefLifeSpanHandler::new(state)
}

wrap_client! {
    pub struct FunCefBrowserClient {
        render_handler: RenderHandler,
        life_span_handler: LifeSpanHandler,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life_span_handler.clone())
        }

        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render_handler.clone())
        }
    }
}

#[must_use]
pub fn new_fun_cef_browser_client(
    render_handler: RenderHandler,
    life_span_handler: LifeSpanHandler,
) -> Client {
    FunCefBrowserClient::new(render_handler, life_span_handler)
}

pub struct CefUiBrowser {
    config: BrowserUiConfig,
    client: Client,
    compositor: SharedCefUiCompositor,
    state: SharedBrowserState,
}

impl CefUiBrowser {
    pub fn create(
        config: BrowserUiConfig,
        compositor: SharedCefUiCompositor,
    ) -> Result<Self, CefUiBrowserError> {
        if config.dev_server_url.is_none() && !register_fun_ui_scheme_handler_factory() {
            return Err(CefUiBrowserError::SchemeFactoryRejected);
        }

        let paint_sink: Arc<dyn CefPaintSink> = Arc::new(compositor.clone());
        let render_handler = new_fun_cef_render_handler_for_viewport(
            paint_sink,
            CefUiScaleFactor::ONE,
            config.viewport_width,
            config.viewport_height,
        );
        let state = Arc::new(Mutex::new(BrowserState::new()));
        with_browser_state(&state, |state| state.lifecycle.mark_creating());
        let life_span_handler = new_fun_cef_life_span_handler(Arc::clone(&state));
        let mut client = new_fun_cef_browser_client(render_handler, life_span_handler);
        let window_info = windowless_window_info(&config);
        let page_url = config.page_url();
        let settings = config.browser_settings();
        let created = browser_host_create_browser(
            Some(&window_info),
            Some(&mut client),
            Some(&page_url),
            Some(&settings),
            None,
            None,
        );
        if created == 0 {
            return Err(CefUiBrowserError::CreationRejected);
        }

        Ok(Self {
            config,
            client,
            compositor,
            state,
        })
    }

    pub fn create_main_window_from_env(
        viewport_width: u32,
        viewport_height: u32,
    ) -> Result<Self, CefUiBrowserError> {
        let config = BrowserUiConfig::main_window_from_env(viewport_width, viewport_height)?;
        Self::create(config, SharedCefUiCompositor::default())
    }

    #[must_use]
    pub const fn config(&self) -> &BrowserUiConfig {
        &self.config
    }

    #[must_use]
    pub fn lifecycle(&self) -> BrowserLifecycle {
        self.state
            .lock()
            .map(|state| state.lifecycle.clone())
            .unwrap_or_else(|_| {
                let mut lifecycle = BrowserLifecycle::new();
                lifecycle.mark_closed();
                lifecycle
            })
    }

    #[must_use]
    pub const fn compositor(&self) -> &SharedCefUiCompositor {
        &self.compositor
    }

    pub fn close(&mut self) {
        let _retained_client = &self.client;
        let browser = self.state.lock().ok().and_then(|mut state| {
            state.lifecycle.begin_close();
            state.browser.take()
        });
        if let Some(browser) = browser
            && browser.is_valid() != 0
            && let Some(host) = browser.host()
        {
            host.close_browser(1);
        } else {
            with_browser_state(&self.state, |state| state.lifecycle.mark_closed());
        }
    }
}

fn windowless_window_info(config: &BrowserUiConfig) -> WindowInfo {
    let mut window_info = WindowInfo::default().set_as_windowless(null_cef_window_handle());
    window_info.bounds = Rect {
        x: 0,
        y: 0,
        width: config.viewport_width.min(i32::MAX as u32) as i32,
        height: config.viewport_height.min(i32::MAX as u32) as i32,
    };
    window_info
}

#[cfg(target_os = "windows")]
fn null_cef_window_handle() -> cef::sys::cef_window_handle_t {
    cef::sys::HWND(std::ptr::null_mut())
}

#[cfg(target_os = "linux")]
fn null_cef_window_handle() -> cef::sys::cef_window_handle_t {
    0
}

#[cfg(target_os = "macos")]
fn null_cef_window_handle() -> cef::sys::cef_window_handle_t {
    std::ptr::null_mut()
}

impl Drop for CefUiBrowser {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_page_is_transparent_fun_ui_url() {
        let config = BrowserUiConfig::default();

        assert_eq!(config.page_url_str(), "fun-ui://main/index.html");
        assert_eq!(config.browser_settings().background_color, 0x0000_0000);
        assert_eq!(
            config.browser_settings().windowless_frame_rate,
            CEF_UI_WINDOWLESS_FRAME_RATE_HZ
        );
    }
}
