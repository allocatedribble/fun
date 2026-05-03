use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};

use crate::model::UiPatchBatch;

const BROWSER_UI_PROTOCOL_VERSION: u32 = 1;
const BROWSER_UI_SCHEMA_REVISION: u32 = 1;
const DEFAULT_MAX_PACKET_BYTES: u32 = 64 * 1024;
const DEFAULT_MAX_QUEUE_LEN: usize = 256;
const MAX_UI_HIT_REGIONS: usize = 64;
const MAX_HOST_COMMAND_ID_BYTES: usize = 128;
const MAX_HOST_COMMAND_JSON_BYTES: usize = 64 * 1024;
const MAX_HOST_DIAGNOSTICS: usize = 32;
const MAX_HOST_DIAGNOSTIC_TEXT_BYTES: usize = 512;

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct BrowserUiProtocolVersion(pub u32);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct BrowserUiRequestId(pub u64);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct BrowserUiSequence(pub u64);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct BrowserUiSchemaRevision(pub u32);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct BrowserUiRevision(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum UiEnvelopeKind {
    Event,
    Request,
    Response,
    Error,
    Patch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum UiEnvelopeChannel {
    Control,
    State,
}

impl UiEnvelopeChannel {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::Control => "control",
            Self::State => "state",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct UiEnvelope {
    pub protocol_version: BrowserUiProtocolVersion,
    pub schema_revision: BrowserUiSchemaRevision,
    pub channel: UiEnvelopeChannel,
    pub kind: UiEnvelopeKind,
    pub request_id: Option<BrowserUiRequestId>,
    pub sequence: BrowserUiSequence,
    pub payload: UiEnvelopePayload,
}

impl UiEnvelope {
    #[must_use]
    pub const fn control(
        kind: UiEnvelopeKind,
        request_id: Option<BrowserUiRequestId>,
        sequence: BrowserUiSequence,
        payload: UiControlPayload,
    ) -> Self {
        Self {
            protocol_version: BrowserUiProtocolVersion(BROWSER_UI_PROTOCOL_VERSION),
            schema_revision: BrowserUiSchemaRevision(BROWSER_UI_SCHEMA_REVISION),
            channel: UiEnvelopeChannel::Control,
            kind,
            request_id,
            sequence,
            payload: UiEnvelopePayload::Control { payload },
        }
    }

    #[must_use]
    pub const fn state_patch(sequence: BrowserUiSequence, payload: UiStatePatchPayload) -> Self {
        Self {
            protocol_version: BrowserUiProtocolVersion(BROWSER_UI_PROTOCOL_VERSION),
            schema_revision: BrowserUiSchemaRevision(BROWSER_UI_SCHEMA_REVISION),
            channel: UiEnvelopeChannel::State,
            kind: UiEnvelopeKind::Patch,
            request_id: None,
            sequence,
            payload: UiEnvelopePayload::StatePatch { patch: payload },
        }
    }

    #[must_use]
    pub fn model_patch_batch(sequence: BrowserUiSequence, batch: UiPatchBatch) -> Self {
        Self {
            protocol_version: BrowserUiProtocolVersion(BROWSER_UI_PROTOCOL_VERSION),
            schema_revision: BrowserUiSchemaRevision(BROWSER_UI_SCHEMA_REVISION),
            channel: UiEnvelopeChannel::State,
            kind: UiEnvelopeKind::Patch,
            request_id: None,
            sequence,
            payload: UiEnvelopePayload::ModelPatchBatch { batch },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum UiEnvelopePayload {
    Empty,
    Control { payload: UiControlPayload },
    StatePatch { patch: UiStatePatchPayload },
    ModelPatchBatch { batch: UiPatchBatch },
    Error { error: UiErrorPayload },
    JsonBytes { bytes: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum UiControlPayload {
    Ready,
    RouteChanged {
        route: BrowserUiRouteState,
    },
    HitRegionsChanged {
        mode: BrowserUiHitRegionMode,
        regions: Vec<BrowserUiHitRegion>,
    },
    TextEntryChanged {
        active: bool,
    },
    MenuCommand {
        command: BrowserUiMenuCommand,
    },
    ChatSubmit {
        message: String,
    },
    SettingsChanged {
        key: String,
        value_json: Vec<u8>,
    },
    HostCommand {
        request: HostCommandRequest,
    },
    HostCommandResult {
        command_id: HostCommandId,
        response: HostCommandResponse,
    },
    HostEvent {
        event: String,
        payload: Vec<u8>,
    },
    Lifecycle {
        state: UiLifecycleState,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compactly::v1::Encode)]
pub enum UiLifecycleState {
    PageLoaded,
    PageHidden,
    PageVisible,
    Shutdown,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct UiStatePatchPayload {
    pub base_revision: BrowserUiRevision,
    pub target_revision: BrowserUiRevision,
    pub patches: Vec<UiStatePatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct UiStatePatch {
    pub path: UiStatePath,
    pub value: UiStateValue,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum UiStatePath {
    HudHealth,
    HudArmor,
    HudAmmo,
    ObjectiveLabel,
    ScoreboardRows,
    Loadout,
    ChatRows,
    Loading,
    DiagnosticsSummary,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum UiStateValue {
    Bool { value: bool },
    U16 { value: u16 },
    U32 { value: u32 },
    Text { value: String },
    JsonBytes { bytes: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct UiErrorPayload {
    pub code: UiErrorCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compactly::v1::Encode)]
pub enum UiErrorCode {
    UnknownMethod,
    InvalidPayload,
    MissingCapability,
    QueueClosed,
    Internal,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct BrowserUiSizeBudget {
    pub max_bytes: u32,
}

impl BrowserUiSizeBudget {
    #[must_use]
    pub const fn host_command_default() -> Self {
        Self {
            max_bytes: MAX_HOST_COMMAND_JSON_BYTES as u32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum HostCommandTarget {
    Game,
    Launcher,
    Editor,
    Project,
    Runtime,
    Preview,
    Material,
    Entity,
    Diagnostics,
    Auth,
    Backend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum HostCapability {
    ReadLauncher,
    JoinGame,
    OpenProject,
    EditProject,
    ReadEntities,
    MutateEntities,
    ControlRuntime,
    ReadDiagnostics,
    MaterialShaderRead,
    MaterialShaderWrite,
    RequestBackendTicket,
    UseDevTools,
}

impl HostCapability {
    #[must_use]
    pub const fn as_wire_str(self) -> &'static str {
        match self {
            Self::ReadLauncher => "read_launcher",
            Self::JoinGame => "join_game",
            Self::OpenProject => "open_project",
            Self::EditProject => "edit_project",
            Self::ReadEntities => "read_entities",
            Self::MutateEntities => "mutate_entities",
            Self::ControlRuntime => "control_runtime",
            Self::ReadDiagnostics => "read_diagnostics",
            Self::MaterialShaderRead => "material_shader_read",
            Self::MaterialShaderWrite => "material_shader_write",
            Self::RequestBackendTicket => "request_backend_ticket",
            Self::UseDevTools => "use_dev_tools",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct HostCommandId {
    value: String,
}

impl HostCommandId {
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    #[must_use]
    pub fn target(&self) -> Option<HostCommandTarget> {
        host_command_target(self.as_str())
    }

    #[must_use]
    pub fn required_capability(&self) -> Option<HostCapability> {
        host_command_required_capability(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct HostCommandRequest {
    pub command_id: HostCommandId,
    pub request_id: BrowserUiRequestId,
    pub payload: Vec<u8>,
    pub capability: HostCapability,
    pub size_budget: BrowserUiSizeBudget,
}

impl HostCommandRequest {
    #[must_use]
    pub fn new(
        request_id: BrowserUiRequestId,
        command_id: impl Into<String>,
        payload: Vec<u8>,
    ) -> Option<Self> {
        let command_id = HostCommandId::new(command_id);
        let capability = command_id.required_capability()?;
        Some(Self {
            command_id,
            request_id,
            payload,
            capability,
            size_budget: BrowserUiSizeBudget::host_command_default(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum HostCommandResponse {
    Ok {
        payload: Vec<u8>,
        diagnostics: Vec<HostDiagnostic>,
    },
    Rejected {
        reason: HostCommandRejection,
    },
    Failed {
        error: HostCommandError,
        diagnostics: Vec<HostDiagnostic>,
    },
}

impl HostCommandResponse {
    #[must_use]
    pub fn status_wire_str(&self) -> &'static str {
        match self {
            Self::Ok { .. } => "ok",
            Self::Rejected { .. } => "rejected",
            Self::Failed { .. } => "error",
        }
    }

    #[must_use]
    pub fn payload_bytes(&self) -> &[u8] {
        match self {
            Self::Ok { payload, .. } => payload,
            Self::Rejected { .. } => &[],
            Self::Failed { error, .. } => error.message.as_bytes(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct HostDiagnostic {
    pub code: String,
    pub level: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum HostCommandRejection {
    UnknownCommand,
    InvalidCommandId,
    MissingCapability,
    CapabilityMismatch,
    OversizePayload,
    InvalidPayload,
    HostShuttingDown,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct HostCommandError {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum BrowserUiCapability {
    ReadHudState,
    ReadScoreboard,
    SendChat,
    ControlMenuState,
    ReadDiagnostics,
    DebugOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct BrowserUiPacketHeader {
    pub request_id: BrowserUiRequestId,
    pub protocol_version: BrowserUiProtocolVersion,
    pub sequence: BrowserUiSequence,
    pub base_revision: Option<BrowserUiRevision>,
    pub size_budget: BrowserUiSizeBudget,
}

impl BrowserUiPacketHeader {
    #[must_use]
    pub const fn new(
        request_id: BrowserUiRequestId,
        sequence: BrowserUiSequence,
        base_revision: Option<BrowserUiRevision>,
    ) -> Self {
        Self {
            request_id,
            protocol_version: BrowserUiProtocolVersion(BROWSER_UI_PROTOCOL_VERSION),
            sequence,
            base_revision,
            size_budget: BrowserUiSizeBudget {
                max_bytes: DEFAULT_MAX_PACKET_BYTES,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum BrowserUiPacket {
    RuntimeToBrowser {
        header: BrowserUiPacketHeader,
        payload: RuntimeToBrowserPayload,
    },
    BrowserToRuntime {
        header: BrowserUiPacketHeader,
        payload: BrowserToRuntimePayload,
    },
}

impl BrowserUiPacket {
    #[must_use]
    pub fn header(&self) -> &BrowserUiPacketHeader {
        match self {
            Self::RuntimeToBrowser { header, .. } | Self::BrowserToRuntime { header, .. } => header,
        }
    }

    #[must_use]
    pub fn required_capability(&self) -> BrowserUiCapability {
        match self {
            Self::RuntimeToBrowser { payload, .. } => payload.required_capability(),
            Self::BrowserToRuntime { payload, .. } => payload.required_capability(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum RuntimeToBrowserPayload {
    HudState { state: HudStatePacket },
    Scoreboard { scoreboard: ScoreboardPacket },
    MenuState { state: MenuStatePacket },
    ChatAccepted { ack: ChatAcceptedPacket },
    Diagnostics { overlay: DiagnosticsOverlayPacket },
    Pong,
}

impl RuntimeToBrowserPayload {
    #[must_use]
    pub fn required_capability(&self) -> BrowserUiCapability {
        match self {
            Self::HudState { .. } => BrowserUiCapability::ReadHudState,
            Self::Scoreboard { .. } => BrowserUiCapability::ReadScoreboard,
            Self::MenuState { .. } => BrowserUiCapability::ControlMenuState,
            Self::ChatAccepted { .. } | Self::Pong => BrowserUiCapability::SendChat,
            Self::Diagnostics { .. } => BrowserUiCapability::ReadDiagnostics,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum BrowserToRuntimePayload {
    Ready,
    RouteChanged { route: BrowserUiRouteState },
    ChatSubmit { message: String },
    MenuCommand { command: BrowserUiMenuCommand },
    DiagnosticsOverlaySet { visible: bool },
    Ping,
}

impl BrowserToRuntimePayload {
    #[must_use]
    pub fn required_capability(&self) -> BrowserUiCapability {
        match self {
            Self::Ready | Self::Ping | Self::RouteChanged { .. } => {
                BrowserUiCapability::ReadHudState
            }
            Self::ChatSubmit { .. } => BrowserUiCapability::SendChat,
            Self::MenuCommand { .. } => BrowserUiCapability::ControlMenuState,
            Self::DiagnosticsOverlaySet { .. } => BrowserUiCapability::DebugOverlay,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct HudStatePacket {
    pub health: u16,
    pub armor: u16,
    pub ammo_in_magazine: u16,
    pub ammo_reserve: u16,
    pub objective_label: String,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct ScoreboardPacket {
    pub teams: Vec<ScoreboardTeamRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct ScoreboardTeamRow {
    pub team_id: u16,
    pub display_name: String,
    pub score: u32,
    pub players_alive: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compactly::v1::Encode)]
pub enum BrowserUiRouteState {
    Hud,
    PauseMenu,
    Loadout,
    Scoreboard,
    Chat,
    Loading,
    Diagnostics,
    DevtoolsOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum BrowserUiHitRegionMode {
    Gameplay,
    HudPassive,
    UiModal,
    TextEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum BrowserUiHitRegionId {
    Chat,
    Minimap,
    Scoreboard,
    PauseMenu,
    Loadout,
    Settings,
    Diagnostics,
    Devtools,
}

impl BrowserUiHitRegionId {
    #[must_use]
    pub fn from_wire_str(value: &str) -> Option<Self> {
        match value {
            "chat" => Some(Self::Chat),
            "minimap" => Some(Self::Minimap),
            "scoreboard" => Some(Self::Scoreboard),
            "pause_menu" => Some(Self::PauseMenu),
            "loadout" => Some(Self::Loadout),
            "settings" => Some(Self::Settings),
            "diagnostics" => Some(Self::Diagnostics),
            "devtools" => Some(Self::Devtools),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct BrowserUiHitRegion {
    pub id: BrowserUiHitRegionId,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compactly::v1::Encode)]
pub enum BrowserUiMenuCommand {
    Resume,
    OpenSettings,
    LeaveMatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compactly::v1::Encode)]
pub struct MenuStatePacket {
    pub open: bool,
    pub route: BrowserUiRouteState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, compactly::v1::Encode)]
pub struct ChatAcceptedPacket {
    pub local_sequence: BrowserUiSequence,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct DiagnosticsOverlayPacket {
    pub visible: bool,
    pub rows: Vec<DiagnosticsOverlayRow>,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct DiagnosticsOverlayRow {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserUiProtocolValidationContext {
    pub protocol_version: BrowserUiProtocolVersion,
    pub granted_capabilities: Vec<BrowserUiCapability>,
    pub granted_host_capabilities: Vec<HostCapability>,
    pub revision: BrowserUiRevision,
    pub max_payload_bytes: u32,
}

impl BrowserUiProtocolValidationContext {
    #[must_use]
    pub fn local_game_ui() -> Self {
        Self {
            protocol_version: BrowserUiProtocolVersion(BROWSER_UI_PROTOCOL_VERSION),
            granted_capabilities: vec![
                BrowserUiCapability::ReadHudState,
                BrowserUiCapability::ReadScoreboard,
                BrowserUiCapability::SendChat,
                BrowserUiCapability::ControlMenuState,
                BrowserUiCapability::ReadDiagnostics,
            ],
            granted_host_capabilities: vec![
                HostCapability::ReadLauncher,
                HostCapability::JoinGame,
                HostCapability::OpenProject,
                HostCapability::EditProject,
                HostCapability::ReadEntities,
                HostCapability::MutateEntities,
                HostCapability::ControlRuntime,
                HostCapability::ReadDiagnostics,
                HostCapability::MaterialShaderRead,
                HostCapability::MaterialShaderWrite,
                HostCapability::RequestBackendTicket,
                HostCapability::UseDevTools,
            ],
            revision: BrowserUiRevision(0),
            max_payload_bytes: DEFAULT_MAX_PACKET_BYTES,
        }
    }

    #[must_use]
    pub fn grants(&self, capability: BrowserUiCapability) -> bool {
        self.granted_capabilities.contains(&capability)
    }

    #[must_use]
    pub fn grants_host(&self, capability: HostCapability) -> bool {
        self.granted_host_capabilities.contains(&capability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserUiProtocolValidationError {
    StaleVersion,
    StaleSchemaRevision,
    MissingCapability,
    StaleRevision,
    OversizePayload,
    OversizeChatMessage,
    InvalidEnvelopeLane,
    MissingRequestId,
    UnexpectedRequestId,
    EmptyStatePatch,
    NonAdvancingStateRevision,
    TooManyHitRegions,
    InvalidHitRegion,
    DuplicateHitRegion,
    InvalidHostCommand,
}

pub fn validate_browser_ui_packet(
    packet: &BrowserUiPacket,
    context: &BrowserUiProtocolValidationContext,
) -> Result<(), BrowserUiProtocolValidationError> {
    let header = packet.header();
    if header.protocol_version != context.protocol_version {
        return Err(BrowserUiProtocolValidationError::StaleVersion);
    }
    if let Some(base_revision) = header.base_revision
        && base_revision != context.revision
    {
        return Err(BrowserUiProtocolValidationError::StaleRevision);
    }
    validate_packet_budget(packet, context)?;
    if !context.grants(packet.required_capability()) {
        return Err(BrowserUiProtocolValidationError::MissingCapability);
    }
    validate_payload(packet)
}

fn validate_packet_budget(
    packet: &BrowserUiPacket,
    context: &BrowserUiProtocolValidationContext,
) -> Result<(), BrowserUiProtocolValidationError> {
    let payload_len = compactly::v1::encode(packet).len();
    let sender_budget = packet.header().size_budget.max_bytes as usize;
    let runtime_budget = context.max_payload_bytes as usize;
    if payload_len > sender_budget || payload_len > runtime_budget {
        return Err(BrowserUiProtocolValidationError::OversizePayload);
    }
    Ok(())
}

fn validate_payload(packet: &BrowserUiPacket) -> Result<(), BrowserUiProtocolValidationError> {
    match packet {
        BrowserUiPacket::BrowserToRuntime {
            payload: BrowserToRuntimePayload::ChatSubmit { message },
            ..
        } if message.len() > 512 => Err(BrowserUiProtocolValidationError::OversizeChatMessage),
        BrowserUiPacket::RuntimeToBrowser { .. } | BrowserUiPacket::BrowserToRuntime { .. } => {
            Ok(())
        }
    }
}

pub fn validate_ui_envelope(
    envelope: &UiEnvelope,
    context: &BrowserUiProtocolValidationContext,
) -> Result<(), BrowserUiProtocolValidationError> {
    if envelope.protocol_version != context.protocol_version {
        return Err(BrowserUiProtocolValidationError::StaleVersion);
    }
    if envelope.schema_revision != BrowserUiSchemaRevision(BROWSER_UI_SCHEMA_REVISION) {
        return Err(BrowserUiProtocolValidationError::StaleSchemaRevision);
    }
    validate_envelope_budget(envelope, context)?;
    validate_envelope_request_id(envelope)?;
    validate_envelope_lane(envelope)?;
    validate_envelope_payload(envelope)?;
    validate_host_command_authority(envelope, context)
}

fn validate_envelope_budget(
    envelope: &UiEnvelope,
    context: &BrowserUiProtocolValidationContext,
) -> Result<(), BrowserUiProtocolValidationError> {
    let payload_len = compactly::v1::encode(envelope).len();
    if payload_len > context.max_payload_bytes as usize {
        return Err(BrowserUiProtocolValidationError::OversizePayload);
    }
    Ok(())
}

fn validate_envelope_request_id(
    envelope: &UiEnvelope,
) -> Result<(), BrowserUiProtocolValidationError> {
    match envelope.kind {
        UiEnvelopeKind::Request | UiEnvelopeKind::Response | UiEnvelopeKind::Error
            if envelope.request_id.is_none() =>
        {
            Err(BrowserUiProtocolValidationError::MissingRequestId)
        }
        UiEnvelopeKind::Event | UiEnvelopeKind::Patch if envelope.request_id.is_some() => {
            Err(BrowserUiProtocolValidationError::UnexpectedRequestId)
        }
        UiEnvelopeKind::Event
        | UiEnvelopeKind::Request
        | UiEnvelopeKind::Response
        | UiEnvelopeKind::Error
        | UiEnvelopeKind::Patch => Ok(()),
    }
}

fn validate_envelope_lane(envelope: &UiEnvelope) -> Result<(), BrowserUiProtocolValidationError> {
    match (envelope.channel, envelope.kind, &envelope.payload) {
        (UiEnvelopeChannel::Control, UiEnvelopeKind::Patch, _) => {
            Err(BrowserUiProtocolValidationError::InvalidEnvelopeLane)
        }
        (UiEnvelopeChannel::Control, _, UiEnvelopePayload::StatePatch { .. }) => {
            Err(BrowserUiProtocolValidationError::InvalidEnvelopeLane)
        }
        (UiEnvelopeChannel::Control, _, UiEnvelopePayload::ModelPatchBatch { .. }) => {
            Err(BrowserUiProtocolValidationError::InvalidEnvelopeLane)
        }
        (UiEnvelopeChannel::State, UiEnvelopeKind::Patch, UiEnvelopePayload::StatePatch { .. }) => {
            Ok(())
        }
        (
            UiEnvelopeChannel::State,
            UiEnvelopeKind::Patch,
            UiEnvelopePayload::ModelPatchBatch { .. },
        ) => Ok(()),
        (UiEnvelopeChannel::State, _, _) => {
            Err(BrowserUiProtocolValidationError::InvalidEnvelopeLane)
        }
        (UiEnvelopeChannel::Control, _, _) => Ok(()),
    }
}

fn validate_envelope_payload(
    envelope: &UiEnvelope,
) -> Result<(), BrowserUiProtocolValidationError> {
    match &envelope.payload {
        UiEnvelopePayload::Control {
            payload: UiControlPayload::ChatSubmit { message },
        } if message.len() > 512 => Err(BrowserUiProtocolValidationError::OversizeChatMessage),
        UiEnvelopePayload::Control {
            payload: UiControlPayload::HitRegionsChanged { regions, .. },
        } if regions.len() > MAX_UI_HIT_REGIONS => {
            Err(BrowserUiProtocolValidationError::TooManyHitRegions)
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::HitRegionsChanged { regions, .. },
        } if regions.iter().any(|region| {
            region.w <= 0
                || region.h <= 0
                || region.x < 0
                || region.y < 0
                || region.x.saturating_add(region.w) < region.x
                || region.y.saturating_add(region.h) < region.y
        }) =>
        {
            Err(BrowserUiProtocolValidationError::InvalidHitRegion)
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::HitRegionsChanged { regions, .. },
        } if has_duplicate_hit_region_ids(regions) => {
            Err(BrowserUiProtocolValidationError::DuplicateHitRegion)
        }
        UiEnvelopePayload::Control {
            payload:
                UiControlPayload::HostCommand {
                    request:
                        HostCommandRequest {
                            command_id,
                            payload,
                            size_budget,
                            ..
                        },
                },
        } if !is_valid_host_command_payload(command_id, payload, *size_budget) => {
            Err(BrowserUiProtocolValidationError::InvalidHostCommand)
        }
        UiEnvelopePayload::Control {
            payload:
                UiControlPayload::HostCommandResult {
                    command_id,
                    response,
                },
        } if !is_valid_host_command_result(command_id, response) => {
            Err(BrowserUiProtocolValidationError::InvalidHostCommand)
        }
        UiEnvelopePayload::Control {
            payload: UiControlPayload::HostEvent { event, payload },
        } if event.is_empty()
            || event.len() > 128
            || payload.len() > MAX_HOST_COMMAND_JSON_BYTES =>
        {
            Err(BrowserUiProtocolValidationError::InvalidHostCommand)
        }
        UiEnvelopePayload::StatePatch { patch } if patch.patches.is_empty() => {
            Err(BrowserUiProtocolValidationError::EmptyStatePatch)
        }
        UiEnvelopePayload::StatePatch { patch } if patch.target_revision <= patch.base_revision => {
            Err(BrowserUiProtocolValidationError::NonAdvancingStateRevision)
        }
        UiEnvelopePayload::ModelPatchBatch { batch } if batch.is_empty() => {
            Err(BrowserUiProtocolValidationError::EmptyStatePatch)
        }
        UiEnvelopePayload::Empty
        | UiEnvelopePayload::Control { .. }
        | UiEnvelopePayload::StatePatch { .. }
        | UiEnvelopePayload::ModelPatchBatch { .. }
        | UiEnvelopePayload::Error { .. }
        | UiEnvelopePayload::JsonBytes { .. } => Ok(()),
    }
}

fn has_duplicate_hit_region_ids(regions: &[BrowserUiHitRegion]) -> bool {
    for (index, region) in regions.iter().enumerate() {
        if regions
            .iter()
            .skip(index.saturating_add(1))
            .any(|other| other.id == region.id)
        {
            return true;
        }
    }
    false
}

fn validate_host_command_authority(
    envelope: &UiEnvelope,
    context: &BrowserUiProtocolValidationContext,
) -> Result<(), BrowserUiProtocolValidationError> {
    match &envelope.payload {
        UiEnvelopePayload::Control {
            payload: UiControlPayload::HostCommand { request },
        } => {
            if Some(request.request_id) != envelope.request_id {
                return Err(BrowserUiProtocolValidationError::UnexpectedRequestId);
            }
            let Some(required_capability) = request.command_id.required_capability() else {
                return Err(BrowserUiProtocolValidationError::InvalidHostCommand);
            };
            if required_capability != request.capability {
                return Err(BrowserUiProtocolValidationError::InvalidHostCommand);
            }
            if !context.grants_host(required_capability) {
                return Err(BrowserUiProtocolValidationError::MissingCapability);
            }
            Ok(())
        }
        UiEnvelopePayload::Empty
        | UiEnvelopePayload::Control { .. }
        | UiEnvelopePayload::StatePatch { .. }
        | UiEnvelopePayload::ModelPatchBatch { .. }
        | UiEnvelopePayload::Error { .. }
        | UiEnvelopePayload::JsonBytes { .. } => Ok(()),
    }
}

fn is_valid_host_command_payload(
    command_id: &HostCommandId,
    payload_json: &[u8],
    size_budget: BrowserUiSizeBudget,
) -> bool {
    is_valid_host_command_id(command_id.as_str())
        && command_id.target().is_some()
        && command_id.required_capability().is_some()
        && size_budget.max_bytes > 0
        && size_budget.max_bytes as usize <= MAX_HOST_COMMAND_JSON_BYTES
        && payload_json.len() <= size_budget.max_bytes as usize
        && payload_json.len() <= MAX_HOST_COMMAND_JSON_BYTES
}

fn is_valid_host_command_result(
    command_id: &HostCommandId,
    response: &HostCommandResponse,
) -> bool {
    if !is_valid_host_command_id(command_id.as_str()) {
        return false;
    }
    match response {
        HostCommandResponse::Ok {
            payload,
            diagnostics,
        } => payload.len() <= MAX_HOST_COMMAND_JSON_BYTES && is_valid_host_diagnostics(diagnostics),
        HostCommandResponse::Rejected { .. } => true,
        HostCommandResponse::Failed { error, diagnostics } => {
            is_valid_host_error(error) && is_valid_host_diagnostics(diagnostics)
        }
    }
}

fn is_valid_host_command_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_HOST_COMMAND_ID_BYTES
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'.' || byte == b'_'
        })
        && value.split('.').all(|segment| !segment.is_empty())
}

fn is_valid_host_error(error: &HostCommandError) -> bool {
    is_valid_host_command_id(&error.code)
        && !error.message.is_empty()
        && error.message.len() <= MAX_HOST_DIAGNOSTIC_TEXT_BYTES
}

fn is_valid_host_diagnostics(diagnostics: &[HostDiagnostic]) -> bool {
    diagnostics.len() <= MAX_HOST_DIAGNOSTICS
        && diagnostics.iter().all(|diagnostic| {
            is_valid_host_command_id(&diagnostic.code)
                && matches!(
                    diagnostic.level.as_str(),
                    "error" | "warning" | "info" | "trace"
                )
                && diagnostic.message.len() <= MAX_HOST_DIAGNOSTIC_TEXT_BYTES
        })
}

fn host_command_target(value: &str) -> Option<HostCommandTarget> {
    let first_segment = value.split('.').next()?;
    match first_segment {
        "game" | "games" => Some(HostCommandTarget::Game),
        "launcher" => Some(HostCommandTarget::Launcher),
        "editor" => Some(HostCommandTarget::Editor),
        "project" | "projects" => Some(HostCommandTarget::Project),
        "host" | "runtime" | "viewport" => Some(HostCommandTarget::Runtime),
        "preview" => Some(HostCommandTarget::Preview),
        "material" => Some(HostCommandTarget::Material),
        "entity" | "entity_stream" | "live_entity_stream" => Some(HostCommandTarget::Entity),
        "diagnostics" => Some(HostCommandTarget::Diagnostics),
        "account" | "auth" => Some(HostCommandTarget::Auth),
        "backend" => Some(HostCommandTarget::Backend),
        _ => None,
    }
}

fn host_command_required_capability(value: &str) -> Option<HostCapability> {
    match value {
        "host.commands.list" | "host.snapshot.get" | "launcher.state.get" | "launcher.show"
        | "launcher.hide" | "games.list" => Some(HostCapability::ReadLauncher),
        "games.join" => Some(HostCapability::JoinGame),
        "projects.authorized.list"
        | "project.current.get"
        | "projects.recent.list"
        | "project.bsn.index"
        | "project.picker.open" => Some(HostCapability::OpenProject),
        "project.open" | "project.edit.open" | "editor.activate" | "editor.deactivate" => {
            Some(HostCapability::EditProject)
        }
        "editor.overlay.toggle" => Some(HostCapability::EditProject),
        "editor.status.get" | "editor.commands.list" | "editor.events.list" => {
            Some(HostCapability::ReadLauncher)
        }
        "entity_stream.open"
        | "live_entity_stream.open"
        | "entity_stream.page"
        | "entity_stream.close"
        | "entity.details.get" => Some(HostCapability::ReadEntities),
        "entity.transform.patch" => Some(HostCapability::MutateEntities),
        "runtime.host.status"
        | "runtime.status.get"
        | "runtime.inspector.attach"
        | "runtime.server.launch"
        | "runtime.input.set_owner"
        | "viewport.client.launch"
        | "viewport.client.stop"
        | "viewport.client.focus"
        | "viewport.client.resize"
        | "preview.viewport.ensure"
        | "preview.viewport.stop"
        | "preview.renderer.ensure"
        | "preview.renderer.resize"
        | "preview.renderer.frame.get"
        | "preview.renderer.scene.set"
        | "preview.renderer.status.get"
        | "bevy.demo.launch" => Some(HostCapability::ControlRuntime),
        "runtime.diagnostics.list" | "diagnostics.list" => Some(HostCapability::ReadDiagnostics),
        "material.shader.list" | "material.shader.load" => Some(HostCapability::MaterialShaderRead),
        "material.shader.save" => Some(HostCapability::MaterialShaderWrite),
        "account.login"
        | "account.register"
        | "account.logout"
        | "auth.ticket.request"
        | "auth.account.ticket.request"
        | "auth.logout" => Some(HostCapability::RequestBackendTicket),
        "auth.session.get" | "backend.auth.session.get" | "auth.backend.session.get" => {
            Some(HostCapability::ReadLauncher)
        }
        "host.commandbar.execute" => Some(HostCapability::ControlRuntime),
        "window.minimize" | "window.maximize.toggle" | "window.hide" | "window.close" => {
            Some(HostCapability::UseDevTools)
        }
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostEnvelopeDelivery {
    pub entrypoint: &'static str,
    pub envelope: UiEnvelope,
}

impl HostEnvelopeDelivery {
    pub const ENTRYPOINT: &'static str = "window.fun.receiveFromHost";

    #[must_use]
    pub const fn new(envelope: UiEnvelope) -> Self {
        Self {
            entrypoint: Self::ENTRYPOINT,
            envelope,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserBridgeQueues {
    to_browser: VecDeque<BrowserUiPacket>,
    to_runtime: VecDeque<BrowserUiPacket>,
    host_to_js: VecDeque<UiEnvelope>,
    js_to_host: VecDeque<UiEnvelope>,
    max_queue_len: usize,
    accepting_messages: bool,
}

impl BrowserBridgeQueues {
    #[must_use]
    pub fn new() -> Self {
        Self {
            to_browser: VecDeque::new(),
            to_runtime: VecDeque::new(),
            host_to_js: VecDeque::new(),
            js_to_host: VecDeque::new(),
            max_queue_len: DEFAULT_MAX_QUEUE_LEN,
            accepting_messages: true,
        }
    }

    pub fn stop_accepting_messages(&mut self) {
        self.accepting_messages = false;
    }

    pub fn push_to_browser(&mut self, packet: BrowserUiPacket) -> Result<(), BrowserBridgeError> {
        if !self.accepting_messages {
            return Err(BrowserBridgeError::Closed);
        }
        if self.to_browser.len() >= self.max_queue_len {
            return Err(BrowserBridgeError::QueueFull);
        }
        self.to_browser.push_back(packet);
        Ok(())
    }

    pub fn push_to_runtime(&mut self, packet: BrowserUiPacket) -> Result<(), BrowserBridgeError> {
        if !self.accepting_messages {
            return Err(BrowserBridgeError::Closed);
        }
        if self.to_runtime.len() >= self.max_queue_len {
            return Err(BrowserBridgeError::QueueFull);
        }
        self.to_runtime.push_back(packet);
        Ok(())
    }

    pub fn push_host_envelope(&mut self, envelope: UiEnvelope) -> Result<(), BrowserBridgeError> {
        if !self.accepting_messages {
            return Err(BrowserBridgeError::Closed);
        }
        if self.host_to_js.len() >= self.max_queue_len {
            return Err(BrowserBridgeError::QueueFull);
        }
        self.host_to_js.push_back(envelope);
        Ok(())
    }

    pub fn push_js_envelope(&mut self, envelope: UiEnvelope) -> Result<(), BrowserBridgeError> {
        if !self.accepting_messages {
            return Err(BrowserBridgeError::Closed);
        }
        if self.js_to_host.len() >= self.max_queue_len {
            return Err(BrowserBridgeError::QueueFull);
        }
        self.js_to_host.push_back(envelope);
        Ok(())
    }

    pub fn pop_for_browser(&mut self) -> Option<BrowserUiPacket> {
        self.to_browser.pop_front()
    }

    pub fn pop_for_runtime(&mut self) -> Option<BrowserUiPacket> {
        self.to_runtime.pop_front()
    }

    pub fn pop_host_envelope_for_js(&mut self) -> Option<UiEnvelope> {
        self.host_to_js.pop_front()
    }

    pub fn pop_js_envelope_for_host(&mut self) -> Option<UiEnvelope> {
        self.js_to_host.pop_front()
    }
}

impl Default for BrowserBridgeQueues {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub struct SharedBrowserBridgeQueues {
    queues: Arc<Mutex<BrowserBridgeQueues>>,
}

impl SharedBrowserBridgeQueues {
    #[must_use]
    pub fn new() -> Self {
        Self {
            queues: Arc::new(Mutex::new(BrowserBridgeQueues::new())),
        }
    }

    pub fn push_js_envelope(&self, envelope: UiEnvelope) -> Result<(), BrowserBridgeError> {
        self.queues
            .lock()
            .map_err(|_| BrowserBridgeError::Closed)?
            .push_js_envelope(envelope)
    }

    pub fn push_host_envelope(&self, envelope: UiEnvelope) -> Result<(), BrowserBridgeError> {
        self.queues
            .lock()
            .map_err(|_| BrowserBridgeError::Closed)?
            .push_host_envelope(envelope)
    }

    pub fn pop_js_envelope_for_host(&self) -> Option<UiEnvelope> {
        self.queues
            .lock()
            .ok()
            .and_then(|mut queues| queues.pop_js_envelope_for_host())
    }

    pub fn pop_host_envelope_for_js(&self) -> Option<UiEnvelope> {
        self.queues
            .lock()
            .ok()
            .and_then(|mut queues| queues.pop_host_envelope_for_js())
    }
}

impl Default for SharedBrowserBridgeQueues {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserBridgeError {
    Closed,
    QueueFull,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_ui_packet_requires_negotiated_capability() {
        let mut context = BrowserUiProtocolValidationContext::local_game_ui();
        context
            .granted_capabilities
            .retain(|capability| *capability != BrowserUiCapability::DebugOverlay);
        let packet = BrowserUiPacket::BrowserToRuntime {
            header: BrowserUiPacketHeader::new(BrowserUiRequestId(1), BrowserUiSequence(1), None),
            payload: BrowserToRuntimePayload::DiagnosticsOverlaySet { visible: true },
        };

        assert_eq!(
            validate_browser_ui_packet(&packet, &context),
            Err(BrowserUiProtocolValidationError::MissingCapability)
        );
    }

    #[test]
    fn browser_ui_packet_rejects_stale_revision() {
        let context = BrowserUiProtocolValidationContext {
            revision: BrowserUiRevision(10),
            ..BrowserUiProtocolValidationContext::local_game_ui()
        };
        let packet = BrowserUiPacket::BrowserToRuntime {
            header: BrowserUiPacketHeader::new(
                BrowserUiRequestId(1),
                BrowserUiSequence(1),
                Some(BrowserUiRevision(9)),
            ),
            payload: BrowserToRuntimePayload::Ping,
        };

        assert_eq!(
            validate_browser_ui_packet(&packet, &context),
            Err(BrowserUiProtocolValidationError::StaleRevision)
        );
    }

    #[test]
    fn browser_ui_packet_rejects_oversize_chat() {
        let packet = BrowserUiPacket::BrowserToRuntime {
            header: BrowserUiPacketHeader::new(BrowserUiRequestId(1), BrowserUiSequence(1), None),
            payload: BrowserToRuntimePayload::ChatSubmit {
                message: "x".repeat(513),
            },
        };

        assert_eq!(
            validate_browser_ui_packet(
                &packet,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Err(BrowserUiProtocolValidationError::OversizeChatMessage)
        );
    }

    #[test]
    fn ui_envelope_enforces_request_ids() {
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Request,
            None,
            BrowserUiSequence(1),
            UiControlPayload::Ready,
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Err(BrowserUiProtocolValidationError::MissingRequestId)
        );
    }

    #[test]
    fn ui_envelope_keeps_state_patch_on_state_lane() {
        let envelope = UiEnvelope::state_patch(
            BrowserUiSequence(2),
            UiStatePatchPayload {
                base_revision: BrowserUiRevision(1),
                target_revision: BrowserUiRevision(2),
                patches: vec![UiStatePatch {
                    path: UiStatePath::HudHealth,
                    value: UiStateValue::U16 { value: 80 },
                }],
            },
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Ok(())
        );
    }

    #[test]
    fn ui_envelope_keeps_typed_model_patch_on_state_lane() {
        let mut writer = crate::model::UiPatchWriter::default();
        writer
            .set_u16(
                crate::model::GameUiChannel::Hud,
                crate::model::GameUiFieldKey::Health,
                80,
            )
            .expect("hud patch");
        let envelope = UiEnvelope::model_patch_batch(
            BrowserUiSequence(3),
            writer.finish_batch().expect("model patch batch"),
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Ok(())
        );
    }

    #[test]
    fn ui_envelope_rejects_empty_state_patch() {
        let envelope = UiEnvelope::state_patch(
            BrowserUiSequence(2),
            UiStatePatchPayload {
                base_revision: BrowserUiRevision(1),
                target_revision: BrowserUiRevision(2),
                patches: Vec::new(),
            },
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Err(BrowserUiProtocolValidationError::EmptyStatePatch)
        );
    }

    #[test]
    fn ui_envelope_rejects_invalid_hit_region() {
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Event,
            None,
            BrowserUiSequence(4),
            UiControlPayload::HitRegionsChanged {
                mode: BrowserUiHitRegionMode::HudPassive,
                regions: vec![BrowserUiHitRegion {
                    id: BrowserUiHitRegionId::Chat,
                    x: 0,
                    y: 0,
                    w: 0,
                    h: 24,
                }],
            },
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Err(BrowserUiProtocolValidationError::InvalidHitRegion)
        );
    }

    #[test]
    fn ui_envelope_rejects_duplicate_hit_region_ids() {
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Event,
            None,
            BrowserUiSequence(5),
            UiControlPayload::HitRegionsChanged {
                mode: BrowserUiHitRegionMode::HudPassive,
                regions: vec![
                    BrowserUiHitRegion {
                        id: BrowserUiHitRegionId::Chat,
                        x: 0,
                        y: 0,
                        w: 10,
                        h: 10,
                    },
                    BrowserUiHitRegion {
                        id: BrowserUiHitRegionId::Chat,
                        x: 20,
                        y: 0,
                        w: 10,
                        h: 10,
                    },
                ],
            },
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Err(BrowserUiProtocolValidationError::DuplicateHitRegion)
        );
    }

    #[test]
    fn ui_envelope_accepts_bounded_host_command_request() {
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Request,
            Some(BrowserUiRequestId(11)),
            BrowserUiSequence(6),
            UiControlPayload::HostCommand {
                request: HostCommandRequest::new(
                    BrowserUiRequestId(11),
                    "launcher.state.get",
                    b"{}".to_vec(),
                )
                .expect("host command request"),
            },
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Ok(())
        );
    }

    #[test]
    fn ui_envelope_rejects_oversize_host_command_payload() {
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Request,
            Some(BrowserUiRequestId(12)),
            BrowserUiSequence(7),
            UiControlPayload::HostCommand {
                request: HostCommandRequest::new(
                    BrowserUiRequestId(12),
                    "launcher.state.get",
                    vec![0; MAX_HOST_COMMAND_JSON_BYTES.saturating_add(1)],
                )
                .expect("host command request"),
            },
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Err(BrowserUiProtocolValidationError::InvalidHostCommand)
        );
    }

    #[test]
    fn ui_envelope_rejects_host_command_without_negotiated_capability() {
        let mut context = BrowserUiProtocolValidationContext::local_game_ui();
        context
            .granted_host_capabilities
            .retain(|capability| *capability != HostCapability::RequestBackendTicket);
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Request,
            Some(BrowserUiRequestId(13)),
            BrowserUiSequence(8),
            UiControlPayload::HostCommand {
                request: HostCommandRequest::new(
                    BrowserUiRequestId(13),
                    "auth.ticket.request",
                    b"{}".to_vec(),
                )
                .expect("host command request"),
            },
        );

        assert_eq!(
            validate_ui_envelope(&envelope, &context),
            Err(BrowserUiProtocolValidationError::MissingCapability)
        );
    }

    #[test]
    fn ui_envelope_rejects_host_command_capability_spoofing() {
        let mut request = HostCommandRequest::new(
            BrowserUiRequestId(14),
            "auth.ticket.request",
            b"{}".to_vec(),
        )
        .expect("host command request");
        request.capability = HostCapability::ReadLauncher;
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Request,
            Some(BrowserUiRequestId(14)),
            BrowserUiSequence(9),
            UiControlPayload::HostCommand { request },
        );

        assert_eq!(
            validate_ui_envelope(
                &envelope,
                &BrowserUiProtocolValidationContext::local_game_ui()
            ),
            Err(BrowserUiProtocolValidationError::InvalidHostCommand)
        );
    }

    #[test]
    fn bridge_queues_separate_js_and_host_envelopes() {
        let mut queues = BrowserBridgeQueues::new();
        let envelope = UiEnvelope::control(
            UiEnvelopeKind::Event,
            None,
            BrowserUiSequence(1),
            UiControlPayload::Ready,
        );

        queues
            .push_js_envelope(envelope.clone())
            .expect("queue js envelope");

        assert_eq!(queues.pop_js_envelope_for_host(), Some(envelope));
        assert_eq!(queues.pop_host_envelope_for_js(), None);
    }
}
