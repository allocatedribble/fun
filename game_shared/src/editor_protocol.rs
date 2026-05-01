//! Shared editor/runtime control protocol.
//!
//! This module is intentionally wire-first and UI-free. The editor, game
//! client, and game server should all speak these typed packets instead of
//! inventing per-surface command shapes.

pub use thunder::protocol::{
    ChangeMask, ComponentKind, NetEntity, PacketSequence, WorldRevision as EditorWorldRevision,
};

const EDITOR_WIRE_MAGIC: [u8; 4] = *b"FED1";
const EDITOR_WIRE_VERSION: u8 = 1;
const EDITOR_WIRE_HEADER_LEN: usize = 13;
const FNV_OFFSET_BASIS: u32 = 0x811c_9dc5;
const FNV_PRIME: u32 = 0x0100_0193;

/// First protocol version for the editor control plane.
pub const EDITOR_PROTOCOL_VERSION: u32 = 1;

/// Default local-only bind address for editor control endpoints.
pub const DEFAULT_EDITOR_CONTROL_ADDR: &str = "127.0.0.1:0";

/// Deterministic runtime schedule stage for accepted editor transactions.
pub const EDITOR_COMMAND_APPLY_STAGE: &str = "editor_command_apply";

macro_rules! editor_wire_id {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident(pub $inner:ty);
    ) => {
        $(#[$meta])*
        #[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, compactly::v1::Encode)]
        $vis struct $name(pub $inner);
    };
}

editor_wire_id! {
    /// Stable resource identity for editor-visible runtime resources.
    pub struct ResourceKind(pub u32);
}

editor_wire_id! {
    /// Stable editor session identity scoped to one editor launch.
    pub struct EditorSessionId(pub u64);
}

editor_wire_id! {
    /// Stable request identity for audited editor operations.
    pub struct EditorRequestId(pub u64);
}

editor_wire_id! {
    /// Stable transaction identity for ordered editor mutations.
    pub struct EditorTransactionId(pub u64);
}

editor_wire_id! {
    /// Stable archetype identity for editor-spawned entities.
    pub struct EditorArchetypeId(pub u32);
}

editor_wire_id! {
    /// Stable build identity carried in the editor handshake.
    pub struct EditorBuildId(pub u64);
}

editor_wire_id! {
    /// Stable schema identity for editor/runtime metadata.
    pub struct EditorSchemaRevision(pub u64);
}

editor_wire_id! {
    /// Runtime tick captured alongside editor mutations.
    pub struct EditorTick(pub u64);
}

/// Runtime side the editor is attaching to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorTargetKind {
    /// Authoritative game server.
    Server,
    /// Game client local runtime.
    Client,
}

/// Explicit editor capabilities negotiated during authentication.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorCapability {
    ReadEntities,
    ReadComponents,
    ReadResources,
    ReadDiagnostics,
    ControlRuntime,
    MutateEntities,
    ApplyScenePatch,
    ExecuteServerCode,
    ExecuteClientCode,
    PersistIteration,
}

/// Initial editor handshake packet.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorHello {
    pub protocol_version: u32,
    pub editor_build_id: EditorBuildId,
    pub project_id: String,
    pub target_kind: EditorTargetKind,
    pub requested_capabilities: Vec<EditorCapability>,
    pub session_id: EditorSessionId,
    pub nonce: Vec<u8>,
}

/// Auth proof packet sent after the runtime challenge.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorAuth {
    pub token_proof: Vec<u8>,
    pub nonce_response: Vec<u8>,
}

/// Successful runtime welcome after authentication and capability selection.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorWelcome {
    pub granted_capabilities: Vec<EditorCapability>,
    pub target_build_id: EditorBuildId,
    pub world_revision: EditorWorldRevision,
    pub tick_rate_hz: u32,
    pub schema_revision: EditorSchemaRevision,
    pub diagnostic_schema_revision: EditorSchemaRevision,
}

/// Only packet an unauthenticated runtime endpoint may return.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorAuthRequired {
    pub protocol_version: u32,
    pub target_kind: EditorTargetKind,
    pub nonce: Vec<u8>,
}

/// Local bind policy for editor control endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorBindMode {
    /// Bind loopback only for local development.
    LoopbackOnly,
    /// Editor control is disabled until an explicit runtime flag enables it.
    DisabledUntilExplicitlyEnabled,
    /// Remote editor control was explicitly enabled by runtime config.
    ExplicitRemote,
}

/// Runtime control endpoint configuration.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorControlConfig {
    pub target_kind: EditorTargetKind,
    pub bind_mode: EditorBindMode,
    pub bind_addr: String,
    pub enabled: bool,
}

impl EditorControlConfig {
    /// Local development defaults to loopback-only editor control.
    #[must_use]
    pub fn local_development(target_kind: EditorTargetKind) -> Self {
        Self {
            target_kind,
            bind_mode: EditorBindMode::LoopbackOnly,
            bind_addr: DEFAULT_EDITOR_CONTROL_ADDR.to_owned(),
            enabled: true,
        }
    }

    /// Public runtimes default to no editor control endpoint.
    #[must_use]
    pub fn public_disabled(target_kind: EditorTargetKind) -> Self {
        Self {
            target_kind,
            bind_mode: EditorBindMode::DisabledUntilExplicitlyEnabled,
            bind_addr: String::new(),
            enabled: false,
        }
    }

    /// True only when remote editor control was deliberately opted into.
    #[must_use]
    pub const fn permits_remote_editor_control(&self) -> bool {
        self.enabled && matches!(self.bind_mode, EditorBindMode::ExplicitRemote)
    }
}

/// Target scope for an editor execution request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorExecTarget {
    Server,
    Client,
    ServerEntity { entity: NetEntity },
    ClientEntity { entity: NetEntity },
    Resource { resource_kind: ResourceKind },
}

impl EditorExecTarget {
    #[must_use]
    pub const fn target_kind(self) -> EditorTargetKind {
        match self {
            Self::Client | Self::ClientEntity { .. } => EditorTargetKind::Client,
            Self::Server | Self::ServerEntity { .. } | Self::Resource { .. } => {
                EditorTargetKind::Server
            }
        }
    }
}

/// Runtime used by an editor execution request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorExecRuntime {
    /// Registered Rust command owned by the runtime.
    RegisteredRustCommand,
    /// Deterministic editor mutation transaction.
    PatchTransaction,
    /// Privileged development mode for raw Rust execution.
    RawRustDevMode,
    /// Privileged sidecar or sandbox execution mode.
    SidecarDevRuntime,
}

/// Opaque source or plugin reference for an editor execution request.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorSourceRef {
    pub id: String,
    pub revision: Option<String>,
}

/// Key/value execution argument.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorExecArg {
    pub key: String,
    pub value: Vec<u8>,
}

/// Runtime memory budget for privileged editor execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct EditorMemoryBudget {
    pub max_bytes: u64,
}

/// World access requested by an editor execution request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorWorldAccess {
    None,
    ReadOnly,
    QueuedMutation,
}

/// How much execution diagnostic data the runtime should capture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorDiagnosticsPolicy {
    None,
    Summary,
    Events,
}

/// Audited, budgeted, capability-gated runtime execution request.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorExecRequest {
    pub request_id: EditorRequestId,
    pub sequence: PacketSequence,
    pub target: EditorExecTarget,
    pub language_or_runtime: EditorExecRuntime,
    pub source_or_plugin_ref: EditorSourceRef,
    pub args: Vec<EditorExecArg>,
    pub timeout_ms: u32,
    pub memory_budget: EditorMemoryBudget,
    pub world_access: EditorWorldAccess,
    pub diagnostics_policy: EditorDiagnosticsPolicy,
}

impl EditorExecRequest {
    /// Capability required before the runtime may queue this request.
    #[must_use]
    pub fn required_capability(&self) -> EditorCapability {
        match self.language_or_runtime {
            EditorExecRuntime::RegisteredRustCommand | EditorExecRuntime::PatchTransaction => {
                EditorCapability::ControlRuntime
            }
            EditorExecRuntime::RawRustDevMode | EditorExecRuntime::SidecarDevRuntime => {
                match self.target.target_kind() {
                    EditorTargetKind::Server => EditorCapability::ExecuteServerCode,
                    EditorTargetKind::Client => EditorCapability::ExecuteClientCode,
                }
            }
        }
    }

    /// Raw/sidecar execution is deliberately separate from registered commands.
    #[must_use]
    pub const fn is_privileged_dev_execution(&self) -> bool {
        matches!(
            self.language_or_runtime,
            EditorExecRuntime::RawRustDevMode | EditorExecRuntime::SidecarDevRuntime
        )
    }
}

/// Execution status for an editor request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorExecStatus {
    Queued,
    Completed,
    TimedOut,
    Rejected,
    Failed,
}

/// Captured stdout/stderr-style event from an editor execution request.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorStdoutEvent {
    pub sequence: PacketSequence,
    pub stream: EditorOutputStream,
    pub bytes: Vec<u8>,
}

/// Output stream kind for captured execution data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorOutputStream {
    Stdout,
    Stderr,
}

/// Diagnostic streams auto-subscribed when an editor attaches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorDiagnosticStream {
    ServerTickBudget,
    NetworkChannelPressure,
    SnapshotBudget,
    RelevanceDecisions,
    WorldStreamRevisions,
    MutationTransactions,
    ClientFrameTime,
    ScheduleHeatmap,
    RenderPathCounts,
    SolariBudget,
    SolariQueue,
    SolariRadianceCachePressure,
    AvianPhysicsStepTiming,
    AvianCollisionDiagnostics,
    AvianControllerDiagnostics,
}

/// Core editor diagnostics requested on attachment.
pub const DEFAULT_EDITOR_DIAGNOSTIC_STREAMS: &[EditorDiagnosticStream] = &[
    EditorDiagnosticStream::ServerTickBudget,
    EditorDiagnosticStream::NetworkChannelPressure,
    EditorDiagnosticStream::SnapshotBudget,
    EditorDiagnosticStream::RelevanceDecisions,
    EditorDiagnosticStream::WorldStreamRevisions,
    EditorDiagnosticStream::MutationTransactions,
    EditorDiagnosticStream::ClientFrameTime,
    EditorDiagnosticStream::ScheduleHeatmap,
    EditorDiagnosticStream::RenderPathCounts,
    EditorDiagnosticStream::SolariBudget,
    EditorDiagnosticStream::SolariQueue,
    EditorDiagnosticStream::SolariRadianceCachePressure,
    EditorDiagnosticStream::AvianPhysicsStepTiming,
    EditorDiagnosticStream::AvianCollisionDiagnostics,
    EditorDiagnosticStream::AvianControllerDiagnostics,
];

/// Owned list of default editor diagnostic subscriptions.
#[must_use]
pub fn default_editor_diagnostic_subscriptions() -> Vec<EditorDiagnosticStream> {
    DEFAULT_EDITOR_DIAGNOSTIC_STREAMS.to_vec()
}

/// Runtime diagnostic severity for editor display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorDiagnosticSeverity {
    Trace,
    Info,
    Warn,
    Error,
}

/// Runtime diagnostic event sent to an attached editor.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorDiagnosticEvent {
    pub sequence: PacketSequence,
    pub stream: EditorDiagnosticStream,
    pub severity: EditorDiagnosticSeverity,
    pub source: String,
    pub message: String,
    pub payload: Vec<u8>,
}

/// Batch of diagnostic events sent over the editor control plane.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorDiagnosticBatch {
    pub world_revision: EditorWorldRevision,
    pub events: Vec<EditorDiagnosticEvent>,
}

/// Diagnostic stream subscription request.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorDiagnosticSubscription {
    pub streams: Vec<EditorDiagnosticStream>,
    pub since_sequence: Option<PacketSequence>,
}

/// Begin an authoritative editor transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct BeginTransaction {
    pub transaction_id: EditorTransactionId,
    pub base_world_revision: EditorWorldRevision,
    pub base_tick: EditorTick,
}

/// Patch a single registered component on a network entity.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct PatchEntity {
    pub transaction_id: EditorTransactionId,
    pub entity: NetEntity,
    pub component: ComponentKind,
    pub change_mask: ChangeMask,
    pub payload: Vec<u8>,
}

/// Patch a registered editor-visible resource.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct PatchResource {
    pub transaction_id: EditorTransactionId,
    pub resource_kind: ResourceKind,
    pub payload: Vec<u8>,
}

/// Component payload included in an editor spawn request.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorComponentPayload {
    pub component: ComponentKind,
    pub payload: Vec<u8>,
}

/// Spawn an entity from a registered archetype.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct SpawnEntity {
    pub transaction_id: EditorTransactionId,
    pub archetype: EditorArchetypeId,
    pub components: Vec<EditorComponentPayload>,
}

/// Despawn an authoritative network entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct DespawnEntity {
    pub transaction_id: EditorTransactionId,
    pub entity: NetEntity,
}

/// Conflict behavior for committing a transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorConflictPolicy {
    RejectOnConflict,
    RebaseIfClean,
    ForceDevOnly,
}

/// Persistence behavior for accepted transactions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorPersistencePolicy {
    RuntimeOnly,
    PersistIteration,
}

/// Commit a queued editor transaction at the runtime's editor command stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct CommitTransaction {
    pub transaction_id: EditorTransactionId,
    pub conflict_policy: EditorConflictPolicy,
    pub persistence_policy: EditorPersistencePolicy,
}

/// Mutation application result.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorMutationAck {
    pub transaction_id: EditorTransactionId,
    pub status: EditorMutationStatus,
    pub world_revision: EditorWorldRevision,
    pub applied_ops: u32,
    pub conflict_count: u32,
    pub diagnostics: Vec<EditorDiagnosticEvent>,
}

/// Accepted/rejected mutation status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorMutationStatus {
    Accepted,
    RejectedConflict,
    RejectedCapability,
    RejectedValidation,
    Failed,
}

/// Execution result sent after the runtime has completed or rejected a request.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorExecResult {
    pub request_id: EditorRequestId,
    pub status: EditorExecStatus,
    pub stdout_events: Vec<EditorStdoutEvent>,
    pub diagnostic_events: Vec<EditorDiagnosticEvent>,
    pub mutations: Vec<EditorMutationAck>,
    pub duration_ns: u64,
    pub error: Option<String>,
}

/// Auditable editor control-plane event.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorAuditEvent {
    pub sequence: PacketSequence,
    pub capability: EditorCapability,
    pub target_kind: EditorTargetKind,
    pub message: String,
}

/// Opaque entity/component delta for editor-side inspection caches.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorEntityDelta {
    pub world_revision: EditorWorldRevision,
    pub entity: NetEntity,
    pub components: Vec<EditorComponentPayload>,
}

/// Editor-to-runtime packets.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum EditorClientPacket {
    Hello {
        hello: EditorHello,
    },
    Auth {
        auth: EditorAuth,
    },
    Exec {
        request: EditorExecRequest,
    },
    BeginTransaction {
        begin: BeginTransaction,
    },
    PatchEntity {
        patch: PatchEntity,
    },
    PatchResource {
        patch: PatchResource,
    },
    SpawnEntity {
        spawn: SpawnEntity,
    },
    DespawnEntity {
        despawn: DespawnEntity,
    },
    CommitTransaction {
        commit: CommitTransaction,
    },
    SubscribeDiagnostics {
        subscription: EditorDiagnosticSubscription,
    },
    Ping {
        sequence: PacketSequence,
    },
}

/// Runtime-to-editor packets.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum EditorRuntimePacket {
    AuthRequired {
        required: EditorAuthRequired,
    },
    Welcome {
        welcome: EditorWelcome,
    },
    ExecResult {
        result: EditorExecResult,
    },
    MutationAck {
        ack: EditorMutationAck,
    },
    EntityDelta {
        delta: EditorEntityDelta,
    },
    Diagnostics {
        batch: EditorDiagnosticBatch,
    },
    Audit {
        event: EditorAuditEvent,
    },
    Pong {
        sequence: PacketSequence,
        world_revision: EditorWorldRevision,
    },
}

/// Error while encoding or decoding editor protocol packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorProtocolCodecError {
    PayloadTooLarge,
    TruncatedEnvelope,
    InvalidMagic,
    UnsupportedVersion,
    LengthMismatch,
    ChecksumMismatch,
    InvalidPacket,
}

/// Encodes an editor-to-runtime packet into transport bytes.
pub fn encode_editor_client_packet(
    packet: &EditorClientPacket,
) -> Result<Vec<u8>, EditorProtocolCodecError> {
    encode_editor_wire_payload(compactly::v1::encode(packet))
}

/// Decodes an editor-to-runtime packet from transport bytes.
pub fn decode_editor_client_packet(
    bytes: &[u8],
) -> Result<EditorClientPacket, EditorProtocolCodecError> {
    compactly::v1::decode(decode_editor_wire_payload(bytes)?)
        .ok_or(EditorProtocolCodecError::InvalidPacket)
}

/// Encodes a runtime-to-editor packet into transport bytes.
pub fn encode_editor_runtime_packet(
    packet: &EditorRuntimePacket,
) -> Result<Vec<u8>, EditorProtocolCodecError> {
    encode_editor_wire_payload(compactly::v1::encode(packet))
}

/// Decodes a runtime-to-editor packet from transport bytes.
pub fn decode_editor_runtime_packet(
    bytes: &[u8],
) -> Result<EditorRuntimePacket, EditorProtocolCodecError> {
    compactly::v1::decode(decode_editor_wire_payload(bytes)?)
        .ok_or(EditorProtocolCodecError::InvalidPacket)
}

fn encode_editor_wire_payload(payload: Vec<u8>) -> Result<Vec<u8>, EditorProtocolCodecError> {
    let payload_len =
        u32::try_from(payload.len()).map_err(|_| EditorProtocolCodecError::PayloadTooLarge)?;
    let checksum = checksum32(&payload);
    let mut bytes = Vec::with_capacity(EDITOR_WIRE_HEADER_LEN + payload.len());
    bytes.extend_from_slice(&EDITOR_WIRE_MAGIC);
    bytes.push(EDITOR_WIRE_VERSION);
    bytes.extend_from_slice(&payload_len.to_le_bytes());
    bytes.extend_from_slice(&checksum.to_le_bytes());
    bytes.extend_from_slice(&payload);
    Ok(bytes)
}

fn decode_editor_wire_payload(bytes: &[u8]) -> Result<&[u8], EditorProtocolCodecError> {
    if bytes.len() < EDITOR_WIRE_HEADER_LEN {
        return Err(EditorProtocolCodecError::TruncatedEnvelope);
    }
    if bytes[..4] != EDITOR_WIRE_MAGIC[..] {
        return Err(EditorProtocolCodecError::InvalidMagic);
    }
    if bytes[4] != EDITOR_WIRE_VERSION {
        return Err(EditorProtocolCodecError::UnsupportedVersion);
    }

    let payload_len = u32::from_le_bytes(bytes[5..9].try_into().expect("fixed length")) as usize;
    let expected_checksum = u32::from_le_bytes(bytes[9..13].try_into().expect("fixed length"));
    let payload = &bytes[EDITOR_WIRE_HEADER_LEN..];
    if payload.len() != payload_len {
        return Err(EditorProtocolCodecError::LengthMismatch);
    }
    if checksum32(payload) != expected_checksum {
        return Err(EditorProtocolCodecError::ChecksumMismatch);
    }
    Ok(payload)
}

fn checksum32(bytes: &[u8]) -> u32 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// Local development capabilities for a target runtime.
#[must_use]
pub fn local_development_capabilities(
    target_kind: EditorTargetKind,
    enable_privileged_exec: bool,
) -> Vec<EditorCapability> {
    let mut capabilities = match target_kind {
        EditorTargetKind::Server => vec![
            EditorCapability::ReadEntities,
            EditorCapability::ReadComponents,
            EditorCapability::ReadResources,
            EditorCapability::ReadDiagnostics,
            EditorCapability::ControlRuntime,
            EditorCapability::MutateEntities,
            EditorCapability::ApplyScenePatch,
            EditorCapability::PersistIteration,
        ],
        EditorTargetKind::Client => vec![
            EditorCapability::ReadEntities,
            EditorCapability::ReadComponents,
            EditorCapability::ReadResources,
            EditorCapability::ReadDiagnostics,
            EditorCapability::ControlRuntime,
        ],
    };

    if enable_privileged_exec {
        capabilities.push(match target_kind {
            EditorTargetKind::Server => EditorCapability::ExecuteServerCode,
            EditorTargetKind::Client => EditorCapability::ExecuteClientCode,
        });
    }

    capabilities
}

/// Checks whether a granted capability set permits an operation.
#[must_use]
pub fn capability_is_granted(
    granted_capabilities: &[EditorCapability],
    required: EditorCapability,
) -> bool {
    granted_capabilities.contains(&required)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_client_packet_roundtrips_through_envelope() {
        let packet = EditorClientPacket::Hello {
            hello: EditorHello {
                protocol_version: EDITOR_PROTOCOL_VERSION,
                editor_build_id: EditorBuildId(42),
                project_id: "fun".to_owned(),
                target_kind: EditorTargetKind::Server,
                requested_capabilities: local_development_capabilities(
                    EditorTargetKind::Server,
                    false,
                ),
                session_id: EditorSessionId(7),
                nonce: vec![1, 2, 3, 4],
            },
        };

        let bytes = encode_editor_client_packet(&packet).expect("editor hello should encode");
        let decoded =
            decode_editor_client_packet(&bytes).expect("editor hello should decode from envelope");

        assert_eq!(decoded, packet);
    }

    #[test]
    fn editor_runtime_packet_roundtrips_through_envelope() {
        let packet = EditorRuntimePacket::Welcome {
            welcome: EditorWelcome {
                granted_capabilities: local_development_capabilities(
                    EditorTargetKind::Client,
                    true,
                ),
                target_build_id: EditorBuildId(9),
                world_revision: EditorWorldRevision(12),
                tick_rate_hz: 60,
                schema_revision: EditorSchemaRevision(3),
                diagnostic_schema_revision: EditorSchemaRevision(4),
            },
        };

        let bytes = encode_editor_runtime_packet(&packet).expect("welcome should encode");
        let decoded =
            decode_editor_runtime_packet(&bytes).expect("welcome should decode from envelope");

        assert_eq!(decoded, packet);
    }

    #[test]
    fn editor_wire_rejects_bad_checksum() {
        let packet = EditorRuntimePacket::AuthRequired {
            required: EditorAuthRequired {
                protocol_version: EDITOR_PROTOCOL_VERSION,
                target_kind: EditorTargetKind::Server,
                nonce: vec![9, 8, 7],
            },
        };
        let mut bytes = encode_editor_runtime_packet(&packet).expect("packet should encode");
        let last = bytes.len() - 1;
        bytes[last] ^= 0x55;

        assert_eq!(
            decode_editor_runtime_packet(&bytes).expect_err("bad checksum should fail"),
            EditorProtocolCodecError::ChecksumMismatch
        );
    }

    #[test]
    fn privileged_exec_requires_target_specific_execute_capability() {
        let request = EditorExecRequest {
            request_id: EditorRequestId(1),
            sequence: PacketSequence(1),
            target: EditorExecTarget::Client,
            language_or_runtime: EditorExecRuntime::RawRustDevMode,
            source_or_plugin_ref: EditorSourceRef {
                id: "dev-snippet".to_owned(),
                revision: None,
            },
            args: Vec::new(),
            timeout_ms: 100,
            memory_budget: EditorMemoryBudget {
                max_bytes: 16 * 1024 * 1024,
            },
            world_access: EditorWorldAccess::ReadOnly,
            diagnostics_policy: EditorDiagnosticsPolicy::Events,
        };

        assert!(request.is_privileged_dev_execution());
        assert_eq!(
            request.required_capability(),
            EditorCapability::ExecuteClientCode
        );
    }

    #[test]
    fn local_development_bind_policy_is_loopback_only() {
        let config = EditorControlConfig::local_development(EditorTargetKind::Server);

        assert!(config.enabled);
        assert_eq!(config.bind_mode, EditorBindMode::LoopbackOnly);
        assert!(!config.permits_remote_editor_control());
    }

    #[test]
    fn default_diagnostics_cover_core_server_client_render_and_physics_streams() {
        let streams = default_editor_diagnostic_subscriptions();

        assert!(streams.contains(&EditorDiagnosticStream::ServerTickBudget));
        assert!(streams.contains(&EditorDiagnosticStream::ClientFrameTime));
        assert!(streams.contains(&EditorDiagnosticStream::SolariRadianceCachePressure));
        assert!(streams.contains(&EditorDiagnosticStream::AvianPhysicsStepTiming));
    }
}
