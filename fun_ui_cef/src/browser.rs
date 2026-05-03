use cef::{BrowserSettings, CefString};

use crate::scheme::FUN_UI_MAIN_URL;

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
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub windowless_frame_rate: i32,
}

impl BrowserUiConfig {
    #[must_use]
    pub const fn main_window(viewport_width: u32, viewport_height: u32) -> Self {
        Self {
            page: MAIN_BROWSER_PAGE,
            viewport_width,
            viewport_height,
            windowless_frame_rate: 60,
        }
    }

    #[must_use]
    pub fn page_url(&self) -> CefString {
        CefString::from(self.page.url)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn main_page_is_transparent_fun_ui_url() {
        let config = BrowserUiConfig::default();

        assert_eq!(config.page.url, "fun-ui://main/index.html");
        assert_eq!(config.browser_settings().background_color, 0x0000_0000);
    }
}
