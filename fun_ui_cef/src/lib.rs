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
pub mod model;
pub mod render_handler;
pub mod runtime;
pub mod scheme;
pub mod security;

pub use bootstrap::{CefSubprocessExit, maybe_execute_cef_subprocess};
pub use bridge::{
    BrowserBridgeError, BrowserBridgeQueues, BrowserUiCapability, BrowserUiHitRegion,
    BrowserUiHitRegionId, BrowserUiHitRegionMode, BrowserUiPacket,
    BrowserUiProtocolValidationContext, BrowserUiProtocolValidationError, BrowserUiRequestId,
    BrowserUiRouteState, BrowserUiSequence, UiControlPayload, UiEnvelope, UiEnvelopeChannel,
    UiEnvelopeKind, UiEnvelopePayload, validate_browser_ui_packet, validate_ui_envelope,
};
pub use browser::{
    BrowserUiConfig, BrowserUiPage, CefUiBrowser, CefUiBrowserError, MAIN_BROWSER_PAGE,
};
pub use compositor::{
    CefUiCompositor, CefUiOverlayMode, CefUiOverlaySurface, SharedCefUiCompositor,
    UiCompositorState, UiSurfaceGeneration,
};
pub use input::{BrowserUiInputEvent, BrowserUiInputOwner, validate_input_event};
pub use model::{
    CefUiModel, DEFAULT_MAX_PATCHES_PER_BATCH, DEFAULT_MAX_PENDING_PATCH_BATCHES,
    DEFAULT_UI_PATCH_SCHEMA_REVISION, GameUiChannel, GameUiFieldKey, MAX_UI_PATCH_BINARY_BYTES,
    MAX_UI_PATCH_TEXT_BYTES, UiPatchBackpressureQueue, UiPatchBatch, UiPatchOp, UiPatchRecord,
    UiPatchSequence, UiPatchValue, UiPatchWriteError, UiPatchWriter, UiRowId, UiRowRevision,
};
pub use render_handler::{CefPaintFrame, CefUiFrameMetadata, new_fun_cef_render_handler};
pub use runtime::{CefRuntime, CefRuntimeConfig, CefRuntimeError};
pub use scheme::{
    FUN_UI_MAIN_URL, FUN_UI_SCHEME, FunUiAssetRoute, FunUiNavigationBlockReason,
    FunUiNavigationDecision, FunUiNavigationPolicy, FunUiNavigationTarget, FunUiRoute,
    FunUiSchemeRequestOutcome, classify_fun_ui_scheme_request,
    register_fun_ui_scheme_handler_factory, validate_fun_ui_navigation,
};
pub use security::{
    CefUiHelperProcessPolicy, CefUiJavaScriptAuthority, CefUiSecurityPolicy,
    CefUiStateExposurePolicy,
};
