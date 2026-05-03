//! Shared browser/game UI request protocol.
//!
//! Browser UI is presentation-only. These packets describe requests that Rust
//! may accept, reject, or translate into authoritative game actions.

const GAME_UI_PROTOCOL_VERSION_VALUE: u32 = 1;
const GAME_UI_SCHEMA_REVISION_VALUE: u32 = 1;
const DEFAULT_GAME_UI_MAX_PAYLOAD_BYTES: u32 = 64 * 1024;
const MAX_CHAT_MESSAGE_BYTES: usize = 512;
const MAX_SETTING_TEXT_BYTES: usize = 256;

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct GameUiProtocolVersion(pub u32);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct GameUiSchemaRevision(pub u32);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct GameUiRequestId(pub u64);

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode,
)]
pub struct GameUiSequence(pub u64);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct GameUiSizeBudget {
    pub max_bytes: u32,
}

pub const CURRENT_GAME_UI_PROTOCOL_VERSION: GameUiProtocolVersion =
    GameUiProtocolVersion(GAME_UI_PROTOCOL_VERSION_VALUE);
pub const CURRENT_GAME_UI_SCHEMA_REVISION: GameUiSchemaRevision =
    GameUiSchemaRevision(GAME_UI_SCHEMA_REVISION_VALUE);
pub const DEFAULT_GAME_UI_SIZE_BUDGET: GameUiSizeBudget = GameUiSizeBudget {
    max_bytes: DEFAULT_GAME_UI_MAX_PAYLOAD_BYTES,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum GameUiCapability {
    ReadHudState,
    ReadMatchState,
    ReadDiagnostics,
    RequestSpawn,
    ChangeSettings,
    SendChat,
    OpenMenu,
    UseDevTools,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum GameUiMenuTarget {
    Pause,
    Settings,
    Loadout,
    Scoreboard,
    Chat,
    Diagnostics,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum GameUiSettingKey {
    MouseSensitivity,
    MasterVolume,
    DisplayMode,
    DiagnosticsVisible,
}

impl GameUiSettingKey {
    #[must_use]
    pub fn from_wire_key(value: &str) -> Option<Self> {
        match value {
            "mouse_sensitivity" => Some(Self::MouseSensitivity),
            "master_volume" => Some(Self::MasterVolume),
            "display_mode" => Some(Self::DisplayMode),
            "diagnostics_visible" => Some(Self::DiagnosticsVisible),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum GameUiSettingValue {
    Bool { value: bool },
    U32 { value: u32 },
    Text { value: String },
    JsonBytes { bytes: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum GameUiRequestPayload {
    JoinTeam {
        team_id: u16,
    },
    SelectLoadout {
        loadout_id: u32,
    },
    SendChat {
        message: String,
    },
    OpenMenu {
        target: GameUiMenuTarget,
    },
    ChangeSetting {
        key: GameUiSettingKey,
        value: GameUiSettingValue,
    },
    RequestRespawn,
    UseDevTools,
}

impl GameUiRequestPayload {
    #[must_use]
    pub const fn required_capability(&self) -> GameUiCapability {
        match self {
            Self::JoinTeam { .. } | Self::RequestRespawn => GameUiCapability::RequestSpawn,
            Self::SelectLoadout { .. } => GameUiCapability::RequestSpawn,
            Self::SendChat { .. } => GameUiCapability::SendChat,
            Self::OpenMenu { .. } => GameUiCapability::OpenMenu,
            Self::ChangeSetting { .. } => GameUiCapability::ChangeSettings,
            Self::UseDevTools => GameUiCapability::UseDevTools,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct GameUiRequestEnvelope {
    pub protocol_version: GameUiProtocolVersion,
    pub schema_revision: GameUiSchemaRevision,
    pub request_id: GameUiRequestId,
    pub sequence: GameUiSequence,
    pub size_budget: GameUiSizeBudget,
    pub required_capability: GameUiCapability,
    pub payload: GameUiRequestPayload,
}

impl GameUiRequestEnvelope {
    #[must_use]
    pub fn new(
        request_id: GameUiRequestId,
        sequence: GameUiSequence,
        payload: GameUiRequestPayload,
    ) -> Self {
        let required_capability = payload.required_capability();
        Self {
            protocol_version: CURRENT_GAME_UI_PROTOCOL_VERSION,
            schema_revision: CURRENT_GAME_UI_SCHEMA_REVISION,
            request_id,
            sequence,
            size_budget: DEFAULT_GAME_UI_SIZE_BUDGET,
            required_capability,
            payload,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameUiProtocolValidationContext {
    pub protocol_version: GameUiProtocolVersion,
    pub schema_revision: GameUiSchemaRevision,
    pub granted_capabilities: Vec<GameUiCapability>,
    pub max_payload_bytes: u32,
    pub devtools_enabled: bool,
}

impl GameUiProtocolValidationContext {
    #[must_use]
    pub fn local_game_client(devtools_enabled: bool) -> Self {
        let mut granted_capabilities = vec![
            GameUiCapability::ReadHudState,
            GameUiCapability::ReadMatchState,
            GameUiCapability::RequestSpawn,
            GameUiCapability::ChangeSettings,
            GameUiCapability::SendChat,
            GameUiCapability::OpenMenu,
        ];
        if cfg!(debug_assertions) {
            granted_capabilities.push(GameUiCapability::ReadDiagnostics);
        }
        if devtools_enabled {
            granted_capabilities.push(GameUiCapability::UseDevTools);
        }
        Self {
            protocol_version: CURRENT_GAME_UI_PROTOCOL_VERSION,
            schema_revision: CURRENT_GAME_UI_SCHEMA_REVISION,
            granted_capabilities,
            max_payload_bytes: DEFAULT_GAME_UI_MAX_PAYLOAD_BYTES,
            devtools_enabled,
        }
    }

    #[must_use]
    pub fn grants(&self, capability: GameUiCapability) -> bool {
        self.granted_capabilities.contains(&capability)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameUiRequestRejectionReason {
    StaleProtocolVersion,
    StaleSchemaRevision,
    MissingCapability,
    OversizePayload,
    InvalidPayload,
    DevToolsDisabled,
}

pub fn validate_game_ui_request(
    request: &GameUiRequestEnvelope,
    context: &GameUiProtocolValidationContext,
) -> Result<(), GameUiRequestRejectionReason> {
    if request.protocol_version != context.protocol_version {
        return Err(GameUiRequestRejectionReason::StaleProtocolVersion);
    }
    if request.schema_revision != context.schema_revision {
        return Err(GameUiRequestRejectionReason::StaleSchemaRevision);
    }
    validate_game_ui_request_budget(request, context)?;
    if !context.grants(request.required_capability) {
        return Err(GameUiRequestRejectionReason::MissingCapability);
    }
    validate_game_ui_request_payload(request, context)
}

fn validate_game_ui_request_budget(
    request: &GameUiRequestEnvelope,
    context: &GameUiProtocolValidationContext,
) -> Result<(), GameUiRequestRejectionReason> {
    let payload_len = compactly::v1::encode(request).len();
    let sender_budget = request.size_budget.max_bytes as usize;
    let runtime_budget = context.max_payload_bytes as usize;
    if payload_len > sender_budget || payload_len > runtime_budget {
        Err(GameUiRequestRejectionReason::OversizePayload)
    } else {
        Ok(())
    }
}

fn validate_game_ui_request_payload(
    request: &GameUiRequestEnvelope,
    context: &GameUiProtocolValidationContext,
) -> Result<(), GameUiRequestRejectionReason> {
    match &request.payload {
        GameUiRequestPayload::SendChat { message } if message.len() > MAX_CHAT_MESSAGE_BYTES => {
            Err(GameUiRequestRejectionReason::InvalidPayload)
        }
        GameUiRequestPayload::ChangeSetting { value, .. } => validate_setting_value(value),
        GameUiRequestPayload::UseDevTools if !context.devtools_enabled => {
            Err(GameUiRequestRejectionReason::DevToolsDisabled)
        }
        GameUiRequestPayload::JoinTeam { .. }
        | GameUiRequestPayload::SelectLoadout { .. }
        | GameUiRequestPayload::SendChat { .. }
        | GameUiRequestPayload::OpenMenu { .. }
        | GameUiRequestPayload::RequestRespawn
        | GameUiRequestPayload::UseDevTools => Ok(()),
    }
}

fn validate_setting_value(value: &GameUiSettingValue) -> Result<(), GameUiRequestRejectionReason> {
    match value {
        GameUiSettingValue::Text { value } if value.len() > MAX_SETTING_TEXT_BYTES => {
            Err(GameUiRequestRejectionReason::InvalidPayload)
        }
        GameUiSettingValue::JsonBytes { bytes } if bytes.len() > MAX_SETTING_TEXT_BYTES => {
            Err(GameUiRequestRejectionReason::InvalidPayload)
        }
        GameUiSettingValue::Bool { .. }
        | GameUiSettingValue::U32 { .. }
        | GameUiSettingValue::Text { .. }
        | GameUiSettingValue::JsonBytes { .. } => Ok(()),
    }
}

#[must_use]
pub const fn game_ui_capability_name(capability: GameUiCapability) -> &'static str {
    match capability {
        GameUiCapability::ReadHudState => "read_hud_state",
        GameUiCapability::ReadMatchState => "read_match_state",
        GameUiCapability::ReadDiagnostics => "read_diagnostics",
        GameUiCapability::RequestSpawn => "request_spawn",
        GameUiCapability::ChangeSettings => "change_settings",
        GameUiCapability::SendChat => "send_chat",
        GameUiCapability::OpenMenu => "open_menu",
        GameUiCapability::UseDevTools => "use_devtools",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(payload: GameUiRequestPayload) -> GameUiRequestEnvelope {
        GameUiRequestEnvelope::new(GameUiRequestId(7), GameUiSequence(9), payload)
    }

    #[test]
    fn validates_capability_gated_chat_request() {
        let context = GameUiProtocolValidationContext::local_game_client(false);
        let request = request(GameUiRequestPayload::SendChat {
            message: "hello".to_owned(),
        });

        assert_eq!(validate_game_ui_request(&request, &context), Ok(()));
    }

    #[test]
    fn rejects_missing_capability() {
        let mut context = GameUiProtocolValidationContext::local_game_client(false);
        context
            .granted_capabilities
            .retain(|capability| *capability != GameUiCapability::SendChat);
        let request = request(GameUiRequestPayload::SendChat {
            message: "hello".to_owned(),
        });

        assert_eq!(
            validate_game_ui_request(&request, &context),
            Err(GameUiRequestRejectionReason::MissingCapability)
        );
    }

    #[test]
    fn rejects_oversized_chat() {
        let context = GameUiProtocolValidationContext::local_game_client(false);
        let request = request(GameUiRequestPayload::SendChat {
            message: "x".repeat(MAX_CHAT_MESSAGE_BYTES + 1),
        });

        assert_eq!(
            validate_game_ui_request(&request, &context),
            Err(GameUiRequestRejectionReason::InvalidPayload)
        );
    }

    #[test]
    fn rejects_devtools_when_disabled() {
        let context = GameUiProtocolValidationContext::local_game_client(false);
        let request = request(GameUiRequestPayload::UseDevTools);

        assert_eq!(
            validate_game_ui_request(&request, &context),
            Err(GameUiRequestRejectionReason::MissingCapability)
        );
    }
}
