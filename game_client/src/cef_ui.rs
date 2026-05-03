use std::{sync::Arc, time::Duration};

#[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
use crate::cef_ui_dx12::{
    Dx12CefBevyImageState, Dx12CefInterop, Dx12CefInteropError, Dx12CefReadyFrameToken,
    MAX_ACCELERATED_PAINT_FAILURES_BEFORE_FALLBACK,
};
use bevy::{
    asset::{AssetId, RenderAssetUsages},
    diagnostic::{DiagnosticsStore, FrameTimeDiagnosticsPlugin},
    ecs::system::SystemParam,
    input::{
        ButtonInput, ButtonState,
        keyboard::KeyboardInput,
        mouse::{MouseButtonInput, MouseScrollUnit, MouseWheel},
    },
    prelude::*,
    render::{
        Render, RenderApp, RenderSystems,
        extract_resource::{ExtractResource, ExtractResourcePlugin},
        render_asset::RenderAssets,
        render_resource::{
            Extent3d, Origin3d, TexelCopyBufferLayout, TextureDimension, TextureFormat,
        },
        renderer::{RenderDevice, RenderQueue},
        texture::GpuImage,
    },
    window::{CursorMoved, PrimaryWindow},
};
use fun_host::{
    FunClientHostState, FunHostCommandRequest, FunHostCommandResponse, FunHostCommandStatus,
    FunHostMode, FunInputOwner, FunViewportRect,
};
use fun_render::{RenderWorldContext, RenderWorldStatus};
use fun_ui_cef::bridge::{BrowserUiMenuCommand, UiLifecycleState};
use fun_ui_cef::diagnostics::{
    CefUiDiagnosticKind, CefUiDiagnosticSeverity, CefUiTransportCounterSnapshot,
    FUN_UI_DIAGNOSTICS_TARGET, SharedCefUiTransportCounters,
};
#[cfg(test)]
use fun_ui_cef::render_handler::CefUiFrameGeneration;
use fun_ui_cef::{
    BrowserBridgeError, BrowserUiConfig, BrowserUiHitRegion, BrowserUiHitRegionId,
    BrowserUiHitRegionMode, BrowserUiProtocolValidationContext, BrowserUiProtocolValidationError,
    BrowserUiRequestId, BrowserUiRouteState, BrowserUiSequence, CefBrowserKeyEvent,
    CefBrowserKeyEventKind, CefBrowserMouseButton, CefBrowserMouseEvent, CefMessageLoopStrategy,
    CefUiBrowser, CefUiBrowserHandle, CefUiDirtyRectMetadata, CefUiFallbackReason,
    CefUiFullUploadReason, CefUiModel, CefUiPaintTransport, CefUiPaintTransportFallbackReason,
    CefUiRequestedPaintTransport, CefUiSecurityPolicy, FunUiNavigationBlockReason, GameUiChannel,
    GameUiFieldKey, HostCommandError, HostCommandId, HostCommandRejection,
    HostCommandRequest as CefHostCommandRequest, HostCommandResponse as CefHostCommandResponse,
    HostDiagnostic, SharedBrowserBridgeQueues, SharedCefUiCompositor, UiControlPayload, UiEnvelope,
    UiEnvelopeKind, UiEnvelopePayload, UiPatchBackpressureQueue, UiPatchBatch, UiPatchValue,
    UiPatchWriteError, UiPatchWriter, UiSurfaceGeneration, validate_ui_envelope,
};
#[cfg(target_os = "windows")]
use fun_ui_cef::{CefAcceleratedPaintFrame, CefAcceleratedPaintOutcome, CefAcceleratedPaintSink};
use fun_ui_cef::{CefDirtyRect, CefPaintElement, CefUiCompositorFrame};
use game_shared::{
    GameUiMenuTarget, GameUiProtocolValidationContext, GameUiRequestEnvelope, GameUiRequestId,
    GameUiRequestPayload, GameUiRequestRejectionReason, GameUiSequence, GameUiSettingKey,
    GameUiSettingValue, validate_game_ui_request,
};

pub const MAX_CEF_UI_HIT_REGIONS: usize = 64;
const MAX_JS_MESSAGES_PER_FRAME: usize = 64;
const CEF_UI_RENDER_RATE_HZ: u64 = fun_ui_cef::CEF_UI_WINDOWLESS_FRAME_RATE_HZ as u64;
const FUN_CLIENT_FPS_COUNTER_Z_INDEX: i32 = CEF_UI_TEXTURE_Z_INDEX + 20;
const FUN_CLIENT_FPS_COUNTER_REFRESH: Duration = Duration::from_millis(250);
const FUN_CLIENT_FPS_COUNTER_WIDTH: f32 = 88.0;
const FUN_CLIENT_FPS_COUNTER_HEIGHT: f32 = 24.0;
const FUN_CLIENT_FPS_COUNTER_MARGIN: f32 = 12.0;
const CEF_UI_TRANSPORT_COUNTER_REFRESH: Duration = Duration::from_secs(1);
const CEF_UI_GPU_BRIDGE_STARTUP_TIMEOUT: Duration = Duration::from_millis(750);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum GameCefUiSet {
    DrainIncoming,
    InputOwnership,
    CollectModel,
    FlushBridge,
    Diagnostics,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct GameCefUiPlugin;

impl Plugin for GameCefUiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<FrameTimeDiagnosticsPlugin>() {
            app.add_plugins(FrameTimeDiagnosticsPlugin::default());
        }
        app.add_plugins((
            ExtractResourcePlugin::<CefUiTextureUploads>::default(),
            ExtractResourcePlugin::<SharedDx12CefInteropSlot>::default(),
            ExtractResourcePlugin::<CefUiTransportCountersResource>::default(),
        ));
        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            render_app
                .init_resource::<CefUiGpuUploadState>()
                .add_systems(
                    Render,
                    (
                        initialize_dx12_cef_interop_slot,
                        upload_cef_ui_texture_to_gpu,
                        copy_latest_cef_gpu_frame_to_bevy_image,
                    )
                        .chain()
                        .in_set(RenderSystems::PrepareResources),
                );
        }

        app.init_resource::<CefUiStatus>()
            .init_resource::<CefUiFocusMode>()
            .init_resource::<CefUiInputCapture>()
            .init_resource::<CefUiPointerState>()
            .init_resource::<CefUiOverlayClickThroughState>()
            .init_resource::<GameplayInputGate>()
            .init_resource::<CefUiAuthority>()
            .init_resource::<CefUiSecurityPolicyResource>()
            .init_resource::<CefUiBridge>()
            .init_resource::<CefUiFrameStats>()
            .init_resource::<CefUiTransportCountersResource>()
            .init_resource::<CefUiTransportCounterSampler>()
            .init_resource::<CefUiStartupState>()
            .init_resource::<SharedDx12CefInteropSlot>()
            .init_resource::<CefUiMessageLoopPump>()
            .init_resource::<CefUiRenderTexture>()
            .init_resource::<CefUiTextureUploads>()
            .init_resource::<CefUiModelCache>()
            .init_resource::<CefUiDiagnosticsState>()
            .init_resource::<CefUiHostStateEventCache>()
            .init_resource::<FunClientFpsCounterState>()
            .add_message::<CefUiIntent>()
            .add_message::<CefUiRequest>()
            .add_message::<CefUiRouteChanged>()
            .add_message::<CefUiHitRegionsChanged>()
            .add_message::<CefUiTextEntryChanged>()
            .add_message::<CefUiPatch>()
            .add_message::<CefUiCommand>()
            .add_message::<CefUiRouteSet>()
            .add_message::<CefUiModalSet>()
            .add_message::<CefUiDiagnosticPush>()
            .configure_sets(
                PreUpdate,
                (GameCefUiSet::DrainIncoming, GameCefUiSet::InputOwnership).chain(),
            )
            .configure_sets(
                PostUpdate,
                (GameCefUiSet::CollectModel, GameCefUiSet::FlushBridge).chain(),
            )
            .add_systems(Startup, cef_ui_initialize_service)
            .add_systems(
                PreUpdate,
                drive_cef_ui_browser_startup.before(pump_cef_ui_message_loop),
            )
            .add_systems(
                PreUpdate,
                monitor_cef_ui_accelerated_paint_observation
                    .after(drive_cef_ui_browser_startup)
                    .before(pump_cef_ui_message_loop),
            )
            .add_systems(
                PreUpdate,
                (
                    drain_cef_incoming_queue,
                    apply_cef_ui_route_changes,
                    apply_cef_ui_hit_region_changes,
                    apply_cef_ui_text_entry_changes,
                )
                    .chain()
                    .in_set(GameCefUiSet::DrainIncoming),
            )
            .add_systems(
                PreUpdate,
                pump_cef_ui_message_loop.before(GameCefUiSet::DrainIncoming),
            )
            .add_systems(
                PreUpdate,
                (
                    update_cef_ui_focus_mode,
                    update_cef_ui_pointer_state,
                    update_cef_ui_input_capture,
                    forward_bevy_input_to_cef,
                    update_gameplay_input_gate,
                    update_cef_ui_overlay_click_through,
                )
                    .chain()
                    .in_set(GameCefUiSet::InputOwnership),
            )
            .add_systems(
                PostUpdate,
                collect_gameplay_ui_model.in_set(GameCefUiSet::CollectModel),
            )
            .add_systems(
                PostUpdate,
                (
                    push_fun_host_state_events,
                    write_cef_ui_outgoing_messages,
                    send_cef_ui_patch_batch,
                    flush_cef_ui_host_envelopes_to_browser,
                )
                    .chain()
                    .in_set(GameCefUiSet::FlushBridge),
            )
            .add_systems(
                PostUpdate,
                (
                    upload_cef_ui_frame_to_fun_texture,
                    update_fun_client_fps_counter,
                )
                    .chain()
                    .after(GameCefUiSet::FlushBridge),
            )
            .add_systems(
                Last,
                (sample_cef_ui_transport_counters, flush_cef_ui_diagnostics)
                    .chain()
                    .in_set(GameCefUiSet::Diagnostics),
            );
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CefUiRoute {
    #[default]
    Hud,
    PauseMenu,
    Loadout,
    Scoreboard,
    Chat,
    Loading,
    Diagnostics,
    DevtoolsOverlay,
}

impl CefUiRoute {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Hud => "hud",
            Self::PauseMenu => "pause_menu",
            Self::Loadout => "loadout",
            Self::Scoreboard => "scoreboard",
            Self::Chat => "chat",
            Self::Loading => "loading",
            Self::Diagnostics => "diagnostics",
            Self::DevtoolsOverlay => "devtools_overlay",
        }
    }

    #[must_use]
    pub const fn browser_route(self) -> BrowserUiRouteState {
        match self {
            Self::Hud => BrowserUiRouteState::Hud,
            Self::PauseMenu => BrowserUiRouteState::PauseMenu,
            Self::Loadout => BrowserUiRouteState::Loadout,
            Self::Scoreboard => BrowserUiRouteState::Scoreboard,
            Self::Chat => BrowserUiRouteState::Chat,
            Self::Loading => BrowserUiRouteState::Loading,
            Self::Diagnostics => BrowserUiRouteState::Diagnostics,
            Self::DevtoolsOverlay => BrowserUiRouteState::DevtoolsOverlay,
        }
    }

    #[must_use]
    pub const fn modal_reason(self) -> Option<CefUiModalReason> {
        match self {
            Self::Hud => None,
            Self::PauseMenu => Some(CefUiModalReason::PauseMenu),
            Self::Loadout => Some(CefUiModalReason::Loadout),
            Self::Scoreboard => Some(CefUiModalReason::Scoreboard),
            Self::Chat => Some(CefUiModalReason::Chat),
            Self::Loading => Some(CefUiModalReason::Loading),
            Self::Diagnostics => Some(CefUiModalReason::Diagnostics),
            Self::DevtoolsOverlay => Some(CefUiModalReason::DevtoolsOverlay),
        }
    }
}

impl From<BrowserUiRouteState> for CefUiRoute {
    fn from(value: BrowserUiRouteState) -> Self {
        match value {
            BrowserUiRouteState::Hud => Self::Hud,
            BrowserUiRouteState::PauseMenu => Self::PauseMenu,
            BrowserUiRouteState::Loadout => Self::Loadout,
            BrowserUiRouteState::Scoreboard => Self::Scoreboard,
            BrowserUiRouteState::Chat => Self::Chat,
            BrowserUiRouteState::Loading => Self::Loading,
            BrowserUiRouteState::Diagnostics => Self::Diagnostics,
            BrowserUiRouteState::DevtoolsOverlay => Self::DevtoolsOverlay,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CefUiModalReason {
    PauseMenu,
    Loadout,
    Scoreboard,
    Chat,
    Loading,
    Diagnostics,
    DevtoolsOverlay,
}

impl CefUiModalReason {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::PauseMenu => "pause_menu",
            Self::Loadout => "loadout",
            Self::Scoreboard => "scoreboard",
            Self::Chat => "chat",
            Self::Loading => "loading",
            Self::Diagnostics => "diagnostics",
            Self::DevtoolsOverlay => "devtools_overlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefUiError {
    BridgeClosed,
    BridgeQueueFull,
    InvalidEnvelope(BrowserUiProtocolValidationError),
    RejectedUiRequest(GameUiRequestRejectionReason),
    PatchWrite(UiPatchWriteError),
}

impl From<BrowserBridgeError> for CefUiError {
    fn from(value: BrowserBridgeError) -> Self {
        match value {
            BrowserBridgeError::Closed => Self::BridgeClosed,
            BrowserBridgeError::QueueFull => Self::BridgeQueueFull,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CefUiStatus {
    pub initialized: bool,
    pub browser_loaded: bool,
    pub current_route: CefUiRoute,
    pub compositor_visible: bool,
    pub last_frame_generation: Option<UiSurfaceGeneration>,
    pub last_error: Option<CefUiError>,
    pub last_blocked_navigation: Option<FunUiNavigationBlockReason>,
    pub last_js_error: Option<CefUiJsErrorKind>,
}

impl Default for CefUiStatus {
    fn default() -> Self {
        Self {
            initialized: false,
            browser_loaded: false,
            current_route: CefUiRoute::Hud,
            compositor_visible: false,
            last_frame_generation: None,
            last_error: None,
            last_blocked_navigation: None,
            last_js_error: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefUiJsErrorKind {
    InvalidBridgeEnvelope,
    RejectedRequest,
    QueueClosed,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub enum CefUiFocusMode {
    #[default]
    Gameplay,
    HudPassive,
    UiModal,
    TextEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CefUiHitRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CefUiHitRegion {
    pub id: BrowserUiHitRegionId,
    pub rect: CefUiHitRect,
}

impl From<BrowserUiHitRegion> for CefUiHitRegion {
    fn from(value: BrowserUiHitRegion) -> Self {
        Self {
            id: value.id,
            rect: CefUiHitRect {
                x: value.x,
                y: value.y,
                width: value.w,
                height: value.h,
            },
        }
    }
}

impl CefUiHitRegion {
    #[must_use]
    pub const fn contains(self, point: CefUiPointerPosition) -> bool {
        point.x >= self.rect.x
            && point.y >= self.rect.y
            && point.x < self.rect.x.saturating_add(self.rect.width)
            && point.y < self.rect.y.saturating_add(self.rect.height)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct CefUiInputCapture {
    pub capture_keyboard: bool,
    pub capture_pointer: bool,
    pub hit_region_mode: BrowserUiHitRegionMode,
    pub hit_regions: Vec<CefUiHitRegion>,
    pub active_hit_region: Option<BrowserUiHitRegionId>,
    pub modal_reason: Option<CefUiModalReason>,
    pub text_entry_active: bool,
}

impl CefUiInputCapture {
    fn set_hit_regions(&mut self, mode: BrowserUiHitRegionMode, regions: &[BrowserUiHitRegion]) {
        self.hit_region_mode = mode;
        self.hit_regions.clear();
        self.hit_regions.extend(
            regions
                .iter()
                .copied()
                .take(MAX_CEF_UI_HIT_REGIONS)
                .map(CefUiHitRegion::from),
        );
    }

    #[must_use]
    fn hit_region_at(&self, point: CefUiPointerPosition) -> Option<BrowserUiHitRegionId> {
        self.hit_regions
            .iter()
            .find(|region| region.contains(point))
            .map(|region| region.id)
    }
}

impl Default for CefUiInputCapture {
    fn default() -> Self {
        Self {
            capture_keyboard: false,
            capture_pointer: false,
            hit_region_mode: BrowserUiHitRegionMode::Gameplay,
            hit_regions: Vec::with_capacity(MAX_CEF_UI_HIT_REGIONS),
            active_hit_region: None,
            modal_reason: None,
            text_entry_active: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CefUiPointerPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CefUiPointerState {
    pub position: Option<CefUiPointerPosition>,
    pub active_hit_region: Option<BrowserUiHitRegionId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CefUiOverlayClickThroughState {
    pub click_through: bool,
    pub change_count: u64,
}

impl Default for CefUiOverlayClickThroughState {
    fn default() -> Self {
        Self {
            click_through: true,
            change_count: 0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum GameplayInputBlockReason {
    #[default]
    None,
    UiPointerRegion,
    UiModal,
    TextEntry,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct GameplayInputGate {
    pub block_movement: bool,
    pub block_fire: bool,
    pub block_actions: bool,
    pub block_look: bool,
    pub keyboard_owned_by_ui: bool,
    pub pointer_owned_by_ui: bool,
    pub reason: GameplayInputBlockReason,
}

impl GameplayInputGate {
    #[must_use]
    pub const fn blocks_movement(self) -> bool {
        self.block_movement
    }

    #[must_use]
    pub const fn blocks_pointer_actions(self) -> bool {
        self.block_fire || self.block_actions || self.pointer_owned_by_ui
    }

    #[must_use]
    pub const fn blocks_keyboard_actions(self) -> bool {
        self.block_actions || self.keyboard_owned_by_ui
    }

    #[must_use]
    pub const fn blocks_look(self) -> bool {
        self.block_look || self.pointer_owned_by_ui
    }
}

#[derive(Clone)]
pub struct CefUiBrowserControl {
    browser: CefUiBrowserHandle,
}

impl CefUiBrowserControl {
    #[must_use]
    pub const fn new(browser: CefUiBrowserHandle) -> Self {
        Self { browser }
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.browser.is_ready()
    }

    pub fn flush_host_envelopes_to_js(&self) -> usize {
        self.browser.flush_host_envelopes_to_js()
    }
}

struct CefUiBrowserOwner {
    _browser: CefUiBrowser,
}

impl CefUiBrowserOwner {
    fn new(browser: CefUiBrowser) -> Self {
        Self { _browser: browser }
    }
}

#[derive(Debug, Clone, Resource)]
pub struct CefUiStartupConfig {
    browser_config: BrowserUiConfig,
    compositor: SharedCefUiCompositor,
    bridge_queues: SharedBrowserBridgeQueues,
    message_loop_strategy: CefMessageLoopStrategy,
    gpu_bridge_timeout: Duration,
}

impl CefUiStartupConfig {
    #[must_use]
    pub fn new(
        browser_config: BrowserUiConfig,
        compositor: SharedCefUiCompositor,
        bridge_queues: SharedBrowserBridgeQueues,
        message_loop_strategy: CefMessageLoopStrategy,
    ) -> Self {
        Self {
            browser_config,
            compositor,
            bridge_queues,
            message_loop_strategy,
            gpu_bridge_timeout: cef_ui_gpu_bridge_startup_timeout_from_env(),
        }
    }

    #[must_use]
    pub const fn browser_config(&self) -> &BrowserUiConfig {
        &self.browser_config
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CefUiRequestedPaintTransportResource {
    pub requested: CefUiRequestedPaintTransport,
}

impl CefUiRequestedPaintTransportResource {
    #[must_use]
    pub const fn new(requested: CefUiRequestedPaintTransport) -> Self {
        Self { requested }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CefUiStartupStateKind {
    NotStarted,
    WaitingForGpuBridge,
    StartingCpuFallback { reason: CefUiFallbackReason },
    StartingAccelerated,
    Running,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CefUiStartupState {
    pub kind: CefUiStartupStateKind,
    waiting_elapsed: Duration,
    running_transport: Option<CefUiPaintTransport>,
    accelerated_observe_elapsed: Duration,
}

impl Default for CefUiStartupState {
    fn default() -> Self {
        Self {
            kind: CefUiStartupStateKind::NotStarted,
            waiting_elapsed: Duration::ZERO,
            running_transport: None,
            accelerated_observe_elapsed: Duration::ZERO,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dx12CefInteropReady {
    pub generation: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dx12CefInteropState {
    Pending,
    Ready(Dx12CefInteropReady),
    Error { reason: CefUiFallbackReason },
}

#[derive(Debug, Clone, Resource)]
pub struct SharedDx12CefInteropSlot {
    state: std::sync::Arc<std::sync::Mutex<Dx12CefInteropState>>,
    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    interop: std::sync::Arc<std::sync::Mutex<Option<Arc<Dx12CefInterop>>>>,
}

impl Default for SharedDx12CefInteropSlot {
    fn default() -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(Dx12CefInteropState::Pending)),
            #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
            interop: std::sync::Arc::new(std::sync::Mutex::new(None)),
        }
    }
}

impl SharedDx12CefInteropSlot {
    #[must_use]
    pub fn snapshot(&self) -> Dx12CefInteropState {
        self.state
            .lock()
            .map(|state| *state)
            .unwrap_or(Dx12CefInteropState::Error {
                reason: CefUiPaintTransportFallbackReason::DeviceQueueExtractionFailed,
            })
    }

    pub fn set_ready(&self, ready: Dx12CefInteropReady) {
        if let Ok(mut state) = self.state.lock() {
            *state = Dx12CefInteropState::Ready(ready);
        }
    }

    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    pub fn set_ready_with_interop(&self, ready: Dx12CefInteropReady, interop: Arc<Dx12CefInterop>) {
        if let Ok(mut bridge) = self.interop.lock() {
            *bridge = Some(interop);
        }
        self.set_ready(ready);
    }

    pub fn set_error(&self, reason: CefUiFallbackReason) {
        if let Ok(mut state) = self.state.lock() {
            if matches!(*state, Dx12CefInteropState::Pending) {
                *state = Dx12CefInteropState::Error { reason };
            }
        }
    }

    pub fn request_fallback(&self, reason: CefUiFallbackReason) {
        if let Ok(mut state) = self.state.lock() {
            *state = Dx12CefInteropState::Error { reason };
        }
    }

    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    #[must_use]
    pub fn interop(&self) -> Option<Arc<Dx12CefInterop>> {
        self.interop.lock().ok().and_then(|bridge| bridge.clone())
    }
}

impl ExtractResource for SharedDx12CefInteropSlot {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

#[cfg(target_os = "windows")]
impl CefAcceleratedPaintSink for SharedDx12CefInteropSlot {
    fn ingest_cef_accelerated_paint(
        &self,
        frame: CefAcceleratedPaintFrame<'_>,
    ) -> CefAcceleratedPaintOutcome {
        #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
        if let Some(interop) = self.interop() {
            let outcome = interop.ingest_accelerated_paint(frame);
            if let CefAcceleratedPaintOutcome::FallbackRequested { reason } = outcome {
                self.request_fallback(reason);
            }
            return outcome;
        }

        #[cfg(not(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint")))]
        let _ = frame;
        let reason = CefUiPaintTransportFallbackReason::D3d11On12BridgeUnavailable;
        self.request_fallback(reason);
        CefAcceleratedPaintOutcome::FallbackRequested { reason }
    }
}

#[derive(Debug, Clone, Resource)]
pub struct CefUiAuthority {
    pub context: GameUiProtocolValidationContext,
}

impl Default for CefUiAuthority {
    fn default() -> Self {
        Self {
            context: GameUiProtocolValidationContext::local_game_client(
                std::env::var_os("FUN_CEF_UI_DEVTOOLS").is_some(),
            ),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CefUiSecurityPolicyResource {
    pub policy: CefUiSecurityPolicy,
}

#[derive(Debug, Clone, Resource)]
pub struct CefUiBridge {
    queues: SharedBrowserBridgeQueues,
    patch_writer: UiPatchWriter,
    patch_queue: UiPatchBackpressureQueue,
    next_sequence: BrowserUiSequence,
}

impl CefUiBridge {
    #[must_use]
    pub fn new(queues: SharedBrowserBridgeQueues) -> Self {
        Self {
            queues,
            patch_writer: UiPatchWriter::default(),
            patch_queue: UiPatchBackpressureQueue::default(),
            next_sequence: BrowserUiSequence::default(),
        }
    }

    pub fn push_js_envelope(&mut self, envelope: UiEnvelope) -> Result<(), BrowserBridgeError> {
        self.queues.push_js_envelope(envelope)
    }

    pub fn push_patch(
        &mut self,
        channel: GameUiChannel,
        field: GameUiFieldKey,
        value: UiPatchValue,
    ) -> Result<(), UiPatchWriteError> {
        write_patch_value(&mut self.patch_writer, channel, field, value)
    }

    fn pop_js_envelope_for_host(&mut self) -> Option<UiEnvelope> {
        self.queues.pop_js_envelope_for_host()
    }

    fn push_host_envelope(&mut self, envelope: UiEnvelope) -> Result<(), BrowserBridgeError> {
        self.queues.push_host_envelope(envelope)
    }

    fn patch_writer(&mut self) -> &mut UiPatchWriter {
        &mut self.patch_writer
    }

    fn finish_patch_writer(&mut self) {
        if let Some(batch) = self.patch_writer.finish_batch() {
            self.patch_queue.push_batch(batch);
        }
    }

    fn pop_patch_batch(&mut self) -> Option<UiPatchBatch> {
        self.patch_queue.pop_batch()
    }

    fn next_sequence(&mut self) -> BrowserUiSequence {
        self.next_sequence = BrowserUiSequence(self.next_sequence.0.saturating_add(1));
        self.next_sequence
    }

    fn dropped_patch_count(&self) -> u64 {
        self.patch_writer
            .dropped_patch_count()
            .saturating_add(self.patch_queue.dropped_batch_count())
    }

    fn coalesced_patch_count(&self) -> u64 {
        self.patch_queue.coalesced_patch_count()
    }
}

impl Default for CefUiBridge {
    fn default() -> Self {
        Self::new(SharedBrowserBridgeQueues::default())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CefUiFrameStats {
    pub paint_count: u64,
    pub accelerated_paint_count: u64,
    pub dirty_rect_count: u64,
    pub dirty_rect_explosion_count: u64,
    pub last_dirty_rect_union: Option<CefDirtyRect>,
    pub full_frame_upload_count: u64,
    pub last_full_frame_reason: Option<CefUiFullUploadReason>,
    pub uploaded_bytes: u64,
    pub gpu_copied_bytes: u64,
    pub gpu_copy_count: u64,
    pub gpu_copy_fail_count: u64,
    pub cpu_fallback_count: u64,
    pub shared_texture_open_fail_count: u64,
    pub stale_gpu_frame_count: u64,
    pub cef_on_paint_fps: u64,
    pub cef_on_accelerated_paint_fps: u64,
    pub cef_cpu_upload_bytes: u64,
    pub cef_gpu_copy_count: u64,
    pub cef_gpu_copy_bytes: u64,
    pub cef_gpu_copy_ns: u64,
    pub cef_gpu_copy_failures: u64,
    pub cef_transport_fallback_count: u64,
    pub cef_published_generation: u64,
    pub cef_sampled_generation: u64,
    pub js_message_count: u64,
    pub js_message_rejected_count: u64,
    pub patch_batch_count: u64,
    pub dropped_patch_count: u64,
    pub coalesced_patch_count: u64,
    pub overlay_click_through_change_count: u64,
    pub navigation_blocked_count: u64,
}

fn record_cef_dirty_rect_metadata(stats: &mut CefUiFrameStats, metadata: CefUiDirtyRectMetadata) {
    stats.dirty_rect_count = stats
        .dirty_rect_count
        .saturating_add(metadata.dirty_rect_count as u64);
    stats.dirty_rect_explosion_count = stats
        .dirty_rect_explosion_count
        .saturating_add(metadata.dirty_rect_explosion_count);
    stats.last_dirty_rect_union = metadata.dirty_rect_union;
    if let Some(reason) = metadata.full_frame_reason {
        stats.full_frame_upload_count = stats.full_frame_upload_count.saturating_add(1);
        stats.last_full_frame_reason = Some(reason);
    } else {
        stats.last_full_frame_reason = None;
    }
}

#[derive(Debug, Clone, Resource)]
pub struct CefUiTransportCountersResource {
    counters: SharedCefUiTransportCounters,
}

impl CefUiTransportCountersResource {
    #[must_use]
    pub fn new(counters: SharedCefUiTransportCounters) -> Self {
        Self { counters }
    }

    #[must_use]
    fn snapshot(&self) -> CefUiTransportCounterSnapshot {
        self.counters.snapshot()
    }
}

impl Default for CefUiTransportCountersResource {
    fn default() -> Self {
        Self {
            counters: SharedCefUiTransportCounters::default(),
        }
    }
}

impl ExtractResource for CefUiTransportCountersResource {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

#[derive(Debug, Resource)]
struct CefUiTransportCounterSampler {
    sample_timer: Timer,
    last_snapshot: Option<CefUiTransportCounterSnapshot>,
}

impl Default for CefUiTransportCounterSampler {
    fn default() -> Self {
        Self {
            sample_timer: Timer::new(CEF_UI_TRANSPORT_COUNTER_REFRESH, TimerMode::Repeating),
            last_snapshot: None,
        }
    }
}

#[derive(Debug, Clone, Resource)]
pub struct CefUiRenderCompositor {
    compositor: SharedCefUiCompositor,
}

impl CefUiRenderCompositor {
    #[must_use]
    pub fn new(compositor: SharedCefUiCompositor) -> Self {
        Self { compositor }
    }

    #[must_use]
    pub fn compositor(&self) -> &SharedCefUiCompositor {
        &self.compositor
    }
}

#[derive(Debug, Resource)]
pub struct CefUiMessageLoopPump {
    enabled: bool,
    pump_timer: Timer,
    active_logged: bool,
}

impl CefUiMessageLoopPump {
    #[must_use]
    pub fn external_pump_60hz() -> Self {
        Self {
            enabled: true,
            pump_timer: Timer::new(cef_ui_render_interval(), TimerMode::Repeating),
            active_logged: false,
        }
    }

    #[must_use]
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            pump_timer: Timer::new(cef_ui_render_interval(), TimerMode::Repeating),
            active_logged: false,
        }
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn interval(&self) -> Duration {
        self.pump_timer.duration()
    }
}

impl Default for CefUiMessageLoopPump {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Debug, Resource)]
struct CefUiRenderTexture {
    image: Option<Handle<Image>>,
    root_entity: Option<Entity>,
    last_generation: Option<UiSurfaceGeneration>,
    size: Option<UVec2>,
    upload_timer: Timer,
}

impl Default for CefUiRenderTexture {
    fn default() -> Self {
        Self {
            image: None,
            root_entity: None,
            last_generation: None,
            size: None,
            upload_timer: Timer::new(cef_ui_render_interval(), TimerMode::Repeating),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Component)]
struct CefUiRenderTextureRoot;

#[derive(Debug, Clone, Default, Resource)]
struct CefUiTextureUploads {
    latest: Option<CefUiTextureUpload>,
    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    latest_gpu: Option<CefUiGpuTextureUpload>,
}

impl ExtractResource for CefUiTextureUploads {
    type Source = Self;

    fn extract_resource(source: &Self::Source) -> Self {
        source.clone()
    }
}

#[derive(Debug, Clone)]
struct CefUiTextureUpload {
    image: Handle<Image>,
    generation: UiSurfaceGeneration,
    size: UVec2,
    pixels: Vec<u8>,
    dirty_rects: Vec<CefDirtyRect>,
    force_full_upload: bool,
}

#[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
#[derive(Debug, Clone)]
struct CefUiGpuTextureUpload {
    image: Handle<Image>,
    token: Dx12CefReadyFrameToken,
}

#[derive(Debug, Default, Resource)]
struct CefUiGpuUploadState {
    image_id: Option<AssetId<Image>>,
    last_generation: Option<UiSurfaceGeneration>,
    gpu_copy_failure_logged: bool,
    gpu_copy_failure_count: u32,
    stale_gpu_frame_count_for_current_token: u32,
    stale_gpu_frame_token: Option<UiSurfaceGeneration>,
}

#[derive(Debug, Default, Clone, Copy, Component)]
struct FunClientFpsCounterRoot;

#[derive(Debug, Default, Clone, Copy, Component)]
struct FunClientFpsCounterText;

#[derive(Debug, Resource)]
struct FunClientFpsCounterState {
    update_timer: Timer,
    last_label: String,
}

impl Default for FunClientFpsCounterState {
    fn default() -> Self {
        Self {
            update_timer: Timer::new(FUN_CLIENT_FPS_COUNTER_REFRESH, TimerMode::Repeating),
            last_label: "FPS --".to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct FunClientFpsCounterLayout {
    left: f32,
    top: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
struct CefUiDiagnosticsState {
    last_dropped_patch_count: u64,
    last_coalesced_patch_count: u64,
    last_overlay_change_count: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
struct CefUiHostStateEventCache {
    last_sequence: u64,
}

#[derive(Debug, Clone, Message)]
pub enum CefUiIntent {
    Ready,
    ChatSubmit { message: String },
    MenuCommand { command: BrowserUiMenuCommand },
    SettingsChanged { key: String, value_json: Vec<u8> },
    Lifecycle { state: UiLifecycleState },
}

#[derive(Debug, Clone, Message)]
pub struct CefUiRequest {
    pub envelope: GameUiRequestEnvelope,
}

#[derive(Debug, Clone, Copy, Message)]
pub struct CefUiRouteChanged {
    pub route: CefUiRoute,
}

#[derive(Debug, Clone, Message)]
pub struct CefUiHitRegionsChanged {
    pub mode: BrowserUiHitRegionMode,
    pub regions: Vec<BrowserUiHitRegion>,
}

#[derive(Debug, Clone, Copy, Message)]
pub struct CefUiTextEntryChanged {
    pub active: bool,
}

#[derive(Debug, Clone, Message)]
pub struct CefUiPatch {
    pub channel: GameUiChannel,
    pub field: GameUiFieldKey,
    pub value: UiPatchValue,
}

#[derive(Debug, Clone, Copy, Message)]
pub struct CefUiCommand {
    pub command: BrowserUiMenuCommand,
}

#[derive(Debug, Clone, Copy, Message)]
pub struct CefUiRouteSet {
    pub route: CefUiRoute,
}

#[derive(Debug, Clone, Copy, Message)]
pub struct CefUiModalSet {
    pub reason: Option<CefUiModalReason>,
}

#[derive(Debug, Clone, Copy, Message)]
pub struct CefUiDiagnosticPush {
    pub kind: CefUiDiagnosticKind,
    pub severity: CefUiDiagnosticSeverity,
    pub value: u64,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
struct CefUiModelCache {
    previous: Option<CefUiRuntimeModel>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct CefUiRuntimeModel {
    browser_loaded: bool,
    compositor_visible: bool,
    overlay_click_through: bool,
    focus_mode_code: u16,
    render_world_ready: bool,
    expected_chunks: u16,
    received_chunks: u16,
}

impl CefUiRuntimeModel {
    fn from_resources(
        status: &CefUiStatus,
        focus_mode: CefUiFocusMode,
        overlay_state: &CefUiOverlayClickThroughState,
        world_status: &RenderWorldStatus,
        world_context: &RenderWorldContext,
    ) -> Self {
        Self {
            browser_loaded: status.browser_loaded,
            compositor_visible: status.compositor_visible,
            overlay_click_through: overlay_state.click_through,
            focus_mode_code: focus_mode_code(focus_mode),
            render_world_ready: world_status.ready,
            expected_chunks: world_context.expected_chunks,
            received_chunks: world_context
                .received_chunks
                .len()
                .min(usize::from(u16::MAX)) as u16,
        }
    }
}

impl CefUiModel for CefUiRuntimeModel {
    const CHANNEL: GameUiChannel = GameUiChannel::Match;
    const SCHEMA_REVISION: u32 = fun_ui_cef::DEFAULT_UI_PATCH_SCHEMA_REVISION;

    fn write_initial_snapshot(&self, out: &mut UiPatchWriter) {
        let _ = out.set_bool(
            GameUiChannel::Diagnostics,
            GameUiFieldKey::BrowserLoaded,
            self.browser_loaded,
        );
        let _ = out.set_bool(
            GameUiChannel::Diagnostics,
            GameUiFieldKey::CompositorVisible,
            self.compositor_visible,
        );
        let _ = out.set_bool(
            GameUiChannel::Diagnostics,
            GameUiFieldKey::OverlayClickThrough,
            self.overlay_click_through,
        );
        let _ = out.set_u16(
            GameUiChannel::Diagnostics,
            GameUiFieldKey::FocusMode,
            self.focus_mode_code,
        );
        let _ = out.set_bool(
            Self::CHANNEL,
            GameUiFieldKey::LoadingState,
            !self.render_world_ready,
        );
        let _ = out.set_u16(
            Self::CHANNEL,
            GameUiFieldKey::StreamExpectedChunks,
            self.expected_chunks,
        );
        let _ = out.set_u16(
            Self::CHANNEL,
            GameUiFieldKey::StreamReceivedChunks,
            self.received_chunks,
        );
    }

    fn write_patch(&self, previous: &Self, out: &mut UiPatchWriter) {
        if self.browser_loaded != previous.browser_loaded {
            let _ = out.set_bool(
                GameUiChannel::Diagnostics,
                GameUiFieldKey::BrowserLoaded,
                self.browser_loaded,
            );
        }
        if self.compositor_visible != previous.compositor_visible {
            let _ = out.set_bool(
                GameUiChannel::Diagnostics,
                GameUiFieldKey::CompositorVisible,
                self.compositor_visible,
            );
        }
        if self.overlay_click_through != previous.overlay_click_through {
            let _ = out.set_bool(
                GameUiChannel::Diagnostics,
                GameUiFieldKey::OverlayClickThrough,
                self.overlay_click_through,
            );
        }
        if self.focus_mode_code != previous.focus_mode_code {
            let _ = out.set_u16(
                GameUiChannel::Diagnostics,
                GameUiFieldKey::FocusMode,
                self.focus_mode_code,
            );
        }
        if self.render_world_ready != previous.render_world_ready {
            let _ = out.set_bool(
                Self::CHANNEL,
                GameUiFieldKey::LoadingState,
                !self.render_world_ready,
            );
        }
        if self.expected_chunks != previous.expected_chunks {
            let _ = out.set_u16(
                Self::CHANNEL,
                GameUiFieldKey::StreamExpectedChunks,
                self.expected_chunks,
            );
        }
        if self.received_chunks != previous.received_chunks {
            let _ = out.set_u16(
                Self::CHANNEL,
                GameUiFieldKey::StreamReceivedChunks,
                self.received_chunks,
            );
        }
    }
}

const fn focus_mode_code(focus_mode: CefUiFocusMode) -> u16 {
    match focus_mode {
        CefUiFocusMode::Gameplay => 1,
        CefUiFocusMode::HudPassive => 2,
        CefUiFocusMode::UiModal => 3,
        CefUiFocusMode::TextEntry => 4,
    }
}

fn cef_ui_initialize_service(mut status: ResMut<CefUiStatus>) {
    status.initialized = true;
    status.compositor_visible = true;
    status.current_route = CefUiRoute::Hud;
    tracing::info!(
        target: FUN_UI_DIAGNOSTICS_TARGET,
        "CEF UI ECS binding initialized"
    );
}

fn drive_cef_ui_browser_startup(world: &mut World) {
    let Some(startup_config) = world.get_resource::<CefUiStartupConfig>().cloned() else {
        return;
    };
    if world.contains_non_send::<CefUiBrowserOwner>() {
        if let Some(mut startup_state) = world.get_resource_mut::<CefUiStartupState>() {
            startup_state.kind = CefUiStartupStateKind::Running;
        }
        return;
    }
    let slot_state = world
        .get_resource::<SharedDx12CefInteropSlot>()
        .map(SharedDx12CefInteropSlot::snapshot)
        .unwrap_or(Dx12CefInteropState::Error {
            reason: CefUiPaintTransportFallbackReason::DeviceQueueExtractionFailed,
        });
    let delta = world
        .get_resource::<Time>()
        .map(Time::delta)
        .unwrap_or_default();
    let Some(browser_config) =
        world
            .get_resource_mut::<CefUiStartupState>()
            .and_then(|mut state| {
                cef_ui_startup_next_config(&startup_config, slot_state, delta, &mut state)
            })
    else {
        return;
    };
    start_cef_ui_browser(world, startup_config, browser_config);
}

fn cef_ui_startup_next_config(
    startup_config: &CefUiStartupConfig,
    slot_state: Dx12CefInteropState,
    delta: Duration,
    state: &mut CefUiStartupState,
) -> Option<BrowserUiConfig> {
    match state.kind {
        CefUiStartupStateKind::NotStarted => {
            state.waiting_elapsed = Duration::ZERO;
            match startup_config.browser_config.requested_paint_transport {
                CefUiRequestedPaintTransport::Cpu => Some(cef_ui_cpu_start_config(
                    startup_config,
                    CefUiPaintTransportFallbackReason::None,
                )),
                CefUiRequestedPaintTransport::Auto | CefUiRequestedPaintTransport::D3d11On12 => {
                    if !cfg!(target_os = "windows") {
                        return Some(cef_ui_cpu_start_config(
                            startup_config,
                            CefUiPaintTransportFallbackReason::NonWindows,
                        ));
                    }
                    if !startup_config
                        .browser_config
                        .render_backend_hint
                        .is_dx12_compatible()
                    {
                        return Some(cef_ui_cpu_start_config(
                            startup_config,
                            CefUiPaintTransportFallbackReason::RenderBackendNotDx12,
                        ));
                    }
                    state.kind = CefUiStartupStateKind::WaitingForGpuBridge;
                    tracing::info!(
                        target: FUN_UI_DIAGNOSTICS_TARGET,
                        requested_transport = startup_config
                            .browser_config
                            .requested_paint_transport
                            .as_wire_str(),
                        backend = startup_config.browser_config.render_backend_hint.as_wire_str(),
                        timeout_ms = startup_config.gpu_bridge_timeout.as_millis(),
                        "CEF UI waiting for DX12 CEF interop bridge"
                    );
                    None
                }
            }
        }
        CefUiStartupStateKind::WaitingForGpuBridge => {
            state.waiting_elapsed = state.waiting_elapsed.saturating_add(delta);
            match slot_state {
                Dx12CefInteropState::Ready(ready) => {
                    state.kind = CefUiStartupStateKind::StartingAccelerated;
                    tracing::info!(
                        target: FUN_UI_DIAGNOSTICS_TARGET,
                        bridge_generation = ready.generation,
                        "CEF UI DX12 interop bridge ready"
                    );
                    Some(
                        startup_config
                            .browser_config
                            .clone()
                            .with_paint_transport_decision(
                                CefUiPaintTransport::D3d11SharedTextureDx12Copy,
                                CefUiPaintTransportFallbackReason::None,
                            ),
                    )
                }
                Dx12CefInteropState::Error { reason } => {
                    Some(cef_ui_cpu_start_config(startup_config, reason))
                }
                Dx12CefInteropState::Pending
                    if state.waiting_elapsed >= startup_config.gpu_bridge_timeout =>
                {
                    Some(cef_ui_cpu_start_config(
                        startup_config,
                        CefUiPaintTransportFallbackReason::GpuBridgeTimeout,
                    ))
                }
                Dx12CefInteropState::Pending => None,
            }
        }
        CefUiStartupStateKind::StartingCpuFallback { .. }
        | CefUiStartupStateKind::StartingAccelerated
        | CefUiStartupStateKind::Running
        | CefUiStartupStateKind::Failed => None,
    }
}

fn cef_ui_cpu_start_config(
    startup_config: &CefUiStartupConfig,
    reason: CefUiPaintTransportFallbackReason,
) -> BrowserUiConfig {
    let reason = if reason == CefUiPaintTransportFallbackReason::None {
        CefUiPaintTransportFallbackReason::None
    } else {
        tracing::warn!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            fallback_reason = reason.as_wire_str(),
            requested_transport = startup_config
                .browser_config
                .requested_paint_transport
                .as_wire_str(),
            "CEF UI starting CPU paint fallback"
        );
        reason
    };
    startup_config
        .browser_config
        .clone()
        .with_paint_transport_decision(CefUiPaintTransport::CpuPaint, reason)
}

fn start_cef_ui_browser(
    world: &mut World,
    startup_config: CefUiStartupConfig,
    browser_config: BrowserUiConfig,
) {
    if let Some(mut state) = world.get_resource_mut::<CefUiStartupState>() {
        state.kind = match browser_config.paint_transport {
            CefUiPaintTransport::CpuPaint => CefUiStartupStateKind::StartingCpuFallback {
                reason: browser_config.paint_transport_fallback_reason,
            },
            CefUiPaintTransport::D3d11SharedTextureDx12Copy => {
                CefUiStartupStateKind::StartingAccelerated
            }
        };
        state.running_transport = None;
        state.accelerated_observe_elapsed = Duration::ZERO;
    }

    let page_url = browser_config.page_url_str().to_owned();
    let viewport_width = browser_config.viewport_width;
    let viewport_height = browser_config.viewport_height;
    let paint_transport = browser_config.paint_transport;
    #[cfg(target_os = "windows")]
    let accelerated_paint_sink =
        if paint_transport == CefUiPaintTransport::D3d11SharedTextureDx12Copy {
            world
                .get_resource::<SharedDx12CefInteropSlot>()
                .map(|slot| Arc::new(slot.clone()) as Arc<dyn CefAcceleratedPaintSink>)
        } else {
            None
        };
    #[cfg(target_os = "windows")]
    let browser_result = CefUiBrowser::create_with_bridge_and_accelerated_sink(
        browser_config,
        startup_config.compositor.clone(),
        startup_config.bridge_queues.clone(),
        accelerated_paint_sink,
    );
    #[cfg(not(target_os = "windows"))]
    let browser_result = CefUiBrowser::create_with_bridge(
        browser_config,
        startup_config.compositor.clone(),
        startup_config.bridge_queues.clone(),
    );
    let browser = match browser_result {
        Ok(browser) => browser,
        Err(error) => {
            if let Some(mut state) = world.get_resource_mut::<CefUiStartupState>() {
                state.kind = CefUiStartupStateKind::Failed;
            }
            tracing::error!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                %error,
                "failed to load CEF UI page"
            );
            return;
        }
    };
    let handle = browser.handle();
    let counters = browser.transport_counters();
    world.insert_non_send(CefUiBrowserControl::new(handle));
    world.insert_non_send(CefUiBrowserOwner::new(browser));
    world.insert_resource(CefUiBridge::new(startup_config.bridge_queues));
    world.insert_resource(CefUiRenderCompositor::new(startup_config.compositor));
    world.insert_resource(CefUiTransportCountersResource::new(counters));
    if matches!(
        startup_config.message_loop_strategy,
        CefMessageLoopStrategy::ExternalPump
    ) {
        world.insert_resource(CefUiMessageLoopPump::external_pump_60hz());
    }
    if let Some(mut state) = world.get_resource_mut::<CefUiStartupState>() {
        state.kind = CefUiStartupStateKind::Running;
        state.running_transport = Some(paint_transport);
        state.accelerated_observe_elapsed = Duration::ZERO;
    }
    tracing::info!(
        target: FUN_UI_DIAGNOSTICS_TARGET,
        page_url,
        viewport_width,
        viewport_height,
        paint_transport = paint_transport.as_wire_str(),
        "loaded CEF UI page"
    );
}

fn monitor_cef_ui_accelerated_paint_observation(world: &mut World) {
    let Some(startup_config) = world.get_resource::<CefUiStartupConfig>().cloned() else {
        return;
    };
    let Some(snapshot) = world
        .get_resource::<CefUiTransportCountersResource>()
        .map(CefUiTransportCountersResource::snapshot)
    else {
        return;
    };
    let slot_state = world
        .get_resource::<SharedDx12CefInteropSlot>()
        .map(SharedDx12CefInteropSlot::snapshot)
        .unwrap_or(Dx12CefInteropState::Pending);
    let delta = world
        .get_resource::<Time>()
        .map(Time::delta)
        .unwrap_or_default();
    let fallback_reason = {
        let Some(mut state) = world.get_resource_mut::<CefUiStartupState>() else {
            return;
        };
        if state.kind != CefUiStartupStateKind::Running
            || state.running_transport != Some(CefUiPaintTransport::D3d11SharedTextureDx12Copy)
        {
            return;
        }
        if let Dx12CefInteropState::Error { reason } = slot_state {
            reason
        } else {
            if snapshot.cef_on_accelerated_paint_count > 0 {
                return;
            }
            state.accelerated_observe_elapsed =
                state.accelerated_observe_elapsed.saturating_add(delta);
            if snapshot.cef_on_paint_count >= 3
                || state.accelerated_observe_elapsed >= startup_config.gpu_bridge_timeout
            {
                CefUiPaintTransportFallbackReason::AcceleratedPaintNotObserved
            } else {
                return;
            }
        }
    };
    let _old_browser = world.remove_non_send::<CefUiBrowserOwner>();
    let _old_control = world.remove_non_send::<CefUiBrowserControl>();
    if let Some(mut state) = world.get_resource_mut::<CefUiStartupState>() {
        state.kind = CefUiStartupStateKind::StartingCpuFallback {
            reason: fallback_reason,
        };
        state.running_transport = None;
        state.accelerated_observe_elapsed = Duration::ZERO;
    }
    if let Some(mut stats) = world.get_resource_mut::<CefUiFrameStats>() {
        stats.cpu_fallback_count = stats.cpu_fallback_count.saturating_add(1);
        if fallback_reason == CefUiPaintTransportFallbackReason::GpuCopyFenceTimeout {
            stats.stale_gpu_frame_count = stats.stale_gpu_frame_count.saturating_add(1);
        }
    }
    tracing::warn!(
        target: FUN_UI_DIAGNOSTICS_TARGET,
        cef_on_paint_count = snapshot.cef_on_paint_count,
        cef_on_accelerated_paint_count = snapshot.cef_on_accelerated_paint_count,
        fallback_reason = fallback_reason.as_wire_str(),
        "CEF UI accelerated path requested CPU browser recreation"
    );
    let browser_config = startup_config
        .browser_config
        .clone()
        .with_paint_transport_decision(CefUiPaintTransport::CpuPaint, fallback_reason);
    start_cef_ui_browser(world, startup_config, browser_config);
}

fn initialize_dx12_cef_interop_slot(
    slot: Option<Res<SharedDx12CefInteropSlot>>,
    render_device: Option<Res<RenderDevice>>,
    render_queue: Option<Res<RenderQueue>>,
) {
    let Some(slot) = slot else {
        return;
    };
    if !matches!(slot.snapshot(), Dx12CefInteropState::Pending) {
        return;
    }
    let (Some(render_device), Some(render_queue)) = (render_device, render_queue) else {
        return;
    };
    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    {
        match unsafe { Dx12CefInterop::try_init_from_wgpu(&render_device, &render_queue) } {
            Ok(interop) => {
                let native_ptrs = interop.native_pointer_summary();
                let ring_len = interop.ring_len();
                let next_fence_value = interop.next_fence_value();
                slot.set_ready_with_interop(Dx12CefInteropReady { generation: 1 }, interop);
                tracing::info!(
                    target: FUN_UI_DIAGNOSTICS_TARGET,
                    bridge_generation = 1_u64,
                    d3d12_device = native_ptrs.d3d12_device,
                    d3d12_queue = native_ptrs.d3d12_queue,
                    d3d11_device = native_ptrs.d3d11_device,
                    d3d11_context = native_ptrs.d3d11_context,
                    d3d11_on12 = native_ptrs.d3d11_on12,
                    fence = native_ptrs.fence,
                    ring_len,
                    next_fence_value,
                    "CEF UI D3D11On12 bridge initialized"
                );
            }
            Err(error) => {
                slot.set_error(dx12_cef_interop_fallback_reason(error));
                tracing::warn!(
                    target: FUN_UI_DIAGNOSTICS_TARGET,
                    failure = error.failure.as_str(),
                    detail = error.detail,
                    hresult = error.hresult,
                    fallback_reason = dx12_cef_interop_fallback_reason(error).as_wire_str(),
                    "CEF UI D3D11On12 bridge initialization failed"
                );
            }
        }
        return;
    }

    #[cfg(not(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint")))]
    {
        let _ = (render_device, render_queue);
        slot.set_error(CefUiPaintTransportFallbackReason::D3d11On12BridgeUnavailable);
        tracing::info!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            "CEF UI DX12 interop slot observed render device and queue; D3D11On12 bridge feature is not compiled"
        );
    }
}

#[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
fn latest_cef_gpu_frame_token(
    slot: Option<&SharedDx12CefInteropSlot>,
) -> Option<Dx12CefReadyFrameToken> {
    let interop = slot?.interop()?;
    interop.latest_ready_frame_token()
}

#[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
fn windows_dxgi_bgra8_unorm() -> crate::cef_ui_dx12::DxgiFormat {
    windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM
}

#[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
const fn dx12_cef_interop_fallback_reason(
    error: Dx12CefInteropError,
) -> CefUiPaintTransportFallbackReason {
    match error.failure {
        crate::cef_ui_dx12::Dx12CefInteropFailure::WrongBackend => {
            CefUiPaintTransportFallbackReason::RenderBackendNotDx12
        }
        crate::cef_ui_dx12::Dx12CefInteropFailure::DeviceHalUnavailable
        | crate::cef_ui_dx12::Dx12CefInteropFailure::QueueHalUnavailable => {
            CefUiPaintTransportFallbackReason::DeviceQueueExtractionFailed
        }
        crate::cef_ui_dx12::Dx12CefInteropFailure::D3d11On12CreateDeviceFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::D3d11DeviceMissing
        | crate::cef_ui_dx12::Dx12CefInteropFailure::D3d11ImmediateContextMissing
        | crate::cef_ui_dx12::Dx12CefInteropFailure::D3d11On12QueryFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::FenceCreateFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::CopyCommandAllocatorCreateFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::CopyCommandListCreateFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::CopyCommandListCloseFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::CopyCommandListResetFailed => {
            CefUiPaintTransportFallbackReason::D3d11On12BridgeUnavailable
        }
        crate::cef_ui_dx12::Dx12CefInteropFailure::SharedTextureHandleMissing => {
            CefUiPaintTransportFallbackReason::SharedTextureUnsupported
        }
        crate::cef_ui_dx12::Dx12CefInteropFailure::SharedTextureOpenFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::SharedTextureUnsupportedFormat
        | crate::cef_ui_dx12::Dx12CefInteropFailure::InvalidFrameDimensions => {
            CefUiPaintTransportFallbackReason::SharedTextureUnsupported
        }
        crate::cef_ui_dx12::Dx12CefInteropFailure::DestinationTextureCreateFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::DestinationTextureWrapFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::BevyTargetTextureHalUnavailable
        | crate::cef_ui_dx12::Dx12CefInteropFailure::BevyTargetTextureUnsupported
        | crate::cef_ui_dx12::Dx12CefInteropFailure::BevyTextureCopyFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::FenceSignalFailed
        | crate::cef_ui_dx12::Dx12CefInteropFailure::OutputTextureRingUnavailable => {
            CefUiPaintTransportFallbackReason::OutputTextureAllocationUnavailable
        }
    }
}

fn drain_cef_incoming_queue(
    mut bridge: ResMut<CefUiBridge>,
    mut status: ResMut<CefUiStatus>,
    mut stats: ResMut<CefUiFrameStats>,
    authority: Res<CefUiAuthority>,
    mut writers: CefUiIncomingWriters,
) {
    let context = BrowserUiProtocolValidationContext::local_game_ui();
    for _ in 0..MAX_JS_MESSAGES_PER_FRAME {
        let Some(envelope) = bridge.pop_js_envelope_for_host() else {
            break;
        };
        stats.js_message_count = stats.js_message_count.saturating_add(1);
        if let Err(error) = validate_ui_envelope(&envelope, &context) {
            status.last_error = Some(CefUiError::InvalidEnvelope(error));
            status.last_js_error = Some(CefUiJsErrorKind::InvalidBridgeEnvelope);
            stats.js_message_rejected_count = stats.js_message_rejected_count.saturating_add(1);
            tracing::warn!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                ?error,
                "rejected JS bridge envelope"
            );
            continue;
        }
        if let Err(error) =
            drain_validated_envelope(envelope, &mut status, &authority.context, &mut writers)
        {
            stats.js_message_rejected_count = stats.js_message_rejected_count.saturating_add(1);
            status.last_error = Some(error);
            status.last_js_error = Some(CefUiJsErrorKind::RejectedRequest);
            tracing::warn!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                ?error,
                "rejected JS UI request"
            );
        }
    }
}

#[derive(SystemParam)]
struct CefUiIncomingWriters<'w> {
    intents: MessageWriter<'w, CefUiIntent>,
    requests: MessageWriter<'w, CefUiRequest>,
    host_commands: MessageWriter<'w, FunHostCommandRequest>,
    routes: MessageWriter<'w, CefUiRouteChanged>,
    hit_regions: MessageWriter<'w, CefUiHitRegionsChanged>,
    text_entry: MessageWriter<'w, CefUiTextEntryChanged>,
}

fn game_ui_request_from_control(
    request_id: BrowserUiRequestId,
    sequence: BrowserUiSequence,
    payload: &UiControlPayload,
) -> Result<Option<GameUiRequestEnvelope>, GameUiRequestRejectionReason> {
    let payload = match payload {
        UiControlPayload::ChatSubmit { message } => GameUiRequestPayload::SendChat {
            message: message.clone(),
        },
        UiControlPayload::MenuCommand { command } => GameUiRequestPayload::OpenMenu {
            target: menu_target_for_command(*command),
        },
        UiControlPayload::SettingsChanged { key, value_json } => {
            let Some(key) = GameUiSettingKey::from_wire_key(key) else {
                return Err(GameUiRequestRejectionReason::InvalidPayload);
            };
            GameUiRequestPayload::ChangeSetting {
                key,
                value: GameUiSettingValue::JsonBytes {
                    bytes: value_json.clone(),
                },
            }
        }
        UiControlPayload::Ready
        | UiControlPayload::RouteChanged { .. }
        | UiControlPayload::HitRegionsChanged { .. }
        | UiControlPayload::TextEntryChanged { .. }
        | UiControlPayload::HostCommand { .. }
        | UiControlPayload::HostEvent { .. }
        | UiControlPayload::Lifecycle { .. } => return Ok(None),
        UiControlPayload::HostCommandResult { .. } => {
            return Err(GameUiRequestRejectionReason::InvalidPayload);
        }
    };
    Ok(Some(GameUiRequestEnvelope::new(
        GameUiRequestId(request_id.0),
        GameUiSequence(sequence.0),
        payload,
    )))
}

fn menu_target_for_command(command: BrowserUiMenuCommand) -> GameUiMenuTarget {
    match command {
        BrowserUiMenuCommand::Resume | BrowserUiMenuCommand::LeaveMatch => GameUiMenuTarget::Pause,
        BrowserUiMenuCommand::OpenSettings => GameUiMenuTarget::Settings,
    }
}

fn write_validated_intent(payload: UiControlPayload, intents: &mut MessageWriter<CefUiIntent>) {
    match payload {
        UiControlPayload::ChatSubmit { message } => {
            intents.write(CefUiIntent::ChatSubmit { message });
        }
        UiControlPayload::MenuCommand { command } => {
            intents.write(CefUiIntent::MenuCommand { command });
        }
        UiControlPayload::SettingsChanged { key, value_json } => {
            intents.write(CefUiIntent::SettingsChanged { key, value_json });
        }
        UiControlPayload::Ready
        | UiControlPayload::RouteChanged { .. }
        | UiControlPayload::HitRegionsChanged { .. }
        | UiControlPayload::TextEntryChanged { .. }
        | UiControlPayload::HostCommand { .. }
        | UiControlPayload::HostCommandResult { .. }
        | UiControlPayload::HostEvent { .. }
        | UiControlPayload::Lifecycle { .. } => {}
    }
}

fn drain_validated_envelope(
    envelope: UiEnvelope,
    status: &mut CefUiStatus,
    authority: &GameUiProtocolValidationContext,
    writers: &mut CefUiIncomingWriters,
) -> Result<(), CefUiError> {
    let sequence = envelope.sequence;
    let request_id = envelope.request_id;
    let kind = envelope.kind;
    match envelope.payload {
        UiEnvelopePayload::Control { payload } if kind == UiEnvelopeKind::Request => {
            if let Some(request_id) = request_id {
                match payload {
                    UiControlPayload::HostCommand {
                        request:
                            CefHostCommandRequest {
                                command_id,
                                payload,
                                ..
                            },
                    } => {
                        writers.host_commands.write(FunHostCommandRequest::new(
                            request_id.0,
                            String::from(command_id.as_str()),
                            payload,
                        ));
                    }
                    payload => {
                        let Some(request) =
                            game_ui_request_from_control(request_id, sequence, &payload)
                                .map_err(CefUiError::RejectedUiRequest)?
                        else {
                            return Ok(());
                        };
                        validate_game_ui_request(&request, authority)
                            .map_err(CefUiError::RejectedUiRequest)?;
                        writers.requests.write(CefUiRequest { envelope: request });
                        write_validated_intent(payload, &mut writers.intents);
                    }
                }
            }
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::Ready,
        } => {
            mark_browser_loaded(status);
            writers.intents.write(CefUiIntent::Ready);
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::RouteChanged { route },
        } => {
            writers.routes.write(CefUiRouteChanged {
                route: CefUiRoute::from(route),
            });
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::HitRegionsChanged { mode, regions },
        } => {
            writers
                .hit_regions
                .write(CefUiHitRegionsChanged { mode, regions });
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::TextEntryChanged { active },
        } => {
            writers.text_entry.write(CefUiTextEntryChanged { active });
        }
        UiEnvelopePayload::Control {
            payload:
                UiControlPayload::ChatSubmit { .. }
                | UiControlPayload::MenuCommand { .. }
                | UiControlPayload::SettingsChanged { .. }
                | UiControlPayload::HostCommand { .. }
                | UiControlPayload::HostCommandResult { .. }
                | UiControlPayload::HostEvent { .. },
        } => {
            return Err(CefUiError::RejectedUiRequest(
                GameUiRequestRejectionReason::InvalidPayload,
            ));
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::Lifecycle { state },
        } => {
            if matches!(
                state,
                UiLifecycleState::PageLoaded | UiLifecycleState::PageVisible
            ) {
                mark_browser_loaded(status);
            }
            writers.intents.write(CefUiIntent::Lifecycle { state });
        }
        UiEnvelopePayload::Error { .. }
        | UiEnvelopePayload::Empty
        | UiEnvelopePayload::StatePatch { .. }
        | UiEnvelopePayload::ModelPatchBatch { .. }
        | UiEnvelopePayload::JsonBytes { .. } => {}
    }
    Ok(())
}

fn mark_browser_loaded(status: &mut CefUiStatus) {
    if !status.browser_loaded {
        tracing::info!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            "CEF UI page loaded"
        );
    }
    status.browser_loaded = true;
}

fn apply_cef_ui_route_changes(
    mut status: ResMut<CefUiStatus>,
    mut changes: MessageReader<CefUiRouteChanged>,
) {
    for change in changes.read() {
        status.current_route = change.route;
    }
}

fn apply_cef_ui_hit_region_changes(
    mut input_capture: ResMut<CefUiInputCapture>,
    mut changes: MessageReader<CefUiHitRegionsChanged>,
) {
    for change in changes.read() {
        input_capture.set_hit_regions(change.mode, &change.regions);
    }
}

fn apply_cef_ui_text_entry_changes(
    mut input_capture: ResMut<CefUiInputCapture>,
    mut changes: MessageReader<CefUiTextEntryChanged>,
) {
    for change in changes.read() {
        input_capture.text_entry_active = change.active;
    }
}

fn update_cef_ui_focus_mode(
    status: Res<CefUiStatus>,
    input_capture: Res<CefUiInputCapture>,
    host: Option<Res<FunClientHostState>>,
    mut focus_mode: ResMut<CefUiFocusMode>,
) {
    *focus_mode = focus_mode_for_owner(status.as_ref(), input_capture.as_ref(), host.as_deref());
}

fn focus_mode_for_owner(
    status: &CefUiStatus,
    input_capture: &CefUiInputCapture,
    host: Option<&FunClientHostState>,
) -> CefUiFocusMode {
    if input_capture.text_entry_active {
        return CefUiFocusMode::TextEntry;
    }
    match host.map(|host| host.state.input_owner) {
        Some(FunInputOwner::LauncherUi | FunInputOwner::EditorUi | FunInputOwner::GameMenuUi) => {
            CefUiFocusMode::UiModal
        }
        Some(FunInputOwner::TextEntry | FunInputOwner::Commandbar) => CefUiFocusMode::TextEntry,
        Some(FunInputOwner::Gameplay) | None => {
            if status.current_route.modal_reason().is_some()
                || matches!(
                    input_capture.hit_region_mode,
                    BrowserUiHitRegionMode::UiModal
                )
            {
                CefUiFocusMode::UiModal
            } else if status.compositor_visible {
                CefUiFocusMode::HudPassive
            } else {
                CefUiFocusMode::Gameplay
            }
        }
    }
}

fn update_cef_ui_pointer_state(
    windows: Query<&Window, With<PrimaryWindow>>,
    input_capture: Res<CefUiInputCapture>,
    mut pointer_state: ResMut<CefUiPointerState>,
) {
    let position = windows
        .single()
        .ok()
        .and_then(Window::cursor_position)
        .map(|position| CefUiPointerPosition {
            x: position.x.max(0.0).min(i32::MAX as f32) as i32,
            y: position.y.max(0.0).min(i32::MAX as f32) as i32,
        });
    let active_hit_region = position.and_then(|point| input_capture.hit_region_at(point));
    if pointer_state.position != position || pointer_state.active_hit_region != active_hit_region {
        pointer_state.position = position;
        pointer_state.active_hit_region = active_hit_region;
    }
}

fn update_cef_ui_input_capture(
    focus_mode: Res<CefUiFocusMode>,
    status: Res<CefUiStatus>,
    pointer_state: Res<CefUiPointerState>,
    mut input_capture: ResMut<CefUiInputCapture>,
) {
    input_capture.modal_reason = status.current_route.modal_reason();
    input_capture.active_hit_region = pointer_state.active_hit_region;
    match *focus_mode {
        CefUiFocusMode::Gameplay => {
            input_capture.capture_keyboard = false;
            input_capture.capture_pointer = false;
        }
        CefUiFocusMode::HudPassive => {
            input_capture.capture_keyboard = false;
            input_capture.capture_pointer = pointer_state.active_hit_region.is_some();
        }
        CefUiFocusMode::UiModal => {
            input_capture.capture_keyboard = true;
            input_capture.capture_pointer = true;
        }
        CefUiFocusMode::TextEntry => {
            input_capture.capture_keyboard = true;
            input_capture.capture_pointer = pointer_state.active_hit_region.is_some();
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "CEF OSR input forwarding consumes the window, pointer, mouse, wheel and keyboard lanes together"
)]
fn forward_bevy_input_to_cef(
    browser_control: Option<NonSend<CefUiBrowserControl>>,
    primary_window: Query<Entity, With<PrimaryWindow>>,
    focus_mode: Res<CefUiFocusMode>,
    input_capture: Res<CefUiInputCapture>,
    pointer_state: Res<CefUiPointerState>,
    keys: Res<ButtonInput<KeyCode>>,
    mut cursor_moved: MessageReader<CursorMoved>,
    mut mouse_buttons: MessageReader<MouseButtonInput>,
    mut mouse_wheel: MessageReader<MouseWheel>,
    mut keyboard_inputs: MessageReader<KeyboardInput>,
) {
    let Some(browser_control) = browser_control else {
        return;
    };
    if !browser_control.is_ready() {
        return;
    }
    let Ok(primary_window_entity) = primary_window.single() else {
        return;
    };

    let forward_pointer = should_forward_pointer_to_cef(*focus_mode, input_capture.as_ref());
    let forward_keyboard = should_forward_keyboard_to_cef(*focus_mode, input_capture.as_ref());

    browser_control
        .browser
        .set_focus(forward_keyboard || forward_pointer);

    if forward_pointer {
        for event in cursor_moved.read() {
            if event.window != primary_window_entity {
                continue;
            }
            let position = cef_pointer_position_from_vec2(event.position);
            browser_control.browser.send_mouse_move(
                CefBrowserMouseEvent {
                    x: position.x,
                    y: position.y,
                    modifiers: cef_event_modifiers(&keys),
                },
                false,
            );
        }

        let pointer_position = pointer_state
            .position
            .unwrap_or(CefUiPointerPosition { x: 0, y: 0 });
        for event in mouse_buttons.read() {
            if event.window != primary_window_entity {
                continue;
            }
            let Some(button) = cef_mouse_button(event.button) else {
                continue;
            };
            browser_control.browser.send_mouse_click(
                CefBrowserMouseEvent {
                    x: pointer_position.x,
                    y: pointer_position.y,
                    modifiers: cef_event_modifiers(&keys),
                },
                button,
                matches!(event.state, ButtonState::Released),
                1,
            );
        }

        for event in mouse_wheel.read() {
            if event.window != primary_window_entity {
                continue;
            }
            let scale = match event.unit {
                MouseScrollUnit::Line => MouseScrollUnit::SCROLL_UNIT_CONVERSION_FACTOR,
                MouseScrollUnit::Pixel => 1.0,
            };
            browser_control.browser.send_mouse_wheel(
                CefBrowserMouseEvent {
                    x: pointer_position.x,
                    y: pointer_position.y,
                    modifiers: cef_event_modifiers(&keys),
                },
                scaled_cef_wheel_delta(event.x, scale),
                scaled_cef_wheel_delta(event.y, scale),
            );
        }
    } else {
        cursor_moved.clear();
        mouse_buttons.clear();
        mouse_wheel.clear();
    }

    if forward_keyboard {
        for event in keyboard_inputs.read() {
            if event.window != primary_window_entity {
                continue;
            }
            let Some(windows_key_code) = windows_virtual_key_code(event.key_code) else {
                continue;
            };
            let modifiers = cef_event_modifiers(&keys);
            match event.state {
                ButtonState::Pressed => {
                    browser_control.browser.send_key_event(CefBrowserKeyEvent {
                        kind: CefBrowserKeyEventKind::RawKeyDown,
                        modifiers,
                        windows_key_code,
                        native_key_code: windows_key_code,
                        character: 0,
                        unmodified_character: 0,
                        is_system_key: false,
                    });
                    if let Some(text) = event.text.as_deref() {
                        for character in text.encode_utf16() {
                            browser_control.browser.send_key_event(CefBrowserKeyEvent {
                                kind: CefBrowserKeyEventKind::Char,
                                modifiers,
                                windows_key_code,
                                native_key_code: windows_key_code,
                                character,
                                unmodified_character: character,
                                is_system_key: false,
                            });
                        }
                    }
                }
                ButtonState::Released => {
                    browser_control.browser.send_key_event(CefBrowserKeyEvent {
                        kind: CefBrowserKeyEventKind::KeyUp,
                        modifiers,
                        windows_key_code,
                        native_key_code: windows_key_code,
                        character: 0,
                        unmodified_character: 0,
                        is_system_key: false,
                    });
                }
            }
        }
    } else {
        keyboard_inputs.clear();
    }
}

fn should_forward_pointer_to_cef(
    focus_mode: CefUiFocusMode,
    input_capture: &CefUiInputCapture,
) -> bool {
    input_capture.capture_pointer
        || matches!(
            focus_mode,
            CefUiFocusMode::HudPassive | CefUiFocusMode::UiModal | CefUiFocusMode::TextEntry
        )
}

fn should_forward_keyboard_to_cef(
    focus_mode: CefUiFocusMode,
    input_capture: &CefUiInputCapture,
) -> bool {
    input_capture.capture_keyboard
        || matches!(
            focus_mode,
            CefUiFocusMode::UiModal | CefUiFocusMode::TextEntry
        )
}

fn cef_pointer_position_from_vec2(position: Vec2) -> CefUiPointerPosition {
    CefUiPointerPosition {
        x: position.x.max(0.0).min(i32::MAX as f32) as i32,
        y: position.y.max(0.0).min(i32::MAX as f32) as i32,
    }
}

fn scaled_cef_wheel_delta(value: f32, scale: f32) -> i32 {
    (value * scale)
        .round()
        .clamp(i32::MIN as f32, i32::MAX as f32) as i32
}

fn cef_mouse_button(button: MouseButton) -> Option<CefBrowserMouseButton> {
    match button {
        MouseButton::Left => Some(CefBrowserMouseButton::Left),
        MouseButton::Right => Some(CefBrowserMouseButton::Right),
        MouseButton::Middle => Some(CefBrowserMouseButton::Middle),
        MouseButton::Back | MouseButton::Forward | MouseButton::Other(_) => None,
    }
}

fn cef_event_modifiers(keys: &ButtonInput<KeyCode>) -> u32 {
    let mut modifiers = 0;
    if keys.any_pressed([KeyCode::ShiftLeft, KeyCode::ShiftRight]) {
        modifiers |= 1 << 1;
    }
    if keys.any_pressed([KeyCode::ControlLeft, KeyCode::ControlRight]) {
        modifiers |= 1 << 2;
    }
    if keys.any_pressed([KeyCode::AltLeft, KeyCode::AltRight]) {
        modifiers |= 1 << 3;
    }
    if keys.any_pressed([KeyCode::SuperLeft, KeyCode::SuperRight]) {
        modifiers |= 1 << 7;
    }
    modifiers
}

fn windows_virtual_key_code(key_code: KeyCode) -> Option<i32> {
    match key_code {
        KeyCode::Backspace => Some(0x08),
        KeyCode::Tab => Some(0x09),
        KeyCode::Enter | KeyCode::NumpadEnter => Some(0x0d),
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Some(0x10),
        KeyCode::ControlLeft | KeyCode::ControlRight => Some(0x11),
        KeyCode::AltLeft | KeyCode::AltRight => Some(0x12),
        KeyCode::Pause => Some(0x13),
        KeyCode::CapsLock => Some(0x14),
        KeyCode::Escape => Some(0x1b),
        KeyCode::Space => Some(0x20),
        KeyCode::PageUp => Some(0x21),
        KeyCode::PageDown => Some(0x22),
        KeyCode::End => Some(0x23),
        KeyCode::Home => Some(0x24),
        KeyCode::ArrowLeft => Some(0x25),
        KeyCode::ArrowUp => Some(0x26),
        KeyCode::ArrowRight => Some(0x27),
        KeyCode::ArrowDown => Some(0x28),
        KeyCode::Insert => Some(0x2d),
        KeyCode::Delete => Some(0x2e),
        KeyCode::Digit0 | KeyCode::Numpad0 => Some(0x30),
        KeyCode::Digit1 | KeyCode::Numpad1 => Some(0x31),
        KeyCode::Digit2 | KeyCode::Numpad2 => Some(0x32),
        KeyCode::Digit3 | KeyCode::Numpad3 => Some(0x33),
        KeyCode::Digit4 | KeyCode::Numpad4 => Some(0x34),
        KeyCode::Digit5 | KeyCode::Numpad5 => Some(0x35),
        KeyCode::Digit6 | KeyCode::Numpad6 => Some(0x36),
        KeyCode::Digit7 | KeyCode::Numpad7 => Some(0x37),
        KeyCode::Digit8 | KeyCode::Numpad8 => Some(0x38),
        KeyCode::Digit9 | KeyCode::Numpad9 => Some(0x39),
        KeyCode::KeyA => Some(0x41),
        KeyCode::KeyB => Some(0x42),
        KeyCode::KeyC => Some(0x43),
        KeyCode::KeyD => Some(0x44),
        KeyCode::KeyE => Some(0x45),
        KeyCode::KeyF => Some(0x46),
        KeyCode::KeyG => Some(0x47),
        KeyCode::KeyH => Some(0x48),
        KeyCode::KeyI => Some(0x49),
        KeyCode::KeyJ => Some(0x4a),
        KeyCode::KeyK => Some(0x4b),
        KeyCode::KeyL => Some(0x4c),
        KeyCode::KeyM => Some(0x4d),
        KeyCode::KeyN => Some(0x4e),
        KeyCode::KeyO => Some(0x4f),
        KeyCode::KeyP => Some(0x50),
        KeyCode::KeyQ => Some(0x51),
        KeyCode::KeyR => Some(0x52),
        KeyCode::KeyS => Some(0x53),
        KeyCode::KeyT => Some(0x54),
        KeyCode::KeyU => Some(0x55),
        KeyCode::KeyV => Some(0x56),
        KeyCode::KeyW => Some(0x57),
        KeyCode::KeyX => Some(0x58),
        KeyCode::KeyY => Some(0x59),
        KeyCode::KeyZ => Some(0x5a),
        KeyCode::SuperLeft | KeyCode::SuperRight => Some(0x5b),
        KeyCode::NumpadMultiply => Some(0x6a),
        KeyCode::NumpadAdd => Some(0x6b),
        KeyCode::NumpadSubtract => Some(0x6d),
        KeyCode::NumpadDecimal => Some(0x6e),
        KeyCode::NumpadDivide => Some(0x6f),
        KeyCode::F1 => Some(0x70),
        KeyCode::F2 => Some(0x71),
        KeyCode::F3 => Some(0x72),
        KeyCode::F4 => Some(0x73),
        KeyCode::F5 => Some(0x74),
        KeyCode::F6 => Some(0x75),
        KeyCode::F7 => Some(0x76),
        KeyCode::F8 => Some(0x77),
        KeyCode::F9 => Some(0x78),
        KeyCode::F10 => Some(0x79),
        KeyCode::F11 => Some(0x7a),
        KeyCode::F12 => Some(0x7b),
        KeyCode::NumLock => Some(0x90),
        KeyCode::ScrollLock => Some(0x91),
        KeyCode::Semicolon => Some(0xba),
        KeyCode::Equal => Some(0xbb),
        KeyCode::Comma => Some(0xbc),
        KeyCode::Minus => Some(0xbd),
        KeyCode::Period => Some(0xbe),
        KeyCode::Slash => Some(0xbf),
        KeyCode::Backquote => Some(0xc0),
        KeyCode::BracketLeft => Some(0xdb),
        KeyCode::Backslash | KeyCode::IntlBackslash => Some(0xdc),
        KeyCode::BracketRight => Some(0xdd),
        KeyCode::Quote => Some(0xde),
        _ => None,
    }
}

fn update_gameplay_input_gate(
    focus_mode: Res<CefUiFocusMode>,
    input_capture: Res<CefUiInputCapture>,
    mut gate: ResMut<GameplayInputGate>,
) {
    *gate = match *focus_mode {
        CefUiFocusMode::Gameplay => GameplayInputGate::default(),
        CefUiFocusMode::HudPassive if input_capture.capture_pointer => GameplayInputGate {
            block_movement: false,
            block_fire: true,
            block_actions: true,
            block_look: true,
            keyboard_owned_by_ui: false,
            pointer_owned_by_ui: true,
            reason: GameplayInputBlockReason::UiPointerRegion,
        },
        CefUiFocusMode::HudPassive => GameplayInputGate::default(),
        CefUiFocusMode::UiModal => GameplayInputGate {
            block_movement: true,
            block_fire: true,
            block_actions: true,
            block_look: true,
            keyboard_owned_by_ui: true,
            pointer_owned_by_ui: true,
            reason: GameplayInputBlockReason::UiModal,
        },
        CefUiFocusMode::TextEntry => GameplayInputGate {
            block_movement: true,
            block_fire: true,
            block_actions: true,
            block_look: true,
            keyboard_owned_by_ui: true,
            pointer_owned_by_ui: input_capture.capture_pointer,
            reason: GameplayInputBlockReason::TextEntry,
        },
    };
}

fn update_cef_ui_overlay_click_through(
    focus_mode: Res<CefUiFocusMode>,
    input_capture: Res<CefUiInputCapture>,
    mut overlay_state: ResMut<CefUiOverlayClickThroughState>,
    mut stats: ResMut<CefUiFrameStats>,
) {
    let desired_click_through = match *focus_mode {
        CefUiFocusMode::Gameplay | CefUiFocusMode::HudPassive | CefUiFocusMode::TextEntry => {
            !input_capture.capture_pointer
        }
        CefUiFocusMode::UiModal => false,
    };
    if overlay_state.click_through != desired_click_through {
        overlay_state.click_through = desired_click_through;
        overlay_state.change_count = overlay_state.change_count.saturating_add(1);
        stats.overlay_click_through_change_count =
            stats.overlay_click_through_change_count.saturating_add(1);
        tracing::debug!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            click_through = desired_click_through,
            focus_mode = ?*focus_mode,
            active_hit_region = ?input_capture.active_hit_region,
            "CEF UI input passthrough state changed"
        );
    }
}

fn collect_gameplay_ui_model(
    status: Res<CefUiStatus>,
    focus_mode: Res<CefUiFocusMode>,
    overlay_state: Res<CefUiOverlayClickThroughState>,
    world_status: Res<RenderWorldStatus>,
    world_context: Res<RenderWorldContext>,
    mut cache: ResMut<CefUiModelCache>,
    mut bridge: ResMut<CefUiBridge>,
) {
    let model = CefUiRuntimeModel::from_resources(
        &status,
        *focus_mode,
        &overlay_state,
        &world_status,
        &world_context,
    );
    match cache.previous {
        Some(previous) => model.write_patch(&previous, bridge.patch_writer()),
        None => model.write_initial_snapshot(bridge.patch_writer()),
    }
    cache.previous = Some(model);
}

fn push_fun_host_state_events(
    host: Res<FunClientHostState>,
    mut cache: ResMut<CefUiHostStateEventCache>,
    mut bridge: ResMut<CefUiBridge>,
    mut status: ResMut<CefUiStatus>,
) {
    let sequence = host.sequence();
    if sequence == cache.last_sequence {
        return;
    }
    cache.last_sequence = sequence;
    let payload = match host.snapshot_payload_json() {
        Ok(payload) => payload,
        Err(error) => {
            tracing::warn!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                ?error,
                "failed to encode FunHostState event"
            );
            return;
        }
    };
    let envelope = UiEnvelope::control(
        UiEnvelopeKind::Event,
        None,
        bridge.next_sequence(),
        UiControlPayload::HostEvent {
            event: String::from("host.state.patch"),
            payload,
        },
    );
    if let Err(error) = bridge.push_host_envelope(envelope) {
        status.last_error = Some(CefUiError::from(error));
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy system parameters expose each CEF bridge lane directly to scheduler ordering"
)]
fn write_cef_ui_outgoing_messages(
    mut bridge: ResMut<CefUiBridge>,
    mut status: ResMut<CefUiStatus>,
    mut patches: MessageReader<CefUiPatch>,
    mut commands: MessageReader<CefUiCommand>,
    mut route_sets: MessageReader<CefUiRouteSet>,
    mut modal_sets: MessageReader<CefUiModalSet>,
    mut diagnostics: MessageReader<CefUiDiagnosticPush>,
    mut host_responses: MessageReader<FunHostCommandResponse>,
) {
    for patch in patches.read() {
        if let Err(error) = bridge.push_patch(patch.channel, patch.field, patch.value.clone()) {
            status.last_error = Some(CefUiError::PatchWrite(error));
        }
    }
    for route_set in route_sets.read() {
        status.current_route = route_set.route;
        let route = route_set.route.as_wire_str();
        if let Err(error) =
            bridge
                .patch_writer()
                .set_text(GameUiChannel::Settings, GameUiFieldKey::Route, route)
        {
            status.last_error = Some(CefUiError::PatchWrite(error));
        }
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Event,
            None,
            bridge.next_sequence(),
            UiControlPayload::RouteChanged {
                route: route_set.route.browser_route(),
            },
        );
        if let Err(error) = bridge.push_host_envelope(envelope) {
            status.last_error = Some(CefUiError::from(error));
        }
    }
    for modal_set in modal_sets.read() {
        let reason = modal_set
            .reason
            .map_or("none", CefUiModalReason::as_wire_str);
        if let Err(error) = bridge.patch_writer().set_text(
            GameUiChannel::Settings,
            GameUiFieldKey::ModalReason,
            reason,
        ) {
            status.last_error = Some(CefUiError::PatchWrite(error));
        }
    }
    for command in commands.read() {
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Event,
            None,
            bridge.next_sequence(),
            UiControlPayload::MenuCommand {
                command: command.command,
            },
        );
        if let Err(error) = bridge.push_host_envelope(envelope) {
            status.last_error = Some(CefUiError::from(error));
        }
    }
    for diagnostic in diagnostics.read() {
        if let Err(error) = write_diagnostic_patch(bridge.patch_writer(), diagnostic) {
            status.last_error = Some(CefUiError::PatchWrite(error));
        }
    }
    for response in host_responses.read() {
        let envelope_kind = match response.status {
            FunHostCommandStatus::Ok => UiEnvelopeKind::Response,
            FunHostCommandStatus::Error => UiEnvelopeKind::Error,
        };
        let payload = UiControlPayload::HostCommandResult {
            command_id: HostCommandId::new(response.command_id.clone()),
            response: cef_host_response_from_fun_host(response),
        };
        let envelope = UiEnvelope::control(
            envelope_kind,
            Some(BrowserUiRequestId(response.request_id)),
            bridge.next_sequence(),
            payload,
        );
        if let Err(error) = bridge.push_host_envelope(envelope) {
            status.last_error = Some(CefUiError::from(error));
        }
    }
}

fn cef_host_response_from_fun_host(response: &FunHostCommandResponse) -> CefHostCommandResponse {
    match response.status {
        FunHostCommandStatus::Ok => CefHostCommandResponse::Ok {
            payload: response.payload_json.clone(),
            diagnostics: Vec::new(),
        },
        FunHostCommandStatus::Error => {
            let Some(error_code) = response.error_code else {
                return CefHostCommandResponse::Failed {
                    error: HostCommandError {
                        code: String::from("host.unknown_error"),
                        message: String::from(
                            "Fun host command failed without a typed error code.",
                        ),
                    },
                    diagnostics: Vec::new(),
                };
            };
            match error_code {
                fun_host::FunHostCommandErrorCode::UnknownCommand => {
                    CefHostCommandResponse::Rejected {
                        reason: HostCommandRejection::UnknownCommand,
                    }
                }
                fun_host::FunHostCommandErrorCode::OversizePayload => {
                    CefHostCommandResponse::Rejected {
                        reason: HostCommandRejection::OversizePayload,
                    }
                }
                fun_host::FunHostCommandErrorCode::InvalidPayload => {
                    CefHostCommandResponse::Rejected {
                        reason: HostCommandRejection::InvalidPayload,
                    }
                }
                fun_host::FunHostCommandErrorCode::ProjectUnauthorized => {
                    CefHostCommandResponse::Rejected {
                        reason: HostCommandRejection::MissingCapability,
                    }
                }
                fun_host::FunHostCommandErrorCode::HostShuttingDown => {
                    CefHostCommandResponse::Rejected {
                        reason: HostCommandRejection::HostShuttingDown,
                    }
                }
                fun_host::FunHostCommandErrorCode::BackendSessionRequired
                | fun_host::FunHostCommandErrorCode::ReturnToGameUnavailable
                | fun_host::FunHostCommandErrorCode::ResponseEncodingFailed => {
                    CefHostCommandResponse::Failed {
                        error: HostCommandError {
                            code: String::from(error_code.as_wire_str()),
                            message: format!(
                                "Fun host command failed: {}",
                                error_code.as_wire_str()
                            ),
                        },
                        diagnostics: host_diagnostics_from_payload(&response.payload_json),
                    }
                }
            }
        }
    }
}

fn host_diagnostics_from_payload(payload_json: &[u8]) -> Vec<HostDiagnostic> {
    if payload_json.is_empty() {
        return Vec::new();
    }
    vec![HostDiagnostic {
        code: String::from("host.command.failed"),
        level: String::from("warning"),
        message: String::from_utf8_lossy(payload_json)
            .chars()
            .take(512)
            .collect(),
    }]
}

fn send_cef_ui_patch_batch(
    mut bridge: ResMut<CefUiBridge>,
    mut status: ResMut<CefUiStatus>,
    mut stats: ResMut<CefUiFrameStats>,
) {
    bridge.finish_patch_writer();
    let Some(batch) = bridge.pop_patch_batch() else {
        return;
    };
    let envelope = UiEnvelope::model_patch_batch(bridge.next_sequence(), batch);
    match bridge.push_host_envelope(envelope) {
        Ok(()) => {
            stats.patch_batch_count = stats.patch_batch_count.saturating_add(1);
        }
        Err(error) => {
            status.last_error = Some(CefUiError::from(error));
        }
    }
}

fn flush_cef_ui_host_envelopes_to_browser(
    browser_control: Option<NonSend<CefUiBrowserControl>>,
    mut stats: ResMut<CefUiFrameStats>,
) {
    let Some(browser_control) = browser_control else {
        return;
    };
    let flushed = browser_control.flush_host_envelopes_to_js();
    stats.js_message_count = stats.js_message_count.saturating_add(flushed as u64);
}

const CEF_UI_TEXTURE_BYTES_PER_PIXEL: usize = 4;
const CEF_UI_TEXTURE_Z_INDEX: i32 = 900_000;

fn cef_ui_render_interval() -> Duration {
    Duration::from_nanos(1_000_000_000 / CEF_UI_RENDER_RATE_HZ)
}

fn cef_ui_gpu_bridge_startup_timeout_from_env() -> Duration {
    std::env::var("FUN_CEF_UI_GPU_BRIDGE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|millis| *millis > 0)
        .map(Duration::from_millis)
        .unwrap_or(CEF_UI_GPU_BRIDGE_STARTUP_TIMEOUT)
}

fn pump_cef_ui_message_loop(time: Res<Time>, mut pump: ResMut<CefUiMessageLoopPump>) {
    if !pump.enabled {
        return;
    }
    if pump.pump_timer.tick(time.delta()).just_finished() {
        if !pump.active_logged {
            tracing::info!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                render_rate_hz = CEF_UI_RENDER_RATE_HZ,
                "CEF UI external message loop pump active"
            );
            pump.active_logged = true;
        }
        fun_ui_cef::pump_cef_message_loop_work();
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy UI systems need independent access to host, diagnostics, and spawned UI nodes"
)]
fn update_fun_client_fps_counter(
    mut commands: Commands,
    time: Res<Time>,
    host: Option<Res<FunClientHostState>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    diagnostics: Res<DiagnosticsStore>,
    mut counter: ResMut<FunClientFpsCounterState>,
    mut roots: Query<(&mut Node, &mut Visibility), With<FunClientFpsCounterRoot>>,
    mut texts: Query<&mut Text, With<FunClientFpsCounterText>>,
) {
    let host = host.as_deref();
    let mode = host.map_or(FunHostMode::Game, |host| host.state.mode);
    let preview_rect = host.and_then(|host| host.state.game.viewport_layout.rect);
    let window_size = windows.single().ok().map(window_logical_size);
    let layout = fun_client_fps_counter_layout(mode, preview_rect, window_size);

    let Some(layout) = layout else {
        for (_, mut visibility) in &mut roots {
            *visibility = Visibility::Hidden;
        }
        return;
    };

    counter.update_timer.tick(time.delta());
    if counter.update_timer.just_finished() || counter.last_label == "FPS --" {
        counter.last_label = fun_client_fps_label(&diagnostics);
    }

    let mut positioned_root = false;
    for (mut node, mut visibility) in &mut roots {
        if positioned_root {
            *visibility = Visibility::Hidden;
            continue;
        }
        node.left = px(layout.left);
        node.top = px(layout.top);
        *visibility = Visibility::Visible;
        positioned_root = true;
    }

    if !positioned_root {
        spawn_fun_client_fps_counter(&mut commands, layout, &counter.last_label);
    }

    for mut text in &mut texts {
        text.0.clone_from(&counter.last_label);
    }
}

fn spawn_fun_client_fps_counter(
    commands: &mut Commands,
    layout: FunClientFpsCounterLayout,
    label: &str,
) {
    commands
        .spawn((
            Name::new("Fun Client FPS Counter"),
            FunClientFpsCounterRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(layout.left),
                top: px(layout.top),
                width: px(FUN_CLIENT_FPS_COUNTER_WIDTH),
                height: px(FUN_CLIENT_FPS_COUNTER_HEIGHT),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba_u8(5, 9, 13, 184)),
            GlobalZIndex(FUN_CLIENT_FPS_COUNTER_Z_INDEX),
        ))
        .with_child((
            Text::new(label),
            TextFont {
                font_size: FontSize::Px(12.0),
                ..default()
            },
            TextColor(Color::srgba_u8(218, 235, 248, 255)),
            FunClientFpsCounterText,
        ));
}

fn window_logical_size(window: &Window) -> UVec2 {
    UVec2::new(
        window.resolution.width().max(0.0).round() as u32,
        window.resolution.height().max(0.0).round() as u32,
    )
}

fn fun_client_fps_label(diagnostics: &DiagnosticsStore) -> String {
    diagnostics
        .get(&FrameTimeDiagnosticsPlugin::FPS)
        .and_then(|diagnostic| diagnostic.smoothed())
        .map_or_else(|| "FPS --".to_owned(), |fps| format!("{fps:>3.0} FPS"))
}

fn fun_client_fps_counter_layout(
    mode: FunHostMode,
    preview_rect: Option<FunViewportRect>,
    window_size: Option<UVec2>,
) -> Option<FunClientFpsCounterLayout> {
    match mode {
        FunHostMode::Game => {
            let window_size = window_size?;
            if window_size.x == 0 || window_size.y == 0 {
                return None;
            }
            Some(FunClientFpsCounterLayout {
                left: fps_counter_axis_position(
                    0.0,
                    window_size.x as f32,
                    FUN_CLIENT_FPS_COUNTER_WIDTH,
                ),
                top: fps_counter_axis_position(
                    0.0,
                    window_size.y as f32,
                    FUN_CLIENT_FPS_COUNTER_HEIGHT,
                ),
            })
        }
        FunHostMode::Editor | FunHostMode::EditorOverlay => {
            let rect = preview_rect?;
            if rect.width == 0 || rect.height == 0 {
                return None;
            }
            let left = fps_counter_axis_position(
                rect.x.max(0) as f32,
                rect.width as f32,
                FUN_CLIENT_FPS_COUNTER_WIDTH,
            );
            let top = fps_counter_axis_position(
                rect.y.max(0) as f32,
                rect.height as f32,
                FUN_CLIENT_FPS_COUNTER_HEIGHT,
            );
            Some(FunClientFpsCounterLayout { left, top })
        }
        FunHostMode::Boot
        | FunHostMode::Launcher
        | FunHostMode::Loading
        | FunHostMode::Shutdown => None,
    }
}

fn fps_counter_axis_position(origin: f32, extent: f32, counter_extent: f32) -> f32 {
    if extent > counter_extent + FUN_CLIENT_FPS_COUNTER_MARGIN.mul_add(2.0, 0.0) {
        origin + FUN_CLIENT_FPS_COUNTER_MARGIN
    } else {
        origin
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy system parameters expose each texture upload resource directly to scheduler ordering"
)]
fn upload_cef_ui_frame_to_fun_texture(
    mut commands: Commands,
    render_compositor: Option<Res<CefUiRenderCompositor>>,
    dx12_slot: Option<Res<SharedDx12CefInteropSlot>>,
    time: Res<Time>,
    mut render_texture: ResMut<CefUiRenderTexture>,
    mut texture_uploads: ResMut<CefUiTextureUploads>,
    mut images: ResMut<Assets<Image>>,
    mut image_nodes: Query<&mut ImageNode, With<CefUiRenderTextureRoot>>,
    mut status: ResMut<CefUiStatus>,
    mut stats: ResMut<CefUiFrameStats>,
) {
    if !render_texture
        .upload_timer
        .tick(time.delta())
        .just_finished()
    {
        return;
    }

    #[cfg(not(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint")))]
    let _ = dx12_slot;

    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    if let Some(token) = latest_cef_gpu_frame_token(dx12_slot.as_deref()) {
        if render_texture.last_generation == Some(token.generation) {
            return;
        }
        let size = UVec2::new(token.width, token.height);
        if size.x == 0 || size.y == 0 || token.format != windows_dxgi_bgra8_unorm() {
            return;
        }
        let image_handle = if render_texture.image.is_none() || render_texture.size != Some(size) {
            let image = new_cef_ui_texture_image_uninit(size);
            let image_handle = images.add(image);
            render_texture.image = Some(image_handle.clone());
            render_texture.size = Some(size);
            tracing::info!(
                target: FUN_UI_DIAGNOSTICS_TARGET,
                width = size.x,
                height = size.y,
                render_rate_hz = CEF_UI_RENDER_RATE_HZ,
                "CEF UI texture attached to Fun render"
            );
            sync_cef_ui_image_node(
                &mut commands,
                &mut render_texture,
                &mut image_nodes,
                image_handle.clone(),
            );
            image_handle
        } else {
            let Some(image_handle) = render_texture.image.as_ref() else {
                return;
            };
            image_handle.clone()
        };
        texture_uploads.latest = None;
        texture_uploads.latest_gpu = Some(CefUiGpuTextureUpload {
            image: image_handle,
            token,
        });
        render_texture.last_generation = Some(token.generation);
        status.browser_loaded = true;
        status.compositor_visible = true;
        status.last_frame_generation = Some(token.generation);
        stats.paint_count = stats.paint_count.saturating_add(1);
        record_cef_dirty_rect_metadata(&mut stats, token.dirty_rect_metadata);
        stats.cef_published_generation = token.generation.0;
        stats.cef_sampled_generation = token.generation.0;
        return;
    }

    let Some(render_compositor) = render_compositor else {
        return;
    };
    let Some(frame) = render_compositor
        .compositor()
        .with_compositor(|compositor| compositor.consume_ready().cloned())
        .flatten()
    else {
        return;
    };
    if frame.element != CefPaintElement::View
        || render_texture.last_generation == Some(frame.metadata.generation)
    {
        return;
    }
    let Some(size) = cef_ui_frame_texture_size(&frame) else {
        return;
    };
    let Some(expected_byte_len) = cef_ui_texture_byte_len(size) else {
        return;
    };
    if frame.pixels().len() != expected_byte_len {
        return;
    }
    let requires_generation_resync = cef_ui_frame_requires_full_texture_upload(
        render_texture.last_generation,
        frame.metadata.generation,
    );
    if requires_generation_resync && let Some(previous_generation) = render_texture.last_generation
    {
        tracing::debug!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            previous_generation = previous_generation.0,
            next_generation = frame.metadata.generation.0,
            "CEF UI full texture resync after skipped paint generation"
        );
    }
    let force_full_upload = requires_generation_resync
        || frame
            .metadata
            .dirty_rects
            .iter()
            .any(|rect| cef_dirty_rect_bounds(size, *rect).is_none());

    let (image_handle, uploaded_bytes) = if render_texture.image.is_none()
        || render_texture.size != Some(size)
    {
        let image = new_cef_ui_texture_image(size, frame.pixels().to_vec());
        let image_handle = images.add(image);
        let upload_handle = image_handle.clone();
        render_texture.image = Some(image_handle.clone());
        render_texture.size = Some(size);
        tracing::info!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            width = size.x,
            height = size.y,
            render_rate_hz = CEF_UI_RENDER_RATE_HZ,
            "CEF UI texture attached to Fun render"
        );
        sync_cef_ui_image_node(
            &mut commands,
            &mut render_texture,
            &mut image_nodes,
            image_handle,
        );
        (upload_handle, expected_byte_len)
    } else {
        let Some(image_handle) = render_texture.image.as_ref() else {
            return;
        };
        let uploaded_bytes =
            cef_ui_texture_upload_byte_count(size, &frame.metadata.dirty_rects, force_full_upload)
                .unwrap_or(0);
        (image_handle.clone(), uploaded_bytes)
    };
    texture_uploads.latest = Some(CefUiTextureUpload {
        image: image_handle,
        generation: frame.metadata.generation,
        size,
        pixels: frame.pixels().to_vec(),
        dirty_rects: frame.metadata.dirty_rects.clone(),
        force_full_upload,
    });
    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    {
        texture_uploads.latest_gpu = None;
    }

    render_texture.last_generation = Some(frame.metadata.generation);
    status.browser_loaded = true;
    status.compositor_visible = true;
    status.last_frame_generation = Some(frame.metadata.generation);
    stats.paint_count = stats.paint_count.saturating_add(1);
    record_cef_dirty_rect_metadata(&mut stats, frame.dirty_rect_metadata);
    stats.uploaded_bytes = stats.uploaded_bytes.saturating_add(uploaded_bytes as u64);
    stats.cef_published_generation = frame.metadata.generation.0;
    stats.cef_sampled_generation = frame.metadata.generation.0;
}

fn upload_cef_ui_texture_to_gpu(
    uploads: Option<Res<CefUiTextureUploads>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    render_queue: Res<RenderQueue>,
    mut upload_state: ResMut<CefUiGpuUploadState>,
) {
    let Some(uploads) = uploads else {
        return;
    };
    let Some(upload) = uploads.latest.as_ref() else {
        return;
    };
    let image_id = upload.image.id();
    if upload_state.image_id == Some(image_id)
        && upload_state.last_generation == Some(upload.generation)
    {
        return;
    }
    let force_full_upload = upload.force_full_upload
        || upload_state.image_id != Some(image_id)
        || cef_ui_frame_requires_full_texture_upload(
            upload_state.last_generation,
            upload.generation,
        );
    let Some(gpu_image) = gpu_images.get(&upload.image) else {
        return;
    };
    let Some(uploaded_bytes) =
        write_cef_ui_upload_to_gpu(&render_queue, gpu_image, upload, force_full_upload)
    else {
        return;
    };
    upload_state.image_id = Some(image_id);
    upload_state.last_generation = Some(upload.generation);
    tracing::trace!(
        target: FUN_UI_DIAGNOSTICS_TARGET,
        generation = upload.generation.0,
        uploaded_bytes,
        force_full_upload,
        dirty_rect_count = upload.dirty_rects.len(),
        "CEF UI texture uploaded to Bevy GPU image"
    );
}

fn copy_latest_cef_gpu_frame_to_bevy_image(
    uploads: Option<Res<CefUiTextureUploads>>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    dx12_slot: Option<Res<SharedDx12CefInteropSlot>>,
    counters: Option<Res<CefUiTransportCountersResource>>,
    mut upload_state: ResMut<CefUiGpuUploadState>,
) {
    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    {
        let Some(uploads) = uploads else {
            return;
        };
        let Some(upload) = uploads.latest_gpu.as_ref() else {
            return;
        };
        let image_id = upload.image.id();
        if upload_state.image_id == Some(image_id)
            && upload_state.last_generation == Some(upload.token.generation)
        {
            return;
        }
        let Some(gpu_image) = gpu_images.get(&upload.image) else {
            return;
        };
        let Some(interop) = dx12_slot.as_ref().and_then(|slot| slot.interop()) else {
            return;
        };
        let target_state_before = if upload_state.image_id == Some(image_id) {
            Dx12CefBevyImageState::PixelShaderResource
        } else {
            Dx12CefBevyImageState::Common
        };
        match interop.copy_ready_frame_to_bevy_image(upload.token, gpu_image, target_state_before) {
            Ok(Some(result)) => {
                upload_state.image_id = Some(image_id);
                upload_state.last_generation = Some(result.generation);
                upload_state.gpu_copy_failure_logged = false;
                upload_state.gpu_copy_failure_count = 0;
                upload_state.stale_gpu_frame_count_for_current_token = 0;
                upload_state.stale_gpu_frame_token = None;
                tracing::trace!(
                    target: FUN_UI_DIAGNOSTICS_TARGET,
                    generation = result.generation.0,
                    width = result.width,
                    height = result.height,
                    fence_value = result.fence_value,
                    target_format = ?result.target_format,
                    "CEF UI GPU frame sampled by Bevy image"
                );
            }
            Ok(None) => {
                let stale_count =
                    record_cef_gpu_frame_stall(&mut upload_state, upload.token.generation);
                if stale_count == MAX_ACCELERATED_PAINT_FAILURES_BEFORE_FALLBACK {
                    if let Some(counters) = counters.as_ref() {
                        counters.counters.record_stale_gpu_frame();
                        counters.counters.record_transport_fallback();
                    }
                    if let Some(slot) = dx12_slot.as_ref() {
                        slot.request_fallback(
                            CefUiPaintTransportFallbackReason::GpuCopyFenceTimeout,
                        );
                    }
                    if !upload_state.gpu_copy_failure_logged {
                        upload_state.gpu_copy_failure_logged = true;
                        tracing::warn!(
                            target: FUN_UI_DIAGNOSTICS_TARGET,
                            generation = upload.token.generation.0,
                            stale_frame_count = stale_count,
                            fallback_after = MAX_ACCELERATED_PAINT_FAILURES_BEFORE_FALLBACK,
                            fallback_reason = CefUiPaintTransportFallbackReason::GpuCopyFenceTimeout.as_wire_str(),
                            "CEF UI GPU frame copy stalled before Bevy image sampling"
                        );
                    }
                }
            }
            Err(error) => {
                if let Some(counters) = counters.as_ref() {
                    counters.counters.record_gpu_copy_failure();
                }
                upload_state.gpu_copy_failure_count =
                    upload_state.gpu_copy_failure_count.saturating_add(1);
                if let Some(slot) = dx12_slot.as_ref() {
                    if upload_state.gpu_copy_failure_count
                        == MAX_ACCELERATED_PAINT_FAILURES_BEFORE_FALLBACK
                    {
                        if let Some(counters) = counters.as_ref() {
                            counters.counters.record_transport_fallback();
                        }
                        slot.request_fallback(dx12_cef_interop_fallback_reason(error));
                    }
                }
                if !upload_state.gpu_copy_failure_logged {
                    upload_state.gpu_copy_failure_logged = true;
                    tracing::warn!(
                        target: FUN_UI_DIAGNOSTICS_TARGET,
                        generation = upload.token.generation.0,
                        failure = error.failure.as_str(),
                        detail = error.detail,
                        hresult = error.hresult,
                        consecutive_failures = upload_state.gpu_copy_failure_count,
                        fallback_after = MAX_ACCELERATED_PAINT_FAILURES_BEFORE_FALLBACK,
                        "CEF UI GPU frame copy to Bevy image failed"
                    );
                }
            }
        }
        return;
    }

    #[cfg(not(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint")))]
    {
        let _ = (uploads, gpu_images, dx12_slot, counters);
        let _ = &mut upload_state;
    }
}

#[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
fn record_cef_gpu_frame_stall(
    upload_state: &mut CefUiGpuUploadState,
    generation: UiSurfaceGeneration,
) -> u32 {
    if upload_state.stale_gpu_frame_token == Some(generation) {
        upload_state.stale_gpu_frame_count_for_current_token = upload_state
            .stale_gpu_frame_count_for_current_token
            .saturating_add(1);
    } else {
        upload_state.stale_gpu_frame_token = Some(generation);
        upload_state.stale_gpu_frame_count_for_current_token = 1;
    }
    upload_state.stale_gpu_frame_count_for_current_token
}

fn sample_cef_ui_transport_counters(
    time: Res<Time>,
    counters: Res<CefUiTransportCountersResource>,
    dx12_slot: Option<Res<SharedDx12CefInteropSlot>>,
    mut sampler: ResMut<CefUiTransportCounterSampler>,
    mut stats: ResMut<CefUiFrameStats>,
) {
    if !sampler.sample_timer.tick(time.delta()).just_finished() {
        return;
    }
    let snapshot = counters.snapshot();
    if let Some(previous) = sampler.last_snapshot {
        stats.cef_on_paint_fps = snapshot
            .cef_on_paint_count
            .saturating_sub(previous.cef_on_paint_count);
        stats.cef_on_accelerated_paint_fps = snapshot
            .cef_on_accelerated_paint_count
            .saturating_sub(previous.cef_on_accelerated_paint_count);
    }
    stats.accelerated_paint_count = stats
        .accelerated_paint_count
        .max(snapshot.cef_on_accelerated_paint_count);
    stats.cef_cpu_upload_bytes = snapshot.cef_cpu_upload_bytes;
    stats.gpu_copy_count = stats.gpu_copy_count.max(snapshot.cef_gpu_copy_count);
    stats.cef_gpu_copy_count = snapshot.cef_gpu_copy_count;
    stats.gpu_copied_bytes = stats.gpu_copied_bytes.max(snapshot.cef_gpu_copy_bytes);
    stats.cef_gpu_copy_bytes = snapshot.cef_gpu_copy_bytes;
    stats.cef_gpu_copy_ns = snapshot.cef_gpu_copy_ns;
    stats.gpu_copy_fail_count = stats
        .gpu_copy_fail_count
        .max(snapshot.cef_gpu_copy_failures);
    stats.cef_gpu_copy_failures = snapshot.cef_gpu_copy_failures;
    stats.cpu_fallback_count = stats
        .cpu_fallback_count
        .max(snapshot.cef_transport_fallback_count);
    stats.cef_transport_fallback_count = snapshot.cef_transport_fallback_count;
    stats.stale_gpu_frame_count = stats
        .stale_gpu_frame_count
        .max(snapshot.cef_stale_gpu_frame_count);
    if snapshot.cef_published_generation != 0 {
        stats.cef_published_generation = snapshot.cef_published_generation;
    }
    #[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
    if let Some(diagnostics) = dx12_slot
        .as_ref()
        .and_then(|slot| slot.interop())
        .map(|interop| interop.diagnostics_snapshot())
    {
        stats.shared_texture_open_fail_count = diagnostics.shared_texture_open_failure_count;
        stats.gpu_copied_bytes = stats.gpu_copied_bytes.max(diagnostics.gpu_copy_bytes);
        stats.gpu_copy_fail_count = stats
            .gpu_copy_fail_count
            .max(diagnostics.gpu_copy_failure_count);
        stats.cpu_fallback_count = stats.cpu_fallback_count.max(diagnostics.fallback_count);
    }
    #[cfg(not(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint")))]
    let _ = dx12_slot;
    sampler.last_snapshot = Some(snapshot);
}

fn write_cef_ui_upload_to_gpu(
    render_queue: &RenderQueue,
    gpu_image: &GpuImage,
    upload: &CefUiTextureUpload,
    force_full_upload: bool,
) -> Option<usize> {
    let expected_byte_len = cef_ui_texture_byte_len(upload.size)?;
    if upload.pixels.len() != expected_byte_len
        || gpu_image.texture_descriptor.size.width != upload.size.x
        || gpu_image.texture_descriptor.size.height != upload.size.y
        || gpu_image.texture_descriptor.format != TextureFormat::Bgra8UnormSrgb
    {
        return None;
    }
    let dirty_rects_are_valid = upload
        .dirty_rects
        .iter()
        .all(|rect| cef_dirty_rect_bounds(upload.size, *rect).is_some());
    if force_full_upload || upload.dirty_rects.is_empty() || !dirty_rects_are_valid {
        write_cef_full_texture_to_gpu(render_queue, gpu_image, upload);
        return Some(expected_byte_len);
    }

    let mut uploaded_bytes = 0usize;
    for rect in &upload.dirty_rects {
        uploaded_bytes = uploaded_bytes.checked_add(write_cef_dirty_rect_to_gpu(
            render_queue,
            gpu_image,
            upload,
            *rect,
        )?)?;
    }
    Some(uploaded_bytes)
}

fn write_cef_full_texture_to_gpu(
    render_queue: &RenderQueue,
    gpu_image: &GpuImage,
    upload: &CefUiTextureUpload,
) {
    render_queue.write_texture(
        gpu_image.texture.as_image_copy(),
        &upload.pixels,
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(upload.size.x * CEF_UI_TEXTURE_BYTES_PER_PIXEL as u32),
            rows_per_image: Some(upload.size.y),
        },
        Extent3d {
            width: upload.size.x,
            height: upload.size.y,
            depth_or_array_layers: 1,
        },
    );
}

fn write_cef_dirty_rect_to_gpu(
    render_queue: &RenderQueue,
    gpu_image: &GpuImage,
    upload: &CefUiTextureUpload,
    rect: CefDirtyRect,
) -> Option<usize> {
    let (x, y, width, height) = cef_dirty_rect_bounds(upload.size, rect)?;
    let offset = cef_dirty_rect_offset_bytes(upload.size, x, y)?;
    let mut texture_copy = gpu_image.texture.as_image_copy();
    texture_copy.origin = Origin3d { x, y, z: 0 };
    render_queue.write_texture(
        texture_copy,
        &upload.pixels,
        TexelCopyBufferLayout {
            offset,
            bytes_per_row: Some(upload.size.x * CEF_UI_TEXTURE_BYTES_PER_PIXEL as u32),
            rows_per_image: Some(upload.size.y),
        },
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    cef_dirty_rect_byte_len(upload.size, rect)
}

fn sync_cef_ui_image_node(
    commands: &mut Commands,
    render_texture: &mut CefUiRenderTexture,
    image_nodes: &mut Query<&mut ImageNode, With<CefUiRenderTextureRoot>>,
    image_handle: Handle<Image>,
) {
    if let Some(root_entity) = render_texture.root_entity {
        if let Ok(mut image_node) = image_nodes.get_mut(root_entity) {
            image_node.image = image_handle;
            image_node.image_mode = NodeImageMode::Stretch;
            return;
        }
        render_texture.root_entity = None;
    }

    let root_entity = commands
        .spawn((
            Name::new("CEF UI Render Texture"),
            CefUiRenderTextureRoot,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                top: px(0),
                width: percent(100),
                height: percent(100),
                ..default()
            },
            ImageNode {
                image: image_handle,
                image_mode: NodeImageMode::Stretch,
                ..default()
            },
            GlobalZIndex(CEF_UI_TEXTURE_Z_INDEX),
        ))
        .id();
    render_texture.root_entity = Some(root_entity);
}

fn new_cef_ui_texture_image(size: UVec2, pixels: Vec<u8>) -> Image {
    Image::new(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        pixels,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

#[cfg(all(target_os = "windows", feature = "cef_ui_dx12_accelerated_paint"))]
fn new_cef_ui_texture_image_uninit(size: UVec2) -> Image {
    Image::new_uninit(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    )
}

fn cef_ui_frame_requires_full_texture_upload(
    last_generation: Option<UiSurfaceGeneration>,
    next_generation: UiSurfaceGeneration,
) -> bool {
    match last_generation {
        None => true,
        Some(last_generation) => next_generation.0 != last_generation.0.saturating_add(1),
    }
}

fn cef_ui_frame_texture_size(frame: &CefUiCompositorFrame) -> Option<UVec2> {
    if frame.metadata.width <= 0 || frame.metadata.height <= 0 {
        return None;
    }
    Some(UVec2::new(
        u32::try_from(frame.metadata.width).ok()?,
        u32::try_from(frame.metadata.height).ok()?,
    ))
}

fn cef_ui_texture_byte_len(size: UVec2) -> Option<usize> {
    usize::try_from(size.x)
        .ok()?
        .checked_mul(usize::try_from(size.y).ok()?)?
        .checked_mul(CEF_UI_TEXTURE_BYTES_PER_PIXEL)
}

fn cef_ui_texture_upload_byte_count(
    size: UVec2,
    dirty_rects: &[CefDirtyRect],
    force_full_upload: bool,
) -> Option<usize> {
    if force_full_upload || dirty_rects.is_empty() {
        return cef_ui_texture_byte_len(size);
    }
    dirty_rects.iter().try_fold(0usize, |uploaded, rect| {
        uploaded.checked_add(cef_dirty_rect_byte_len(size, *rect)?)
    })
}

fn cef_dirty_rect_byte_len(size: UVec2, rect: CefDirtyRect) -> Option<usize> {
    let (_, _, width, height) = cef_dirty_rect_bounds(size, rect)?;
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(CEF_UI_TEXTURE_BYTES_PER_PIXEL)
}

fn cef_dirty_rect_offset_bytes(size: UVec2, x: u32, y: u32) -> Option<u64> {
    if x >= size.x || y >= size.y {
        return None;
    }
    let offset = usize::try_from(y)
        .ok()?
        .checked_mul(usize::try_from(size.x).ok()?)?
        .checked_add(usize::try_from(x).ok()?)?
        .checked_mul(CEF_UI_TEXTURE_BYTES_PER_PIXEL)?;
    u64::try_from(offset).ok()
}

#[cfg(test)]
fn cef_dirty_rect_bytes(src: &[u8], size: UVec2, rect: CefDirtyRect) -> Option<Vec<u8>> {
    let (x, y, width, height) = cef_dirty_rect_bounds(size, rect)?;
    let texture_width = usize::try_from(size.x).ok()?;
    let x = usize::try_from(x).ok()?;
    let y = usize::try_from(y).ok()?;
    let width = usize::try_from(width).ok()?;
    let height = usize::try_from(height).ok()?;
    let row_bytes = width.checked_mul(CEF_UI_TEXTURE_BYTES_PER_PIXEL)?;
    let mut bytes = Vec::with_capacity(row_bytes.checked_mul(height)?);
    for row in y..y.checked_add(height)? {
        let offset = row
            .checked_mul(texture_width)?
            .checked_add(x)?
            .checked_mul(CEF_UI_TEXTURE_BYTES_PER_PIXEL)?;
        let end = offset.checked_add(row_bytes)?;
        bytes.extend_from_slice(src.get(offset..end)?);
    }
    Some(bytes)
}

fn cef_dirty_rect_bounds(size: UVec2, rect: CefDirtyRect) -> Option<(u32, u32, u32, u32)> {
    if rect.x < 0 || rect.y < 0 || rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    let x = u32::try_from(rect.x).ok()?;
    let y = u32::try_from(rect.y).ok()?;
    let width = u32::try_from(rect.width).ok()?;
    let height = u32::try_from(rect.height).ok()?;
    if x.checked_add(width)? > size.x || y.checked_add(height)? > size.y {
        return None;
    }
    Some((x, y, width, height))
}

#[cfg(test)]
fn copy_cef_dirty_rects_to_texture_data(
    dst: &mut [u8],
    src: &[u8],
    size: UVec2,
    dirty_rects: &[CefDirtyRect],
) -> Option<usize> {
    copy_cef_frame_to_texture_data(dst, src, size, dirty_rects, false)
}

#[cfg(test)]
fn copy_cef_frame_to_texture_data(
    dst: &mut [u8],
    src: &[u8],
    size: UVec2,
    dirty_rects: &[CefDirtyRect],
    force_full_upload: bool,
) -> Option<usize> {
    let expected_byte_len = cef_ui_texture_byte_len(size)?;
    if dst.len() != expected_byte_len || src.len() != expected_byte_len {
        return None;
    }
    if force_full_upload || dirty_rects.is_empty() {
        dst.copy_from_slice(src);
        return Some(expected_byte_len);
    }

    let mut copied_bytes = 0usize;
    for rect in dirty_rects {
        copied_bytes = copied_bytes
            .checked_add(copy_cef_dirty_rect_to_texture_data(dst, src, size, *rect)?)?;
    }
    Some(copied_bytes)
}

#[cfg(test)]
fn copy_cef_dirty_rect_to_texture_data(
    dst: &mut [u8],
    src: &[u8],
    size: UVec2,
    rect: CefDirtyRect,
) -> Option<usize> {
    if rect.x < 0 || rect.y < 0 || rect.width <= 0 || rect.height <= 0 {
        return None;
    }
    let x = usize::try_from(rect.x).ok()?;
    let y = usize::try_from(rect.y).ok()?;
    let width = usize::try_from(rect.width).ok()?;
    let height = usize::try_from(rect.height).ok()?;
    let texture_width = usize::try_from(size.x).ok()?;
    let texture_height = usize::try_from(size.y).ok()?;
    if x.checked_add(width)? > texture_width || y.checked_add(height)? > texture_height {
        return None;
    }

    let row_bytes = width.checked_mul(CEF_UI_TEXTURE_BYTES_PER_PIXEL)?;
    let mut copied_bytes = 0usize;
    for row in y..y.checked_add(height)? {
        let offset = row
            .checked_mul(texture_width)?
            .checked_add(x)?
            .checked_mul(CEF_UI_TEXTURE_BYTES_PER_PIXEL)?;
        let end = offset.checked_add(row_bytes)?;
        dst.get_mut(offset..end)?
            .copy_from_slice(src.get(offset..end)?);
        copied_bytes = copied_bytes.checked_add(row_bytes)?;
    }
    Some(copied_bytes)
}

fn flush_cef_ui_diagnostics(
    bridge: Res<CefUiBridge>,
    mut stats: ResMut<CefUiFrameStats>,
    mut diagnostics_state: ResMut<CefUiDiagnosticsState>,
) {
    let dropped_patch_count = bridge.dropped_patch_count();
    let coalesced_patch_count = bridge.coalesced_patch_count();
    stats.dropped_patch_count = dropped_patch_count;
    stats.coalesced_patch_count = coalesced_patch_count;
    if dropped_patch_count != diagnostics_state.last_dropped_patch_count {
        tracing::warn!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            dropped_patch_count,
            "CEF UI outgoing patch dropped due to backpressure"
        );
        diagnostics_state.last_dropped_patch_count = dropped_patch_count;
    }
    if coalesced_patch_count != diagnostics_state.last_coalesced_patch_count {
        tracing::debug!(
            target: FUN_UI_DIAGNOSTICS_TARGET,
            coalesced_patch_count,
            "CEF UI outgoing patches coalesced"
        );
        diagnostics_state.last_coalesced_patch_count = coalesced_patch_count;
    }
    if stats.overlay_click_through_change_count != diagnostics_state.last_overlay_change_count {
        diagnostics_state.last_overlay_change_count = stats.overlay_click_through_change_count;
    }
}

fn write_patch_value(
    writer: &mut UiPatchWriter,
    channel: GameUiChannel,
    field: GameUiFieldKey,
    value: UiPatchValue,
) -> Result<(), UiPatchWriteError> {
    match value {
        UiPatchValue::Bool { value } => writer.set_bool(channel, field, value),
        UiPatchValue::U16 { value } => writer.set_u16(channel, field, value),
        UiPatchValue::U32 { value } => writer.set_u32(channel, field, value),
        UiPatchValue::I32 { value } => writer.set_i32(channel, field, value),
        UiPatchValue::Text { value } => writer.set_text(channel, field, &value),
        UiPatchValue::Binary { bytes } => writer.set_binary(channel, field, &bytes),
    }
}

fn write_diagnostic_patch(
    writer: &mut UiPatchWriter,
    diagnostic: &CefUiDiagnosticPush,
) -> Result<(), UiPatchWriteError> {
    let severity = match diagnostic.severity {
        CefUiDiagnosticSeverity::Info => 1_u32,
        CefUiDiagnosticSeverity::Warn => 2,
        CefUiDiagnosticSeverity::Error => 3,
    };
    let kind = match diagnostic.kind {
        CefUiDiagnosticKind::RuntimeInitialize => 1_u32,
        CefUiDiagnosticKind::SubprocessHandled => 2,
        CefUiDiagnosticKind::BrowserCreated => 3,
        CefUiDiagnosticKind::PageLoaded => 4,
        CefUiDiagnosticKind::BrowserClosed => 5,
        CefUiDiagnosticKind::PaintReceived => 6,
        CefUiDiagnosticKind::DirtyRectUpload => 7,
        CefUiDiagnosticKind::SchemeRequestServed => 8,
        CefUiDiagnosticKind::SchemeRequestRejected => 9,
        CefUiDiagnosticKind::NavigationBlocked => 10,
        CefUiDiagnosticKind::JsBridgeMessageReceived => 11,
        CefUiDiagnosticKind::BridgePacketRejected => 12,
        CefUiDiagnosticKind::JsBridgeMessageRejected => 13,
        CefUiDiagnosticKind::OutgoingPatchCoalesced => 14,
        CefUiDiagnosticKind::OutgoingPatchDropped => 15,
        CefUiDiagnosticKind::OverlayStateChanged => 16,
        CefUiDiagnosticKind::PaintTransportSelected => 17,
        CefUiDiagnosticKind::AcceleratedPaintReceived => 18,
        CefUiDiagnosticKind::AcceleratedPaintRejected => 19,
        CefUiDiagnosticKind::ShutdownStarted => 20,
    };
    let value = diagnostic
        .value
        .min(u64::from(u32::MAX))
        .saturating_add(u64::from(severity) << 28)
        .saturating_add(u64::from(kind) << 20)
        .min(u64::from(u32::MAX)) as u32;
    writer.set_u32(
        GameUiChannel::Diagnostics,
        GameUiFieldKey::DiagnosticsSummary,
        value,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_entry_focus_captures_keyboard_and_blocks_gameplay_without_pointer_region() {
        let status = CefUiStatus {
            initialized: true,
            browser_loaded: true,
            current_route: CefUiRoute::Hud,
            compositor_visible: true,
            last_frame_generation: None,
            last_error: None,
            last_blocked_navigation: None,
            last_js_error: None,
        };
        let mut input_capture = CefUiInputCapture {
            text_entry_active: true,
            ..Default::default()
        };
        let focus_mode = if input_capture.text_entry_active {
            CefUiFocusMode::TextEntry
        } else if status.current_route.modal_reason().is_some() {
            CefUiFocusMode::UiModal
        } else if status.compositor_visible {
            CefUiFocusMode::HudPassive
        } else {
            CefUiFocusMode::Gameplay
        };
        input_capture.modal_reason = status.current_route.modal_reason();
        match focus_mode {
            CefUiFocusMode::TextEntry => {
                input_capture.capture_keyboard = true;
                input_capture.capture_pointer = false;
            }
            CefUiFocusMode::Gameplay | CefUiFocusMode::HudPassive | CefUiFocusMode::UiModal => {}
        }
        let gate = GameplayInputGate {
            block_movement: true,
            block_fire: true,
            block_actions: true,
            block_look: true,
            keyboard_owned_by_ui: true,
            pointer_owned_by_ui: input_capture.capture_pointer,
            reason: GameplayInputBlockReason::TextEntry,
        };

        assert_eq!(focus_mode, CefUiFocusMode::TextEntry);
        assert!(input_capture.capture_keyboard);
        assert!(!input_capture.capture_pointer);
        assert!(gate.blocks_movement());
        assert!(gate.blocks_keyboard_actions());
    }

    #[test]
    fn passive_hud_captures_pointer_only_inside_declared_region() {
        let mut input_capture = CefUiInputCapture::default();
        input_capture.set_hit_regions(
            BrowserUiHitRegionMode::HudPassive,
            &[BrowserUiHitRegion {
                id: BrowserUiHitRegionId::Chat,
                x: 20,
                y: 720,
                w: 420,
                h: 80,
            }],
        );

        assert_eq!(
            input_capture.hit_region_at(CefUiPointerPosition { x: 24, y: 724 }),
            Some(BrowserUiHitRegionId::Chat)
        );
        assert_eq!(
            input_capture.hit_region_at(CefUiPointerPosition { x: 10, y: 724 }),
            None
        );
    }

    #[test]
    fn fun_host_input_owner_controls_cef_focus_mode() {
        let status = CefUiStatus {
            initialized: true,
            browser_loaded: true,
            current_route: CefUiRoute::Hud,
            compositor_visible: true,
            last_frame_generation: None,
            last_error: None,
            last_blocked_navigation: None,
            last_js_error: None,
        };
        let input_capture = CefUiInputCapture::default();
        let mut host = FunClientHostState::default();

        host.set_input_owner(FunInputOwner::EditorUi);
        assert_eq!(
            focus_mode_for_owner(&status, &input_capture, Some(&host)),
            CefUiFocusMode::UiModal
        );

        host.set_input_owner(FunInputOwner::Commandbar);
        assert_eq!(
            focus_mode_for_owner(&status, &input_capture, Some(&host)),
            CefUiFocusMode::TextEntry
        );

        host.set_input_owner(FunInputOwner::Gameplay);
        assert_eq!(
            focus_mode_for_owner(&status, &input_capture, Some(&host)),
            CefUiFocusMode::HudPassive
        );
    }

    #[test]
    fn modal_focus_disables_overlay_click_through() {
        let mut overlay_state = CefUiOverlayClickThroughState::default();
        let mut stats = CefUiFrameStats::default();
        let input_capture = CefUiInputCapture {
            capture_pointer: true,
            ..Default::default()
        };
        let desired_click_through = match CefUiFocusMode::UiModal {
            CefUiFocusMode::Gameplay | CefUiFocusMode::HudPassive | CefUiFocusMode::TextEntry => {
                !input_capture.capture_pointer
            }
            CefUiFocusMode::UiModal => false,
        };
        if overlay_state.click_through != desired_click_through {
            overlay_state.click_through = desired_click_through;
            overlay_state.change_count = overlay_state.change_count.saturating_add(1);
            stats.overlay_click_through_change_count =
                stats.overlay_click_through_change_count.saturating_add(1);
        }

        assert!(!overlay_state.click_through);
        assert_eq!(overlay_state.change_count, 1);
        assert_eq!(stats.overlay_click_through_change_count, 1);
    }

    #[test]
    fn cef_input_forwarding_follows_focus_capture_rules() {
        let mut input_capture = CefUiInputCapture::default();

        assert!(!should_forward_pointer_to_cef(
            CefUiFocusMode::Gameplay,
            &input_capture
        ));
        assert!(!should_forward_keyboard_to_cef(
            CefUiFocusMode::Gameplay,
            &input_capture
        ));

        assert!(should_forward_pointer_to_cef(
            CefUiFocusMode::HudPassive,
            &input_capture
        ));
        assert!(!should_forward_keyboard_to_cef(
            CefUiFocusMode::HudPassive,
            &input_capture
        ));

        input_capture.capture_pointer = true;
        assert!(should_forward_pointer_to_cef(
            CefUiFocusMode::Gameplay,
            &input_capture
        ));

        input_capture.capture_keyboard = true;
        assert!(should_forward_keyboard_to_cef(
            CefUiFocusMode::TextEntry,
            &input_capture
        ));
    }

    #[test]
    fn cef_input_helpers_map_window_coordinates_and_keys() {
        assert_eq!(
            cef_pointer_position_from_vec2(Vec2::new(14.8, -8.0)),
            CefUiPointerPosition { x: 14, y: 0 }
        );
        assert_eq!(scaled_cef_wheel_delta(1.25, 100.0), 125);
        assert_eq!(windows_virtual_key_code(KeyCode::F1), Some(0x70));
        assert_eq!(windows_virtual_key_code(KeyCode::KeyA), Some(0x41));
        assert_eq!(
            cef_mouse_button(MouseButton::Left),
            Some(CefBrowserMouseButton::Left)
        );
        assert_eq!(cef_mouse_button(MouseButton::Back), None);
    }

    #[test]
    fn cef_texture_copy_updates_only_declared_dirty_rects() {
        let size = UVec2::new(4, 2);
        let src = (0..32_u8).collect::<Vec<_>>();
        let mut dst = vec![0_u8; 32];
        let copied = copy_cef_dirty_rects_to_texture_data(
            &mut dst,
            &src,
            size,
            &[CefDirtyRect::new(1, 0, 2, 2)],
        )
        .expect("dirty rect copy");

        assert_eq!(copied, 16);
        assert_eq!(&dst[4..12], &src[4..12]);
        assert_eq!(&dst[20..28], &src[20..28]);
        assert_eq!(&dst[0..4], &[0, 0, 0, 0]);
        assert_eq!(&dst[12..20], &[0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&dst[28..32], &[0, 0, 0, 0]);
    }

    #[test]
    fn cef_texture_copy_rejects_out_of_bounds_dirty_rects() {
        let size = UVec2::new(2, 2);
        let src = vec![1_u8; 16];
        let mut dst = vec![0_u8; 16];

        assert_eq!(
            copy_cef_dirty_rects_to_texture_data(
                &mut dst,
                &src,
                size,
                &[CefDirtyRect::new(1, 1, 2, 1)]
            ),
            None
        );
    }

    #[test]
    fn cef_texture_copy_can_force_full_frame_after_skipped_generation() {
        let size = UVec2::new(4, 2);
        let src = (0..32_u8).collect::<Vec<_>>();
        let mut dst = vec![0_u8; 32];
        let copied = copy_cef_frame_to_texture_data(
            &mut dst,
            &src,
            size,
            &[CefDirtyRect::new(1, 0, 2, 2)],
            true,
        )
        .expect("forced full frame copy");

        assert_eq!(copied, 32);
        assert_eq!(dst, src);
    }

    #[test]
    fn cef_dirty_rect_bytes_extracts_compact_rows_for_render_queue_upload() {
        let size = UVec2::new(4, 3);
        let src = (0..48_u8).collect::<Vec<_>>();
        let bytes = cef_dirty_rect_bytes(&src, size, CefDirtyRect::new(1, 1, 2, 2))
            .expect("dirty rect bytes");

        assert_eq!(bytes.len(), 16);
        assert_eq!(&bytes[0..8], &src[20..28]);
        assert_eq!(&bytes[8..16], &src[36..44]);
        assert_eq!(
            cef_dirty_rect_byte_len(size, CefDirtyRect::new(1, 1, 2, 2)),
            Some(16)
        );
        assert_eq!(cef_dirty_rect_offset_bytes(size, 1, 1), Some(20));
    }

    #[test]
    fn cef_texture_upload_byte_count_tracks_dirty_or_full_uploads() {
        let size = UVec2::new(4, 3);
        assert_eq!(
            cef_ui_texture_upload_byte_count(size, &[CefDirtyRect::new(1, 1, 2, 2)], false),
            Some(16)
        );
        assert_eq!(
            cef_ui_texture_upload_byte_count(size, &[CefDirtyRect::new(1, 1, 2, 2)], true),
            Some(48)
        );
        assert_eq!(cef_ui_texture_upload_byte_count(size, &[], false), Some(48));
        assert_eq!(
            cef_ui_texture_upload_byte_count(size, &[CefDirtyRect::new(3, 2, 2, 2)], false),
            None
        );
        assert_eq!(
            cef_ui_texture_upload_byte_count(size, &[CefDirtyRect::new(3, 2, 2, 2)], true),
            Some(48)
        );
    }

    #[test]
    fn cef_frame_generation_gap_requires_full_texture_upload() {
        assert!(cef_ui_frame_requires_full_texture_upload(
            None,
            CefUiFrameGeneration(1)
        ));
        assert!(!cef_ui_frame_requires_full_texture_upload(
            Some(CefUiFrameGeneration(7)),
            CefUiFrameGeneration(8)
        ));
        assert!(cef_ui_frame_requires_full_texture_upload(
            Some(CefUiFrameGeneration(7)),
            CefUiFrameGeneration(9)
        ));
        assert!(cef_ui_frame_requires_full_texture_upload(
            Some(CefUiFrameGeneration(7)),
            CefUiFrameGeneration(6)
        ));
    }

    #[test]
    fn fps_counter_layout_hides_launcher_and_editor_without_preview() {
        assert_eq!(
            fun_client_fps_counter_layout(FunHostMode::Launcher, None, Some(UVec2::new(1280, 720))),
            None
        );
        assert_eq!(
            fun_client_fps_counter_layout(FunHostMode::Editor, None, Some(UVec2::new(1280, 720))),
            None
        );
    }

    #[test]
    fn fps_counter_layout_targets_game_or_editor_preview_only() {
        assert_eq!(
            fun_client_fps_counter_layout(FunHostMode::Game, None, Some(UVec2::new(1280, 720))),
            Some(FunClientFpsCounterLayout {
                left: 12.0,
                top: 12.0,
            })
        );
        assert_eq!(
            fun_client_fps_counter_layout(
                FunHostMode::Editor,
                Some(FunViewportRect {
                    x: 320,
                    y: 120,
                    width: 640,
                    height: 360,
                    scale_factor_milli: 1000,
                }),
                Some(UVec2::new(1280, 720))
            ),
            Some(FunClientFpsCounterLayout {
                left: 332.0,
                top: 132.0,
            })
        );
    }

    #[test]
    fn cef_render_texture_upload_timer_is_fixed_sixty_hz() {
        let render_texture = CefUiRenderTexture::default();
        let message_loop_pump = CefUiMessageLoopPump::external_pump_60hz();

        assert_eq!(CEF_UI_RENDER_RATE_HZ, 60);
        assert_eq!(
            render_texture.upload_timer.duration(),
            Duration::from_nanos(16_666_666)
        );
        assert!(message_loop_pump.enabled());
        assert_eq!(
            message_loop_pump.interval(),
            render_texture.upload_timer.duration()
        );
    }

    #[test]
    fn validates_authoritative_chat_request_before_intent() {
        let payload = UiControlPayload::ChatSubmit {
            message: "squad ready".to_owned(),
        };
        let request =
            game_ui_request_from_control(BrowserUiRequestId(1), BrowserUiSequence(2), &payload)
                .expect("valid request conversion")
                .expect("chat request");
        let authority = GameUiProtocolValidationContext::local_game_client(false);

        assert_eq!(validate_game_ui_request(&request, &authority), Ok(()));
    }

    #[test]
    fn bridge_keeps_patch_batches_bounded_and_coalesced() {
        let mut bridge = CefUiBridge::default();
        bridge
            .push_patch(
                GameUiChannel::Hud,
                GameUiFieldKey::Health,
                UiPatchValue::U16 { value: 90 },
            )
            .expect("first patch");
        bridge.finish_patch_writer();
        bridge
            .push_patch(
                GameUiChannel::Hud,
                GameUiFieldKey::Health,
                UiPatchValue::U16 { value: 80 },
            )
            .expect("second patch");
        bridge.finish_patch_writer();

        assert_eq!(bridge.coalesced_patch_count(), 1);
        assert!(bridge.pop_patch_batch().is_some());
        assert!(bridge.pop_patch_batch().is_none());
    }
}
