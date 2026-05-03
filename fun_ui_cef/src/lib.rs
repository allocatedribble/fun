//! CEF-backed game UI subsystem for Fun.
//!
//! This crate deliberately stays outside `fun_render`: CEF owns browser UI
//! pixels and bridge state, while Bevy render remains the game renderer.

pub mod bootstrap;
pub mod bridge;
pub mod browser;
pub mod compositor;
pub mod diagnostics;
pub mod input;
pub mod render_handler;
pub mod runtime;
pub mod scheme;

pub use bootstrap::{CefSubprocessExit, maybe_execute_cef_subprocess};
pub use bridge::{
    BrowserUiCapability, BrowserUiPacket, BrowserUiProtocolValidationContext,
    BrowserUiProtocolValidationError, validate_browser_ui_packet,
};
pub use browser::{BrowserUiConfig, BrowserUiPage, MAIN_BROWSER_PAGE};
pub use compositor::{UiCompositorState, UiSurfaceGeneration};
pub use runtime::{CefRuntime, CefRuntimeConfig, CefRuntimeError};
pub use scheme::{FUN_UI_MAIN_URL, FUN_UI_SCHEME, FunUiRoute};
