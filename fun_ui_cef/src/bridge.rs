use std::collections::VecDeque;

const BROWSER_UI_PROTOCOL_VERSION: u32 = 1;
const DEFAULT_MAX_PACKET_BYTES: u32 = 64 * 1024;
const DEFAULT_MAX_QUEUE_LEN: usize = 256;

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
pub struct BrowserUiRevision(pub u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct BrowserUiSizeBudget {
    pub max_bytes: u32,
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
    Menu,
    Scoreboard,
    Chat,
    Loading,
    Diagnostics,
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
            ],
            revision: BrowserUiRevision(0),
            max_payload_bytes: DEFAULT_MAX_PACKET_BYTES,
        }
    }

    #[must_use]
    pub fn grants(&self, capability: BrowserUiCapability) -> bool {
        self.granted_capabilities.contains(&capability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrowserUiProtocolValidationError {
    StaleVersion,
    MissingCapability,
    StaleRevision,
    OversizePayload,
    OversizeChatMessage,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserBridgeQueues {
    to_browser: VecDeque<BrowserUiPacket>,
    to_runtime: VecDeque<BrowserUiPacket>,
    max_queue_len: usize,
    accepting_messages: bool,
}

impl BrowserBridgeQueues {
    #[must_use]
    pub fn new() -> Self {
        Self {
            to_browser: VecDeque::new(),
            to_runtime: VecDeque::new(),
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

    pub fn pop_for_browser(&mut self) -> Option<BrowserUiPacket> {
        self.to_browser.pop_front()
    }

    pub fn pop_for_runtime(&mut self) -> Option<BrowserUiPacket> {
        self.to_runtime.pop_front()
    }
}

impl Default for BrowserBridgeQueues {
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
}
