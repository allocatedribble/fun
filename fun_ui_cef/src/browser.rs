use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

use cef::rc::Rc as _;
use cef::wrapper::message_router::{
    BrowserSideCallback, BrowserSideHandler, BrowserSideRouter, MessageRouterBrowserSide,
    MessageRouterBrowserSideHandlerCallbacks, MessageRouterConfig,
};
use cef::{
    Browser, BrowserSettings, CefString, Client, DisplayHandler, Frame, ImplBrowser,
    ImplBrowserHost, ImplClient, ImplDisplayHandler, ImplFrame, ImplLifeSpanHandler, KeyEvent,
    KeyEventType, LifeSpanHandler, LogSeverity, MouseButtonType, MouseEvent, PaintElementType,
    ProcessId, ProcessMessage, Rect, RenderHandler, WindowInfo, WrapClient, WrapDisplayHandler,
    WrapLifeSpanHandler, browser_host_create_browser, wrap_client, wrap_display_handler,
    wrap_life_span_handler,
};
use serde_json::{Value, json};

use crate::diagnostics::{FUN_UI_DIAGNOSTICS_TARGET, SharedCefUiTransportCounters};
use crate::render_handler::CefUiScaleFactor;
use crate::scheme::{
    FUN_UI_MAIN_URL, FunUiDevServerError, FunUiDevServerUrl, fun_ui_dev_server_from_env,
    register_fun_ui_scheme_handler_factory,
};
use crate::{
    bridge::{
        BrowserUiHitRegion, BrowserUiHitRegionId, BrowserUiHitRegionMode, BrowserUiRequestId,
        BrowserUiSequence, HostCommandError, HostCommandRejection, HostCommandRequest,
        HostCommandResponse, HostDiagnostic, SharedBrowserBridgeQueues, UiControlPayload,
        UiEnvelope, UiEnvelopeKind,
    },
    compositor::SharedCefUiCompositor,
    render_handler::{
        CefAcceleratedPaintSinkSlot, CefPaintSink,
        new_fun_cef_render_handler_for_viewport_with_counters_and_accelerated_sink,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefUiPaintTransport {
    CpuPaint,
    D3d11SharedTextureDx12Copy,
}

impl CefUiPaintTransport {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::CpuPaint => "cpu_paint",
            Self::D3d11SharedTextureDx12Copy => "d3d11_shared_texture_dx12_copy",
        }
    }

    #[must_use]
    pub const fn shared_texture_enabled(self) -> bool {
        matches!(self, Self::D3d11SharedTextureDx12Copy)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefUiPaintTransportFallbackReason {
    None,
    NonWindows,
    RenderBackendNotDx12,
    D3d11On12BridgeUnavailable,
    SharedTextureUnsupported,
    OutputTextureAllocationUnavailable,
    GpuCopyFenceTimeout,
    GpuBridgeTimeout,
    AcceleratedPaintNotObserved,
    DeviceQueueExtractionFailed,
}

pub type CefUiFallbackReason = CefUiPaintTransportFallbackReason;

impl CefUiPaintTransportFallbackReason {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::NonWindows => "non_windows",
            Self::RenderBackendNotDx12 => "render_backend_not_dx12",
            Self::D3d11On12BridgeUnavailable => "d3d11on12_bridge_unavailable",
            Self::SharedTextureUnsupported => "shared_texture_unsupported",
            Self::OutputTextureAllocationUnavailable => "output_texture_allocation_unavailable",
            Self::GpuCopyFenceTimeout => "gpu_copy_fence_timeout",
            Self::GpuBridgeTimeout => "gpu_bridge_timeout",
            Self::AcceleratedPaintNotObserved => "accelerated_paint_not_observed",
            Self::DeviceQueueExtractionFailed => "device_queue_extraction_failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefUiRenderBackendHint {
    Dx12,
    Vulkan,
    Other,
    Auto,
}

impl CefUiRenderBackendHint {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Dx12 => "dx12",
            Self::Vulkan => "vulkan",
            Self::Other => "other",
            Self::Auto => "auto",
        }
    }

    #[must_use]
    pub const fn is_dx12_compatible(self) -> bool {
        matches!(self, Self::Dx12)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefUiRequestedPaintTransport {
    Cpu,
    Auto,
    D3d11On12,
}

impl CefUiRequestedPaintTransport {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Cpu => "cpu",
            Self::Auto => "auto",
            Self::D3d11On12 => "d3d11on12",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserUiConfig {
    pub page: BrowserUiPage,
    pub dev_server_url: Option<FunUiDevServerUrl>,
    pub viewport_width: u32,
    pub viewport_height: u32,
    pub windowless_frame_rate: i32,
    pub requested_paint_transport: CefUiRequestedPaintTransport,
    pub paint_transport: CefUiPaintTransport,
    pub paint_transport_fallback_reason: CefUiPaintTransportFallbackReason,
    pub render_backend_hint: CefUiRenderBackendHint,
    pub accelerated_paint_debug: bool,
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
            requested_paint_transport: CefUiRequestedPaintTransport::Cpu,
            paint_transport: CefUiPaintTransport::CpuPaint,
            paint_transport_fallback_reason: CefUiPaintTransportFallbackReason::None,
            render_backend_hint: CefUiRenderBackendHint::Auto,
            accelerated_paint_debug: false,
        }
    }

    pub fn main_window_from_env(
        viewport_width: u32,
        viewport_height: u32,
    ) -> Result<Self, FunUiDevServerError> {
        let mut config = Self {
            dev_server_url: fun_ui_dev_server_from_env()?,
            ..Self::main_window(viewport_width, viewport_height)
        };
        config.apply_paint_transport_env();
        Ok(config)
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

    fn apply_paint_transport_env(&mut self) {
        self.render_backend_hint = render_backend_hint_from_env();
        self.accelerated_paint_debug = env_flag_enabled("FUN_CEF_UI_ACCELERATED_PAINT_DEBUG");
        let request = paint_transport_request_from_env();
        self.requested_paint_transport = request;
        let decision = select_paint_transport(
            request,
            self.render_backend_hint,
            cfg!(target_os = "windows"),
            false,
            true,
            true,
        );
        self.paint_transport = decision.transport;
        self.paint_transport_fallback_reason = decision.fallback_reason;
    }

    #[must_use]
    pub fn with_paint_transport_decision(
        mut self,
        paint_transport: CefUiPaintTransport,
        fallback_reason: CefUiPaintTransportFallbackReason,
    ) -> Self {
        self.paint_transport = paint_transport;
        self.paint_transport_fallback_reason = fallback_reason;
        self
    }
}

impl Default for BrowserUiConfig {
    fn default() -> Self {
        Self::main_window(1280, 720)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CefUiPaintTransportDecision {
    transport: CefUiPaintTransport,
    fallback_reason: CefUiPaintTransportFallbackReason,
}

fn paint_transport_request_from_env() -> CefUiRequestedPaintTransport {
    let paint_transport = std::env::var("FUN_CEF_UI_PAINT_TRANSPORT").ok();
    if let Some(request) = paint_transport
        .as_deref()
        .and_then(parse_paint_transport_request)
    {
        return request;
    }
    if let Some(value) = paint_transport {
        tracing::warn!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            value,
            "FUN_CEF_UI_PAINT_TRANSPORT ignored unsupported value"
        );
    }

    let accelerated_paint = std::env::var("FUN_CEF_UI_ACCELERATED_PAINT").ok();
    if let Some(request) = accelerated_paint
        .as_deref()
        .and_then(parse_accelerated_paint_request)
    {
        return request;
    }
    if let Some(value) = accelerated_paint {
        tracing::warn!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            value,
            "FUN_CEF_UI_ACCELERATED_PAINT ignored unsupported value"
        );
    }
    CefUiRequestedPaintTransport::Cpu
}

fn parse_paint_transport_request(value: &str) -> Option<CefUiRequestedPaintTransport> {
    match value.trim().to_ascii_lowercase().as_str() {
        "cpu" | "cpu_paint" => Some(CefUiRequestedPaintTransport::Cpu),
        "auto" => Some(CefUiRequestedPaintTransport::Auto),
        "d3d11on12" | "d3d11_shared_texture_dx12_copy" | "d3d11-shared-texture-dx12-copy" => {
            Some(CefUiRequestedPaintTransport::D3d11On12)
        }
        _ => None,
    }
}

fn parse_accelerated_paint_request(value: &str) -> Option<CefUiRequestedPaintTransport> {
    match value.trim().to_ascii_lowercase().as_str() {
        "0" | "false" | "off" | "cpu" => Some(CefUiRequestedPaintTransport::Cpu),
        "1" | "true" | "on" | "d3d11on12" => Some(CefUiRequestedPaintTransport::D3d11On12),
        "auto" => Some(CefUiRequestedPaintTransport::Auto),
        _ => None,
    }
}

fn render_backend_hint_from_env() -> CefUiRenderBackendHint {
    let Some(value) = std::env::var("FUN_RENDER_BACKEND").ok() else {
        return default_render_backend_hint();
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "dx12" | "d3d12" | "directx12" => CefUiRenderBackendHint::Dx12,
        "vulkan" => CefUiRenderBackendHint::Vulkan,
        "auto" => default_render_backend_hint(),
        _ => CefUiRenderBackendHint::Other,
    }
}

#[must_use]
const fn default_render_backend_hint() -> CefUiRenderBackendHint {
    if cfg!(target_os = "windows") {
        CefUiRenderBackendHint::Dx12
    } else {
        CefUiRenderBackendHint::Auto
    }
}

fn env_flag_enabled(name: &str) -> bool {
    std::env::var(name).ok().is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "on"
        )
    })
}

#[must_use]
const fn select_paint_transport(
    request: CefUiRequestedPaintTransport,
    backend_hint: CefUiRenderBackendHint,
    windows: bool,
    d3d11on12_ready: bool,
    shared_texture_supported: bool,
    output_texture_allocation_ready: bool,
) -> CefUiPaintTransportDecision {
    match request {
        CefUiRequestedPaintTransport::Cpu => CefUiPaintTransportDecision {
            transport: CefUiPaintTransport::CpuPaint,
            fallback_reason: CefUiPaintTransportFallbackReason::None,
        },
        CefUiRequestedPaintTransport::Auto | CefUiRequestedPaintTransport::D3d11On12 => {
            if !windows {
                return CefUiPaintTransportDecision {
                    transport: CefUiPaintTransport::CpuPaint,
                    fallback_reason: CefUiPaintTransportFallbackReason::NonWindows,
                };
            }
            if !backend_hint.is_dx12_compatible() {
                return CefUiPaintTransportDecision {
                    transport: CefUiPaintTransport::CpuPaint,
                    fallback_reason: CefUiPaintTransportFallbackReason::RenderBackendNotDx12,
                };
            }
            if !d3d11on12_ready {
                return CefUiPaintTransportDecision {
                    transport: CefUiPaintTransport::CpuPaint,
                    fallback_reason: CefUiPaintTransportFallbackReason::D3d11On12BridgeUnavailable,
                };
            }
            if !shared_texture_supported {
                return CefUiPaintTransportDecision {
                    transport: CefUiPaintTransport::CpuPaint,
                    fallback_reason: CefUiPaintTransportFallbackReason::SharedTextureUnsupported,
                };
            }
            if !output_texture_allocation_ready {
                return CefUiPaintTransportDecision {
                    transport: CefUiPaintTransport::CpuPaint,
                    fallback_reason:
                        CefUiPaintTransportFallbackReason::OutputTextureAllocationUnavailable,
                };
            }
            CefUiPaintTransportDecision {
                transport: CefUiPaintTransport::D3d11SharedTextureDx12Copy,
                fallback_reason: CefUiPaintTransportFallbackReason::None,
            }
        }
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
type BrowserQueryCallback = Arc<Mutex<dyn BrowserSideCallback>>;

fn with_browser_state(state: &SharedBrowserState, update: impl FnOnce(&mut BrowserState)) {
    if let Ok(mut state) = state.lock() {
        update(&mut state);
    }
}

fn browser_from_state(state: &SharedBrowserState) -> Option<Browser> {
    state.lock().ok().and_then(|state| state.browser.clone())
}

#[derive(Clone)]
struct BrowserBridgeEndpoint {
    queues: SharedBrowserBridgeQueues,
    callbacks: Arc<Mutex<BTreeMap<BrowserUiRequestId, BrowserQueryCallback>>>,
}

impl BrowserBridgeEndpoint {
    fn new(queues: SharedBrowserBridgeQueues) -> Self {
        Self {
            queues,
            callbacks: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    fn push_js_query(
        &self,
        request: &str,
        callback: BrowserQueryCallback,
    ) -> Result<(), BrowserBridgeQueryError> {
        let Some(envelope) = envelope_from_cef_query(request)? else {
            callback
                .lock()
                .map_err(|_| BrowserBridgeQueryError::CallbackUnavailable)?
                .success_str("");
            return Ok(());
        };
        if let Some(request_id) = envelope.request_id {
            self.callbacks
                .lock()
                .map_err(|_| BrowserBridgeQueryError::CallbackUnavailable)?
                .insert(request_id, callback);
        } else {
            callback
                .lock()
                .map_err(|_| BrowserBridgeQueryError::CallbackUnavailable)?
                .success_str("");
        }
        self.queues
            .push_js_envelope(envelope)
            .map_err(|_| BrowserBridgeQueryError::QueueClosed)
    }

    fn flush_host_envelopes(&self, state: &SharedBrowserState) -> usize {
        let mut flushed = 0usize;
        while let Some(envelope) = self.queues.pop_host_envelope_for_js() {
            let Some(payload) = host_envelope_to_json(&envelope) else {
                continue;
            };
            let delivered = envelope
                .request_id
                .and_then(|request_id| {
                    self.callbacks
                        .lock()
                        .ok()
                        .and_then(|mut callbacks| callbacks.remove(&request_id))
                })
                .and_then(|callback| {
                    callback
                        .lock()
                        .ok()
                        .map(|callback| callback.success_str(&payload))
                })
                .is_some();
            if !delivered {
                execute_fun_receive_from_host(state, &payload);
            }
            flushed = flushed.saturating_add(1);
        }
        flushed
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BrowserBridgeQueryError {
    InvalidJson,
    InvalidEnvelope,
    QueueClosed,
    CallbackUnavailable,
}

fn envelope_from_cef_query(request: &str) -> Result<Option<UiEnvelope>, BrowserBridgeQueryError> {
    let value =
        serde_json::from_str::<Value>(request).map_err(|_| BrowserBridgeQueryError::InvalidJson)?;
    if let Some(envelope) = host_command_envelope_from_value(&value)? {
        return Ok(Some(envelope));
    }
    event_envelope_from_value(&value)
}

fn host_command_envelope_from_value(
    value: &Value,
) -> Result<Option<UiEnvelope>, BrowserBridgeQueryError> {
    let Some(request) = value
        .pointer("/payload/Control/payload/HostCommand/request")
        .or_else(|| value.pointer("/payload/control/payload/host_command/request"))
    else {
        return Ok(None);
    };
    let command_id = request
        .get("command_id")
        .and_then(Value::as_str)
        .ok_or(BrowserBridgeQueryError::InvalidEnvelope)?;
    let request_id = value
        .get("request_id")
        .and_then(Value::as_u64)
        .or_else(|| request.get("request_id").and_then(Value::as_u64))
        .ok_or(BrowserBridgeQueryError::InvalidEnvelope)?;
    let sequence = value.get("sequence").and_then(Value::as_u64).unwrap_or(0);
    let payload = json_bytes_from_value(request.get("payload").unwrap_or(&Value::Null))?;
    let host_request = HostCommandRequest::new(BrowserUiRequestId(request_id), command_id, payload)
        .ok_or(BrowserBridgeQueryError::InvalidEnvelope)?;
    Ok(Some(UiEnvelope::control(
        UiEnvelopeKind::Request,
        Some(BrowserUiRequestId(request_id)),
        BrowserUiSequence(sequence),
        UiControlPayload::HostCommand {
            request: host_request,
        },
    )))
}

fn event_envelope_from_value(value: &Value) -> Result<Option<UiEnvelope>, BrowserBridgeQueryError> {
    if !value
        .get("kind")
        .and_then(Value::as_str)
        .is_some_and(|kind| kind.eq_ignore_ascii_case("event"))
    {
        return Ok(None);
    }
    let Some(event) = value.get("event").and_then(Value::as_str) else {
        return Ok(None);
    };
    if event != "ui.hit_regions.changed" {
        return Ok(None);
    }
    let payload = value
        .get("payload")
        .ok_or(BrowserBridgeQueryError::InvalidEnvelope)?;
    let mode = payload
        .get("mode")
        .and_then(Value::as_str)
        .and_then(hit_region_mode_from_wire)
        .ok_or(BrowserBridgeQueryError::InvalidEnvelope)?;
    let regions = payload
        .get("regions")
        .and_then(Value::as_array)
        .ok_or(BrowserBridgeQueryError::InvalidEnvelope)?
        .iter()
        .filter_map(hit_region_from_value)
        .collect::<Vec<_>>();
    let sequence = value.get("sequence").and_then(Value::as_u64).unwrap_or(0);
    Ok(Some(UiEnvelope::control(
        UiEnvelopeKind::Event,
        None,
        BrowserUiSequence(sequence),
        UiControlPayload::HitRegionsChanged { mode, regions },
    )))
}

fn json_bytes_from_value(value: &Value) -> Result<Vec<u8>, BrowserBridgeQueryError> {
    if let Some(bytes) = value.as_array() {
        return bytes
            .iter()
            .map(|byte| {
                byte.as_u64()
                    .and_then(|byte| u8::try_from(byte).ok())
                    .ok_or(BrowserBridgeQueryError::InvalidEnvelope)
            })
            .collect();
    }
    if value.is_null() {
        return Ok(b"null".to_vec());
    }
    serde_json::to_vec(value).map_err(|_| BrowserBridgeQueryError::InvalidEnvelope)
}

fn hit_region_mode_from_wire(value: &str) -> Option<BrowserUiHitRegionMode> {
    match value {
        "gameplay" => Some(BrowserUiHitRegionMode::Gameplay),
        "hud_passive" => Some(BrowserUiHitRegionMode::HudPassive),
        "ui_modal" => Some(BrowserUiHitRegionMode::UiModal),
        "text_entry" => Some(BrowserUiHitRegionMode::TextEntry),
        _ => None,
    }
}

fn hit_region_from_value(value: &Value) -> Option<BrowserUiHitRegion> {
    Some(BrowserUiHitRegion {
        id: BrowserUiHitRegionId::from_wire_str(value.get("id")?.as_str()?)?,
        x: i32::try_from(value.get("x")?.as_i64()?).ok()?,
        y: i32::try_from(value.get("y")?.as_i64()?).ok()?,
        w: i32::try_from(value.get("w")?.as_i64()?).ok()?,
        h: i32::try_from(value.get("h")?.as_i64()?).ok()?,
    })
}

fn host_envelope_to_json(envelope: &UiEnvelope) -> Option<String> {
    let kind = match envelope.kind {
        UiEnvelopeKind::Event => "event",
        UiEnvelopeKind::Request => "request",
        UiEnvelopeKind::Response => "response",
        UiEnvelopeKind::Error => "error",
        UiEnvelopeKind::Patch => "patch",
    };
    let payload = match &envelope.payload {
        crate::bridge::UiEnvelopePayload::Control {
            payload:
                UiControlPayload::HostCommandResult {
                    command_id,
                    response,
                },
        } => json!({
            "Control": {
                "payload": {
                    "HostCommandResult": {
                        "command_id": command_id.as_str(),
                        "response": host_command_response_to_json(response),
                    }
                }
            }
        }),
        crate::bridge::UiEnvelopePayload::Control {
            payload: UiControlPayload::HostEvent { event, payload },
        } => json!({
            "Control": {
                "payload": {
                    "HostEvent": {
                        "event": event,
                        "payload": payload,
                    }
                }
            }
        }),
        _ => return None,
    };
    serde_json::to_string(&json!({
        "protocol_version": envelope.protocol_version.0,
        "schema_revision": envelope.schema_revision.0,
        "channel": envelope.channel.as_wire_str(),
        "kind": kind,
        "request_id": envelope.request_id.map(|request_id| request_id.0),
        "sequence": envelope.sequence.0,
        "payload": payload,
    }))
    .ok()
}

fn host_command_response_to_json(response: &HostCommandResponse) -> Value {
    match response {
        HostCommandResponse::Ok {
            payload,
            diagnostics,
        } => json!({
            "Ok": {
                "payload": payload,
                "diagnostics": host_diagnostics_to_json(diagnostics),
            }
        }),
        HostCommandResponse::Rejected { reason } => json!({
            "Rejected": {
                "reason": host_rejection_to_wire(*reason),
            }
        }),
        HostCommandResponse::Failed { error, diagnostics } => json!({
            "Failed": {
                "error": host_error_to_json(error),
                "diagnostics": host_diagnostics_to_json(diagnostics),
            }
        }),
    }
}

fn host_diagnostics_to_json(diagnostics: &[HostDiagnostic]) -> Vec<Value> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            json!({
                "code": diagnostic.code,
                "level": diagnostic.level,
                "message": diagnostic.message,
            })
        })
        .collect()
}

fn host_error_to_json(error: &HostCommandError) -> Value {
    json!({
        "code": error.code,
        "message": error.message,
    })
}

fn host_rejection_to_wire(reason: HostCommandRejection) -> &'static str {
    match reason {
        HostCommandRejection::UnknownCommand => "unknown_command",
        HostCommandRejection::InvalidCommandId => "invalid_command_id",
        HostCommandRejection::MissingCapability => "missing_capability",
        HostCommandRejection::CapabilityMismatch => "capability_mismatch",
        HostCommandRejection::OversizePayload => "oversize_payload",
        HostCommandRejection::InvalidPayload => "invalid_payload",
        HostCommandRejection::HostShuttingDown => "host_shutting_down",
    }
}

fn execute_fun_receive_from_host(state: &SharedBrowserState, payload_json: &str) {
    let Some(frame) = browser_from_state(state).and_then(|browser| browser.main_frame()) else {
        return;
    };
    let code_string = format!(
        "window.fun && window.fun.receiveFromHost && window.fun.receiveFromHost({payload_json});"
    );
    let code = CefString::from(code_string.as_str());
    frame.execute_java_script(Some(&code), Some(&CefString::from(FUN_UI_MAIN_URL)), 1);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CefBrowserMouseEvent {
    pub x: i32,
    pub y: i32,
    pub modifiers: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefBrowserMouseButton {
    Left,
    Right,
    Middle,
}

impl From<CefBrowserMouseButton> for MouseButtonType {
    fn from(value: CefBrowserMouseButton) -> Self {
        match value {
            CefBrowserMouseButton::Left => Self::LEFT,
            CefBrowserMouseButton::Right => Self::RIGHT,
            CefBrowserMouseButton::Middle => Self::MIDDLE,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefBrowserKeyEventKind {
    RawKeyDown,
    KeyUp,
    Char,
}

impl From<CefBrowserKeyEventKind> for KeyEventType {
    fn from(value: CefBrowserKeyEventKind) -> Self {
        match value {
            CefBrowserKeyEventKind::RawKeyDown => Self::RAWKEYDOWN,
            CefBrowserKeyEventKind::KeyUp => Self::KEYUP,
            CefBrowserKeyEventKind::Char => Self::CHAR,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CefBrowserKeyEvent {
    pub kind: CefBrowserKeyEventKind,
    pub modifiers: u32,
    pub windows_key_code: i32,
    pub native_key_code: i32,
    pub character: u16,
    pub unmodified_character: u16,
    pub is_system_key: bool,
}

#[derive(Clone)]
pub struct CefUiBrowserHandle {
    state: SharedBrowserState,
    bridge_endpoint: BrowserBridgeEndpoint,
}

impl CefUiBrowserHandle {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.state
            .lock()
            .map(|state| matches!(state.lifecycle.state(), BrowserLifecycleState::Ready { .. }))
            .unwrap_or(false)
    }

    pub fn set_focus(&self, focused: bool) -> bool {
        let Some(host) = browser_from_state(&self.state).and_then(|browser| browser.host()) else {
            return false;
        };
        host.set_focus(i32::from(focused));
        true
    }

    pub fn send_mouse_move(&self, event: CefBrowserMouseEvent, mouse_leave: bool) -> bool {
        let Some(host) = browser_from_state(&self.state).and_then(|browser| browser.host()) else {
            return false;
        };
        host.send_mouse_move_event(
            Some(&MouseEvent {
                x: event.x,
                y: event.y,
                modifiers: event.modifiers,
            }),
            i32::from(mouse_leave),
        );
        true
    }

    pub fn send_mouse_click(
        &self,
        event: CefBrowserMouseEvent,
        button: CefBrowserMouseButton,
        mouse_up: bool,
        click_count: i32,
    ) -> bool {
        let Some(host) = browser_from_state(&self.state).and_then(|browser| browser.host()) else {
            return false;
        };
        host.send_mouse_click_event(
            Some(&MouseEvent {
                x: event.x,
                y: event.y,
                modifiers: event.modifiers,
            }),
            button.into(),
            i32::from(mouse_up),
            click_count.max(1),
        );
        true
    }

    pub fn send_mouse_wheel(
        &self,
        event: CefBrowserMouseEvent,
        delta_x: i32,
        delta_y: i32,
    ) -> bool {
        let Some(host) = browser_from_state(&self.state).and_then(|browser| browser.host()) else {
            return false;
        };
        host.send_mouse_wheel_event(
            Some(&MouseEvent {
                x: event.x,
                y: event.y,
                modifiers: event.modifiers,
            }),
            delta_x,
            delta_y,
        );
        true
    }

    pub fn send_key_event(&self, event: CefBrowserKeyEvent) -> bool {
        let Some(host) = browser_from_state(&self.state).and_then(|browser| browser.host()) else {
            return false;
        };
        host.send_key_event(Some(&KeyEvent {
            size: std::mem::size_of::<cef::sys::cef_key_event_t>(),
            type_: event.kind.into(),
            modifiers: event.modifiers,
            windows_key_code: event.windows_key_code,
            native_key_code: event.native_key_code,
            is_system_key: i32::from(event.is_system_key),
            character: event.character,
            unmodified_character: event.unmodified_character,
            focus_on_editable_field: 0,
        }));
        true
    }

    pub fn flush_host_envelopes_to_js(&self) -> usize {
        self.bridge_endpoint.flush_host_envelopes(&self.state)
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
        message_router: Arc<BrowserSideRouter>,
        bridge_endpoint: BrowserBridgeEndpoint,
    }

    impl LifeSpanHandler {
        fn on_after_created(&self, browser: Option<&mut Browser>) {
            let Some(browser) = browser else {
                return;
            };
            let _handler_id = self.message_router.add_handler(
                Arc::new(FunCefBrowserBridgeHandler {
                    endpoint: self.bridge_endpoint.clone(),
                }),
                true,
            );
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
            self.message_router
                .on_before_close(browser.as_ref().map(|browser| (*browser).clone()));
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
fn new_fun_cef_life_span_handler(
    state: SharedBrowserState,
    message_router: Arc<BrowserSideRouter>,
    bridge_endpoint: BrowserBridgeEndpoint,
) -> LifeSpanHandler {
    FunCefLifeSpanHandler::new(state, message_router, bridge_endpoint)
}

wrap_display_handler! {
    struct FunCefDisplayHandler;

    impl DisplayHandler {
        fn on_console_message(
            &self,
            _browser: Option<&mut Browser>,
            level: LogSeverity,
            message: Option<&CefString>,
            source: Option<&CefString>,
            line: std::os::raw::c_int,
        ) -> std::os::raw::c_int {
            let message = message
                .map(CefString::to_string)
                .unwrap_or_else(|| "<empty CEF console message>".to_owned());
            let source = source
                .map(CefString::to_string)
                .unwrap_or_else(|| "<unknown>".to_owned());
            tracing::warn!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                ?level,
                source,
                line,
                message,
                "CEF UI console message"
            );
            0
        }
    }
}

#[must_use]
fn new_fun_cef_display_handler() -> DisplayHandler {
    FunCefDisplayHandler::new()
}

struct FunCefBrowserBridgeHandler {
    endpoint: BrowserBridgeEndpoint,
}

impl BrowserSideHandler for FunCefBrowserBridgeHandler {
    fn on_query_str(
        &self,
        _browser: Option<Browser>,
        _frame: Option<Frame>,
        _query_id: i64,
        request: &str,
        _persistent: bool,
        callback: BrowserQueryCallback,
    ) -> bool {
        match self.endpoint.push_js_query(request, callback.clone()) {
            Ok(()) => true,
            Err(error) => {
                if let Ok(callback) = callback.lock() {
                    callback.failure(400, &format!("Fun host bridge query rejected: {error:?}"));
                }
                true
            }
        }
    }
}

wrap_client! {
    pub struct FunCefBrowserClient {
        render_handler: RenderHandler,
        life_span_handler: LifeSpanHandler,
        display_handler: DisplayHandler,
        message_router: Arc<BrowserSideRouter>,
    }

    impl Client {
        fn life_span_handler(&self) -> Option<LifeSpanHandler> {
            Some(self.life_span_handler.clone())
        }

        fn render_handler(&self) -> Option<RenderHandler> {
            Some(self.render_handler.clone())
        }

        fn display_handler(&self) -> Option<DisplayHandler> {
            Some(self.display_handler.clone())
        }

        fn on_process_message_received(
            &self,
            browser: Option<&mut Browser>,
            frame: Option<&mut Frame>,
            source_process: ProcessId,
            message: Option<&mut ProcessMessage>,
        ) -> std::os::raw::c_int {
            i32::from(self.message_router.on_process_message_received(
                browser.map(|browser| browser.clone()),
                frame.map(|frame| frame.clone()),
                source_process,
                message.map(|message| message.clone()),
            ))
        }
    }
}

#[must_use]
pub fn new_fun_cef_browser_client(
    render_handler: RenderHandler,
    life_span_handler: LifeSpanHandler,
    message_router: Arc<BrowserSideRouter>,
) -> Client {
    FunCefBrowserClient::new(
        render_handler,
        life_span_handler,
        new_fun_cef_display_handler(),
        message_router,
    )
}

pub struct CefUiBrowser {
    config: BrowserUiConfig,
    client: Client,
    compositor: SharedCefUiCompositor,
    transport_counters: SharedCefUiTransportCounters,
    state: SharedBrowserState,
    bridge_endpoint: BrowserBridgeEndpoint,
}

impl CefUiBrowser {
    pub fn create(
        config: BrowserUiConfig,
        compositor: SharedCefUiCompositor,
    ) -> Result<Self, CefUiBrowserError> {
        Self::create_with_bridge(config, compositor, SharedBrowserBridgeQueues::default())
    }

    pub fn create_with_bridge(
        config: BrowserUiConfig,
        compositor: SharedCefUiCompositor,
        bridge_queues: SharedBrowserBridgeQueues,
    ) -> Result<Self, CefUiBrowserError> {
        Self::create_with_bridge_and_accelerated_sink(
            config,
            compositor,
            bridge_queues,
            CefAcceleratedPaintSinkSlot::default(),
        )
    }

    pub fn create_with_bridge_and_accelerated_sink(
        config: BrowserUiConfig,
        compositor: SharedCefUiCompositor,
        bridge_queues: SharedBrowserBridgeQueues,
        accelerated_paint_sink: CefAcceleratedPaintSinkSlot,
    ) -> Result<Self, CefUiBrowserError> {
        if config.dev_server_url.is_none() && !register_fun_ui_scheme_handler_factory() {
            return Err(CefUiBrowserError::SchemeFactoryRejected);
        }

        let transport_counters = SharedCefUiTransportCounters::default();
        if config.paint_transport_fallback_reason != CefUiPaintTransportFallbackReason::None {
            transport_counters.record_transport_fallback();
        }
        let paint_sink: Arc<dyn CefPaintSink> = Arc::new(compositor.clone());
        let render_handler =
            new_fun_cef_render_handler_for_viewport_with_counters_and_accelerated_sink(
                paint_sink,
                CefUiScaleFactor::ONE,
                config.viewport_width,
                config.viewport_height,
                transport_counters.clone(),
                accelerated_paint_sink,
            );
        let state = Arc::new(Mutex::new(BrowserState::new()));
        with_browser_state(&state, |state| state.lifecycle.mark_creating());
        let bridge_endpoint = BrowserBridgeEndpoint::new(bridge_queues);
        let message_router = BrowserSideRouter::new(MessageRouterConfig::default());
        let life_span_handler = new_fun_cef_life_span_handler(
            Arc::clone(&state),
            Arc::clone(&message_router),
            bridge_endpoint.clone(),
        );
        let mut client =
            new_fun_cef_browser_client(render_handler, life_span_handler, message_router);
        let window_info = windowless_window_info(&config);
        log_paint_transport_selection(&config, &window_info);
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
            transport_counters,
            state,
            bridge_endpoint,
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

    #[must_use]
    pub fn transport_counters(&self) -> SharedCefUiTransportCounters {
        self.transport_counters.clone()
    }

    #[must_use]
    pub fn handle(&self) -> CefUiBrowserHandle {
        CefUiBrowserHandle {
            state: Arc::clone(&self.state),
            bridge_endpoint: self.bridge_endpoint.clone(),
        }
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
    window_info.shared_texture_enabled = i32::from(config.paint_transport.shared_texture_enabled());
    window_info
}

fn log_paint_transport_selection(config: &BrowserUiConfig, window_info: &WindowInfo) {
    let shared_texture_enabled = window_info.shared_texture_enabled != 0;
    let d3d11on12_ready = config.paint_transport == CefUiPaintTransport::D3d11SharedTextureDx12Copy;
    match config.paint_transport {
        CefUiPaintTransport::CpuPaint => tracing::info!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            backend = config.render_backend_hint.as_wire_str(),
            windows = cfg!(target_os = "windows"),
            shared_texture_enabled,
            d3d11on12_ready,
            fallback_reason = config.paint_transport_fallback_reason.as_wire_str(),
            accelerated_paint_debug = config.accelerated_paint_debug,
            "CEF UI paint transport selected: cpu_paint"
        ),
        CefUiPaintTransport::D3d11SharedTextureDx12Copy => tracing::info!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            backend = config.render_backend_hint.as_wire_str(),
            windows = cfg!(target_os = "windows"),
            shared_texture_enabled,
            d3d11on12_ready,
            fallback_reason = config.paint_transport_fallback_reason.as_wire_str(),
            accelerated_paint_debug = config.accelerated_paint_debug,
            "CEF UI paint transport selected: d3d11_shared_texture_dx12_copy"
        ),
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
        assert_eq!(
            config.browser_settings().windowless_frame_rate,
            CEF_UI_WINDOWLESS_FRAME_RATE_HZ
        );
    }

    #[test]
    fn windowless_cpu_paint_disables_shared_textures() {
        let config = BrowserUiConfig::default();
        let window_info = windowless_window_info(&config);

        assert_eq!(window_info.windowless_rendering_enabled, 1);
        assert_eq!(window_info.shared_texture_enabled, 0);
    }

    #[test]
    #[cfg(windows)]
    fn windowless_accelerated_paint_enables_shared_textures() {
        let config = BrowserUiConfig {
            paint_transport: CefUiPaintTransport::D3d11SharedTextureDx12Copy,
            ..BrowserUiConfig::default()
        };

        let window_info = windowless_window_info(&config);

        assert_eq!(window_info.windowless_rendering_enabled, 1);
        assert_eq!(window_info.shared_texture_enabled, 1);
    }

    #[test]
    fn explicit_d3d11on12_transport_falls_back_until_bridge_exists() {
        let decision = select_paint_transport(
            CefUiRequestedPaintTransport::D3d11On12,
            CefUiRenderBackendHint::Dx12,
            true,
            false,
            true,
            true,
        );

        assert_eq!(decision.transport, CefUiPaintTransport::CpuPaint);
        assert_eq!(
            decision.fallback_reason,
            CefUiPaintTransportFallbackReason::D3d11On12BridgeUnavailable
        );
    }

    #[test]
    fn accelerated_transport_requires_windows_dx12() {
        let non_windows = select_paint_transport(
            CefUiRequestedPaintTransport::Auto,
            CefUiRenderBackendHint::Dx12,
            false,
            true,
            true,
            true,
        );
        let vulkan = select_paint_transport(
            CefUiRequestedPaintTransport::Auto,
            CefUiRenderBackendHint::Vulkan,
            true,
            true,
            true,
            true,
        );

        assert_eq!(
            non_windows.fallback_reason,
            CefUiPaintTransportFallbackReason::NonWindows
        );
        assert_eq!(
            vulkan.fallback_reason,
            CefUiPaintTransportFallbackReason::RenderBackendNotDx12
        );
    }

    #[test]
    fn accelerated_transport_selects_when_all_gates_are_ready() {
        let decision = select_paint_transport(
            CefUiRequestedPaintTransport::Auto,
            CefUiRenderBackendHint::Dx12,
            true,
            true,
            true,
            true,
        );

        assert_eq!(
            decision.transport,
            CefUiPaintTransport::D3d11SharedTextureDx12Copy
        );
        assert_eq!(
            decision.fallback_reason,
            CefUiPaintTransportFallbackReason::None
        );
    }
}
