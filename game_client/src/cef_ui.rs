use bevy::{ecs::system::SystemParam, prelude::*};
use fun_render::{RenderWorldContext, RenderWorldStatus};
use fun_ui_cef::bridge::{BrowserUiMenuCommand, UiLifecycleState};
use fun_ui_cef::diagnostics::{CefUiDiagnosticKind, CefUiDiagnosticSeverity};
use fun_ui_cef::{
    BrowserBridgeError, BrowserBridgeQueues, BrowserUiHitRegion,
    BrowserUiProtocolValidationContext, BrowserUiProtocolValidationError, BrowserUiRequestId,
    BrowserUiRouteState, BrowserUiSequence, CefUiModel, GameUiChannel, GameUiFieldKey,
    UiControlPayload, UiEnvelope, UiEnvelopeKind, UiEnvelopePayload, UiPatchBackpressureQueue,
    UiPatchBatch, UiPatchValue, UiPatchWriteError, UiPatchWriter, UiSurfaceGeneration,
    validate_ui_envelope,
};

pub const MAX_CEF_UI_HIT_REGIONS: usize = 64;
const MAX_JS_MESSAGES_PER_FRAME: usize = 64;

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
            .init_resource::<CefUiBridge>()
            .init_resource::<CefUiFrameStats>()
            .init_resource::<CefUiModelCache>()
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
                (update_cef_ui_focus_mode, update_cef_ui_input_capture)
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
        }
    }
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
    pub id: u16,
    pub rect: CefUiHitRect,
    pub captures_pointer: bool,
}

impl From<BrowserUiHitRegion> for CefUiHitRegion {
    fn from(value: BrowserUiHitRegion) -> Self {
        Self {
            id: value.id,
            rect: CefUiHitRect {
                x: value.x,
                y: value.y,
                width: value.width,
                height: value.height,
            },
            captures_pointer: value.captures_pointer,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct CefUiInputCapture {
    pub capture_keyboard: bool,
    pub capture_pointer: bool,
    pub hit_regions: Vec<CefUiHitRegion>,
    pub modal_reason: Option<CefUiModalReason>,
    pub text_entry_active: bool,
}

impl CefUiInputCapture {
    fn set_hit_regions(&mut self, regions: &[BrowserUiHitRegion]) {
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
    fn has_pointer_hit_region(&self) -> bool {
        self.hit_regions
            .iter()
            .any(|region| region.captures_pointer)
    }
}

impl Default for CefUiInputCapture {
    fn default() -> Self {
        Self {
            capture_keyboard: false,
            capture_pointer: false,
            hit_regions: Vec::with_capacity(MAX_CEF_UI_HIT_REGIONS),
            modal_reason: None,
            text_entry_active: false,
        }
    }
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
    pub patch_batch_count: u64,
    pub dropped_patch_count: u64,
    pub coalesced_patch_count: u64,
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
    pub request_id: BrowserUiRequestId,
    pub sequence: BrowserUiSequence,
    pub payload: UiControlPayload,
}

#[derive(Debug, Clone, Copy, Message)]
pub struct CefUiRouteChanged {
    pub route: CefUiRoute,
}

#[derive(Debug, Clone, Message)]
pub struct CefUiHitRegionsChanged {
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
    render_world_ready: bool,
    expected_chunks: u16,
    received_chunks: u16,
}

impl CefUiRuntimeModel {
    fn from_resources(
        status: &CefUiStatus,
        world_status: &RenderWorldStatus,
        world_context: &RenderWorldContext,
    ) -> Self {
        Self {
            browser_loaded: status.browser_loaded,
            compositor_visible: status.compositor_visible,
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

fn cef_ui_initialize_service(mut status: ResMut<CefUiStatus>) {
    status.initialized = true;
    status.compositor_visible = true;
    status.current_route = CefUiRoute::Hud;
}

fn drain_cef_incoming_queue(
    mut bridge: ResMut<CefUiBridge>,
    mut status: ResMut<CefUiStatus>,
    mut stats: ResMut<CefUiFrameStats>,
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
            continue;
        }
        drain_validated_envelope(
            envelope,
            &mut status,
            &mut writers.intents,
            &mut writers.requests,
            &mut writers.routes,
            &mut writers.hit_regions,
            &mut writers.text_entry,
        );
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

fn drain_validated_envelope(
    envelope: UiEnvelope,
    status: &mut CefUiStatus,
    intents: &mut MessageWriter<CefUiIntent>,
    requests: &mut MessageWriter<CefUiRequest>,
    routes: &mut MessageWriter<CefUiRouteChanged>,
    hit_regions: &mut MessageWriter<CefUiHitRegionsChanged>,
    text_entry: &mut MessageWriter<CefUiTextEntryChanged>,
) {
    let sequence = envelope.sequence;
    let request_id = envelope.request_id;
    let kind = envelope.kind;
    match envelope.payload {
        UiEnvelopePayload::Control { payload } if kind == UiEnvelopeKind::Request => {
            if let Some(request_id) = request_id {
                requests.write(CefUiRequest {
                    request_id,
                    sequence,
                    payload,
                });
            }
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::Ready,
        } => {
            status.browser_loaded = true;
            intents.write(CefUiIntent::Ready);
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::RouteChanged { route },
        } => {
            routes.write(CefUiRouteChanged {
                route: CefUiRoute::from(route),
            });
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::HitRegionsChanged { regions },
        } => {
            hit_regions.write(CefUiHitRegionsChanged { regions });
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::TextEntryChanged { active },
        } => {
            text_entry.write(CefUiTextEntryChanged { active });
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::ChatSubmit { message },
        } => {
            intents.write(CefUiIntent::ChatSubmit { message });
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::MenuCommand { command },
        } => {
            intents.write(CefUiIntent::MenuCommand { command });
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::SettingsChanged { key, value_json },
        } => {
            intents.write(CefUiIntent::SettingsChanged { key, value_json });
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::Lifecycle { state },
        } => {
            if matches!(
                state,
                UiLifecycleState::PageLoaded | UiLifecycleState::PageVisible
            ) {
                status.browser_loaded = true;
            }
            intents.write(CefUiIntent::Lifecycle { state });
        }
        UiEnvelopePayload::Error { .. }
        | UiEnvelopePayload::Empty
        | UiEnvelopePayload::StatePatch { .. }
        | UiEnvelopePayload::ModelPatchBatch { .. }
        | UiEnvelopePayload::JsonBytes { .. } => {}
    }
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
        input_capture.set_hit_regions(&change.regions);
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
    } else if status.current_route.modal_reason().is_some() {
        CefUiFocusMode::UiModal
    } else if status.compositor_visible {
        CefUiFocusMode::HudPassive
    } else {
        CefUiFocusMode::Gameplay
    };
}

fn update_cef_ui_input_capture(
    focus_mode: Res<CefUiFocusMode>,
    status: Res<CefUiStatus>,
    mut input_capture: ResMut<CefUiInputCapture>,
) {
    input_capture.modal_reason = status.current_route.modal_reason();
    match *focus_mode {
        CefUiFocusMode::Gameplay => {
            input_capture.capture_keyboard = false;
            input_capture.capture_pointer = false;
        }
        CefUiFocusMode::HudPassive => {
            input_capture.capture_keyboard = false;
            input_capture.capture_pointer = input_capture.has_pointer_hit_region();
        }
        CefUiFocusMode::UiModal | CefUiFocusMode::TextEntry => {
            input_capture.capture_keyboard = true;
            input_capture.capture_pointer = true;
        }
    }
}

fn collect_gameplay_ui_model(
    status: Res<CefUiStatus>,
    world_status: Res<RenderWorldStatus>,
    world_context: Res<RenderWorldContext>,
    mut cache: ResMut<CefUiModelCache>,
    mut bridge: ResMut<CefUiBridge>,
) {
    let model = CefUiRuntimeModel::from_resources(&status, &world_status, &world_context);
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

fn flush_cef_ui_diagnostics(bridge: Res<CefUiBridge>, mut stats: ResMut<CefUiFrameStats>) {
    stats.dropped_patch_count = bridge.dropped_patch_count();
    stats.coalesced_patch_count = bridge.coalesced_patch_count();
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
        CefUiDiagnosticKind::BrowserClosed => 4,
        CefUiDiagnosticKind::PaintReceived => 5,
        CefUiDiagnosticKind::BridgePacketRejected => 6,
        CefUiDiagnosticKind::ShutdownStarted => 7,
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
    fn text_entry_focus_captures_keyboard_and_pointer() {
        let status = CefUiStatus {
            initialized: true,
            browser_loaded: true,
            current_route: CefUiRoute::Hud,
            compositor_visible: true,
            last_frame_generation: None,
            last_error: None,
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
                input_capture.capture_pointer = true;
            }
            CefUiFocusMode::Gameplay | CefUiFocusMode::HudPassive | CefUiFocusMode::UiModal => {}
        }

        assert_eq!(focus_mode, CefUiFocusMode::TextEntry);
        assert!(input_capture.capture_keyboard);
        assert!(input_capture.capture_pointer);
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
