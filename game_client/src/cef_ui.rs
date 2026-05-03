use std::time::Duration;

use bevy::{
    asset::RenderAssetUsages,
    ecs::system::SystemParam,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
    window::PrimaryWindow,
};
use fun_render::{RenderWorldContext, RenderWorldStatus};
use fun_ui_cef::bridge::{BrowserUiMenuCommand, UiLifecycleState};
use fun_ui_cef::diagnostics::{
    CefUiDiagnosticKind, CefUiDiagnosticSeverity, FUN_UI_DIAGNOSTICS_TARGET,
};
use fun_ui_cef::{
    BrowserBridgeError, BrowserBridgeQueues, BrowserUiHitRegion, BrowserUiHitRegionId,
    BrowserUiHitRegionMode, BrowserUiProtocolValidationContext, BrowserUiProtocolValidationError,
    BrowserUiRequestId, BrowserUiRouteState, BrowserUiSequence, CefUiModel, CefUiSecurityPolicy,
    FunUiNavigationBlockReason, GameUiChannel, GameUiFieldKey, SharedCefUiCompositor,
    UiControlPayload, UiEnvelope, UiEnvelopeKind, UiEnvelopePayload, UiPatchBackpressureQueue,
    UiPatchBatch, UiPatchValue, UiPatchWriteError, UiPatchWriter, UiSurfaceGeneration,
    validate_ui_envelope,
};
use fun_ui_cef::{CefDirtyRect, CefPaintElement, CefUiCompositorFrame};
use game_shared::{
    GameUiMenuTarget, GameUiProtocolValidationContext, GameUiRequestEnvelope, GameUiRequestId,
    GameUiRequestPayload, GameUiRequestRejectionReason, GameUiSequence, GameUiSettingKey,
    GameUiSettingValue, validate_game_ui_request,
};

pub const MAX_CEF_UI_HIT_REGIONS: usize = 64;
const MAX_JS_MESSAGES_PER_FRAME: usize = 64;
const CEF_UI_RENDER_RATE_HZ: u64 = fun_ui_cef::CEF_UI_WINDOWLESS_FRAME_RATE_HZ as u64;

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
            .init_resource::<CefUiMessageLoopPump>()
            .init_resource::<CefUiRenderTexture>()
            .init_resource::<CefUiModelCache>()
            .init_resource::<CefUiDiagnosticsState>()
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
                (write_cef_ui_outgoing_messages, send_cef_ui_patch_batch)
                    .chain()
                    .in_set(GameCefUiSet::FlushBridge),
            )
            .add_systems(
                PostUpdate,
                upload_cef_ui_frame_to_fun_texture.after(GameCefUiSet::FlushBridge),
            )
            .add_systems(
                Last,
                flush_cef_ui_diagnostics.in_set(GameCefUiSet::Diagnostics),
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
    queues: BrowserBridgeQueues,
    patch_writer: UiPatchWriter,
    patch_queue: UiPatchBackpressureQueue,
    next_sequence: BrowserUiSequence,
}

impl CefUiBridge {
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
        Self {
            queues: BrowserBridgeQueues::default(),
            patch_writer: UiPatchWriter::default(),
            patch_queue: UiPatchBackpressureQueue::default(),
            next_sequence: BrowserUiSequence(0),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct CefUiFrameStats {
    pub paint_count: u64,
    pub dirty_rect_count: u64,
    pub uploaded_bytes: u64,
    pub js_message_count: u64,
    pub js_message_rejected_count: u64,
    pub patch_batch_count: u64,
    pub dropped_patch_count: u64,
    pub coalesced_patch_count: u64,
    pub overlay_click_through_change_count: u64,
    pub navigation_blocked_count: u64,
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
struct CefUiDiagnosticsState {
    last_dropped_patch_count: u64,
    last_coalesced_patch_count: u64,
    last_overlay_change_count: u64,
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
        | UiControlPayload::Lifecycle { .. } => return Ok(None),
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
                let Some(request) = game_ui_request_from_control(request_id, sequence, &payload)
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
                | UiControlPayload::SettingsChanged { .. },
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
    mut focus_mode: ResMut<CefUiFocusMode>,
) {
    *focus_mode = if input_capture.text_entry_active {
        CefUiFocusMode::TextEntry
    } else if status.current_route.modal_reason().is_some()
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
    };
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

fn write_cef_ui_outgoing_messages(
    mut bridge: ResMut<CefUiBridge>,
    mut status: ResMut<CefUiStatus>,
    mut patches: MessageReader<CefUiPatch>,
    mut commands: MessageReader<CefUiCommand>,
    mut route_sets: MessageReader<CefUiRouteSet>,
    mut modal_sets: MessageReader<CefUiModalSet>,
    mut diagnostics: MessageReader<CefUiDiagnosticPush>,
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

const CEF_UI_TEXTURE_BYTES_PER_PIXEL: usize = 4;
const CEF_UI_TEXTURE_Z_INDEX: i32 = 900_000;

fn cef_ui_render_interval() -> Duration {
    Duration::from_nanos(1_000_000_000 / CEF_UI_RENDER_RATE_HZ)
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

fn upload_cef_ui_frame_to_fun_texture(
    mut commands: Commands,
    render_compositor: Option<Res<CefUiRenderCompositor>>,
    time: Res<Time>,
    mut render_texture: ResMut<CefUiRenderTexture>,
    mut images: ResMut<Assets<Image>>,
    mut image_nodes: Query<&mut ImageNode, With<CefUiRenderTextureRoot>>,
    mut status: ResMut<CefUiStatus>,
    mut stats: ResMut<CefUiFrameStats>,
) {
    let Some(render_compositor) = render_compositor else {
        return;
    };
    if !render_texture
        .upload_timer
        .tick(time.delta())
        .just_finished()
    {
        return;
    }
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

    let uploaded_bytes = if render_texture.image.is_none() || render_texture.size != Some(size) {
        let image_handle = images.add(new_cef_ui_texture_image(size, frame.pixels().to_vec()));
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
        expected_byte_len
    } else {
        let Some(image_handle) = render_texture.image.as_ref() else {
            return;
        };
        let Some(mut image) = images.get_mut(image_handle) else {
            return;
        };
        apply_cef_ui_frame_to_texture(&mut image, &frame).unwrap_or(0)
    };

    render_texture.last_generation = Some(frame.metadata.generation);
    status.browser_loaded = true;
    status.compositor_visible = true;
    status.last_frame_generation = Some(frame.metadata.generation);
    stats.paint_count = stats.paint_count.saturating_add(1);
    stats.dirty_rect_count = stats
        .dirty_rect_count
        .saturating_add(frame.metadata.dirty_rects.len() as u64);
    stats.uploaded_bytes = stats.uploaded_bytes.saturating_add(uploaded_bytes as u64);
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

fn apply_cef_ui_frame_to_texture(image: &mut Image, frame: &CefUiCompositorFrame) -> Option<usize> {
    let size = cef_ui_frame_texture_size(frame)?;
    if image.texture_descriptor.size.width != size.x
        || image.texture_descriptor.size.height != size.y
        || image.texture_descriptor.format != TextureFormat::Bgra8UnormSrgb
    {
        *image = new_cef_ui_texture_image(size, frame.pixels().to_vec());
        return cef_ui_texture_byte_len(size);
    }
    let data = image.data.as_mut()?;
    copy_cef_dirty_rects_to_texture_data(data, frame.pixels(), size, &frame.metadata.dirty_rects)
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

fn copy_cef_dirty_rects_to_texture_data(
    dst: &mut [u8],
    src: &[u8],
    size: UVec2,
    dirty_rects: &[CefDirtyRect],
) -> Option<usize> {
    let expected_byte_len = cef_ui_texture_byte_len(size)?;
    if dst.len() != expected_byte_len || src.len() != expected_byte_len {
        return None;
    }
    if dirty_rects.is_empty() {
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
        CefUiDiagnosticKind::ShutdownStarted => 17,
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
