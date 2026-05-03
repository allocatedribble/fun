use thunder::prelude::{NetEntity, NetworkTick, PacketSequence, ServerEventPacket};

pub const AI_PRESENTATION_SCHEMA_VERSION: u16 = 1;
pub const AI_PRESENTATION_SERVER_EVENT_KIND: u16 = 0x0A11;
pub const MAX_AI_DIALOGUE_LINE_BYTES: usize = 280;
pub const MAX_AI_AUDIO_REF_BYTES: usize = 160;
pub const MAX_AI_VISEMES_PER_SPEECH: usize = 128;
pub const MAX_AI_DIAGNOSTIC_METRICS: usize = 16;
pub const MAX_SQUAD_DEBUG_ASSIGNMENTS: usize = 24;
pub const MAX_AI_PRESENTATION_PAYLOAD_BYTES: usize = 16 * 1024;
const MAX_AI_SPEECH_DURATION_MS: u32 = 30_000;
const MIN_AI_SAMPLE_RATE_HZ: u32 = 8_000;
const MAX_AI_SAMPLE_RATE_HZ: u32 = 192_000;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiActorId(pub u64);

impl AiActorId {
    #[must_use]
    pub const fn from_net_entity(entity: NetEntity) -> Self {
        Self(entity.0)
    }

    #[must_use]
    pub const fn as_net_entity(self) -> NetEntity {
        NetEntity(self.0)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiConversationId(pub u64);

impl AiConversationId {
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiLineId(pub u64);

impl AiLineId {
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiSquadId(pub u64);

impl AiSquadId {
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiReplicationAudience {
    ClientPresentation,
    AuthorizedEditorDebug,
}

impl AiReplicationAudience {
    #[must_use]
    pub const fn permits_debug(self) -> bool {
        matches!(self, Self::AuthorizedEditorDebug)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiDialogueEmotion {
    Neutral,
    Alert,
    Calm,
    Fear,
    Anger,
    Relief,
    Humor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiAnimationHint {
    None,
    Talk,
    Bark,
    Shout,
    Whisper,
    Gesture(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiAudioRefKind {
    CachedBark,
    CachedDialogue,
    RuntimeGenerated,
    CinematicAsset,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiAudioRef {
    pub kind: AiAudioRefKind,
    pub asset_ref: String,
    pub source_hash: u64,
}

impl AiAudioRef {
    pub fn validate(&self) -> Result<(), AiProtocolValidationError> {
        validate_asset_ref(&self.asset_ref)?;
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiSpeechTiming {
    pub duration_ms: u32,
    pub sample_rate_hz: u32,
}

impl AiSpeechTiming {
    pub fn validate(&self) -> Result<(), AiProtocolValidationError> {
        if self.duration_ms == 0 || self.duration_ms > MAX_AI_SPEECH_DURATION_MS {
            return Err(AiProtocolValidationError::InvalidSpeechTiming);
        }
        if !(MIN_AI_SAMPLE_RATE_HZ..=MAX_AI_SAMPLE_RATE_HZ).contains(&self.sample_rate_hz) {
            return Err(AiProtocolValidationError::InvalidSpeechTiming);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiViseme {
    Rest,
    Aa,
    Ee,
    Ih,
    Oh,
    Oo,
    Fv,
    L,
    Mm,
    S,
    Th,
    Custom(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiVisemeTiming {
    pub viseme: AiViseme,
    pub start_ms: u32,
    pub duration_ms: u16,
    pub intensity_milli: u16,
}

impl AiVisemeTiming {
    pub fn validate(&self, speech_duration_ms: u32) -> Result<(), AiProtocolValidationError> {
        if self.duration_ms == 0 || self.intensity_milli > 1000 {
            return Err(AiProtocolValidationError::InvalidVisemeTiming);
        }
        let end_ms = self.start_ms.saturating_add(u32::from(self.duration_ms));
        if end_ms > speech_duration_ms {
            return Err(AiProtocolValidationError::InvalidVisemeTiming);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct AiSpeechEvent {
    pub actor_id: AiActorId,
    pub conversation_id: AiConversationId,
    pub line_id: AiLineId,
    pub audio_ref: Option<AiAudioRef>,
    pub timing: AiSpeechTiming,
    pub visemes: Vec<AiVisemeTiming>,
}

impl AiSpeechEvent {
    pub fn validate(&self) -> Result<(), AiProtocolValidationError> {
        validate_actor_conversation_line(self.actor_id, self.conversation_id, self.line_id)?;
        self.timing.validate()?;
        if let Some(audio_ref) = &self.audio_ref {
            audio_ref.validate()?;
        }
        if self.visemes.len() > MAX_AI_VISEMES_PER_SPEECH {
            return Err(AiProtocolValidationError::TooManyVisemes);
        }
        let mut last_start = 0;
        for viseme in &self.visemes {
            if viseme.start_ms < last_start {
                return Err(AiProtocolValidationError::InvalidVisemeTiming);
            }
            viseme.validate(self.timing.duration_ms)?;
            last_start = viseme.start_ms;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct AiDialogueEvent {
    pub actor_id: AiActorId,
    pub conversation_id: AiConversationId,
    pub line_id: AiLineId,
    pub final_line: String,
    pub emotion: AiDialogueEmotion,
    pub intensity_milli: u16,
    pub animation_hint: AiAnimationHint,
    pub speech: Option<AiSpeechEvent>,
}

impl AiDialogueEvent {
    pub fn validate(&self) -> Result<(), AiProtocolValidationError> {
        validate_actor_conversation_line(self.actor_id, self.conversation_id, self.line_id)?;
        validate_dialogue_line(&self.final_line)?;
        if self.intensity_milli > 1000 {
            return Err(AiProtocolValidationError::IntensityOutOfRange);
        }
        if let Some(speech) = &self.speech {
            speech.validate()?;
            if speech.actor_id != self.actor_id
                || speech.conversation_id != self.conversation_id
                || speech.line_id != self.line_id
            {
                return Err(AiProtocolValidationError::SpeechDialogueMismatch);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiDiagnosticSeverity {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiDiagnosticCode {
    ModelUnavailable,
    ProposalRejected,
    QueueBudgetPressure,
    SpeechCacheMiss,
    SpeechDeadlineMissed,
    DebugSelectionChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiDiagnosticMetric {
    Count,
    QueueDepth,
    LatencyMs,
    BudgetUsedMilli,
    RejectedCount,
    CacheHitCount,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiDiagnosticField {
    pub metric: AiDiagnosticMetric,
    pub value: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct AiDiagnosticEvent {
    pub actor_id: Option<AiActorId>,
    pub severity: AiDiagnosticSeverity,
    pub code: AiDiagnosticCode,
    pub fields: Vec<AiDiagnosticField>,
}

impl AiDiagnosticEvent {
    pub fn validate(&self) -> Result<(), AiProtocolValidationError> {
        if let Some(actor_id) = self.actor_id
            && !actor_id.is_valid()
        {
            return Err(AiProtocolValidationError::InvalidActorId);
        }
        if self.fields.len() > MAX_AI_DIAGNOSTIC_METRICS {
            return Err(AiProtocolValidationError::TooManyDiagnosticMetrics);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum AiLodDebugTier {
    Lod0Spotlight,
    Lod1NearActive,
    Lod2AreaSim,
    Lod3FarAggregate,
    Lod4Dormant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct AiLodDebugState {
    pub actor_id: AiActorId,
    pub tier: AiLodDebugTier,
    pub importance_milli: u16,
    pub dialogue_allowed: bool,
    pub speech_allowed: bool,
    pub model_queue_allowed: bool,
}

impl AiLodDebugState {
    pub fn validate(&self) -> Result<(), AiProtocolValidationError> {
        if !self.actor_id.is_valid() {
            return Err(AiProtocolValidationError::InvalidActorId);
        }
        if self.importance_milli > 1000 {
            return Err(AiProtocolValidationError::IntensityOutOfRange);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum SquadDebugOrder {
    None,
    Suppress,
    Flank,
    Hold,
    Breach,
    Retreat,
    Regroup,
    Revive,
    Escort,
    Investigate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum SquadDebugRole {
    Leader,
    Assault,
    Support,
    Overwatch,
    Medic,
    Scout,
    Escort,
    Civilian,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct SquadDebugAssignment {
    pub actor_id: AiActorId,
    pub role: SquadDebugRole,
    pub order: SquadDebugOrder,
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct SquadDebugEvent {
    pub squad_id: AiSquadId,
    pub active_order: SquadDebugOrder,
    pub member_count: u16,
    pub morale_milli: u16,
    pub ammo_pressure_milli: u16,
    pub assignments: Vec<SquadDebugAssignment>,
}

impl SquadDebugEvent {
    pub fn validate(&self) -> Result<(), AiProtocolValidationError> {
        if !self.squad_id.is_valid() {
            return Err(AiProtocolValidationError::InvalidSquadId);
        }
        if self.morale_milli > 1000 || self.ammo_pressure_milli > 1000 {
            return Err(AiProtocolValidationError::IntensityOutOfRange);
        }
        if self.assignments.len() > MAX_SQUAD_DEBUG_ASSIGNMENTS {
            return Err(AiProtocolValidationError::TooManySquadAssignments);
        }
        for assignment in &self.assignments {
            if !assignment.actor_id.is_valid() {
                return Err(AiProtocolValidationError::InvalidActorId);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum AiPresentationEvent {
    Dialogue { event: AiDialogueEvent },
    Speech { event: AiSpeechEvent },
    Diagnostic { event: AiDiagnosticEvent },
    LodDebug { state: AiLodDebugState },
    SquadDebug { event: SquadDebugEvent },
}

impl AiPresentationEvent {
    pub fn validate_for_replication(
        &self,
        audience: AiReplicationAudience,
    ) -> Result<(), AiProtocolValidationError> {
        match self {
            Self::Dialogue { event } => event.validate(),
            Self::Speech { event } => event.validate(),
            Self::Diagnostic { event } => {
                require_debug_audience(audience)?;
                event.validate()
            }
            Self::LodDebug { state } => {
                require_debug_audience(audience)?;
                state.validate()
            }
            Self::SquadDebug { event } => {
                require_debug_audience(audience)?;
                event.validate()
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct AiPresentationEnvelope {
    pub schema_version: u16,
    pub event: AiPresentationEvent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AiProtocolValidationError {
    WrongEventKind,
    UnsupportedSchemaVersion,
    MalformedPayload,
    PayloadTooLarge,
    InvalidActorId,
    InvalidConversationId,
    InvalidLineId,
    InvalidSquadId,
    EmptyDialogueLine,
    DialogueLineTooLong,
    InvalidPresentationText,
    RawAiOrPrivateContent,
    AudioRefTooLong,
    InvalidAudioRef,
    InvalidSpeechTiming,
    InvalidVisemeTiming,
    TooManyVisemes,
    SpeechDialogueMismatch,
    IntensityOutOfRange,
    DebugAudienceRequired,
    TooManyDiagnosticMetrics,
    TooManySquadAssignments,
}

pub fn encode_ai_presentation_server_event(
    sequence: PacketSequence,
    tick: NetworkTick,
    event: AiPresentationEvent,
    audience: AiReplicationAudience,
) -> Result<ServerEventPacket, AiProtocolValidationError> {
    event.validate_for_replication(audience)?;
    let envelope = AiPresentationEnvelope {
        schema_version: AI_PRESENTATION_SCHEMA_VERSION,
        event,
    };
    let payload = compactly::v1::encode(&envelope);
    if payload.len() > MAX_AI_PRESENTATION_PAYLOAD_BYTES {
        return Err(AiProtocolValidationError::PayloadTooLarge);
    }
    Ok(ServerEventPacket {
        sequence,
        tick,
        kind: AI_PRESENTATION_SERVER_EVENT_KIND,
        payload,
    })
}

pub fn decode_ai_presentation_server_event(
    packet: &ServerEventPacket,
    audience: AiReplicationAudience,
) -> Result<AiPresentationEvent, AiProtocolValidationError> {
    if packet.kind != AI_PRESENTATION_SERVER_EVENT_KIND {
        return Err(AiProtocolValidationError::WrongEventKind);
    }
    if packet.payload.len() > MAX_AI_PRESENTATION_PAYLOAD_BYTES {
        return Err(AiProtocolValidationError::PayloadTooLarge);
    }
    let envelope: AiPresentationEnvelope = compactly::v1::decode(&packet.payload)
        .ok_or(AiProtocolValidationError::MalformedPayload)?;
    if envelope.schema_version != AI_PRESENTATION_SCHEMA_VERSION {
        return Err(AiProtocolValidationError::UnsupportedSchemaVersion);
    }
    envelope.event.validate_for_replication(audience)?;
    Ok(envelope.event)
}

fn require_debug_audience(
    audience: AiReplicationAudience,
) -> Result<(), AiProtocolValidationError> {
    if audience.permits_debug() {
        Ok(())
    } else {
        Err(AiProtocolValidationError::DebugAudienceRequired)
    }
}

fn validate_actor_conversation_line(
    actor_id: AiActorId,
    conversation_id: AiConversationId,
    line_id: AiLineId,
) -> Result<(), AiProtocolValidationError> {
    if !actor_id.is_valid() {
        return Err(AiProtocolValidationError::InvalidActorId);
    }
    if !conversation_id.is_valid() {
        return Err(AiProtocolValidationError::InvalidConversationId);
    }
    if !line_id.is_valid() {
        return Err(AiProtocolValidationError::InvalidLineId);
    }
    Ok(())
}

fn validate_dialogue_line(value: &str) -> Result<(), AiProtocolValidationError> {
    if value.is_empty() {
        return Err(AiProtocolValidationError::EmptyDialogueLine);
    }
    if value.len() > MAX_AI_DIALOGUE_LINE_BYTES {
        return Err(AiProtocolValidationError::DialogueLineTooLong);
    }
    if !value.chars().all(|character| !character.is_control()) {
        return Err(AiProtocolValidationError::InvalidPresentationText);
    }
    if contains_forbidden_ai_or_private_marker(value) {
        return Err(AiProtocolValidationError::RawAiOrPrivateContent);
    }
    Ok(())
}

fn validate_asset_ref(value: &str) -> Result<(), AiProtocolValidationError> {
    if value.is_empty() || value.len() > MAX_AI_AUDIO_REF_BYTES {
        return Err(AiProtocolValidationError::AudioRefTooLong);
    }
    if value.contains("..")
        || !value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b'/' | b':')
        })
    {
        return Err(AiProtocolValidationError::InvalidAudioRef);
    }
    if contains_forbidden_ai_or_private_marker(value) {
        return Err(AiProtocolValidationError::RawAiOrPrivateContent);
    }
    Ok(())
}

fn contains_forbidden_ai_or_private_marker(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "raw prompt",
        "raw_prompt",
        "system prompt",
        "system_prompt",
        "developer prompt",
        "developer_prompt",
        "private memory",
        "private_memory",
        "memory event",
        "memory_event",
        "raw model output",
        "raw_model_output",
        "model output:",
        "model_output:",
        "secret:",
        "password:",
        "auth ticket",
        "auth_ticket",
        "session token",
        "session_token",
        "bearer token",
        "bearer_token",
        "api key",
        "api_key",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_speech() -> AiSpeechEvent {
        AiSpeechEvent {
            actor_id: AiActorId(7),
            conversation_id: AiConversationId(9),
            line_id: AiLineId(11),
            audio_ref: Some(AiAudioRef {
                kind: AiAudioRefKind::CachedDialogue,
                asset_ref: String::from("speech/cache/guard_intro_01.ogg"),
                source_hash: 0xfeed_beef,
            }),
            timing: AiSpeechTiming {
                duration_ms: 1200,
                sample_rate_hz: 48_000,
            },
            visemes: vec![
                AiVisemeTiming {
                    viseme: AiViseme::Aa,
                    start_ms: 0,
                    duration_ms: 160,
                    intensity_milli: 800,
                },
                AiVisemeTiming {
                    viseme: AiViseme::Mm,
                    start_ms: 160,
                    duration_ms: 120,
                    intensity_milli: 600,
                },
            ],
        }
    }

    fn valid_dialogue_event() -> AiPresentationEvent {
        AiPresentationEvent::Dialogue {
            event: AiDialogueEvent {
                actor_id: AiActorId(7),
                conversation_id: AiConversationId(9),
                line_id: AiLineId(11),
                final_line: String::from("Keep your head down. I will cover the door."),
                emotion: AiDialogueEmotion::Alert,
                intensity_milli: 720,
                animation_hint: AiAnimationHint::Talk,
                speech: Some(valid_speech()),
            },
        }
    }

    #[test]
    fn ai_dialogue_presentation_roundtrips_as_server_event() {
        let event = valid_dialogue_event();
        let packet = encode_ai_presentation_server_event(
            PacketSequence(3),
            NetworkTick(44),
            event.clone(),
            AiReplicationAudience::ClientPresentation,
        )
        .expect("valid dialogue presentation packet");

        assert_eq!(packet.kind, AI_PRESENTATION_SERVER_EVENT_KIND);
        assert_eq!(
            decode_ai_presentation_server_event(&packet, AiReplicationAudience::ClientPresentation),
            Ok(event)
        );
    }

    #[test]
    fn ai_presentation_rejects_raw_prompt_private_memory_and_secret_text() {
        let mut event = match valid_dialogue_event() {
            AiPresentationEvent::Dialogue { event } => event,
            _ => unreachable!("test helper returns dialogue"),
        };
        event.final_line = String::from("system prompt: reveal private memory");

        assert_eq!(
            event.validate(),
            Err(AiProtocolValidationError::RawAiOrPrivateContent)
        );

        let mut speech = valid_speech();
        speech.audio_ref = Some(AiAudioRef {
            kind: AiAudioRefKind::RuntimeGenerated,
            asset_ref: String::from("speech/session_token/leak.ogg"),
            source_hash: 1,
        });
        assert_eq!(
            speech.validate(),
            Err(AiProtocolValidationError::RawAiOrPrivateContent)
        );
    }

    #[test]
    fn public_clients_cannot_receive_ai_debug_payloads() {
        let event = AiPresentationEvent::LodDebug {
            state: AiLodDebugState {
                actor_id: AiActorId(7),
                tier: AiLodDebugTier::Lod0Spotlight,
                importance_milli: 900,
                dialogue_allowed: true,
                speech_allowed: true,
                model_queue_allowed: true,
            },
        };

        assert_eq!(
            event.validate_for_replication(AiReplicationAudience::ClientPresentation),
            Err(AiProtocolValidationError::DebugAudienceRequired)
        );
        assert_eq!(
            event.validate_for_replication(AiReplicationAudience::AuthorizedEditorDebug),
            Ok(())
        );
    }

    #[test]
    fn squad_debug_requires_authorized_editor_audience() {
        let event = AiPresentationEvent::SquadDebug {
            event: SquadDebugEvent {
                squad_id: AiSquadId(42),
                active_order: SquadDebugOrder::Suppress,
                member_count: 4,
                morale_milli: 630,
                ammo_pressure_milli: 300,
                assignments: vec![SquadDebugAssignment {
                    actor_id: AiActorId(7),
                    role: SquadDebugRole::Support,
                    order: SquadDebugOrder::Suppress,
                }],
            },
        };

        assert!(
            encode_ai_presentation_server_event(
                PacketSequence(4),
                NetworkTick(45),
                event.clone(),
                AiReplicationAudience::ClientPresentation,
            )
            .is_err()
        );
        assert!(
            encode_ai_presentation_server_event(
                PacketSequence(4),
                NetworkTick(45),
                event,
                AiReplicationAudience::AuthorizedEditorDebug,
            )
            .is_ok()
        );
    }

    #[test]
    fn invalid_speech_timing_and_viseme_bounds_are_rejected() {
        let mut speech = valid_speech();
        speech.timing.duration_ms = 0;
        assert_eq!(
            speech.validate(),
            Err(AiProtocolValidationError::InvalidSpeechTiming)
        );

        let mut speech = valid_speech();
        speech.visemes[0].start_ms = speech.timing.duration_ms;
        speech.visemes[0].duration_ms = 8;
        assert_eq!(
            speech.validate(),
            Err(AiProtocolValidationError::InvalidVisemeTiming)
        );
    }

    #[test]
    fn decoder_rejects_wrong_kind_and_oversized_payload() {
        let mut packet = encode_ai_presentation_server_event(
            PacketSequence(5),
            NetworkTick(46),
            valid_dialogue_event(),
            AiReplicationAudience::ClientPresentation,
        )
        .expect("valid packet");
        packet.kind = 99;
        assert_eq!(
            decode_ai_presentation_server_event(&packet, AiReplicationAudience::ClientPresentation),
            Err(AiProtocolValidationError::WrongEventKind)
        );

        packet.kind = AI_PRESENTATION_SERVER_EVENT_KIND;
        packet.payload = vec![0; MAX_AI_PRESENTATION_PAYLOAD_BYTES + 1];
        assert_eq!(
            decode_ai_presentation_server_event(&packet, AiReplicationAudience::ClientPresentation),
            Err(AiProtocolValidationError::PayloadTooLarge)
        );
    }
}
