//! CEF-backed game UI subsystem for Fun.
//!
//! This crate deliberately stays outside `fun_render`: CEF owns browser UI
//! lifetime, page loading, bridge state, and offscreen paint production. The
//! game client decides how those paint frames are presented by the normal Fun
//! render stack.

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
    BrowserUiRouteState, BrowserUiSequence, HostCapability, HostCommandError, HostCommandId,
    HostCommandRejection, HostCommandRequest, HostCommandResponse, HostCommandTarget,
    HostDiagnostic, SharedBrowserBridgeQueues, UiControlPayload, UiEnvelope, UiEnvelopeChannel,
    UiEnvelopeKind, UiEnvelopePayload, validate_browser_ui_packet, validate_ui_envelope,
};
pub use browser::{
    BrowserUiConfig, BrowserUiPage, CEF_UI_WINDOWLESS_FRAME_RATE_HZ, CefBrowserKeyEvent,
    CefBrowserKeyEventKind, CefBrowserMouseButton, CefBrowserMouseEvent, CefUiBrowser,
    CefUiBrowserError, CefUiBrowserHandle, MAIN_BROWSER_PAGE,
};
pub use compositor::{
    CefUiCompositor, CefUiCompositorFrame, CefUiUploadPlan, SharedCefUiCompositor,
    UiCompositorState, UiSurfaceGeneration,
};
pub use input::{BrowserUiInputEvent, BrowserUiInputOwner, validate_input_event};
pub use model::{
    CefUiModel, DEFAULT_MAX_PATCHES_PER_BATCH, DEFAULT_MAX_PENDING_PATCH_BATCHES,
    DEFAULT_UI_PATCH_SCHEMA_REVISION, GameUiChannel, GameUiFieldKey, MAX_UI_PATCH_BINARY_BYTES,
    MAX_UI_PATCH_TEXT_BYTES, UiPatchBackpressureQueue, UiPatchBatch, UiPatchOp, UiPatchRecord,
    UiPatchSequence, UiPatchValue, UiPatchWriteError, UiPatchWriter, UiRowId, UiRowRevision,
};
pub use render_handler::{
    CefDirtyRect, CefPaintElement, CefPaintFrame, CefUiFrameMetadata, new_fun_cef_render_handler,
};
pub use runtime::{
    CefMessageLoopStrategy, CefRuntime, CefRuntimeConfig, CefRuntimeError,
    pump_cef_message_loop_work,
};
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
