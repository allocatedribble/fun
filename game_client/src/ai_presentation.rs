use std::collections::VecDeque;

use bevy::prelude::*;
use fun_ai_core::{AiActorId, AiPrivacyClass, AiTaskKind, AuthorityTier};

const MAX_AI_DIALOGUE_TEXT_BYTES: usize = 280;
const MAX_AI_VOICE_ID_BYTES: usize = 64;
const MAX_AI_PRESENTATION_QUEUE: usize = 128;
const DEFAULT_SUBTITLE_TTL_FRAMES: u16 = 180;
const DEFAULT_SPEECH_TTL_FRAMES: u16 = 240;
const DEFAULT_SPEECH_DURATION_FRAMES: u16 = 90;

pub struct FunAiClientPresentationPlugin;

impl Plugin for FunAiClientPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientAiAuthorityBoundary>()
            .init_resource::<ClientAiPresentationState>()
            .init_resource::<ClientAiStatusSnapshot>()
            .add_message::<ReplicatedAiDialogueEvent>()
            .add_message::<ClientAiSpeechPlaybackRequest>()
            .add_message::<ClientAiSubtitleEvent>()
            .add_systems(
                Update,
                (
                    consume_replicated_dialogue_events,
                    play_ai_speech,
                    update_ai_subtitles,
                    drive_ai_visemes,
                    display_ai_debug,
                )
                    .chain(),
            );
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientAiPresentationCapability {
    SpeechPlayback,
    Subtitles,
    AnimationLipsync,
    BarkPlayback,
    DebugOverlay,
    StatusVisualization,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientAiForbiddenAuthority {
    AuthoritativeCombat,
    Inventory,
    Damage,
    QuestState,
    MultiplayerNpcDecision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientLocalInferencePurpose {
    LocalCommandInterpretation,
    NonAuthoritativeBark,
    Accessibility,
    LocalEditorPreview,
    ClientUx,
    AuthoritativeCombat,
    Inventory,
    Damage,
    QuestState,
    MultiplayerNpcDecision,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientAiPresentationRejection {
    AuthoritativeGameplay,
    OversizeText,
    OversizeVoiceId,
    InvalidControlText,
    UntrustedSource,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Resource)]
pub struct ClientAiAuthorityBoundary {
    owns_speech_playback: bool,
    owns_subtitles: bool,
    owns_animation_lipsync: bool,
    owns_bark_playback: bool,
    owns_debug_overlay: bool,
    owns_status_visualization: bool,
    owns_authoritative_combat: bool,
    owns_inventory: bool,
    owns_damage: bool,
    owns_quest_state: bool,
    owns_multiplayer_npc_decisions: bool,
}

impl ClientAiAuthorityBoundary {
    pub const fn owns_presentation(self, capability: ClientAiPresentationCapability) -> bool {
        match capability {
            ClientAiPresentationCapability::SpeechPlayback => self.owns_speech_playback,
            ClientAiPresentationCapability::Subtitles => self.owns_subtitles,
            ClientAiPresentationCapability::AnimationLipsync => self.owns_animation_lipsync,
            ClientAiPresentationCapability::BarkPlayback => self.owns_bark_playback,
            ClientAiPresentationCapability::DebugOverlay => self.owns_debug_overlay,
            ClientAiPresentationCapability::StatusVisualization => self.owns_status_visualization,
        }
    }

    pub const fn owns_forbidden_authority(self, authority: ClientAiForbiddenAuthority) -> bool {
        match authority {
            ClientAiForbiddenAuthority::AuthoritativeCombat => self.owns_authoritative_combat,
            ClientAiForbiddenAuthority::Inventory => self.owns_inventory,
            ClientAiForbiddenAuthority::Damage => self.owns_damage,
            ClientAiForbiddenAuthority::QuestState => self.owns_quest_state,
            ClientAiForbiddenAuthority::MultiplayerNpcDecision => {
                self.owns_multiplayer_npc_decisions
            }
        }
    }

    pub const fn authority_tier(self) -> AuthorityTier {
        AuthorityTier::Presentation
    }
}

impl Default for ClientAiAuthorityBoundary {
    fn default() -> Self {
        Self {
            owns_speech_playback: true,
            owns_subtitles: true,
            owns_animation_lipsync: true,
            owns_bark_playback: true,
            owns_debug_overlay: true,
            owns_status_visualization: true,
            owns_authoritative_combat: false,
            owns_inventory: false,
            owns_damage: false,
            owns_quest_state: false,
            owns_multiplayer_npc_decisions: false,
        }
    }
}

#[derive(Component, Clone, Debug, Eq, PartialEq)]
pub struct ClientAiPresentationActor {
    actor_id: AiActorId,
}

impl ClientAiPresentationActor {
    pub const fn new(actor_id: AiActorId) -> Self {
        Self { actor_id }
    }

    pub const fn actor_id(&self) -> &AiActorId {
        &self.actor_id
    }
}

#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientAiSpeechState {
    active: bool,
    remaining_frames: u16,
}

impl ClientAiSpeechState {
    pub const fn active(self) -> bool {
        self.active
    }
}

#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientAiSubtitleState {
    visible: bool,
    remaining_frames: u16,
}

#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientAiVisemeState {
    viseme_index: u8,
    intensity_milli: u16,
}

impl ClientAiVisemeState {
    pub const fn intensity_milli(self) -> u16 {
        self.intensity_milli
    }
}

#[derive(Component, Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientAiDebugOverlay {
    visible: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReplicatedAiDialogueSource {
    ServerReplicated,
    LocalEditorPreview,
    ClientLocal,
}

#[derive(Clone, Debug, Message)]
pub struct ReplicatedAiDialogueEvent {
    actor_id: AiActorId,
    line_id: u64,
    source: ReplicatedAiDialogueSource,
    text: String,
    voice_id: Option<String>,
    privacy_class: AiPrivacyClass,
}

impl ReplicatedAiDialogueEvent {
    pub fn server_replicated(
        actor_id: AiActorId,
        line_id: u64,
        text: impl Into<String>,
        voice_id: Option<String>,
    ) -> Self {
        Self {
            actor_id,
            line_id,
            source: ReplicatedAiDialogueSource::ServerReplicated,
            text: text.into(),
            voice_id,
            privacy_class: AiPrivacyClass::RuntimeLocal,
        }
    }

    pub fn local_editor_preview(
        actor_id: AiActorId,
        line_id: u64,
        text: impl Into<String>,
    ) -> Self {
        Self {
            actor_id,
            line_id,
            source: ReplicatedAiDialogueSource::LocalEditorPreview,
            text: text.into(),
            voice_id: None,
            privacy_class: AiPrivacyClass::ProjectLocal,
        }
    }

    pub fn client_local(actor_id: AiActorId, line_id: u64, text: impl Into<String>) -> Self {
        Self {
            actor_id,
            line_id,
            source: ReplicatedAiDialogueSource::ClientLocal,
            text: text.into(),
            voice_id: None,
            privacy_class: AiPrivacyClass::PrivateUserContent,
        }
    }

    pub const fn actor_id(&self) -> &AiActorId {
        &self.actor_id
    }

    pub const fn source(&self) -> ReplicatedAiDialogueSource {
        self.source
    }

    pub fn text(&self) -> &str {
        &self.text
    }
}

#[derive(Clone, Debug, Message)]
pub struct ClientAiSpeechPlaybackRequest {
    actor_id: AiActorId,
    line_id: u64,
    text_hash: u64,
    voice_id: Option<String>,
    ttl_frames: u16,
}

#[derive(Clone, Debug, Message)]
pub struct ClientAiSubtitleEvent {
    actor_id: AiActorId,
    line_id: u64,
    text: String,
    ttl_frames: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingSpeech {
    actor_id: AiActorId,
    line_id: u64,
    text_hash: u64,
    voice_id: Option<String>,
    remaining_frames: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveSubtitle {
    actor_id: AiActorId,
    line_id: u64,
    text: String,
    remaining_frames: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ActiveViseme {
    actor_id: AiActorId,
    line_id: u64,
    viseme_index: u8,
    intensity_milli: u16,
    remaining_frames: u16,
}

#[derive(Debug, Default, Resource)]
pub struct ClientAiPresentationState {
    speech_queue: VecDeque<PendingSpeech>,
    active_subtitles: Vec<ActiveSubtitle>,
    active_visemes: Vec<ActiveViseme>,
    rejected_events: u64,
    consumed_dialogue_events: u64,
}

impl ClientAiPresentationState {
    pub fn queued_speech_count(&self) -> usize {
        self.speech_queue.len()
    }

    pub fn active_subtitle_count(&self) -> usize {
        self.active_subtitles.len()
    }

    pub fn active_viseme_count(&self) -> usize {
        self.active_visemes.len()
    }

    pub const fn rejected_events(&self) -> u64 {
        self.rejected_events
    }

    fn push_speech(&mut self, speech: PendingSpeech) {
        if self.speech_queue.len() >= MAX_AI_PRESENTATION_QUEUE {
            self.speech_queue.pop_front();
        }
        self.speech_queue.push_back(speech);
    }

    fn push_subtitle(&mut self, subtitle: ActiveSubtitle) {
        if self.active_subtitles.len() >= MAX_AI_PRESENTATION_QUEUE {
            self.active_subtitles.remove(0);
        }
        self.active_subtitles.push(subtitle);
    }

    fn push_viseme(&mut self, viseme: ActiveViseme) {
        if self.active_visemes.len() >= MAX_AI_PRESENTATION_QUEUE {
            self.active_visemes.remove(0);
        }
        self.active_visemes.push(viseme);
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Resource)]
pub struct ClientAiStatusSnapshot {
    consumed_dialogue_events: u64,
    rejected_events: u64,
    queued_speech: u16,
    active_subtitles: u16,
    active_visemes: u16,
    last_task_kind: Option<AiTaskKind>,
}

impl ClientAiStatusSnapshot {
    pub const fn rejected_events(self) -> u64 {
        self.rejected_events
    }

    pub const fn queued_speech(self) -> u16 {
        self.queued_speech
    }
}

pub const fn validate_client_local_inference_purpose(
    purpose: ClientLocalInferencePurpose,
) -> Result<AiTaskKind, ClientAiPresentationRejection> {
    match purpose {
        ClientLocalInferencePurpose::LocalCommandInterpretation => Ok(AiTaskKind::CommandbarRoute),
        ClientLocalInferencePurpose::NonAuthoritativeBark => Ok(AiTaskKind::NpcBark),
        ClientLocalInferencePurpose::Accessibility => Ok(AiTaskKind::SpeechAsr),
        ClientLocalInferencePurpose::LocalEditorPreview => Ok(AiTaskKind::CommandbarAnswer),
        ClientLocalInferencePurpose::ClientUx => Ok(AiTaskKind::CommandbarAnswer),
        ClientLocalInferencePurpose::AuthoritativeCombat
        | ClientLocalInferencePurpose::Inventory
        | ClientLocalInferencePurpose::Damage
        | ClientLocalInferencePurpose::QuestState
        | ClientLocalInferencePurpose::MultiplayerNpcDecision => {
            Err(ClientAiPresentationRejection::AuthoritativeGameplay)
        }
    }
}

pub fn consume_replicated_dialogue_events(
    mut events: MessageReader<ReplicatedAiDialogueEvent>,
    mut speech_requests: MessageWriter<ClientAiSpeechPlaybackRequest>,
    mut subtitle_events: MessageWriter<ClientAiSubtitleEvent>,
    mut presentation: ResMut<ClientAiPresentationState>,
) {
    for event in events.read() {
        match validate_dialogue_event(event) {
            Ok(()) => {
                let text_hash = stable_text_hash(event.text());
                let speech = ClientAiSpeechPlaybackRequest {
                    actor_id: event.actor_id.clone(),
                    line_id: event.line_id,
                    text_hash,
                    voice_id: event.voice_id.clone(),
                    ttl_frames: DEFAULT_SPEECH_TTL_FRAMES,
                };
                let subtitle = ClientAiSubtitleEvent {
                    actor_id: event.actor_id.clone(),
                    line_id: event.line_id,
                    text: event.text.clone(),
                    ttl_frames: DEFAULT_SUBTITLE_TTL_FRAMES,
                };
                presentation.consumed_dialogue_events =
                    presentation.consumed_dialogue_events.saturating_add(1);
                speech_requests.write(speech);
                subtitle_events.write(subtitle);
            }
            Err(_) => {
                presentation.rejected_events = presentation.rejected_events.saturating_add(1);
            }
        }
    }
}

pub fn play_ai_speech(
    mut speech_requests: MessageReader<ClientAiSpeechPlaybackRequest>,
    mut presentation: ResMut<ClientAiPresentationState>,
) {
    for request in speech_requests.read() {
        presentation.push_speech(PendingSpeech {
            actor_id: request.actor_id.clone(),
            line_id: request.line_id,
            text_hash: request.text_hash,
            voice_id: request.voice_id.clone(),
            remaining_frames: request.ttl_frames.min(DEFAULT_SPEECH_TTL_FRAMES),
        });
        presentation.push_viseme(ActiveViseme {
            actor_id: request.actor_id.clone(),
            line_id: request.line_id,
            viseme_index: 0,
            intensity_milli: 1000,
            remaining_frames: DEFAULT_SPEECH_DURATION_FRAMES,
        });
    }

    let mut remaining = VecDeque::with_capacity(presentation.speech_queue.len());
    while let Some(mut speech) = presentation.speech_queue.pop_front() {
        speech.remaining_frames = speech.remaining_frames.saturating_sub(1);
        if speech.remaining_frames > 0 {
            remaining.push_back(speech);
        }
    }
    presentation.speech_queue = remaining;
}

pub fn update_ai_subtitles(
    mut subtitle_events: MessageReader<ClientAiSubtitleEvent>,
    mut presentation: ResMut<ClientAiPresentationState>,
) {
    for event in subtitle_events.read() {
        presentation.push_subtitle(ActiveSubtitle {
            actor_id: event.actor_id.clone(),
            line_id: event.line_id,
            text: event.text.clone(),
            remaining_frames: event.ttl_frames.min(DEFAULT_SUBTITLE_TTL_FRAMES),
        });
    }

    for subtitle in &mut presentation.active_subtitles {
        subtitle.remaining_frames = subtitle.remaining_frames.saturating_sub(1);
    }
    presentation
        .active_subtitles
        .retain(|subtitle| subtitle.remaining_frames > 0);
}

pub fn drive_ai_visemes(mut presentation: ResMut<ClientAiPresentationState>) {
    for viseme in &mut presentation.active_visemes {
        viseme.remaining_frames = viseme.remaining_frames.saturating_sub(1);
        let elapsed = DEFAULT_SPEECH_DURATION_FRAMES.saturating_sub(viseme.remaining_frames);
        viseme.viseme_index = u8::try_from(elapsed % 12).expect("viseme index remains below 12");
        viseme.intensity_milli = if viseme.remaining_frames == 0 {
            0
        } else {
            250 + u16::from(viseme.viseme_index % 4) * 125
        };
    }
    presentation
        .active_visemes
        .retain(|viseme| viseme.remaining_frames > 0);
}

pub fn display_ai_debug(
    presentation: Res<ClientAiPresentationState>,
    mut snapshot: ResMut<ClientAiStatusSnapshot>,
) {
    let queued_speech = u16::try_from(presentation.speech_queue.len()).unwrap_or(u16::MAX);
    let active_subtitles = u16::try_from(presentation.active_subtitles.len()).unwrap_or(u16::MAX);
    let active_visemes = u16::try_from(presentation.active_visemes.len()).unwrap_or(u16::MAX);
    *snapshot = ClientAiStatusSnapshot {
        consumed_dialogue_events: presentation.consumed_dialogue_events,
        rejected_events: presentation.rejected_events,
        queued_speech,
        active_subtitles,
        active_visemes,
        last_task_kind: Some(AiTaskKind::SpeechTts),
    };
}

fn validate_dialogue_event(
    event: &ReplicatedAiDialogueEvent,
) -> Result<(), ClientAiPresentationRejection> {
    if !matches!(
        event.source,
        ReplicatedAiDialogueSource::ServerReplicated
            | ReplicatedAiDialogueSource::LocalEditorPreview
    ) {
        return Err(ClientAiPresentationRejection::UntrustedSource);
    }
    if event.text.len() > MAX_AI_DIALOGUE_TEXT_BYTES {
        return Err(ClientAiPresentationRejection::OversizeText);
    }
    if !valid_presentation_text(&event.text) {
        return Err(ClientAiPresentationRejection::InvalidControlText);
    }
    if let Some(voice_id) = &event.voice_id {
        if voice_id.len() > MAX_AI_VOICE_ID_BYTES {
            return Err(ClientAiPresentationRejection::OversizeVoiceId);
        }
        if !valid_identifier_text(voice_id) {
            return Err(ClientAiPresentationRejection::InvalidControlText);
        }
    }
    let _privacy_class = event.privacy_class;
    Ok(())
}

fn valid_presentation_text(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|character| character == '\n' || character == '\t' || !character.is_control())
}

fn valid_identifier_text(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
}

fn stable_text_hash(value: &str) -> u64 {
    value.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor_id(value: &str) -> AiActorId {
        AiActorId::new(String::from(value)).expect("test actor id")
    }

    #[test]
    fn client_ai_authority_boundary_excludes_authoritative_gameplay() {
        let boundary = ClientAiAuthorityBoundary::default();

        assert!(boundary.owns_presentation(ClientAiPresentationCapability::SpeechPlayback));
        assert!(boundary.owns_presentation(ClientAiPresentationCapability::Subtitles));
        assert!(boundary.owns_presentation(ClientAiPresentationCapability::AnimationLipsync));
        assert!(boundary.owns_presentation(ClientAiPresentationCapability::BarkPlayback));
        assert!(boundary.owns_presentation(ClientAiPresentationCapability::DebugOverlay));
        assert!(boundary.owns_presentation(ClientAiPresentationCapability::StatusVisualization));
        assert!(
            !boundary.owns_forbidden_authority(ClientAiForbiddenAuthority::AuthoritativeCombat)
        );
        assert!(!boundary.owns_forbidden_authority(ClientAiForbiddenAuthority::Inventory));
        assert!(!boundary.owns_forbidden_authority(ClientAiForbiddenAuthority::Damage));
        assert!(!boundary.owns_forbidden_authority(ClientAiForbiddenAuthority::QuestState));
        assert!(
            !boundary.owns_forbidden_authority(ClientAiForbiddenAuthority::MultiplayerNpcDecision)
        );
        assert_eq!(boundary.authority_tier(), AuthorityTier::Presentation);
    }

    #[test]
    fn local_inference_policy_allows_only_client_ux_purposes() {
        assert_eq!(
            validate_client_local_inference_purpose(
                ClientLocalInferencePurpose::LocalCommandInterpretation
            ),
            Ok(AiTaskKind::CommandbarRoute)
        );
        assert_eq!(
            validate_client_local_inference_purpose(
                ClientLocalInferencePurpose::NonAuthoritativeBark
            ),
            Ok(AiTaskKind::NpcBark)
        );
        assert_eq!(
            validate_client_local_inference_purpose(ClientLocalInferencePurpose::Accessibility),
            Ok(AiTaskKind::SpeechAsr)
        );
        assert_eq!(
            validate_client_local_inference_purpose(
                ClientLocalInferencePurpose::LocalEditorPreview
            ),
            Ok(AiTaskKind::CommandbarAnswer)
        );
        assert_eq!(
            validate_client_local_inference_purpose(ClientLocalInferencePurpose::ClientUx),
            Ok(AiTaskKind::CommandbarAnswer)
        );
        assert_eq!(
            validate_client_local_inference_purpose(
                ClientLocalInferencePurpose::AuthoritativeCombat
            ),
            Err(ClientAiPresentationRejection::AuthoritativeGameplay)
        );
        assert_eq!(
            validate_client_local_inference_purpose(
                ClientLocalInferencePurpose::MultiplayerNpcDecision
            ),
            Err(ClientAiPresentationRejection::AuthoritativeGameplay)
        );
    }

    #[test]
    fn replicated_dialogue_events_are_bounded_and_presentation_only() {
        let event = ReplicatedAiDialogueEvent::server_replicated(
            actor_id("npc.guard"),
            7,
            String::from("Hold position."),
            Some(String::from("guard_alpha")),
        );
        assert!(validate_dialogue_event(&event).is_ok());

        let too_long = ReplicatedAiDialogueEvent::server_replicated(
            actor_id("npc.guard"),
            8,
            "x".repeat(MAX_AI_DIALOGUE_TEXT_BYTES + 1),
            None,
        );
        assert_eq!(
            validate_dialogue_event(&too_long),
            Err(ClientAiPresentationRejection::OversizeText)
        );

        let client_local =
            ReplicatedAiDialogueEvent::client_local(actor_id("npc.guard"), 9, "client says damage");
        assert_eq!(
            validate_dialogue_event(&client_local),
            Err(ClientAiPresentationRejection::UntrustedSource)
        );
    }

    #[test]
    fn presentation_systems_consume_dialogue_into_speech_subtitles_visemes() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(FunAiClientPresentationPlugin);
        app.world_mut()
            .write_message(ReplicatedAiDialogueEvent::server_replicated(
                actor_id("npc.squadmate"),
                11,
                String::from("Moving."),
                Some(String::from("squadmate_one")),
            ));

        app.update();

        let presentation = app.world().resource::<ClientAiPresentationState>();
        assert_eq!(presentation.queued_speech_count(), 1);
        assert_eq!(presentation.active_subtitle_count(), 1);
        assert_eq!(presentation.active_viseme_count(), 1);
        assert_eq!(presentation.rejected_events(), 0);

        let snapshot = *app.world().resource::<ClientAiStatusSnapshot>();
        assert_eq!(snapshot.queued_speech(), 1);
        assert_eq!(snapshot.rejected_events(), 0);
    }
}
