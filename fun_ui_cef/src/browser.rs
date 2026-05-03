use std::sync::Arc;

use cef::rc::Rc as _;
use cef::{
    BrowserSettings, CefString, Client, ImplClient, RenderHandler, WindowInfo, WrapClient,
    browser_host_create_browser, wrap_client,
};

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
            windowless_frame_rate: 60,
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

wrap_client! {
    pub struct FunCefBrowserClient {
        render_handler: RenderHandler,
    }

    impl Client {
        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render_handler.clone())
        }
    }
}

#[must_use]
pub fn new_fun_cef_browser_client(render_handler: RenderHandler) -> Client {
    FunCefBrowserClient::new(render_handler)
}

pub struct CefUiBrowser {
    config: BrowserUiConfig,
    client: Client,
    compositor: SharedCefUiCompositor,
    lifecycle: BrowserLifecycle,
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
        let mut client = new_fun_cef_browser_client(render_handler);
        let window_info = WindowInfo::default().set_as_windowless(null_cef_window_handle());
        let page_url = config.page_url();
        let settings = config.browser_settings();
        let mut lifecycle = BrowserLifecycle::new();
        lifecycle.mark_creating();
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
            lifecycle,
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
    pub const fn lifecycle(&self) -> &BrowserLifecycle {
        &self.lifecycle
    }

    #[must_use]
    pub const fn compositor(&self) -> &SharedCefUiCompositor {
        &self.compositor
    }

    pub fn close(&mut self) {
        let _retained_client = &self.client;
        self.lifecycle.begin_close();
        self.lifecycle.mark_closed();
    }
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
    }
}
