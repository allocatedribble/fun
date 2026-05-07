#[cfg(all(feature = "diagnostics", debug_assertions))]
use std::time::Instant;
use std::{
    collections::{HashMap, HashSet},
    env,
    path::Path,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use avian3d::prelude::PhysicsPlugins;
use bevy::{
    app::ScheduleRunnerPlugin, asset::AssetPlugin, log::LogPlugin, mesh::MeshPlugin, prelude::*,
    scene::ScenePlugin,
};
use bevy_quinnet::server::{
    ConnectionEvent, ConnectionLostEvent, EndpointAddrConfiguration, QuinnetServer,
    QuinnetServerPlugin, ServerEndpointConfiguration, ServerEndpointConfigurationDefaultables,
    certificate::CertificateRetrievalMode,
};
use fun_warden_protocol::{
    BoundedAscii, MAX_MATCH_SESSION_ID_BYTES, WardenAdmissionTicket, WardenSessionDecisionKind,
};
use fun_warden_server::{
    GameServerAdmissionDecision, VerifiedAdmissionTicket, WardenAdmissionTicketSignatureVerifier,
    WardenAdmissionVerificationError, admission_decision_for_game_server,
};
use game_scene::StreamedWorldEntity;
use game_shared::{
    DEFAULT_TICK_RATE_HZ, DEMO_LEVEL_ID, GAME_SERVER_BIND_ADDR, GAME_SERVER_CERT_FILE,
    GAME_SERVER_KEY_FILE, GAME_TITLE,
};
use ring::hmac;
use thunder::prelude::*;
use tracing::{error, info};

mod ai;
mod telemetry_gate;

const GAME_PROTOCOL_VERSION: u32 = 2;
const MAX_CLIENT_CONTROL_PACKET_BYTES: usize = 64 * 1024;
const MAX_AUTH_TICKET_BYTES: usize = 1024;
const MAX_WARDEN_ADMISSION_TICKET_BYTES: usize = 4096;
const GAME_SERVER_TLS_MODE_ENV: &str = "FUN_GAME_SERVER_TLS_MODE";
const GAME_SERVER_TLS_MODE_DEVELOPMENT: &str = "development";
const GAME_SERVER_TLS_MODE_PRODUCTION: &str = "production";
const GAME_SERVER_DEV_TICKET_ENV: &str = "FUN_GAME_SERVER_DEV_SESSION_TOKEN";
const GAME_SERVER_WARDEN_GATE_ENV: &str = "FUN_WARDEN_SERVER_GATE";
const GAME_SERVER_WARDEN_GATE_OBSERVE: &str = "observe";
const GAME_SERVER_WARDEN_GATE_REQUIRE_ADMISSION: &str = "require_admission_ticket";
const GAME_SERVER_WARDEN_ADMISSION_HMAC_KEY_HEX_ENV: &str = "FUN_WARDEN_ADMISSION_HMAC_KEY_HEX";
const GAME_SERVER_WARDEN_ADMISSION_KEY_ID_ENV: &str = "FUN_WARDEN_ADMISSION_KEY_ID";
const GAME_SERVER_WARDEN_MATCH_SESSION_ID_ENV: &str = "FUN_WARDEN_MATCH_SESSION_ID";
const GAME_SERVER_WARDEN_DEFAULT_ADMISSION_KEY_ID: &str = "server_warden_lookup_key:v1";

fn main() {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(Duration::from_secs_f64(
            1.0 / DEFAULT_TICK_RATE_HZ,
        ))),
        AssetPlugin::default(),
        LogPlugin::default(),
        MeshPlugin,
        ScenePlugin,
        PhysicsPlugins::default(),
        QuinnetServerPlugin::default(),
        ThunderPlugin::default(),
        ai::FunAiServerPlugin,
    ))
    .insert_resource(GameServerSecurityConfig::from_env())
    .init_resource::<ServerWorldStream>()
    .init_resource::<ServerEditorSchema>()
    .init_resource::<ServerEditorInspectorState>()
    .insert_resource(ServerLogConfig::from_env())
    .init_resource::<ConnectedClients>()
    .init_resource::<ReadyClients>()
    .init_resource::<ClientRelevanceSets>()
    .init_resource::<PendingWorldStreams>()
    .init_resource::<ClientAdmissionStates>()
    .add_systems(
        Startup,
        (
            start_endpoint,
            start_server_editor_inspector,
            game_scene::spawn_default_scene,
            game_scene::apply_scene_stable_identities,
        )
            .chain(),
    )
    .add_systems(
        Update,
        (
            cleanup_disconnected_clients,
            queue_world_stream_for_new_clients,
            receive_client_control,
            apply_server_editor_mutations,
            rebuild_world_stream,
            update_server_editor_inspector_snapshot,
            send_pending_world_streams,
        )
            .chain(),
    );

    #[cfg(all(feature = "diagnostics", debug_assertions))]
    app.init_resource::<ServerWorldDiagnostics>()
        .init_resource::<ServerProfiler>()
        .init_resource::<ServerEditorControlPlane>()
        .add_systems(
            Startup,
            (log_server_editor_control_plane, log_server_editor_schema),
        )
        .add_systems(PreUpdate, begin_server_profiler_tick)
        .add_systems(Update, log_streamable_inventory);

    #[cfg(feature = "server-ingest")]
    app.insert_resource(telemetry_gate::server_telemetry_contract());
    #[cfg(not(feature = "server-ingest"))]
    app.insert_resource(ServerTelemetryResource);

    app.run();
}

#[cfg(not(feature = "server-ingest"))]
#[derive(Debug, Clone, Copy, Resource)]
struct ServerTelemetryResource;

fn start_endpoint(mut server: ResMut<QuinnetServer>, security: Res<GameServerSecurityConfig>) {
    let limits = ChannelLimits::default();
    server
        .start_endpoint(ServerEndpointConfiguration {
            addr_config: EndpointAddrConfiguration::from_string(GAME_SERVER_BIND_ADDR)
                .expect("game server bind address should be valid"),
            cert_mode: security.certificate_mode(),
            defaultables: ServerEndpointConfigurationDefaultables {
                send_channels_cfg: ServerChannel::channels_configuration(limits),
                ..Default::default()
            },
        })
        .expect("game server endpoint should start");

    info!(
        "Starting {GAME_TITLE} game server at {DEFAULT_TICK_RATE_HZ:.0} Hz on {GAME_SERVER_BIND_ADDR}"
    );
}

#[derive(Debug, Clone, Resource)]
struct GameServerSecurityConfig {
    tls_mode: GameServerTlsMode,
    ticket_verifier: ClientTicketVerifier,
    warden_gate: WardenAdmissionGate,
}

impl GameServerSecurityConfig {
    fn from_env() -> Self {
        let tls_mode = GameServerTlsMode::from_env();
        assert_production_tls_material(tls_mode);
        Self {
            tls_mode,
            ticket_verifier: ClientTicketVerifier::from_env(tls_mode),
            warden_gate: WardenAdmissionGate::from_env(tls_mode),
        }
    }

    fn certificate_mode(&self) -> CertificateRetrievalMode {
        certificate_mode_for_tls(self.tls_mode)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GameServerTlsMode {
    DevelopmentSelfSigned,
    ProductionConfigured,
}

impl GameServerTlsMode {
    fn from_env() -> Self {
        match env::var(GAME_SERVER_TLS_MODE_ENV) {
            Ok(value) if value.eq_ignore_ascii_case(GAME_SERVER_TLS_MODE_DEVELOPMENT) => {
                Self::DevelopmentSelfSigned
            }
            Ok(value) if value.eq_ignore_ascii_case(GAME_SERVER_TLS_MODE_PRODUCTION) => {
                Self::ProductionConfigured
            }
            Ok(value) => panic!(
                "{GAME_SERVER_TLS_MODE_ENV} must be `{GAME_SERVER_TLS_MODE_DEVELOPMENT}` or `{GAME_SERVER_TLS_MODE_PRODUCTION}`, got `{value}`"
            ),
            Err(_) => Self::ProductionConfigured,
        }
    }
}

fn assert_production_tls_material(tls_mode: GameServerTlsMode) {
    if tls_mode == GameServerTlsMode::ProductionConfigured {
        assert!(
            Path::new(GAME_SERVER_CERT_FILE).is_file() && Path::new(GAME_SERVER_KEY_FILE).is_file(),
            "{GAME_SERVER_TLS_MODE_ENV}={GAME_SERVER_TLS_MODE_PRODUCTION} requires configured cert/key files at `{GAME_SERVER_CERT_FILE}` and `{GAME_SERVER_KEY_FILE}`"
        );
    }
}

fn certificate_mode_for_tls(tls_mode: GameServerTlsMode) -> CertificateRetrievalMode {
    match tls_mode {
        GameServerTlsMode::DevelopmentSelfSigned => {
            CertificateRetrievalMode::LoadFromFileOrGenerateSelfSigned {
                cert_file: GAME_SERVER_CERT_FILE.to_owned(),
                key_file: GAME_SERVER_KEY_FILE.to_owned(),
                save_on_disk: true,
                server_hostname: "127.0.0.1".to_owned(),
            }
        }
        GameServerTlsMode::ProductionConfigured => CertificateRetrievalMode::LoadFromFile {
            cert_file: GAME_SERVER_CERT_FILE.to_owned(),
            key_file: GAME_SERVER_KEY_FILE.to_owned(),
        },
    }
}

#[derive(Resource)]
struct ServerEditorSchema {
    #[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
    registry: game_shared::EditorSchemaRegistry<'static>,
}

#[derive(Debug, Clone, Resource, Default)]
struct ServerEditorInspectorState {
    state: game_shared::EditorInspectorRuntimeState,
}

impl Default for ServerEditorSchema {
    fn default() -> Self {
        Self {
            registry: game_shared::EditorSchemaRegistry::new(
                server_editor_components(),
                server_editor_resources(),
            ),
        }
    }
}

fn server_editor_components() -> &'static [game_shared::EditorComponentRegistration] {
    game_shared::editor_component_registry! {
        transform => {
            kind: game_shared::EDITOR_COMPONENT_KIND_TRANSFORM,
            type: Transform,
            reflect: "bevy_transform::components::transform::Transform",
            serializer: "fun.editor.transform.encode.v1",
            deserializer: "fun.editor.transform.decode.v1",
            validator: game_shared::validate_non_empty_payload,
            capability: game_shared::EditorCapability::MutateEntities,
            mutability: game_shared::EditorMutability::RuntimeMutable,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "Transform",
                "Transform",
                game_shared::EditorSchemaWidget::Transform3d,
                game_shared::EditorSchemaImportance::Primary,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_transform",
            ),
            max_payload_bytes: 128,
        },
        name => {
            kind: game_shared::EDITOR_COMPONENT_KIND_NAME,
            type: Name,
            reflect: "bevy_ecs::name::Name",
            serializer: "fun.editor.name.encode.v1",
            deserializer: "fun.editor.name.decode.v1",
            validator: game_shared::validate_non_empty_payload,
            capability: game_shared::EditorCapability::MutateEntities,
            mutability: game_shared::EditorMutability::RuntimeMutable,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "Name",
                "Identity",
                game_shared::EditorSchemaWidget::Text,
                game_shared::EditorSchemaImportance::Secondary,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_name",
            ),
            max_payload_bytes: 256,
        },
        network_identity => {
            kind: game_shared::EDITOR_COMPONENT_KIND_NETWORK_IDENTITY,
            type: NetworkIdentity,
            reflect: "thunder::bevy_integration::NetworkIdentity",
            serializer: "fun.editor.network_identity.encode.v1",
            deserializer: "fun.editor.network_identity.decode.v1",
            validator: game_shared::reject_editor_mutation,
            capability: game_shared::EditorCapability::MutateEntities,
            mutability: game_shared::EditorMutability::ReadOnly,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "Network Identity",
                "Network",
                game_shared::EditorSchemaWidget::ReadOnlyStruct,
                game_shared::EditorSchemaImportance::Diagnostic,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_network_identity",
            ),
            max_payload_bytes: 64,
        },
        network_authority => {
            kind: game_shared::EDITOR_COMPONENT_KIND_NETWORK_AUTHORITY,
            type: NetworkAuthority,
            reflect: "thunder::bevy_integration::NetworkAuthority",
            serializer: "fun.editor.network_authority.encode.v1",
            deserializer: "fun.editor.network_authority.decode.v1",
            validator: game_shared::reject_editor_mutation,
            capability: game_shared::EditorCapability::MutateEntities,
            mutability: game_shared::EditorMutability::ReadOnly,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "Authority",
                "Network",
                game_shared::EditorSchemaWidget::ReadOnlyStruct,
                game_shared::EditorSchemaImportance::Diagnostic,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_network_authority",
            ),
            max_payload_bytes: 32,
        },
        streamed_world => {
            kind: game_shared::EDITOR_COMPONENT_KIND_WORLD_CATALOG_REF,
            type: StreamedWorldEntity,
            reflect: "game_scene::StreamedWorldEntity",
            serializer: "fun.editor.streamed_world.encode.v1",
            deserializer: "fun.editor.streamed_world.decode.v1",
            validator: game_shared::validate_non_empty_payload,
            capability: game_shared::EditorCapability::ApplyScenePatch,
            mutability: game_shared::EditorMutability::PersistentMutable,
            serialization: game_shared::EditorSerializationPolicy::Compactly,
            replication: game_shared::EditorReplicationPolicy::ServerAuthoritative,
            ui: game_shared::EditorUiHints::new(
                "World Catalog",
                "World",
                game_shared::EditorSchemaWidget::ReadOnlyStruct,
                game_shared::EditorSchemaImportance::Advanced,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_streamed_world",
            ),
            max_payload_bytes: 128,
        },
    }
}

fn server_editor_resources() -> &'static [game_shared::EditorResourceRegistration] {
    game_shared::editor_resource_registry! {
        diagnostics => {
            kind: game_shared::EDITOR_RESOURCE_KIND_RUNTIME_DIAGNOSTICS,
            type: ServerLogConfig,
            reflect: "game_server::ServerLogConfig",
            serializer: "fun.editor.server_log_config.encode.v1",
            deserializer: "fun.editor.server_log_config.decode.v1",
            validator: game_shared::validate_non_empty_payload,
            capability: game_shared::EditorCapability::ControlRuntime,
            mutability: game_shared::EditorMutability::RuntimeMutable,
            policy: game_shared::EditorResourcePolicy::VisualRuntimeSetting,
            ui: game_shared::EditorUiHints::new(
                "Runtime Diagnostics",
                "Diagnostics",
                game_shared::EditorSchemaWidget::Toggle,
                game_shared::EditorSchemaImportance::Advanced,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_runtime_diagnostics",
            ),
            max_payload_bytes: 16,
        },
        match_state => {
            kind: game_shared::EDITOR_RESOURCE_KIND_MATCH_STATE,
            type: ServerWorldStream,
            reflect: "game_server::ServerWorldStream",
            serializer: "fun.editor.server_world_stream.encode.v1",
            deserializer: "fun.editor.server_world_stream.decode.v1",
            validator: game_shared::reject_editor_mutation,
            capability: game_shared::EditorCapability::PersistIteration,
            mutability: game_shared::EditorMutability::ReadOnly,
            policy: game_shared::EditorResourcePolicy::CriticalServerState,
            ui: game_shared::EditorUiHints::new(
                "Match World Stream",
                "Server",
                game_shared::EditorSchemaWidget::ResourcePanel,
                game_shared::EditorSchemaImportance::Diagnostic,
            ),
            diagnostics: game_shared::EditorDiagnosticLabels::new(
                "fun::editor::schema",
                "server_match_state",
            ),
            max_payload_bytes: 0,
        },
    }
}

fn server_editor_component_schemas() -> Vec<game_shared::EditorComponentSchema> {
    server_editor_components()
        .iter()
        .enumerate()
        .map(|(index, registration)| {
            registration.schema(game_shared::EditorStableTypeId((index as u64) + 1))
        })
        .collect()
}

fn start_server_editor_inspector(inspector: Res<ServerEditorInspectorState>) {
    let execute_server_code_enabled = std::env::var_os("FUN_EDITOR_ENABLE_SERVER_EXEC").is_some();
    let component_schemas = server_editor_component_schemas();
    inspector.state.replace_entities(
        game_shared::EditorWorldRevision(0),
        game_shared::EditorSchemaRevision(1),
        component_schemas.clone(),
        Vec::new(),
    );
    let mut config = game_shared::EditorInspectorServiceConfig::local_development(
        game_shared::EditorTargetKind::Server,
        "fun",
        game_shared::local_development_capabilities(
            game_shared::EditorTargetKind::Server,
            execute_server_code_enabled,
        ),
    );
    config.tick_rate_hz = DEFAULT_TICK_RATE_HZ.round() as u32;
    config.component_schemas = component_schemas;
    config.runtime_state = inspector.state.clone();

    match game_shared::spawn_editor_inspector_service(config) {
        Ok(summary) => {
            if summary.callback_started || summary.bind_addr.is_some() {
                info!(
                    target: "fun::editor::control",
                    callback_started = summary.callback_started,
                    bind_addr = ?summary.bind_addr,
                    "server editor inspector service started"
                );
            }
        }
        Err(error) => {
            tracing::warn!(
                target: "fun::editor::control",
                %error,
                "server editor inspector service did not start"
            );
        }
    }
}

#[derive(Debug, Default, Resource)]
struct ServerWorldStream {
    revision: WorldRevision,
    editor_revision: WorldRevision,
    manifest_signature: u64,
    entity_ids: Vec<NetEntity>,
    chunks: Vec<WorldStreamChunk>,
}

impl ServerWorldStream {
    fn clear_with_new_revision(&mut self) -> bool {
        if self.entity_ids.is_empty() && self.chunks.is_empty() {
            return false;
        }
        let next_revision = WorldRevision(self.revision.0.saturating_add(1).max(1));
        self.revision = next_revision;
        self.editor_revision = next_revision;
        self.manifest_signature = fun_scene::world_stream_manifest_signature(&[]);
        self.entity_ids.clear();
        self.chunks.clear();
        true
    }
}

#[derive(Debug, Default, Resource)]
struct ConnectedClients {
    ids: HashSet<u64>,
}

#[derive(Debug, Default, Resource)]
struct ReadyClients {
    ids: HashSet<u64>,
}

#[derive(Debug, Default, Resource)]
struct ClientRelevanceSets {
    sets: HashMap<u64, ServerClientRelevanceSet>,
}

#[derive(Debug, Default, Clone)]
struct ServerClientRelevanceSet {
    tick: NetworkTick,
    entities: HashSet<NetEntity>,
}

#[derive(Debug, Default, Resource)]
struct PendingWorldStreams {
    ids: HashSet<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientAdmissionStage {
    Unauthenticated,
    Admitted,
    Streaming,
    WorldReady,
    InGame,
}

#[derive(Debug, Default, Resource)]
struct ClientAdmissionStates {
    stages: HashMap<u64, ClientAdmissionStage>,
    warden_decisions: HashMap<u64, WardenSessionDecisionKind>,
    untrusted_pool: HashSet<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClientHelloRejection {
    WrongProtocolVersion,
    MissingAuthTicket,
    AuthTicketTooLarge,
    TicketVerificationUnavailable,
    DevelopmentTicketMismatch,
    MissingWardenAdmissionTicket,
    WardenAdmissionTicketTooLarge,
    WardenAdmissionVerifierUnavailable,
    WardenAdmissionMalformed,
    WardenAdmissionUnsupportedSchema,
    WardenAdmissionKeyMismatch,
    WardenAdmissionSignatureMismatch,
    WardenAdmissionExpired,
    WardenAdmissionMatchSessionMismatch,
    WardenAdmissionDeniedMatchmaking,
    WardenAdmissionDeniedSession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorldReadyRejection {
    NotStreaming,
    DuplicateReady,
    NoWorldStream,
    WrongLevel,
    StaleRevision,
    FutureRevision,
    WrongChunkCount,
    WrongManifestSignature,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClientTicketVerifier {
    FailClosed,
    DevelopmentToken { token: Vec<u8> },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WardenAdmissionGate {
    Observe,
    RequireAdmissionTicket {
        verifier: WardenAdmissionTicketVerifier,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum WardenAdmissionTicketVerifier {
    FailClosed,
    SignedHmacSha256 {
        key: [u8; 32],
        key_id: String,
        match_session_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClientHelloAdmission {
    warden_decision: WardenSessionDecisionKind,
    untrusted_pool: bool,
}

impl ClientTicketVerifier {
    fn from_env(tls_mode: GameServerTlsMode) -> Self {
        match (tls_mode, env::var(GAME_SERVER_DEV_TICKET_ENV)) {
            (GameServerTlsMode::DevelopmentSelfSigned, Ok(token)) if !token.is_empty() => {
                Self::DevelopmentToken {
                    token: token.into_bytes(),
                }
            }
            _ => Self::FailClosed,
        }
    }

    fn verify(&self, token: &[u8]) -> Result<(), ClientHelloRejection> {
        match self {
            Self::FailClosed => Err(ClientHelloRejection::TicketVerificationUnavailable),
            Self::DevelopmentToken { token: expected } if token == expected.as_slice() => Ok(()),
            Self::DevelopmentToken { .. } => Err(ClientHelloRejection::DevelopmentTicketMismatch),
        }
    }
}

impl WardenAdmissionGate {
    fn from_env(tls_mode: GameServerTlsMode) -> Self {
        match env::var(GAME_SERVER_WARDEN_GATE_ENV) {
            Ok(value) if value.eq_ignore_ascii_case(GAME_SERVER_WARDEN_GATE_REQUIRE_ADMISSION) => {
                Self::RequireAdmissionTicket {
                    verifier: WardenAdmissionTicketVerifier::from_env(tls_mode),
                }
            }
            Ok(value) if value.eq_ignore_ascii_case(GAME_SERVER_WARDEN_GATE_OBSERVE) => {
                Self::Observe
            }
            _ => Self::Observe,
        }
    }

    fn validate(&self, hello: &ClientHello) -> Result<ClientHelloAdmission, ClientHelloRejection> {
        match self {
            Self::Observe => Ok(ClientHelloAdmission {
                warden_decision: WardenSessionDecisionKind::ObserveOnly,
                untrusted_pool: false,
            }),
            Self::RequireAdmissionTicket { verifier } => {
                if hello.warden_admission_ticket.is_empty() {
                    return Err(ClientHelloRejection::MissingWardenAdmissionTicket);
                }
                if hello.warden_admission_ticket.len() > MAX_WARDEN_ADMISSION_TICKET_BYTES {
                    return Err(ClientHelloRejection::WardenAdmissionTicketTooLarge);
                }
                verifier.verify(&hello.warden_admission_ticket)
            }
        }
    }
}

impl WardenAdmissionTicketVerifier {
    fn from_env(_tls_mode: GameServerTlsMode) -> Self {
        let Ok(key_hex) = env::var(GAME_SERVER_WARDEN_ADMISSION_HMAC_KEY_HEX_ENV) else {
            return Self::FailClosed;
        };
        let Some(key) = decode_hex_32(&key_hex) else {
            return Self::FailClosed;
        };
        let Ok(match_session_id) = env::var(GAME_SERVER_WARDEN_MATCH_SESSION_ID_ENV) else {
            return Self::FailClosed;
        };
        if BoundedAscii::<MAX_MATCH_SESSION_ID_BYTES>::new(match_session_id.clone()).is_err() {
            return Self::FailClosed;
        }
        let key_id = env::var(GAME_SERVER_WARDEN_ADMISSION_KEY_ID_ENV)
            .unwrap_or_else(|_| String::from(GAME_SERVER_WARDEN_DEFAULT_ADMISSION_KEY_ID));
        if BoundedAscii::<32>::new(key_id.clone()).is_err() {
            return Self::FailClosed;
        }
        Self::SignedHmacSha256 {
            key,
            key_id,
            match_session_id,
        }
    }

    fn verify(&self, ticket: &[u8]) -> Result<ClientHelloAdmission, ClientHelloRejection> {
        match self {
            Self::FailClosed => Err(ClientHelloRejection::WardenAdmissionVerifierUnavailable),
            Self::SignedHmacSha256 {
                key,
                key_id,
                match_session_id,
            } => {
                let ticket = parse_warden_admission_ticket(ticket)?;
                verify_warden_admission_ticket(ticket, key, key_id, match_session_id)
            }
        }
    }
}

fn validate_client_hello(
    hello: &ClientHello,
    ticket_verifier: &ClientTicketVerifier,
    warden_gate: &WardenAdmissionGate,
) -> Result<ClientHelloAdmission, ClientHelloRejection> {
    if hello.protocol_version != GAME_PROTOCOL_VERSION {
        return Err(ClientHelloRejection::WrongProtocolVersion);
    }
    if hello.auth_ticket.is_empty() {
        return Err(ClientHelloRejection::MissingAuthTicket);
    }
    if hello.auth_ticket.len() > MAX_AUTH_TICKET_BYTES {
        return Err(ClientHelloRejection::AuthTicketTooLarge);
    }
    ticket_verifier.verify(&hello.auth_ticket)?;
    warden_gate.validate(hello)
}

fn parse_warden_admission_ticket(
    ticket: &[u8],
) -> Result<WardenAdmissionTicket, ClientHelloRejection> {
    serde_json::from_slice(ticket).map_err(|_| ClientHelloRejection::WardenAdmissionMalformed)
}

fn verify_warden_admission_ticket(
    ticket: WardenAdmissionTicket,
    key: &[u8; 32],
    key_id: &str,
    match_session_id: &str,
) -> Result<ClientHelloAdmission, ClientHelloRejection> {
    if ticket.signature.key_id.as_str() != key_id {
        return Err(ClientHelloRejection::WardenAdmissionKeyMismatch);
    }
    let verifier = HmacWardenAdmissionVerifier { key, key_id };
    let verified =
        VerifiedAdmissionTicket::verify(ticket, match_session_id, current_unix_ms(), &verifier)
            .map_err(map_warden_admission_verification_error)?;
    let decision = admission_decision_for_game_server(&verified)
        .map_err(|_| ClientHelloRejection::WardenAdmissionMalformed)?;
    admission_outcome_for_decision(decision)
}

fn admission_outcome_for_decision(
    decision: GameServerAdmissionDecision,
) -> Result<ClientHelloAdmission, ClientHelloRejection> {
    match decision.decision {
        WardenSessionDecisionKind::Allow | WardenSessionDecisionKind::ObserveOnly => {
            Ok(ClientHelloAdmission {
                warden_decision: decision.decision,
                untrusted_pool: false,
            })
        }
        WardenSessionDecisionKind::QuarantineToUntrustedPool => Ok(ClientHelloAdmission {
            warden_decision: decision.decision,
            untrusted_pool: true,
        }),
        WardenSessionDecisionKind::DenyMatchmaking => {
            Err(ClientHelloRejection::WardenAdmissionDeniedMatchmaking)
        }
        WardenSessionDecisionKind::DenySession => {
            Err(ClientHelloRejection::WardenAdmissionDeniedSession)
        }
    }
}

struct HmacWardenAdmissionVerifier<'a> {
    key: &'a [u8; 32],
    key_id: &'a str,
}

impl WardenAdmissionTicketSignatureVerifier for HmacWardenAdmissionVerifier<'_> {
    fn verify_admission_signature(&self, key_id: &str, payload: &[u8], signature: &[u8]) -> bool {
        if key_id != self.key_id {
            return false;
        }
        let hmac_key = hmac::Key::new(hmac::HMAC_SHA256, self.key);
        hmac::verify(&hmac_key, payload, signature).is_ok()
    }
}

fn map_warden_admission_verification_error(
    error: WardenAdmissionVerificationError,
) -> ClientHelloRejection {
    match error {
        WardenAdmissionVerificationError::UnsupportedSchemaVersion => {
            ClientHelloRejection::WardenAdmissionUnsupportedSchema
        }
        WardenAdmissionVerificationError::Expired => ClientHelloRejection::WardenAdmissionExpired,
        WardenAdmissionVerificationError::MatchSessionMismatch => {
            ClientHelloRejection::WardenAdmissionMatchSessionMismatch
        }
        WardenAdmissionVerificationError::EmptySignatureKeyId => {
            ClientHelloRejection::WardenAdmissionKeyMismatch
        }
        WardenAdmissionVerificationError::EmptySignature
        | WardenAdmissionVerificationError::SignatureInvalid => {
            ClientHelloRejection::WardenAdmissionSignatureMismatch
        }
        WardenAdmissionVerificationError::ProtectedSummaryInvalid => {
            ClientHelloRejection::WardenAdmissionMalformed
        }
    }
}

#[cfg(test)]
fn admission_ticket_signature_bytes(
    ticket: &WardenAdmissionTicket,
    key: &[u8; 32],
) -> Result<[u8; 32], ClientHelloRejection> {
    let bytes = fun_warden_server::admission_ticket_signing_payload(ticket);
    let key = hmac::Key::new(hmac::HMAC_SHA256, key);
    hmac::sign(&key, &bytes)
        .as_ref()
        .try_into()
        .map_err(|_| ClientHelloRejection::WardenAdmissionSignatureMismatch)
}

fn decode_hex_32(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }
    let mut bytes = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        bytes[index] = (high << 4) | low;
    }
    Some(bytes)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn current_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

fn client_can_receive_stream(stage: ClientAdmissionStage) -> bool {
    matches!(
        stage,
        ClientAdmissionStage::Admitted
            | ClientAdmissionStage::Streaming
            | ClientAdmissionStage::WorldReady
            | ClientAdmissionStage::InGame
    )
}

fn validate_world_ready_ack(
    stage: ClientAdmissionStage,
    ack: &WorldStreamAck,
    manifest: &ServerWorldStream,
) -> Result<(), WorldReadyRejection> {
    match stage {
        ClientAdmissionStage::Streaming => {}
        ClientAdmissionStage::WorldReady | ClientAdmissionStage::InGame => {
            return Err(WorldReadyRejection::DuplicateReady);
        }
        ClientAdmissionStage::Unauthenticated | ClientAdmissionStage::Admitted => {
            return Err(WorldReadyRejection::NotStreaming);
        }
    }

    let Some(first_chunk) = manifest.chunks.first() else {
        return Err(WorldReadyRejection::NoWorldStream);
    };
    if ack.level_id != first_chunk.level_id {
        return Err(WorldReadyRejection::WrongLevel);
    }
    if ack.revision.0 < manifest.revision.0 {
        return Err(WorldReadyRejection::StaleRevision);
    }
    if ack.revision.0 > manifest.revision.0 {
        return Err(WorldReadyRejection::FutureRevision);
    }
    if ack.chunk_count != first_chunk.chunk_count {
        return Err(WorldReadyRejection::WrongChunkCount);
    }
    if ack.manifest_signature != manifest.manifest_signature {
        return Err(WorldReadyRejection::WrongManifestSignature);
    }
    Ok(())
}

#[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
#[derive(Debug, Clone, Copy, Resource)]
struct ServerLogConfig {
    stream_verbose: bool,
    net_verbose: bool,
    benchmark_minimal: bool,
}

#[cfg_attr(not(all(feature = "diagnostics", debug_assertions)), allow(dead_code))]
impl ServerLogConfig {
    fn from_env() -> Self {
        Self {
            stream_verbose: std::env::var_os("FUN_LOG_STREAM_VERBOSE").is_some(),
            net_verbose: std::env::var_os("FUN_LOG_NET_VERBOSE").is_some(),
            benchmark_minimal: std::env::var_os("FUN_BENCHMARK_LOG_MINIMAL").is_some(),
        }
    }

    fn stream_verbose(self) -> bool {
        self.stream_verbose && !self.benchmark_minimal
    }

    fn net_verbose(self) -> bool {
        self.net_verbose && !self.benchmark_minimal
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Default, Resource)]
struct ServerWorldDiagnostics {
    logged_streamable_inventory: bool,
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Default, Resource)]
struct ServerProfiler {
    server_tick: u64,
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Resource)]
struct ServerEditorControlPlane {
    config: game_shared::EditorControlConfig,
    granted_capabilities: Vec<game_shared::EditorCapability>,
    diagnostic_streams: Vec<game_shared::EditorDiagnosticStream>,
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
impl Default for ServerEditorControlPlane {
    fn default() -> Self {
        let execute_server_code_enabled =
            std::env::var_os("FUN_EDITOR_ENABLE_SERVER_EXEC").is_some();

        Self {
            config: game_shared::EditorControlConfig::local_development(
                game_shared::EditorTargetKind::Server,
            ),
            granted_capabilities: game_shared::local_development_capabilities(
                game_shared::EditorTargetKind::Server,
                execute_server_code_enabled,
            ),
            diagnostic_streams: game_shared::default_server_editor_diagnostic_subscriptions(),
        }
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn log_server_editor_control_plane(control: Res<ServerEditorControlPlane>) {
    let execute_server_code_enabled = control
        .granted_capabilities
        .contains(&game_shared::EditorCapability::ExecuteServerCode);

    game_shared::fun_diag_info!(
        target: "fun::editor::control",
        protocol_version = game_shared::EDITOR_PROTOCOL_VERSION,
        target_kind = ?control.config.target_kind,
        bind_mode = ?control.config.bind_mode,
        bind_addr = control.config.bind_addr.as_str(),
        enabled = control.config.enabled,
        remote_control_permitted = control.config.permits_remote_editor_control(),
        execute_server_code_enabled = execute_server_code_enabled,
        command_apply_stage = game_shared::EDITOR_COMMAND_APPLY_STAGE,
        granted_capabilities = control.granted_capabilities.len(),
        diagnostic_streams = control.diagnostic_streams.len(),
        "server editor control plane ready"
    );
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn log_server_editor_schema(schema: Res<ServerEditorSchema>) {
    let critical_resources = schema
        .registry
        .resources
        .iter()
        .filter(|resource| {
            matches!(
                resource.policy,
                game_shared::EditorResourcePolicy::CriticalServerState
            )
        })
        .count();

    game_shared::fun_diag_info!(
        target: "fun::editor::schema",
        components = schema.registry.components.len(),
        resources = schema.registry.resources.len(),
        critical_resources,
        "server editor schema registered"
    );
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn begin_server_profiler_tick(mut profiler: ResMut<ServerProfiler>) {
    profiler.server_tick = profiler.server_tick.saturating_add(1);
    game_shared::fun_diag_trace!(
        target: "fun::server::profiler",
        server_tick = profiler.server_tick,
        client_id = tracing::field::Empty,
        packet_type = "server_tick",
        world_revision = tracing::field::Empty,
        chunk_index = tracing::field::Empty,
        chunk_count = tracing::field::Empty,
        bytes = 0usize,
        stage = "fixed_rate_update",
        duration_ns = 0u64,
        "server profiler event"
    );
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[derive(Debug, Clone, Copy)]
struct ServerProfilerEvent {
    stage: &'static str,
    packet_type: &'static str,
    client_id: Option<u64>,
    world_revision: Option<u64>,
    chunk_index: Option<u16>,
    chunk_count: Option<u16>,
    bytes: usize,
    duration_ns: u64,
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn server_profiler_event(profiler: &ServerProfiler, event: ServerProfilerEvent) {
    game_shared::fun_diag_info!(
        target: "fun::server::profiler",
        server_tick = profiler.server_tick,
        client_id = ?event.client_id,
        packet_type = event.packet_type,
        world_revision = ?event.world_revision,
        chunk_index = ?event.chunk_index,
        chunk_count = ?event.chunk_count,
        bytes = event.bytes,
        stage = event.stage,
        duration_ns = event.duration_ns,
        "server profiler event"
    );
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn elapsed_ns(started: Instant) -> u64 {
    started.elapsed().as_nanos().min(u128::from(u64::MAX)) as u64
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
#[allow(
    clippy::type_complexity,
    reason = "Bevy query tuple keeps streamable inventory diagnostics aligned with the exact ECS read set"
)]
fn log_streamable_inventory(
    mut diagnostics: ResMut<ServerWorldDiagnostics>,
    log_config: Res<ServerLogConfig>,
    query: Query<(
        Entity,
        Option<&Name>,
        Option<&NetworkIdentity>,
        Option<&Transform>,
        &StreamedWorldEntity,
    )>,
) {
    if diagnostics.logged_streamable_inventory || query.is_empty() || !log_config.stream_verbose() {
        return;
    }

    diagnostics.logged_streamable_inventory = true;
    game_shared::fun_diag_info!(
        target: "fun::server::stream",
        entities = query.iter().count(),
        "streamable inventory"
    );

    for (entity, name, identity, transform, streamed) in &query {
        game_shared::fun_diag_info!(
            target: "fun::server::stream::entity",
            entity = ?entity,
            name = name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
            identity = identity.map(|identity| identity.entity.0),
            transform = transform
                .map(|transform| format!(
                    "({:.2},{:.2},{:.2})",
                    transform.translation.x, transform.translation.y, transform.translation.z
                ))
                .unwrap_or_else(|| "missing".to_owned()),
            catalog = catalog_summary(streamed.catalog),
            render = render_summary(streamed.render),
            collider = collider_summary(streamed.collider),
            "streamable entity"
        );
    }
}

fn rebuild_world_stream(
    mut manifest: ResMut<ServerWorldStream>,
    mut pending: ResMut<PendingWorldStreams>,
    connected: Res<ConnectedClients>,
    admissions: Res<ClientAdmissionStates>,
    _log_config: Res<ServerLogConfig>,
    #[cfg(all(feature = "diagnostics", debug_assertions))] profiler: Res<ServerProfiler>,
    query: Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        &Transform,
        &StreamedWorldEntity,
        Option<&Name>,
    )>,
) {
    let entity_count = query.iter().count();
    if entity_count == 0 {
        clear_world_stream_if_empty(&mut manifest, &mut pending, _log_config.stream_verbose());
        return;
    }

    #[cfg(all(feature = "diagnostics", debug_assertions))]
    let rebuild_started = Instant::now();
    let mut specs = Vec::with_capacity(entity_count);
    for (identity, authority, transform, streamed, name) in &query {
        if !identity.entity.is_valid() {
            game_shared::fun_diag_warn!(
                target: "fun::server::stream",
                net_entity = identity.entity.0,
                "skipping invalid streamed network entity"
            );
            continue;
        }
        let editor_identity = game_shared::EditorVisibleEntityIdentity::from_network_identity(
            identity.entity,
            identity.class,
            authority.mode,
            name.map(|name| name.as_str()),
            game_shared::EditorEntityMutability::PersistentMutable,
        );

        specs.push(WorldEntitySpec {
            entity: editor_identity.entity,
            name: editor_identity
                .name
                .unwrap_or_else(|| format!("NetEntity-{}", editor_identity.entity.0)),
            class: editor_identity.replication_class,
            authority: editor_identity.authority,
            transform: fun_scene::qtransform(transform),
            catalog: streamed.catalog,
            render: streamed.render,
            collider: streamed.collider,
            color: streamed.color,
        });
    }

    specs.sort_by_key(|spec| spec.entity);
    if specs.is_empty() {
        clear_world_stream_if_empty(&mut manifest, &mut pending, _log_config.stream_verbose());
        return;
    }
    let manifest_signature = fun_scene::world_stream_manifest_signature(&specs);
    if manifest_signature == manifest.manifest_signature {
        return;
    }

    let next_revision = WorldRevision(manifest.revision.0.saturating_add(1).max(1));
    let entity_ids = specs.iter().map(|spec| spec.entity).collect();
    let Ok(chunks) = fun_scene::try_chunk_world_specs(DEMO_LEVEL_ID, next_revision, specs) else {
        game_shared::fun_diag_warn!(
            target: "fun::server::stream",
            revision = next_revision.0,
            "world stream exceeds the maximum supported chunk count"
        );
        return;
    };
    manifest.revision = next_revision;
    manifest.editor_revision = manifest.revision;
    manifest.manifest_signature = manifest_signature;
    manifest.entity_ids = entity_ids;
    manifest.chunks = chunks;
    pending.ids.clear();
    pending
        .ids
        .extend(connected.ids.iter().copied().filter(|client_id| {
            admissions
                .stages
                .get(client_id)
                .copied()
                .is_some_and(client_can_receive_stream)
        }));

    game_shared::fun_diag_info_if!(
        _log_config.stream_verbose(),
        target: "fun::server::stream",
        revision = manifest.revision.0,
        specs = manifest.entity_ids.len(),
        manifest_signature = manifest.manifest_signature,
        chunks = manifest.chunks.len(),
        pending_clients = pending.ids.len(),
        "built world stream"
    );
    #[cfg(all(feature = "diagnostics", debug_assertions))]
    {
        let duration_ns = elapsed_ns(rebuild_started);
        server_profiler_event(
            &profiler,
            ServerProfilerEvent {
                stage: "world_stream_rebuild",
                packet_type: "world_stream_manifest",
                client_id: None,
                world_revision: Some(manifest.revision.0),
                chunk_index: None,
                chunk_count: Some(manifest.chunks.len().min(u16::MAX as usize) as u16),
                bytes: 0,
                duration_ns,
            },
        );
        for chunk in &manifest.chunks {
            server_profiler_event(
                &profiler,
                ServerProfilerEvent {
                    stage: "world_stream_chunk_ready",
                    packet_type: "world_stream",
                    client_id: None,
                    world_revision: Some(chunk.revision.0),
                    chunk_index: Some(chunk.chunk_index.saturating_add(1)),
                    chunk_count: Some(chunk.chunk_count),
                    bytes: 0,
                    duration_ns: 0,
                },
            );
        }
    }
    game_shared::fun_diag_block_if!(_log_config.stream_verbose(), {
        for chunk in &manifest.chunks {
            game_shared::fun_diag_info!(
                target: "fun::server::stream",
                chunk_number = chunk.chunk_index + 1,
                chunk_count = chunk.chunk_count,
                level = %chunk.level_id.0,
                entities = chunk.entities.len(),
                "world stream chunk"
            );
        }
    });
}

fn clear_world_stream_if_empty(
    manifest: &mut ServerWorldStream,
    pending: &mut PendingWorldStreams,
    _stream_verbose: bool,
) {
    if manifest.clear_with_new_revision() {
        pending.ids.clear();
        game_shared::fun_diag_info_if!(
            _stream_verbose,
            target: "fun::server::stream",
            revision = manifest.revision.0,
            reason = "empty_world_stream",
            "cleared world stream"
        );
    }
}

fn queue_world_stream_for_new_clients(
    mut events: MessageReader<ConnectionEvent>,
    mut connected: ResMut<ConnectedClients>,
    mut ready: ResMut<ReadyClients>,
    mut relevance: ResMut<ClientRelevanceSets>,
    mut pending: ResMut<PendingWorldStreams>,
    mut admissions: ResMut<ClientAdmissionStates>,
    _log_config: Res<ServerLogConfig>,
) {
    for event in events.read() {
        connected.ids.insert(event.id);
        ready.ids.remove(&event.id);
        relevance.sets.remove(&event.id);
        pending.ids.remove(&event.id);
        admissions.warden_decisions.remove(&event.id);
        admissions.untrusted_pool.remove(&event.id);
        admissions
            .stages
            .insert(event.id, ClientAdmissionStage::Unauthenticated);
        game_shared::fun_diag_info_if!(
            _log_config.net_verbose(),
            target: "fun::server::net",
            client_id = event.id,
            "client connected"
        );
    }
}

fn cleanup_disconnected_clients(
    mut events: MessageReader<ConnectionLostEvent>,
    mut connected: ResMut<ConnectedClients>,
    mut ready: ResMut<ReadyClients>,
    mut relevance: ResMut<ClientRelevanceSets>,
    mut pending: ResMut<PendingWorldStreams>,
    mut admissions: ResMut<ClientAdmissionStates>,
    _log_config: Res<ServerLogConfig>,
) {
    for event in events.read() {
        clear_client_state(
            event.id,
            &mut connected,
            &mut ready,
            &mut relevance,
            &mut pending,
            &mut admissions,
        );
        game_shared::fun_diag_info_if!(
            _log_config.net_verbose(),
            target: "fun::server::net",
            client_id = event.id,
            "client disconnected"
        );
    }
}

fn clear_client_state(
    client_id: u64,
    connected: &mut ConnectedClients,
    ready: &mut ReadyClients,
    relevance: &mut ClientRelevanceSets,
    pending: &mut PendingWorldStreams,
    admissions: &mut ClientAdmissionStates,
) {
    connected.ids.remove(&client_id);
    ready.ids.remove(&client_id);
    relevance.sets.remove(&client_id);
    pending.ids.remove(&client_id);
    admissions.stages.remove(&client_id);
    admissions.warden_decisions.remove(&client_id);
    admissions.untrusted_pool.remove(&client_id);
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy system parameters are explicit resources for server admission and stream ownership"
)]
fn receive_client_control(
    mut server: ResMut<QuinnetServer>,
    mut connected: ResMut<ConnectedClients>,
    mut ready: ResMut<ReadyClients>,
    mut relevance: ResMut<ClientRelevanceSets>,
    mut pending: ResMut<PendingWorldStreams>,
    mut admissions: ResMut<ClientAdmissionStates>,
    manifest: Res<ServerWorldStream>,
    security: Res<GameServerSecurityConfig>,
    _log_config: Res<ServerLogConfig>,
    #[cfg(all(feature = "diagnostics", debug_assertions))] profiler: Res<ServerProfiler>,
) {
    let Some(endpoint) = server.get_endpoint_mut() else {
        return;
    };

    for client_id in endpoint.clients() {
        while let Some(payload) = endpoint.try_receive_payload(client_id, ClientChannel::Control) {
            let _bytes_len = payload.as_ref().len();
            game_shared::fun_diag_info_if!(
                _log_config.net_verbose(),
                target: "fun::server::net",
                client_id,
                bytes = _bytes_len,
                "received control payload"
            );
            #[cfg(all(feature = "diagnostics", debug_assertions))]
            let decode_started = Instant::now();
            match decode_client_packet_bounded(payload.as_ref(), MAX_CLIENT_CONTROL_PACKET_BYTES) {
                Ok(ClientPacket::Hello { hello }) => {
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "receive_decode",
                            packet_type: "client_hello",
                            client_id: Some(client_id),
                            world_revision: None,
                            chunk_index: None,
                            chunk_count: None,
                            bytes: _bytes_len,
                            duration_ns: elapsed_ns(decode_started),
                        },
                    );
                    if admissions
                        .stages
                        .get(&client_id)
                        .copied()
                        .unwrap_or(ClientAdmissionStage::Unauthenticated)
                        != ClientAdmissionStage::Unauthenticated
                    {
                        error!(target: "fun::server::net", client_id, "rejecting duplicate client hello");
                        endpoint.try_disconnect_client(client_id);
                        clear_client_state(
                            client_id,
                            &mut connected,
                            &mut ready,
                            &mut relevance,
                            &mut pending,
                            &mut admissions,
                        );
                        continue;
                    }
                    let admission = match validate_client_hello(
                        &hello,
                        &security.ticket_verifier,
                        &security.warden_gate,
                    ) {
                        Ok(admission) => admission,
                        Err(rejection) => {
                            error!(target: "fun::server::net", client_id, ?rejection, "rejecting invalid client hello");
                            endpoint.try_disconnect_client(client_id);
                            clear_client_state(
                                client_id,
                                &mut connected,
                                &mut ready,
                                &mut relevance,
                                &mut pending,
                                &mut admissions,
                            );
                            continue;
                        }
                    };
                    admissions
                        .stages
                        .insert(client_id, ClientAdmissionStage::Admitted);
                    admissions
                        .warden_decisions
                        .insert(client_id, admission.warden_decision);
                    if admission.untrusted_pool {
                        admissions.untrusted_pool.insert(client_id);
                    } else {
                        admissions.untrusted_pool.remove(&client_id);
                    }
                    pending.ids.insert(client_id);
                    if admission.warden_decision == WardenSessionDecisionKind::ObserveOnly {
                        info!(
                            target: "fun::server::net",
                            client_id,
                            "client admitted with Warden observe-only decision"
                        );
                    }
                    if admission.untrusted_pool {
                        info!(
                            target: "fun::server::net",
                            client_id,
                            "client routed to Warden untrusted admission pool"
                        );
                    }
                    game_shared::fun_diag_info_if!(
                        _log_config.net_verbose(),
                        target: "fun::server::net",
                        client_id,
                        "client completed Thunder hello"
                    );
                }
                Ok(ClientPacket::WorldReady { ack }) => {
                    let stage = admissions
                        .stages
                        .get(&client_id)
                        .copied()
                        .unwrap_or(ClientAdmissionStage::Unauthenticated);
                    if let Err(rejection) = validate_world_ready_ack(stage, &ack, &manifest) {
                        error!(target: "fun::server::net", client_id, ?rejection, "rejecting world-ready acknowledgement");
                        endpoint.try_disconnect_client(client_id);
                        clear_client_state(
                            client_id,
                            &mut connected,
                            &mut ready,
                            &mut relevance,
                            &mut pending,
                            &mut admissions,
                        );
                        continue;
                    }
                    admissions
                        .stages
                        .insert(client_id, ClientAdmissionStage::WorldReady);
                    ready.ids.insert(client_id);
                    relevance.sets.insert(
                        client_id,
                        ServerClientRelevanceSet {
                            tick: NetworkTick(manifest.editor_revision.0),
                            entities: manifest.entity_ids.iter().copied().collect(),
                        },
                    );
                    admissions
                        .stages
                        .insert(client_id, ClientAdmissionStage::InGame);
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "receive_decode",
                            packet_type: "client_world_ready",
                            client_id: Some(client_id),
                            world_revision: Some(ack.revision.0),
                            chunk_index: None,
                            chunk_count: None,
                            bytes: _bytes_len,
                            duration_ns: elapsed_ns(decode_started),
                        },
                    );
                    game_shared::fun_diag_info_if!(
                        _log_config.net_verbose(),
                        target: "fun::server::net",
                        client_id,
                        level = %ack.level_id.0,
                        revision = ack.revision.0,
                        chunk_count = ack.chunk_count,
                        manifest_signature = ack.manifest_signature,
                        "client loaded world"
                    );
                }
                Ok(_packet) => {
                    if admissions
                        .stages
                        .get(&client_id)
                        .copied()
                        .unwrap_or(ClientAdmissionStage::Unauthenticated)
                        == ClientAdmissionStage::Unauthenticated
                    {
                        error!(target: "fun::server::net", client_id, "rejecting control packet before client admission");
                        endpoint.try_disconnect_client(client_id);
                        clear_client_state(
                            client_id,
                            &mut connected,
                            &mut ready,
                            &mut relevance,
                            &mut pending,
                            &mut admissions,
                        );
                        continue;
                    }
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "receive_decode",
                            packet_type: "client_control",
                            client_id: Some(client_id),
                            world_revision: None,
                            chunk_index: None,
                            chunk_count: None,
                            bytes: _bytes_len,
                            duration_ns: elapsed_ns(decode_started),
                        },
                    );
                    game_shared::fun_diag_debug_if!(
                        _log_config.net_verbose(),
                        target: "fun::server::net",
                        client_id,
                        packet = ?_packet,
                        "ignoring client control packet"
                    );
                }
                Err(error) => {
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "receive_decode_error",
                            packet_type: "client_control_invalid",
                            client_id: Some(client_id),
                            world_revision: None,
                            chunk_index: None,
                            chunk_count: None,
                            bytes: _bytes_len,
                            duration_ns: elapsed_ns(decode_started),
                        },
                    );
                    error!(target: "fun::server::net", client_id, %error, "failed to decode client control packet");
                }
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "editor mutation application is a Bevy system with explicit resource/query access at one deterministic stage"
)]
fn apply_server_editor_mutations(
    inspector: Res<ServerEditorInspectorState>,
    schema: Res<ServerEditorSchema>,
    mut manifest: ResMut<ServerWorldStream>,
    mut server: ResMut<QuinnetServer>,
    ready: Res<ReadyClients>,
    relevance: Res<ClientRelevanceSets>,
    identities: Query<(&NetworkIdentity, &NetworkAuthority, Option<&Name>)>,
    mut transforms: Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        Option<&Name>,
        &mut Transform,
    )>,
) {
    let transactions = inspector.state.drain_mutations(32);
    if transactions.is_empty() {
        return;
    }

    let known_entities = identities
        .iter()
        .map(|(identity, authority, name)| {
            game_shared::EditorVisibleEntityIdentity::from_network_identity(
                identity.entity,
                identity.class,
                authority.mode,
                name.map(|name| name.as_str()),
                game_shared::EditorEntityMutability::PersistentMutable,
            )
        })
        .collect::<Vec<_>>();
    let granted_capabilities =
        game_shared::local_development_capabilities(game_shared::EditorTargetKind::Server, false);

    for transaction in transactions {
        let current_revision = game_shared::EditorWorldRevision(manifest.editor_revision.0);
        let transaction_id = transaction.transaction_id;
        let (status, applied_ops, conflict_count, mut diagnostics, editor_delta, snapshot_delta) =
            apply_server_transform_transaction(
                &transaction,
                current_revision,
                &schema,
                &known_entities,
                &granted_capabilities,
                &mut transforms,
            );

        let world_revision = if status == game_shared::EditorMutationStatus::Accepted {
            manifest.editor_revision =
                WorldRevision(manifest.editor_revision.0.saturating_add(1).max(1));
            game_shared::EditorWorldRevision(manifest.editor_revision.0)
        } else {
            current_revision
        };

        let mut events = Vec::new();
        let ack = game_shared::EditorMutationAck {
            header: editor_response_header(&transaction.header, world_revision),
            transaction_id,
            status,
            world_revision,
            applied_ops,
            conflict_count,
            diagnostics: diagnostics.clone(),
        };
        events.push(game_shared::EditorEventPayload::MutationAck { ack });

        if let Some(mut delta) = editor_delta {
            delta.world_revision = world_revision;
            events.push(game_shared::EditorEventPayload::EntityDelta { delta });
        }

        let relevant_clients = snapshot_delta
            .as_ref()
            .map(|delta| {
                send_editor_snapshot_delta(&mut server, &ready, &relevance, world_revision, delta)
            })
            .unwrap_or(0);

        diagnostics.push(editor_mutation_diagnostic(
            game_shared::DiagnosticLevel::Info,
            "mutation_relevant_clients",
            transaction_id,
            world_revision,
            format!("relevant_clients={relevant_clients}"),
        ));

        for diagnostic in diagnostics {
            inspector
                .state
                .emit_diagnostic(game_shared::DiagnosticPacket::Event { event: diagnostic });
        }
        inspector
            .state
            .emit_diagnostic(game_shared::DiagnosticPacket::Counter {
                counter: game_shared::DiagnosticCounter {
                    target: "server".to_owned(),
                    name: "editor_mutation_applied_ops".to_owned(),
                    value: game_shared::DiagnosticValue::U64 {
                        value: u64::from(applied_ops),
                    },
                    unit: game_shared::DiagnosticUnit::Count,
                    window: game_shared::DiagnosticWindow {
                        sample_count: 1,
                        duration_ns: 0,
                    },
                },
            });

        inspector
            .state
            .complete_mutation(transaction_id, world_revision, events);
    }
}

fn apply_server_transform_transaction(
    transaction: &game_shared::EditorMutationTransaction,
    current_revision: game_shared::EditorWorldRevision,
    schema: &ServerEditorSchema,
    known_entities: &[game_shared::EditorVisibleEntityIdentity],
    granted_capabilities: &[game_shared::EditorCapability],
    transforms: &mut Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        Option<&Name>,
        &mut Transform,
    )>,
) -> (
    game_shared::EditorMutationStatus,
    u32,
    u32,
    Vec<game_shared::EditorDiagnosticEvent>,
    Option<game_shared::EditorEntityDelta>,
    Option<EntityDelta>,
) {
    let transaction_id = transaction.transaction_id;
    let execution_budget = game_shared::EditorExecutionBudget {
        max_ops: 1,
        used_ops: 0,
    };
    if execution_budget
        .consume_ops(transaction.ops.len().min(u32::MAX as usize) as u32)
        .is_none()
    {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_budget_exceeded",
            "server Transform patch accepts one operation per transaction",
        );
    }
    if transaction.ops.len() != 1 {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_invalid_op_count",
            "server Transform patch accepts exactly one operation",
        );
    }

    let game_shared::EditorMutationOp::PatchEntity { patch } = &transaction.ops[0] else {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_unsupported_op",
            "server Transform patch only supports PatchEntity",
        );
    };

    if patch.component != game_shared::EDITOR_COMPONENT_KIND_TRANSFORM {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_unsupported_component",
            "first mutation path only accepts Transform",
        );
    }

    if let Err(error) = game_shared::validate_component_mutation(
        schema.registry,
        game_shared::EditorComponentMutationRequest {
            entity: patch.entity,
            component_kind: patch.component,
            payload: &patch.payload,
            granted_capabilities,
            base_world_revision: transaction.base_world_revision,
            current_world_revision: current_revision,
            known_entities,
            execution_budget,
        },
    ) {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_schema_validation_failed",
            format!("schema validation failed: {error:?}"),
        );
    }

    let Some(transform_patch) = game_shared::decode_editor_transform_patch(&patch.payload) else {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_payload_decode_failed",
            "Transform patch payload did not decode",
        );
    };

    let Some((identity, authority, name, mut transform)) = transforms
        .iter_mut()
        .find(|(identity, _, _, _)| identity.entity == patch.entity)
    else {
        return rejected_mutation(
            transaction_id,
            current_revision,
            "mutation_transform_missing",
            "entity exists but does not expose a mutable Transform component",
        );
    };

    let translation = transform_patch.translation();
    let rotation = transform_patch.rotation_xyzw();
    let scale = transform_patch.scale();
    transform.translation = Vec3::from_array(translation);
    let rotation_quat = Quat::from_xyzw(rotation[0], rotation[1], rotation[2], rotation[3]);
    let rotation_len = rotation_quat.length_squared();
    transform.rotation = if rotation_len.is_finite() && rotation_len > 0.0 {
        rotation_quat.normalize()
    } else {
        Quat::IDENTITY
    };
    transform.scale = Vec3::from_array(scale);

    let diagnostic = editor_mutation_diagnostic(
        game_shared::DiagnosticLevel::Info,
        "mutation_transform_applied",
        transaction_id,
        current_revision,
        format!(
            "entity={} name={} translation=({:.3},{:.3},{:.3})",
            patch.entity.0,
            name.map(|name| name.as_str()).unwrap_or("<unnamed>"),
            translation[0],
            translation[1],
            translation[2]
        ),
    );
    let editor_delta = game_shared::EditorEntityDelta {
        world_revision: current_revision,
        entity: patch.entity,
        components: vec![game_shared::EditorComponentPayload {
            component: patch.component,
            payload: patch.payload.clone(),
        }],
    };
    let snapshot_delta = EntityDelta {
        entity: patch.entity,
        class: identity.class,
        authority: authority.mode,
        transform: Some(fun_scene::qtransform(&transform)),
        body: None,
        components: Vec::new(),
    };

    (
        game_shared::EditorMutationStatus::Accepted,
        1,
        0,
        vec![diagnostic],
        Some(editor_delta),
        Some(snapshot_delta),
    )
}

fn rejected_mutation(
    transaction_id: game_shared::EditorTransactionId,
    world_revision: game_shared::EditorWorldRevision,
    code: &'static str,
    message: impl Into<String>,
) -> (
    game_shared::EditorMutationStatus,
    u32,
    u32,
    Vec<game_shared::EditorDiagnosticEvent>,
    Option<game_shared::EditorEntityDelta>,
    Option<EntityDelta>,
) {
    (
        game_shared::EditorMutationStatus::RejectedValidation,
        0,
        1,
        vec![editor_mutation_diagnostic(
            game_shared::DiagnosticLevel::Warn,
            code,
            transaction_id,
            world_revision,
            message,
        )],
        None,
        None,
    )
}

fn send_editor_snapshot_delta(
    server: &mut QuinnetServer,
    ready: &ReadyClients,
    relevance: &ClientRelevanceSets,
    world_revision: game_shared::EditorWorldRevision,
    delta: &EntityDelta,
) -> usize {
    if ready.ids.is_empty() {
        return 0;
    }
    let packet = ServerPacket::Snapshot {
        snapshot: SnapshotPacket {
            sequence: PacketSequence((world_revision.0 & u64::from(u32::MAX)) as u32),
            server_tick: NetworkTick(world_revision.0),
            baseline_tick: None,
            last_processed_input: PacketSequence(0),
            budget: SnapshotBudget {
                max_bytes: 1_100,
                max_entities: 1,
                max_component_deltas: 0,
            },
            entities: vec![delta.clone()],
        },
    };
    let Ok(bytes) = encode_server_packet(&packet) else {
        return 0;
    };
    let Some(endpoint) = server.get_endpoint_mut() else {
        return 0;
    };

    let mut sent = 0usize;
    for client_id in &ready.ids {
        let relevant = relevance
            .sets
            .get(client_id)
            .map(|set| set.tick.0 <= world_revision.0 && set.entities.contains(&delta.entity))
            .unwrap_or(false);
        if !relevant {
            continue;
        }
        endpoint.try_send_payload_on(*client_id, ServerChannel::Snapshot, bytes.clone());
        sent = sent.saturating_add(1);
    }
    sent
}

fn editor_response_header(
    request: &game_shared::EditorPacketHeader,
    world_revision: game_shared::EditorWorldRevision,
) -> game_shared::EditorPacketHeader {
    game_shared::EditorPacketHeader::new(
        request.request_id,
        request.target,
        PacketSequence(request.sequence.0.saturating_add(1)),
        Some(world_revision),
        request.size_budget,
    )
}

fn editor_mutation_diagnostic(
    level: game_shared::DiagnosticLevel,
    code: &'static str,
    transaction_id: game_shared::EditorTransactionId,
    world_revision: game_shared::EditorWorldRevision,
    message: impl Into<String>,
) -> game_shared::EditorDiagnosticEvent {
    game_shared::DiagnosticEvent {
        target: "server".to_owned(),
        level,
        timestamp_or_tick: game_shared::DiagnosticTimestampOrTick::TimestampNs { ns: unix_ns() },
        fields: vec![
            game_shared::DiagnosticField::text("stream", "mutation_transactions"),
            game_shared::DiagnosticField::text("stage", game_shared::EDITOR_COMMAND_APPLY_STAGE),
            game_shared::DiagnosticField::text("code", code),
            game_shared::DiagnosticField::u64("transaction_id", transaction_id.0),
            game_shared::DiagnosticField::u64("world_revision", world_revision.0),
            game_shared::DiagnosticField::text("message", message),
        ],
        source: game_shared::DiagnosticSource::static_location(file!(), line!(), module_path!()),
        frame_index: None,
        span_id: None,
    }
}

#[allow(
    clippy::type_complexity,
    reason = "Bevy query tuple documents the authoritative server components exposed to the editor snapshot"
)]
fn update_server_editor_inspector_snapshot(
    inspector: Res<ServerEditorInspectorState>,
    manifest: Res<ServerWorldStream>,
    query: Query<(
        &NetworkIdentity,
        &NetworkAuthority,
        Option<&Name>,
        Option<&Transform>,
        Option<&StreamedWorldEntity>,
    )>,
    mut last_diagnostic_revision: Local<u64>,
) {
    let schema_revision = game_shared::EditorSchemaRevision(1);
    let world_revision = game_shared::EditorWorldRevision(manifest.editor_revision.0);
    let schemas = server_editor_component_schemas();
    let mut rows = Vec::with_capacity(query.iter().count());

    for (identity, authority, name, transform, streamed) in &query {
        if !identity.entity.is_valid() {
            continue;
        }

        let mut components = Vec::with_capacity(5);
        if let Some(transform) = transform {
            components.push(editor_component_value(
                game_shared::EDITOR_COMPONENT_KIND_TRANSFORM,
                schema_revision,
                transform_preview(transform),
            ));
        }
        if let Some(name) = name {
            components.push(editor_component_value(
                game_shared::EDITOR_COMPONENT_KIND_NAME,
                schema_revision,
                name.as_str().to_owned(),
            ));
        }
        components.push(editor_component_value(
            game_shared::EDITOR_COMPONENT_KIND_NETWORK_IDENTITY,
            schema_revision,
            format!(
                "net={} shard={} local={} class={:?}",
                identity.entity.0,
                identity.entity.shard(),
                identity.entity.local(),
                identity.class
            ),
        ));
        components.push(editor_component_value(
            game_shared::EDITOR_COMPONENT_KIND_NETWORK_AUTHORITY,
            schema_revision,
            format!("{:?}", authority.mode),
        ));
        if let Some(streamed) = streamed {
            components.push(editor_component_value(
                game_shared::EDITOR_COMPONENT_KIND_WORLD_CATALOG_REF,
                schema_revision,
                streamed_world_preview(streamed),
            ));
        }

        rows.push(game_shared::EditorEntityRow {
            entity: identity.entity,
            target: game_shared::EditorTargetKind::Server,
            display_label: name
                .map(|name| name.as_str().to_owned())
                .unwrap_or_else(|| format!("NetEntity-{}", identity.entity.0)),
            component_count: components.len().min(u16::MAX as usize) as u16,
            schema_revision,
            components,
        });
    }

    inspector
        .state
        .replace_entities(world_revision, schema_revision, schemas, rows);

    if inspector.state.editor_attached() && *last_diagnostic_revision != world_revision.0 {
        *last_diagnostic_revision = world_revision.0;
        inspector
            .state
            .emit_diagnostic(game_shared::DiagnosticPacket::Counter {
                counter: game_shared::DiagnosticCounter {
                    target: "server".to_owned(),
                    name: "world_revision".to_owned(),
                    value: game_shared::DiagnosticValue::U64 {
                        value: world_revision.0,
                    },
                    unit: game_shared::DiagnosticUnit::Count,
                    window: game_shared::DiagnosticWindow {
                        sample_count: 1,
                        duration_ns: 0,
                    },
                },
            });
    }
}

fn editor_component_value(
    component_kind: ComponentKind,
    schema_revision: game_shared::EditorSchemaRevision,
    value_preview: String,
) -> game_shared::EditorEntityComponentValue {
    game_shared::EditorEntityComponentValue {
        component_kind,
        schema_revision,
        value_preview,
        raw_payload: Vec::new(),
    }
}

fn transform_preview(transform: &Transform) -> String {
    format!(
        "translation=({:.2},{:.2},{:.2}) rotation=({:.3},{:.3},{:.3},{:.3}) scale=({:.2},{:.2},{:.2})",
        transform.translation.x,
        transform.translation.y,
        transform.translation.z,
        transform.rotation.x,
        transform.rotation.y,
        transform.rotation.z,
        transform.rotation.w,
        transform.scale.x,
        transform.scale.y,
        transform.scale.z
    )
}

fn streamed_world_preview(streamed: &StreamedWorldEntity) -> String {
    format!(
        "catalog={} render={:?} collider={:?} color={:?}",
        streamed
            .catalog
            .map(|catalog| format!(
                "asset={} material={} collider={}",
                catalog.asset_id, catalog.material_id, catalog.collider_id
            ))
            .unwrap_or_else(|| "none".to_owned()),
        streamed.render,
        streamed.collider,
        streamed.color
    )
}

#[allow(
    clippy::too_many_arguments,
    reason = "Bevy system parameters keep world-stream send state split by authority boundary"
)]
fn send_pending_world_streams(
    mut server: ResMut<QuinnetServer>,
    mut pending: ResMut<PendingWorldStreams>,
    mut ready: ResMut<ReadyClients>,
    mut relevance: ResMut<ClientRelevanceSets>,
    mut admissions: ResMut<ClientAdmissionStates>,
    manifest: Res<ServerWorldStream>,
    _log_config: Res<ServerLogConfig>,
    #[cfg(all(feature = "diagnostics", debug_assertions))] profiler: Res<ServerProfiler>,
) {
    if manifest.chunks.is_empty() || pending.ids.is_empty() {
        return;
    }

    let Some(endpoint) = server.get_endpoint_mut() else {
        return;
    };

    let pending_clients = pending.ids.iter().copied().collect::<Vec<_>>();
    for client_id in pending_clients {
        let stage = admissions
            .stages
            .get(&client_id)
            .copied()
            .unwrap_or(ClientAdmissionStage::Unauthenticated);
        if !client_can_receive_stream(stage) {
            pending.ids.remove(&client_id);
            continue;
        }
        let warden_pool = if admissions.untrusted_pool.contains(&client_id) {
            "untrusted"
        } else {
            "protected"
        };
        let _ = warden_pool;
        ready.ids.remove(&client_id);
        relevance.sets.remove(&client_id);
        game_shared::fun_diag_info_if!(
            _log_config.stream_verbose(),
            target: "fun::server::stream",
            revision = manifest.revision.0,
            client_id,
            warden_pool,
            chunks = manifest.chunks.len(),
            "sending world stream"
        );
        let welcome = ServerPacket::Welcome {
            welcome: ServerWelcome {
                client_id: NetClientId(client_id),
                server_tick: NetworkTick::ZERO,
                feature_bits: 0,
                baseline_tick: NetworkTick::ZERO,
            },
        };

        #[cfg(all(feature = "diagnostics", debug_assertions))]
        let welcome_started = Instant::now();
        match encode_server_packet(&welcome) {
            Ok(bytes) => {
                let _byte_len = bytes.len();
                endpoint.try_send_payload_on(client_id, ServerChannel::Control, bytes);
                #[cfg(all(feature = "diagnostics", debug_assertions))]
                server_profiler_event(
                    &profiler,
                    ServerProfilerEvent {
                        stage: "send_world_stream",
                        packet_type: "server_welcome",
                        client_id: Some(client_id),
                        world_revision: Some(manifest.revision.0),
                        chunk_index: None,
                        chunk_count: Some(manifest.chunks.len().min(u16::MAX as usize) as u16),
                        bytes: _byte_len,
                        duration_ns: elapsed_ns(welcome_started),
                    },
                );
                game_shared::fun_diag_info_if!(
                    _log_config.stream_verbose(),
                    target: "fun::server::stream",
                    client_id,
                    bytes = _byte_len,
                    "sent welcome"
                );
            }
            Err(error) => {
                #[cfg(all(feature = "diagnostics", debug_assertions))]
                server_profiler_event(
                    &profiler,
                    ServerProfilerEvent {
                        stage: "send_world_stream_error",
                        packet_type: "server_welcome",
                        client_id: Some(client_id),
                        world_revision: Some(manifest.revision.0),
                        chunk_index: None,
                        chunk_count: Some(manifest.chunks.len().min(u16::MAX as usize) as u16),
                        bytes: 0,
                        duration_ns: elapsed_ns(welcome_started),
                    },
                );
                error!(target: "fun::server::stream", client_id, %error, "failed to encode welcome");
                continue;
            }
        }

        let mut sent_all_chunks = true;
        for chunk in &manifest.chunks {
            let packet = ServerPacket::WorldStream {
                chunk: chunk.clone(),
            };
            #[cfg(all(feature = "diagnostics", debug_assertions))]
            let chunk_started = Instant::now();
            match encode_server_packet(&packet) {
                Ok(bytes) => {
                    let _byte_len = bytes.len();
                    endpoint.try_send_payload_on(client_id, ServerChannel::Stream, bytes);
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "send_world_stream",
                            packet_type: "world_stream",
                            client_id: Some(client_id),
                            world_revision: Some(chunk.revision.0),
                            chunk_index: Some(chunk.chunk_index.saturating_add(1)),
                            chunk_count: Some(chunk.chunk_count),
                            bytes: _byte_len,
                            duration_ns: elapsed_ns(chunk_started),
                        },
                    );
                    game_shared::fun_diag_info_if!(
                        _log_config.stream_verbose(),
                        target: "fun::server::stream",
                        chunk_number = chunk.chunk_index + 1,
                        chunk_count = chunk.chunk_count,
                        client_id,
                        entities = chunk.entities.len(),
                        bytes = _byte_len,
                        "sent stream chunk"
                    );
                }
                Err(error) => {
                    #[cfg(all(feature = "diagnostics", debug_assertions))]
                    server_profiler_event(
                        &profiler,
                        ServerProfilerEvent {
                            stage: "send_world_stream_error",
                            packet_type: "world_stream",
                            client_id: Some(client_id),
                            world_revision: Some(chunk.revision.0),
                            chunk_index: Some(chunk.chunk_index.saturating_add(1)),
                            chunk_count: Some(chunk.chunk_count),
                            bytes: 0,
                            duration_ns: elapsed_ns(chunk_started),
                        },
                    );
                    error!(target: "fun::server::stream", client_id, %error, "failed to encode world stream");
                    sent_all_chunks = false;
                    break;
                }
            }
        }

        if !sent_all_chunks {
            continue;
        }
        pending.ids.remove(&client_id);
        admissions
            .stages
            .insert(client_id, ClientAdmissionStage::Streaming);
        game_shared::fun_diag_info_if!(
            _log_config.stream_verbose(),
            target: "fun::server::stream",
            revision = manifest.revision.0,
            client_id,
            "completed world stream"
        );
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn catalog_summary(catalog: Option<WorldCatalogRef>) -> String {
    catalog
        .map(|catalog| {
            format!(
                "asset={} material={} collider={}",
                catalog.asset_id, catalog.material_id, catalog.collider_id
            )
        })
        .unwrap_or_else(|| "none".to_owned())
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn render_summary(render: Option<WorldPrimitive>) -> String {
    match render {
        Some(WorldPrimitive::Plane { size }) => {
            let size = size.to_f32(Quantization::MILLIMETERS);
            format!("plane({:.2},{:.2},{:.2})", size[0], size[1], size[2])
        }
        Some(WorldPrimitive::Cuboid { size }) => {
            let size = size.to_f32(Quantization::MILLIMETERS);
            format!("cuboid({:.2},{:.2},{:.2})", size[0], size[1], size[2])
        }
        None => "none".to_owned(),
    }
}

#[cfg(all(feature = "diagnostics", debug_assertions))]
fn collider_summary(collider: Option<WorldCollider>) -> String {
    match collider {
        Some(WorldCollider::Cuboid { size }) => {
            let size = size.to_f32(Quantization::MILLIMETERS);
            format!("cuboid({:.2},{:.2},{:.2})", size[0], size[1], size[2])
        }
        None => "none".to_owned(),
    }
}
fn unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_warden_protocol::{
        Digest32, IntegrityStatus, MatchSessionId, ProtectedProtectionProfile,
        PseudonymousWardenSubjectId, WARDEN_PROTOCOL_SCHEMA_VERSION, WardenAdmissionKeyId,
        WardenAdmissionProtectedSummary, WardenDecisionReasonClass, WardenPolicyMode,
    };

    fn test_manifest() -> ServerWorldStream {
        ServerWorldStream {
            revision: WorldRevision(9),
            editor_revision: WorldRevision(9),
            manifest_signature: 0x1234_5678_9abc_def0,
            entity_ids: vec![NetEntity(1)],
            chunks: vec![WorldStreamChunk {
                level_id: WorldLevelId(DEMO_LEVEL_ID.to_owned()),
                revision: WorldRevision(9),
                chunk_index: 0,
                chunk_count: 2,
                manifest_signature: 0x1234_5678_9abc_def0,
                entities: Vec::new(),
            }],
        }
    }

    fn valid_ack() -> WorldStreamAck {
        WorldStreamAck {
            level_id: WorldLevelId(DEMO_LEVEL_ID.to_owned()),
            revision: WorldRevision(9),
            chunk_count: 2,
            manifest_signature: 0x1234_5678_9abc_def0,
        }
    }

    const TEST_WARDEN_KEY: [u8; 32] = [9; 32];
    const TEST_WARDEN_KEY_ID: &str = "server_warden_lookup_key:v1";
    const TEST_MATCH_SESSION_ID: &str = "match-local-01";

    fn test_hello(auth_ticket: Vec<u8>, warden_admission_ticket: Vec<u8>) -> ClientHello {
        ClientHello {
            protocol_version: GAME_PROTOCOL_VERSION,
            auth_ticket,
            warden_admission_ticket,
            feature_bits: 0,
            oldest_input_sequence: PacketSequence(0),
        }
    }

    fn signed_warden_ticket(
        decision: WardenSessionDecisionKind,
        match_session_id: &str,
        key_id: &str,
        allowed_until_ms: u64,
    ) -> Vec<u8> {
        signed_warden_ticket_with_protected(
            decision,
            match_session_id,
            key_id,
            allowed_until_ms,
            true,
        )
    }

    fn signed_warden_ticket_with_protected(
        decision: WardenSessionDecisionKind,
        match_session_id: &str,
        key_id: &str,
        allowed_until_ms: u64,
        include_protected_summary: bool,
    ) -> Vec<u8> {
        let mut ticket = WardenAdmissionTicket {
            schema_version: WARDEN_PROTOCOL_SCHEMA_VERSION,
            pseudonymous_subject_id: PseudonymousWardenSubjectId::new(String::from(
                "subject-opaque",
            ))
            .expect("subject"),
            match_session_id: MatchSessionId::new(String::from(match_session_id)).expect("match"),
            warden_policy_mode: WardenPolicyMode::Protect,
            allowed_until_ms,
            decision,
            reason_class: WardenDecisionReasonClass::Clean,
            protected: include_protected_summary.then_some(WardenAdmissionProtectedSummary {
                protected_profile: ProtectedProtectionProfile::Ranked,
                protected_bundle_digest: Digest32([9; 32]),
                protected_integrity_status: IntegrityStatus::Passed,
            }),
            signature: fun_warden_protocol::WardenAdmissionSignature {
                key_id: WardenAdmissionKeyId::new(String::from(key_id)).expect("key id"),
                signature_bytes: fun_warden_protocol::BoundedVec::empty(),
            },
        };
        let signature =
            admission_ticket_signature_bytes(&ticket, &TEST_WARDEN_KEY).expect("signature");
        ticket.signature.signature_bytes =
            fun_warden_protocol::BoundedVec::new(signature.to_vec()).expect("signature bytes");
        serde_json::to_vec(&ticket).expect("ticket json")
    }

    #[test]
    fn client_hello_requires_protocol_auth_ticket_and_ticket_verification() {
        let verifier = ClientTicketVerifier::DevelopmentToken {
            token: b"dev-ticket".to_vec(),
        };
        let valid = test_hello(b"dev-ticket".to_vec(), Vec::new());
        assert_eq!(
            validate_client_hello(&valid, &verifier, &WardenAdmissionGate::Observe),
            Ok(ClientHelloAdmission {
                warden_decision: WardenSessionDecisionKind::ObserveOnly,
                untrusted_pool: false,
            })
        );

        let mut missing_token = valid.clone();
        missing_token.auth_ticket.clear();
        assert_eq!(
            validate_client_hello(&missing_token, &verifier, &WardenAdmissionGate::Observe),
            Err(ClientHelloRejection::MissingAuthTicket)
        );

        let mut wrong_version = valid.clone();
        wrong_version.protocol_version = GAME_PROTOCOL_VERSION + 1;
        assert_eq!(
            validate_client_hello(&wrong_version, &verifier, &WardenAdmissionGate::Observe),
            Err(ClientHelloRejection::WrongProtocolVersion)
        );

        let mut wrong_ticket = valid;
        wrong_ticket.auth_ticket = b"wrong-ticket".to_vec();
        assert_eq!(
            validate_client_hello(&wrong_ticket, &verifier, &WardenAdmissionGate::Observe),
            Err(ClientHelloRejection::DevelopmentTicketMismatch)
        );

        assert_eq!(
            validate_client_hello(
                &wrong_ticket,
                &ClientTicketVerifier::FailClosed,
                &WardenAdmissionGate::Observe,
            ),
            Err(ClientHelloRejection::TicketVerificationUnavailable)
        );
    }

    #[test]
    fn warden_admission_gate_fails_closed_until_backend_ticket_verifier_exists() {
        let verifier = ClientTicketVerifier::DevelopmentToken {
            token: b"dev-ticket".to_vec(),
        };
        let hello = test_hello(b"dev-ticket".to_vec(), b"opaque-ticket".to_vec());
        let required_gate = WardenAdmissionGate::RequireAdmissionTicket {
            verifier: WardenAdmissionTicketVerifier::FailClosed,
        };

        assert_eq!(
            validate_client_hello(&hello, &verifier, &required_gate),
            Err(ClientHelloRejection::WardenAdmissionVerifierUnavailable)
        );
    }

    #[test]
    fn warden_admission_gate_verifies_signed_ticket_decisions() {
        let verifier = ClientTicketVerifier::DevelopmentToken {
            token: b"dev-ticket".to_vec(),
        };
        let ticket = signed_warden_ticket(
            WardenSessionDecisionKind::Allow,
            TEST_MATCH_SESSION_ID,
            TEST_WARDEN_KEY_ID,
            current_unix_ms().saturating_add(60_000),
        );
        let hello = test_hello(b"dev-ticket".to_vec(), ticket);
        let gate = WardenAdmissionGate::RequireAdmissionTicket {
            verifier: WardenAdmissionTicketVerifier::SignedHmacSha256 {
                key: TEST_WARDEN_KEY,
                key_id: String::from(TEST_WARDEN_KEY_ID),
                match_session_id: String::from(TEST_MATCH_SESSION_ID),
            },
        };

        assert_eq!(
            validate_client_hello(&hello, &verifier, &gate),
            Ok(ClientHelloAdmission {
                warden_decision: WardenSessionDecisionKind::Allow,
                untrusted_pool: false,
            })
        );
    }

    #[test]
    fn warden_admission_gate_rejects_match_mismatch_and_denied_sessions() {
        let verifier = ClientTicketVerifier::DevelopmentToken {
            token: b"dev-ticket".to_vec(),
        };
        let ticket = signed_warden_ticket(
            WardenSessionDecisionKind::DenySession,
            TEST_MATCH_SESSION_ID,
            TEST_WARDEN_KEY_ID,
            current_unix_ms().saturating_add(60_000),
        );
        let hello = test_hello(b"dev-ticket".to_vec(), ticket);
        let wrong_match_gate = WardenAdmissionGate::RequireAdmissionTicket {
            verifier: WardenAdmissionTicketVerifier::SignedHmacSha256 {
                key: TEST_WARDEN_KEY,
                key_id: String::from(TEST_WARDEN_KEY_ID),
                match_session_id: String::from("other-match"),
            },
        };
        let deny_gate = WardenAdmissionGate::RequireAdmissionTicket {
            verifier: WardenAdmissionTicketVerifier::SignedHmacSha256 {
                key: TEST_WARDEN_KEY,
                key_id: String::from(TEST_WARDEN_KEY_ID),
                match_session_id: String::from(TEST_MATCH_SESSION_ID),
            },
        };

        assert_eq!(
            validate_client_hello(&hello, &verifier, &wrong_match_gate),
            Err(ClientHelloRejection::WardenAdmissionMatchSessionMismatch)
        );
        assert_eq!(
            validate_client_hello(&hello, &verifier, &deny_gate),
            Err(ClientHelloRejection::WardenAdmissionDeniedSession)
        );
    }

    #[test]
    fn warden_admission_gate_routes_quarantine_without_rejecting_session() {
        let verifier = ClientTicketVerifier::DevelopmentToken {
            token: b"dev-ticket".to_vec(),
        };
        let ticket = signed_warden_ticket(
            WardenSessionDecisionKind::QuarantineToUntrustedPool,
            TEST_MATCH_SESSION_ID,
            TEST_WARDEN_KEY_ID,
            current_unix_ms().saturating_add(60_000),
        );
        let hello = test_hello(b"dev-ticket".to_vec(), ticket);
        let gate = WardenAdmissionGate::RequireAdmissionTicket {
            verifier: WardenAdmissionTicketVerifier::SignedHmacSha256 {
                key: TEST_WARDEN_KEY,
                key_id: String::from(TEST_WARDEN_KEY_ID),
                match_session_id: String::from(TEST_MATCH_SESSION_ID),
            },
        };

        assert_eq!(
            validate_client_hello(&hello, &verifier, &gate),
            Ok(ClientHelloAdmission {
                warden_decision: WardenSessionDecisionKind::QuarantineToUntrustedPool,
                untrusted_pool: true,
            })
        );
    }

    #[test]
    fn warden_admission_gate_rejects_missing_required_protected_summary() {
        let verifier = ClientTicketVerifier::DevelopmentToken {
            token: b"dev-ticket".to_vec(),
        };
        let ticket = signed_warden_ticket_with_protected(
            WardenSessionDecisionKind::Allow,
            TEST_MATCH_SESSION_ID,
            TEST_WARDEN_KEY_ID,
            current_unix_ms().saturating_add(60_000),
            false,
        );
        let hello = test_hello(b"dev-ticket".to_vec(), ticket);
        let gate = WardenAdmissionGate::RequireAdmissionTicket {
            verifier: WardenAdmissionTicketVerifier::SignedHmacSha256 {
                key: TEST_WARDEN_KEY,
                key_id: String::from(TEST_WARDEN_KEY_ID),
                match_session_id: String::from(TEST_MATCH_SESSION_ID),
            },
        };

        assert_eq!(
            validate_client_hello(&hello, &verifier, &gate),
            Err(ClientHelloRejection::WardenAdmissionMalformed)
        );
    }

    #[test]
    fn production_tls_mode_never_generates_self_signed_material() {
        assert!(matches!(
            certificate_mode_for_tls(GameServerTlsMode::ProductionConfigured),
            CertificateRetrievalMode::LoadFromFile { .. }
        ));
        assert!(matches!(
            certificate_mode_for_tls(GameServerTlsMode::DevelopmentSelfSigned),
            CertificateRetrievalMode::LoadFromFileOrGenerateSelfSigned { .. }
        ));
    }

    #[test]
    fn production_mode_never_enables_development_ticket_bypass() {
        assert_eq!(
            ClientTicketVerifier::from_env(GameServerTlsMode::ProductionConfigured),
            ClientTicketVerifier::FailClosed
        );
    }

    #[test]
    fn empty_world_stream_clear_tombstones_stale_manifest() {
        let mut manifest = test_manifest();

        assert!(manifest.clear_with_new_revision());
        assert_eq!(manifest.revision, WorldRevision(10));
        assert_eq!(manifest.editor_revision, WorldRevision(10));
        assert_eq!(
            manifest.manifest_signature,
            fun_scene::world_stream_manifest_signature(&[])
        );
        assert!(manifest.entity_ids.is_empty());
        assert!(manifest.chunks.is_empty());
        assert!(!manifest.clear_with_new_revision());
    }

    #[test]
    fn world_ready_ack_must_match_current_stream_manifest() {
        let manifest = test_manifest();
        assert_eq!(
            validate_world_ready_ack(ClientAdmissionStage::Streaming, &valid_ack(), &manifest),
            Ok(())
        );

        let mut wrong_level = valid_ack();
        wrong_level.level_id = WorldLevelId("other-level".to_owned());
        assert_eq!(
            validate_world_ready_ack(ClientAdmissionStage::Streaming, &wrong_level, &manifest),
            Err(WorldReadyRejection::WrongLevel)
        );

        let mut stale = valid_ack();
        stale.revision = WorldRevision(8);
        assert_eq!(
            validate_world_ready_ack(ClientAdmissionStage::Streaming, &stale, &manifest),
            Err(WorldReadyRejection::StaleRevision)
        );

        let mut future = valid_ack();
        future.revision = WorldRevision(10);
        assert_eq!(
            validate_world_ready_ack(ClientAdmissionStage::Streaming, &future, &manifest),
            Err(WorldReadyRejection::FutureRevision)
        );

        let mut wrong_count = valid_ack();
        wrong_count.chunk_count = 1;
        assert_eq!(
            validate_world_ready_ack(ClientAdmissionStage::Streaming, &wrong_count, &manifest),
            Err(WorldReadyRejection::WrongChunkCount)
        );

        let mut wrong_signature = valid_ack();
        wrong_signature.manifest_signature ^= 1;
        assert_eq!(
            validate_world_ready_ack(ClientAdmissionStage::Streaming, &wrong_signature, &manifest),
            Err(WorldReadyRejection::WrongManifestSignature)
        );

        assert_eq!(
            validate_world_ready_ack(ClientAdmissionStage::InGame, &valid_ack(), &manifest),
            Err(WorldReadyRejection::DuplicateReady)
        );
    }

    #[test]
    fn disconnect_cleanup_removes_all_authoritative_client_state() {
        let client_id = 42;
        let mut connected = ConnectedClients::default();
        let mut ready = ReadyClients::default();
        let mut relevance = ClientRelevanceSets::default();
        let mut pending = PendingWorldStreams::default();
        let mut admissions = ClientAdmissionStates::default();

        connected.ids.insert(client_id);
        ready.ids.insert(client_id);
        relevance.sets.insert(
            client_id,
            ServerClientRelevanceSet {
                tick: NetworkTick(9),
                entities: [NetEntity(1)].into_iter().collect(),
            },
        );
        pending.ids.insert(client_id);
        admissions
            .stages
            .insert(client_id, ClientAdmissionStage::InGame);
        admissions.warden_decisions.insert(
            client_id,
            WardenSessionDecisionKind::QuarantineToUntrustedPool,
        );
        admissions.untrusted_pool.insert(client_id);

        clear_client_state(
            client_id,
            &mut connected,
            &mut ready,
            &mut relevance,
            &mut pending,
            &mut admissions,
        );

        assert!(!connected.ids.contains(&client_id));
        assert!(!ready.ids.contains(&client_id));
        assert!(!relevance.sets.contains_key(&client_id));
        assert!(!pending.ids.contains(&client_id));
        assert!(!admissions.stages.contains_key(&client_id));
        assert!(!admissions.warden_decisions.contains_key(&client_id));
        assert!(!admissions.untrusted_pool.contains(&client_id));
    }
}
