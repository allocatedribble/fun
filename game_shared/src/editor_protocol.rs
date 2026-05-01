//! Shared editor/runtime control protocol.
//!
//! This module is intentionally wire-first and UI-free. The editor, game
//! client, and game server should all speak these typed packets instead of
//! inventing per-surface command shapes.

use thunder::physics::{Quantization, QuantizedQuat, QuantizedTransform3, QuantizedVec3};
pub use thunder::protocol::{
    ChangeMask, ComponentKind, NetClientId, NetEntity, PacketSequence,
    WorldRevision as EditorWorldRevision,
};

pub use crate::{
    DiagnosticEvent as EditorDiagnosticEvent, DiagnosticLevel as EditorDiagnosticSeverity,
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

/// Bytes required before an editor wire envelope length can be known.
pub const EDITOR_WIRE_ENVELOPE_HEADER_LEN: usize = EDITOR_WIRE_HEADER_LEN;

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
    /// Typed editor control protocol version.
    pub struct EditorProtocolVersion(pub u32);
}

/// Current editor control protocol version.
pub const CURRENT_EDITOR_PROTOCOL_VERSION: EditorProtocolVersion =
    EditorProtocolVersion(EDITOR_PROTOCOL_VERSION);

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

editor_wire_id! {
    /// Stable schema type identity for editor-visible component/resource data.
    pub struct EditorStableTypeId(pub u64);
}

editor_wire_id! {
    /// Cursor for paged editor queries.
    pub struct EditorPageCursor(pub u64);
}

/// Maximum payload budget declared by a packet sender.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct EditorSizeBudget {
    pub max_bytes: u32,
}

/// Runtime side the editor is attaching to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorTargetKind {
    /// Authoritative game server.
    Server,
    /// Game client local runtime.
    Client,
    /// Specific game client runtime, addressed by network client identity.
    ClientId(NetClientId),
}

impl EditorTargetKind {
    #[must_use]
    pub const fn runtime_class(self) -> Self {
        match self {
            Self::ClientId(_) => Self::Client,
            Self::Server => Self::Server,
            Self::Client => Self::Client,
        }
    }
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
    pub component_schemas: Vec<EditorComponentSchema>,
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

/// Required header carried by every canonical editor protocol packet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct EditorPacketHeader {
    pub request_id: EditorRequestId,
    pub protocol_version: EditorProtocolVersion,
    pub target: EditorTargetKind,
    pub sequence: PacketSequence,
    pub base_world_revision: Option<EditorWorldRevision>,
    pub size_budget: EditorSizeBudget,
}

impl EditorPacketHeader {
    #[must_use]
    pub const fn new(
        request_id: EditorRequestId,
        target: EditorTargetKind,
        sequence: PacketSequence,
        base_world_revision: Option<EditorWorldRevision>,
        size_budget: EditorSizeBudget,
    ) -> Self {
        Self {
            request_id,
            protocol_version: CURRENT_EDITOR_PROTOCOL_VERSION,
            target,
            sequence,
            base_world_revision,
            size_budget,
        }
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
    pub header: EditorPacketHeader,
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
                match self.target.target_kind().runtime_class() {
                    EditorTargetKind::Server => EditorCapability::ExecuteServerCode,
                    EditorTargetKind::Client | EditorTargetKind::ClientId(_) => {
                        EditorCapability::ExecuteClientCode
                    }
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
    ServerTickDuration,
    ServerTickBudget,
    ConnectedClients,
    NetworkChannelPressure,
    PacketBytes,
    WorldRevision,
    MutationCost,
    SnapshotBudget,
    SnapshotBudgetExhaustion,
    RelevanceDecisions,
    WorldStreamRevisions,
    StreamChunks,
    MutationTransactions,
    ClientFps,
    ClientFrameNs,
    ClientFrameTime,
    FrameProfilerSummary,
    GpuSampleStatus,
    RenderCpuMaterialCounters,
    MeshletPathCounts,
    ScheduleHeatmap,
    RenderPathCounts,
    SolariTimings,
    SolariBudget,
    SolariQueue,
    SolariRadianceCachePressure,
    RenderRecovery,
    CameraCount,
    WorldStreamApplyCost,
    AvianPhysicsStepTiming,
    AvianCollisionDiagnostics,
    AvianControllerDiagnostics,
}

/// Server diagnostics requested by default when an editor attaches to a server.
pub const DEFAULT_SERVER_EDITOR_DIAGNOSTIC_STREAMS: &[EditorDiagnosticStream] = &[
    EditorDiagnosticStream::ServerTickDuration,
    EditorDiagnosticStream::ServerTickBudget,
    EditorDiagnosticStream::ConnectedClients,
    EditorDiagnosticStream::NetworkChannelPressure,
    EditorDiagnosticStream::PacketBytes,
    EditorDiagnosticStream::WorldRevision,
    EditorDiagnosticStream::MutationCost,
    EditorDiagnosticStream::SnapshotBudget,
    EditorDiagnosticStream::SnapshotBudgetExhaustion,
    EditorDiagnosticStream::RelevanceDecisions,
    EditorDiagnosticStream::WorldStreamRevisions,
    EditorDiagnosticStream::StreamChunks,
    EditorDiagnosticStream::MutationTransactions,
    EditorDiagnosticStream::AvianPhysicsStepTiming,
    EditorDiagnosticStream::AvianCollisionDiagnostics,
    EditorDiagnosticStream::AvianControllerDiagnostics,
];

/// Client diagnostics requested by default when an editor attaches to a client.
pub const DEFAULT_CLIENT_EDITOR_DIAGNOSTIC_STREAMS: &[EditorDiagnosticStream] = &[
    EditorDiagnosticStream::ClientFps,
    EditorDiagnosticStream::ClientFrameNs,
    EditorDiagnosticStream::ClientFrameTime,
    EditorDiagnosticStream::FrameProfilerSummary,
    EditorDiagnosticStream::GpuSampleStatus,
    EditorDiagnosticStream::RenderCpuMaterialCounters,
    EditorDiagnosticStream::MeshletPathCounts,
    EditorDiagnosticStream::ScheduleHeatmap,
    EditorDiagnosticStream::RenderPathCounts,
    EditorDiagnosticStream::SolariTimings,
    EditorDiagnosticStream::SolariBudget,
    EditorDiagnosticStream::SolariQueue,
    EditorDiagnosticStream::SolariRadianceCachePressure,
    EditorDiagnosticStream::RenderRecovery,
    EditorDiagnosticStream::CameraCount,
    EditorDiagnosticStream::WorldStreamApplyCost,
];

/// Core editor diagnostics requested on attachment.
pub const DEFAULT_EDITOR_DIAGNOSTIC_STREAMS: &[EditorDiagnosticStream] = &[
    EditorDiagnosticStream::ServerTickDuration,
    EditorDiagnosticStream::ServerTickBudget,
    EditorDiagnosticStream::ConnectedClients,
    EditorDiagnosticStream::NetworkChannelPressure,
    EditorDiagnosticStream::PacketBytes,
    EditorDiagnosticStream::WorldRevision,
    EditorDiagnosticStream::MutationCost,
    EditorDiagnosticStream::SnapshotBudget,
    EditorDiagnosticStream::SnapshotBudgetExhaustion,
    EditorDiagnosticStream::RelevanceDecisions,
    EditorDiagnosticStream::WorldStreamRevisions,
    EditorDiagnosticStream::StreamChunks,
    EditorDiagnosticStream::MutationTransactions,
    EditorDiagnosticStream::AvianPhysicsStepTiming,
    EditorDiagnosticStream::AvianCollisionDiagnostics,
    EditorDiagnosticStream::AvianControllerDiagnostics,
    EditorDiagnosticStream::ClientFps,
    EditorDiagnosticStream::ClientFrameNs,
    EditorDiagnosticStream::ClientFrameTime,
    EditorDiagnosticStream::FrameProfilerSummary,
    EditorDiagnosticStream::GpuSampleStatus,
    EditorDiagnosticStream::RenderCpuMaterialCounters,
    EditorDiagnosticStream::MeshletPathCounts,
    EditorDiagnosticStream::ScheduleHeatmap,
    EditorDiagnosticStream::RenderPathCounts,
    EditorDiagnosticStream::SolariTimings,
    EditorDiagnosticStream::SolariBudget,
    EditorDiagnosticStream::SolariQueue,
    EditorDiagnosticStream::SolariRadianceCachePressure,
    EditorDiagnosticStream::RenderRecovery,
    EditorDiagnosticStream::CameraCount,
    EditorDiagnosticStream::WorldStreamApplyCost,
];

/// Owned list of all default editor diagnostic subscriptions.
#[must_use]
pub fn default_editor_diagnostic_subscriptions() -> Vec<EditorDiagnosticStream> {
    DEFAULT_EDITOR_DIAGNOSTIC_STREAMS.to_vec()
}

/// Owned list of server default editor diagnostic subscriptions.
#[must_use]
pub fn default_server_editor_diagnostic_subscriptions() -> Vec<EditorDiagnosticStream> {
    DEFAULT_SERVER_EDITOR_DIAGNOSTIC_STREAMS.to_vec()
}

/// Owned list of client default editor diagnostic subscriptions.
#[must_use]
pub fn default_client_editor_diagnostic_subscriptions() -> Vec<EditorDiagnosticStream> {
    DEFAULT_CLIENT_EDITOR_DIAGNOSTIC_STREAMS.to_vec()
}

/// Batch of diagnostic events sent over the editor control plane.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorDiagnosticBatch {
    pub world_revision: EditorWorldRevision,
    pub packets: Vec<crate::RecordedDiagnosticPacket>,
}

/// Diagnostic stream subscription request.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorDiagnosticSubscription {
    pub streams: Vec<EditorDiagnosticStream>,
    pub since_sequence: Option<PacketSequence>,
}

/// Query for paged editor-visible entities.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorEntityQuery {
    pub cursor: Option<EditorPageCursor>,
    pub limit: u16,
    pub entity: Option<NetEntity>,
    pub component_filter: Option<ComponentKind>,
    pub include_components: bool,
}

/// One editor-visible component value attached to an entity row.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorEntityComponentValue {
    pub component_kind: ComponentKind,
    pub schema_revision: EditorSchemaRevision,
    pub value_preview: String,
    pub raw_payload: Vec<u8>,
}

/// One row in an editor entity page.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorEntityRow {
    pub entity: NetEntity,
    pub target: EditorTargetKind,
    pub display_label: String,
    pub component_count: u16,
    pub schema_revision: EditorSchemaRevision,
    pub components: Vec<EditorEntityComponentValue>,
}

/// Paged entity query result.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorEntityPage {
    pub request_id: EditorRequestId,
    pub world_revision: EditorWorldRevision,
    pub cursor: Option<EditorPageCursor>,
    pub rows: Vec<EditorEntityRow>,
    pub has_more: bool,
}

/// Whether an editor-visible component can be changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorMutability {
    ReadOnly,
    RuntimeMutable,
    PersistentMutable,
}

/// Serialization format for component values crossing the editor wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorSerializationPolicy {
    Compactly,
    OpaqueBytes,
    SchemaRegistered,
}

/// Replication semantics for a component edited through the editor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorReplicationPolicy {
    ServerAuthoritative,
    ClientLocalOnly,
    SharedSchemaOnly,
}

/// Editor-visible component schema.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorComponentSchema {
    pub stable_type_id: EditorStableTypeId,
    pub component_kind: ComponentKind,
    pub display_label: String,
    pub mutability: EditorMutability,
    pub serialization_policy: EditorSerializationPolicy,
    pub replication_policy: EditorReplicationPolicy,
    pub ui_group: String,
    pub ui_widget: String,
    pub importance: String,
    pub diagnostic_label: String,
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

/// Compact transform patch payload for the first server-authoritative editor mutation path.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub struct EditorTransformPatch {
    pub transform: QuantizedTransform3,
    pub scale: QuantizedVec3,
}

impl EditorTransformPatch {
    #[must_use]
    pub fn from_parts(translation: [f32; 3], rotation_xyzw: [f32; 4], scale: [f32; 3]) -> Self {
        Self {
            transform: QuantizedTransform3 {
                translation: QuantizedVec3::from_f32(translation, Quantization::MILLIMETERS),
                rotation: QuantizedQuat::from_f32(rotation_xyzw),
            },
            scale: QuantizedVec3::from_f32(scale, Quantization::MILLIMETERS),
        }
    }

    #[must_use]
    pub fn translation(self) -> [f32; 3] {
        self.transform.translation.to_f32(Quantization::MILLIMETERS)
    }

    #[must_use]
    pub fn rotation_xyzw(self) -> [f32; 4] {
        self.transform.rotation.to_f32()
    }

    #[must_use]
    pub fn scale(self) -> [f32; 3] {
        self.scale.to_f32(Quantization::MILLIMETERS)
    }
}

#[must_use]
pub fn encode_editor_transform_patch(patch: EditorTransformPatch) -> Vec<u8> {
    compactly::v1::encode(&patch)
}

#[must_use]
pub fn decode_editor_transform_patch(bytes: &[u8]) -> Option<EditorTransformPatch> {
    compactly::v1::decode(bytes)
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

/// One operation inside an ordered editor mutation transaction.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum EditorMutationOp {
    PatchEntity { patch: PatchEntity },
    PatchResource { patch: PatchResource },
    SpawnEntity { spawn: SpawnEntity },
    DespawnEntity { despawn: DespawnEntity },
}

impl EditorMutationOp {
    #[must_use]
    pub const fn component_kind(&self) -> Option<ComponentKind> {
        match self {
            Self::PatchEntity { patch } => Some(patch.component),
            Self::PatchResource { .. } | Self::DespawnEntity { .. } => None,
            Self::SpawnEntity { .. } => None,
        }
    }
}

/// Complete mutation request applied at the runtime's editor command stage.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorMutationTransaction {
    pub header: EditorPacketHeader,
    pub transaction_id: EditorTransactionId,
    pub base_world_revision: EditorWorldRevision,
    pub base_tick: EditorTick,
    pub conflict_policy: EditorConflictPolicy,
    pub persistence_policy: EditorPersistencePolicy,
    pub required_capability: EditorCapability,
    pub audit_event: EditorAuditEvent,
    pub diagnostics_event: EditorDiagnosticEvent,
    pub ops: Vec<EditorMutationOp>,
}

impl EditorMutationTransaction {
    #[must_use]
    pub const fn requires_persistence(&self) -> bool {
        matches!(
            self.persistence_policy,
            EditorPersistencePolicy::PersistIteration
        )
    }
}

/// Mutation application result.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorMutationAck {
    pub header: EditorPacketHeader,
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

/// Persistence request emitted after a mutation is accepted.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorPersistenceRequest {
    pub header: EditorPacketHeader,
    pub transaction_id: EditorTransactionId,
    pub base_world_revision: EditorWorldRevision,
    pub persistence_policy: EditorPersistencePolicy,
    pub patch_payload: Vec<u8>,
}

/// Persistence request status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, compactly::v1::Encode)]
pub enum EditorPersistenceStatus {
    Queued,
    Persisted,
    Rejected,
    Failed,
}

/// Persistence result sent to the editor after runtime application.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorPersistenceAck {
    pub header: EditorPacketHeader,
    pub transaction_id: EditorTransactionId,
    pub status: EditorPersistenceStatus,
    pub world_revision: EditorWorldRevision,
    pub diagnostics: Vec<EditorDiagnosticEvent>,
}

/// Execution result sent after the runtime has completed or rejected a request.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorExecResult {
    pub header: EditorPacketHeader,
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

/// Authentication and welcome payloads for the editor handshake lane.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum EditorHandshakePayload {
    Hello { hello: EditorHello },
    Auth { auth: EditorAuth },
    AuthRequired { required: EditorAuthRequired },
    Welcome { welcome: EditorWelcome },
}

/// Canonical handshake packet shared by the editor, server, and client.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorHandshakePacket {
    pub header: EditorPacketHeader,
    pub payload: EditorHandshakePayload,
}

/// Command payloads sent from the editor to an authenticated runtime.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum EditorCommandPayload {
    QueryEntities {
        query: EditorEntityQuery,
    },
    Mutate {
        transaction: EditorMutationTransaction,
    },
    Exec {
        request: EditorExecRequest,
    },
    Persist {
        request: EditorPersistenceRequest,
    },
    SubscribeDiagnostics {
        subscription: EditorDiagnosticSubscription,
    },
    Ping,
}

/// Canonical editor command packet.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorCommandPacket {
    pub header: EditorPacketHeader,
    pub payload: EditorCommandPayload,
}

/// Runtime event payloads sent to the editor.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum EditorEventPayload {
    EntityPage { page: EditorEntityPage },
    EntityDelta { delta: EditorEntityDelta },
    MutationAck { ack: EditorMutationAck },
    ExecResult { result: EditorExecResult },
    PersistenceAck { ack: EditorPersistenceAck },
    Audit { event: EditorAuditEvent },
    Pong { world_revision: EditorWorldRevision },
}

/// Canonical runtime event packet.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorEventPacket {
    pub header: EditorPacketHeader,
    pub payload: EditorEventPayload,
}

/// Canonical diagnostic packet sent on the editor diagnostics stream.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub struct EditorDiagnosticPacket {
    pub header: EditorPacketHeader,
    pub batch: EditorDiagnosticBatch,
}

/// One typed editor runtime protocol, independent of the transport lane.
#[derive(Debug, Clone, PartialEq, Eq, compactly::v1::Encode)]
pub enum EditorProtocolPacket {
    Handshake { packet: EditorHandshakePacket },
    Command { packet: EditorCommandPacket },
    Event { packet: EditorEventPacket },
    Diagnostic { packet: EditorDiagnosticPacket },
}

impl EditorProtocolPacket {
    #[must_use]
    pub const fn header(&self) -> &EditorPacketHeader {
        match self {
            Self::Handshake { packet } => &packet.header,
            Self::Command { packet } => &packet.header,
            Self::Event { packet } => &packet.header,
            Self::Diagnostic { packet } => &packet.header,
        }
    }
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

/// Encodes a canonical editor protocol packet into transport bytes.
pub fn encode_editor_packet(
    packet: &EditorProtocolPacket,
) -> Result<Vec<u8>, EditorProtocolCodecError> {
    encode_editor_wire_payload(compactly::v1::encode(packet))
}

/// Decodes a canonical editor protocol packet from transport bytes.
pub fn decode_editor_packet(
    bytes: &[u8],
) -> Result<EditorProtocolPacket, EditorProtocolCodecError> {
    compactly::v1::decode(decode_editor_wire_payload(bytes)?)
        .ok_or(EditorProtocolCodecError::InvalidPacket)
}

/// Returns the complete envelope length from a received editor wire header.
pub fn editor_wire_envelope_len(bytes: &[u8]) -> Result<usize, EditorProtocolCodecError> {
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
    Ok(EDITOR_WIRE_HEADER_LEN.saturating_add(payload_len))
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

/// Deterministic protocol validation failure surfaced before runtime mutation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditorProtocolValidationError {
    StaleVersion,
    MalformedAuth,
    MissingCapability,
    StaleRevision,
    OversizePayload,
    UnknownComponentKind,
}

/// Runtime-side validation context for editor protocol packets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditorProtocolValidationContext {
    pub protocol_version: EditorProtocolVersion,
    pub granted_capabilities: Vec<EditorCapability>,
    pub world_revision: EditorWorldRevision,
    pub max_payload_bytes: u32,
    pub component_schemas: Vec<EditorComponentSchema>,
}

impl EditorProtocolValidationContext {
    #[must_use]
    pub fn local_development(target_kind: EditorTargetKind) -> Self {
        Self {
            protocol_version: CURRENT_EDITOR_PROTOCOL_VERSION,
            granted_capabilities: local_development_capabilities(target_kind, true),
            world_revision: EditorWorldRevision(0),
            max_payload_bytes: 256 * 1024,
            component_schemas: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_component_schema(mut self, schema: EditorComponentSchema) -> Self {
        self.component_schemas.push(schema);
        self
    }

    #[must_use]
    pub fn grants(&self, capability: EditorCapability) -> bool {
        capability_is_granted(&self.granted_capabilities, capability)
    }

    #[must_use]
    pub fn component_is_registered(&self, component: ComponentKind) -> bool {
        self.component_schemas
            .iter()
            .any(|schema| schema.component_kind == component)
    }
}

/// Validates packet metadata, auth shape, capabilities, revisions, and schemas.
pub fn validate_editor_packet(
    packet: &EditorProtocolPacket,
    context: &EditorProtocolValidationContext,
) -> Result<(), EditorProtocolValidationError> {
    let header = packet.header();
    if header.protocol_version != context.protocol_version {
        return Err(EditorProtocolValidationError::StaleVersion);
    }

    validate_packet_budget(packet, context)?;
    validate_header_revision(header, context)?;

    match packet {
        EditorProtocolPacket::Handshake { packet } => validate_handshake_packet(packet, context),
        EditorProtocolPacket::Command { packet } => validate_command_packet(packet, context),
        EditorProtocolPacket::Event { packet } => validate_event_packet(packet, context),
        EditorProtocolPacket::Diagnostic { .. } => {
            require_capability(context, EditorCapability::ReadDiagnostics)
        }
    }
}

fn validate_packet_budget(
    packet: &EditorProtocolPacket,
    context: &EditorProtocolValidationContext,
) -> Result<(), EditorProtocolValidationError> {
    let payload_len = compactly::v1::encode(packet).len();
    let sender_budget = packet.header().size_budget.max_bytes as usize;
    let runtime_budget = context.max_payload_bytes as usize;
    if payload_len > sender_budget || payload_len > runtime_budget {
        return Err(EditorProtocolValidationError::OversizePayload);
    }
    Ok(())
}

fn validate_header_revision(
    header: &EditorPacketHeader,
    context: &EditorProtocolValidationContext,
) -> Result<(), EditorProtocolValidationError> {
    if let Some(base_world_revision) = header.base_world_revision
        && base_world_revision < context.world_revision
    {
        return Err(EditorProtocolValidationError::StaleRevision);
    }
    Ok(())
}

fn validate_handshake_packet(
    packet: &EditorHandshakePacket,
    context: &EditorProtocolValidationContext,
) -> Result<(), EditorProtocolValidationError> {
    match &packet.payload {
        EditorHandshakePayload::Hello { hello } => {
            if hello.protocol_version != context.protocol_version.0 || hello.nonce.is_empty() {
                return Err(EditorProtocolValidationError::MalformedAuth);
            }
            Ok(())
        }
        EditorHandshakePayload::Auth { auth } => {
            if auth.token_proof.is_empty() || auth.nonce_response.is_empty() {
                return Err(EditorProtocolValidationError::MalformedAuth);
            }
            Ok(())
        }
        EditorHandshakePayload::AuthRequired { required } => {
            if required.protocol_version != context.protocol_version.0 || required.nonce.is_empty()
            {
                return Err(EditorProtocolValidationError::MalformedAuth);
            }
            Ok(())
        }
        EditorHandshakePayload::Welcome { .. } => Ok(()),
    }
}

fn validate_command_packet(
    packet: &EditorCommandPacket,
    context: &EditorProtocolValidationContext,
) -> Result<(), EditorProtocolValidationError> {
    match &packet.payload {
        EditorCommandPayload::QueryEntities { query } => {
            require_capability(context, EditorCapability::ReadEntities)?;
            if query.include_components {
                require_capability(context, EditorCapability::ReadComponents)?;
            }
            if let Some(component) = query.component_filter {
                require_component_schema(context, component)?;
            }
            Ok(())
        }
        EditorCommandPayload::Mutate { transaction } => {
            require_capability(context, EditorCapability::MutateEntities)?;
            require_capability(context, transaction.required_capability)?;
            if transaction.requires_persistence() {
                require_capability(context, EditorCapability::PersistIteration)?;
            }
            if transaction.base_world_revision < context.world_revision {
                return Err(EditorProtocolValidationError::StaleRevision);
            }
            validate_mutation_components(transaction, context)
        }
        EditorCommandPayload::Exec { request } => {
            require_capability(context, request.required_capability())
        }
        EditorCommandPayload::Persist { request } => {
            require_capability(context, EditorCapability::PersistIteration)?;
            if request.base_world_revision < context.world_revision {
                return Err(EditorProtocolValidationError::StaleRevision);
            }
            Ok(())
        }
        EditorCommandPayload::SubscribeDiagnostics { .. } => {
            require_capability(context, EditorCapability::ReadDiagnostics)
        }
        EditorCommandPayload::Ping => Ok(()),
    }
}

fn validate_event_packet(
    packet: &EditorEventPacket,
    context: &EditorProtocolValidationContext,
) -> Result<(), EditorProtocolValidationError> {
    match &packet.payload {
        EditorEventPayload::EntityPage { .. } | EditorEventPayload::EntityDelta { .. } => {
            require_capability(context, EditorCapability::ReadEntities)
        }
        EditorEventPayload::MutationAck { .. } => {
            require_capability(context, EditorCapability::MutateEntities)
        }
        EditorEventPayload::ExecResult { result } => {
            if result.status == EditorExecStatus::Rejected {
                Ok(())
            } else {
                require_capability(context, EditorCapability::ControlRuntime)
            }
        }
        EditorEventPayload::PersistenceAck { .. } => {
            require_capability(context, EditorCapability::PersistIteration)
        }
        EditorEventPayload::Audit { .. } | EditorEventPayload::Pong { .. } => Ok(()),
    }
}

fn validate_mutation_components(
    transaction: &EditorMutationTransaction,
    context: &EditorProtocolValidationContext,
) -> Result<(), EditorProtocolValidationError> {
    for op in &transaction.ops {
        match op {
            EditorMutationOp::PatchEntity { patch } => {
                require_component_schema(context, patch.component)?;
            }
            EditorMutationOp::SpawnEntity { spawn } => {
                for component in &spawn.components {
                    require_component_schema(context, component.component)?;
                }
            }
            EditorMutationOp::PatchResource { .. } | EditorMutationOp::DespawnEntity { .. } => {}
        }
    }
    Ok(())
}

fn require_component_schema(
    context: &EditorProtocolValidationContext,
    component: ComponentKind,
) -> Result<(), EditorProtocolValidationError> {
    if context.component_is_registered(component) {
        Ok(())
    } else {
        Err(EditorProtocolValidationError::UnknownComponentKind)
    }
}

fn require_capability(
    context: &EditorProtocolValidationContext,
    capability: EditorCapability,
) -> Result<(), EditorProtocolValidationError> {
    if context.grants(capability) {
        Ok(())
    } else {
        Err(EditorProtocolValidationError::MissingCapability)
    }
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
        EditorTargetKind::Client | EditorTargetKind::ClientId(_) => vec![
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
            EditorTargetKind::Client | EditorTargetKind::ClientId(_) => {
                EditorCapability::ExecuteClientCode
            }
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

#[must_use]
pub const fn editor_capability_name(capability: EditorCapability) -> &'static str {
    match capability {
        EditorCapability::ReadEntities => "read_entities",
        EditorCapability::ReadComponents => "read_components",
        EditorCapability::ReadResources => "read_resources",
        EditorCapability::ReadDiagnostics => "read_diagnostics",
        EditorCapability::ControlRuntime => "control_runtime",
        EditorCapability::MutateEntities => "mutate_entities",
        EditorCapability::ApplyScenePatch => "apply_scene_patch",
        EditorCapability::ExecuteServerCode => "execute_server_code",
        EditorCapability::ExecuteClientCode => "execute_client_code",
        EditorCapability::PersistIteration => "persist_iteration",
    }
}

#[must_use]
pub fn parse_editor_capability(value: &str) -> Option<EditorCapability> {
    match value {
        "read_entities" => Some(EditorCapability::ReadEntities),
        "read_components" => Some(EditorCapability::ReadComponents),
        "read_resources" => Some(EditorCapability::ReadResources),
        "read_diagnostics" => Some(EditorCapability::ReadDiagnostics),
        "control_runtime" => Some(EditorCapability::ControlRuntime),
        "mutate_entities" => Some(EditorCapability::MutateEntities),
        "apply_scene_patch" => Some(EditorCapability::ApplyScenePatch),
        "execute_server_code" => Some(EditorCapability::ExecuteServerCode),
        "execute_client_code" => Some(EditorCapability::ExecuteClientCode),
        "persist_iteration" => Some(EditorCapability::PersistIteration),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(
        request_id: u64,
        target: EditorTargetKind,
        sequence: u32,
        base_world_revision: Option<EditorWorldRevision>,
    ) -> EditorPacketHeader {
        EditorPacketHeader::new(
            EditorRequestId(request_id),
            target,
            PacketSequence(sequence),
            base_world_revision,
            EditorSizeBudget { max_bytes: 65_536 },
        )
    }

    fn transform_schema() -> EditorComponentSchema {
        EditorComponentSchema {
            stable_type_id: EditorStableTypeId(7),
            component_kind: ComponentKind(11),
            display_label: "Transform".to_owned(),
            mutability: EditorMutability::RuntimeMutable,
            serialization_policy: EditorSerializationPolicy::Compactly,
            replication_policy: EditorReplicationPolicy::ServerAuthoritative,
            ui_group: "Transform".to_owned(),
            ui_widget: "transform3d".to_owned(),
            importance: "primary".to_owned(),
            diagnostic_label: "fun::editor::schema:transform".to_owned(),
        }
    }

    fn diagnostic_event(sequence: u32) -> EditorDiagnosticEvent {
        EditorDiagnosticEvent {
            target: "fun::editor::protocol".to_owned(),
            level: EditorDiagnosticSeverity::Info,
            timestamp_or_tick: crate::DiagnosticTimestampOrTick::Tick {
                tick: u64::from(sequence),
            },
            fields: vec![
                crate::DiagnosticField::text("stream", "mutation_transactions"),
                crate::DiagnosticField::text("message", "mutation queued"),
                crate::DiagnosticField {
                    name: "payload".to_owned(),
                    value: crate::DiagnosticValue::Bytes {
                        value: vec![1, 2, 3],
                    },
                },
            ],
            source: crate::DiagnosticSource::static_location(file!(), line!(), module_path!()),
            frame_index: None,
            span_id: None,
        }
    }

    fn recorded_diagnostic(sequence: u64) -> crate::RecordedDiagnosticPacket {
        crate::RecordedDiagnosticPacket {
            sequence: crate::DiagnosticSequence(sequence),
            packet: crate::DiagnosticPacket::Event {
                event: diagnostic_event(sequence as u32),
            },
        }
    }

    fn audit_event(sequence: u32) -> EditorAuditEvent {
        EditorAuditEvent {
            sequence: PacketSequence(sequence),
            capability: EditorCapability::MutateEntities,
            target_kind: EditorTargetKind::Server,
            message: "authorized mutation".to_owned(),
        }
    }

    fn validation_context() -> EditorProtocolValidationContext {
        EditorProtocolValidationContext::local_development(EditorTargetKind::Server)
            .with_component_schema(transform_schema())
    }

    fn mutation_transaction(base_world_revision: EditorWorldRevision) -> EditorMutationTransaction {
        let transaction_id = EditorTransactionId(99);
        EditorMutationTransaction {
            header: header(42, EditorTargetKind::Server, 42, Some(base_world_revision)),
            transaction_id,
            base_world_revision,
            base_tick: EditorTick(10),
            conflict_policy: EditorConflictPolicy::RejectOnConflict,
            persistence_policy: EditorPersistencePolicy::RuntimeOnly,
            required_capability: EditorCapability::MutateEntities,
            audit_event: audit_event(42),
            diagnostics_event: diagnostic_event(42),
            ops: vec![EditorMutationOp::PatchEntity {
                patch: PatchEntity {
                    transaction_id,
                    entity: NetEntity::from_parts(1, 77),
                    component: transform_schema().component_kind,
                    change_mask: ChangeMask::ALL,
                    payload: vec![4, 5, 6],
                },
            }],
        }
    }

    fn mutation_command_packet(base_world_revision: EditorWorldRevision) -> EditorProtocolPacket {
        EditorProtocolPacket::Command {
            packet: EditorCommandPacket {
                header: header(42, EditorTargetKind::Server, 42, Some(base_world_revision)),
                payload: EditorCommandPayload::Mutate {
                    transaction: mutation_transaction(base_world_revision),
                },
            },
        }
    }

    #[test]
    fn protocol_packets_roundtrip_all_lanes_through_envelope() {
        let handshake = EditorProtocolPacket::Handshake {
            packet: EditorHandshakePacket {
                header: header(1, EditorTargetKind::Server, 1, None),
                payload: EditorHandshakePayload::Hello {
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
                },
            },
        };

        let command = mutation_command_packet(EditorWorldRevision(0));

        let event = EditorProtocolPacket::Event {
            packet: EditorEventPacket {
                header: header(
                    43,
                    EditorTargetKind::Server,
                    43,
                    Some(EditorWorldRevision(1)),
                ),
                payload: EditorEventPayload::MutationAck {
                    ack: EditorMutationAck {
                        header: header(
                            43,
                            EditorTargetKind::Server,
                            43,
                            Some(EditorWorldRevision(1)),
                        ),
                        transaction_id: EditorTransactionId(99),
                        status: EditorMutationStatus::Accepted,
                        world_revision: EditorWorldRevision(1),
                        applied_ops: 1,
                        conflict_count: 0,
                        diagnostics: vec![diagnostic_event(43)],
                    },
                },
            },
        };

        let diagnostic = EditorProtocolPacket::Diagnostic {
            packet: EditorDiagnosticPacket {
                header: header(
                    44,
                    EditorTargetKind::Server,
                    44,
                    Some(EditorWorldRevision(1)),
                ),
                batch: EditorDiagnosticBatch {
                    world_revision: EditorWorldRevision(1),
                    packets: vec![recorded_diagnostic(44)],
                },
            },
        };

        for packet in [handshake, command, event, diagnostic] {
            let bytes = encode_editor_packet(&packet).expect("packet should encode");
            let decoded = decode_editor_packet(&bytes).expect("packet should decode");
            assert_eq!(decoded, packet);
        }
    }

    #[test]
    fn editor_wire_rejects_bad_checksum() {
        let packet = EditorProtocolPacket::Handshake {
            packet: EditorHandshakePacket {
                header: header(2, EditorTargetKind::Server, 2, None),
                payload: EditorHandshakePayload::AuthRequired {
                    required: EditorAuthRequired {
                        protocol_version: EDITOR_PROTOCOL_VERSION,
                        target_kind: EditorTargetKind::Server,
                        nonce: vec![9, 8, 7],
                    },
                },
            },
        };
        let mut bytes = encode_editor_packet(&packet).expect("packet should encode");
        let last = bytes.len() - 1;
        bytes[last] ^= 0x55;

        assert_eq!(
            decode_editor_packet(&bytes).expect_err("bad checksum should fail"),
            EditorProtocolCodecError::ChecksumMismatch
        );
    }

    #[test]
    fn privileged_exec_requires_target_specific_execute_capability() {
        let request = EditorExecRequest {
            header: header(1, EditorTargetKind::Client, 1, None),
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
    fn validation_rejects_stale_protocol_version() {
        let mut packet = mutation_command_packet(EditorWorldRevision(0));
        match &mut packet {
            EditorProtocolPacket::Command { packet } => {
                packet.header.protocol_version = EditorProtocolVersion(0);
            }
            _ => unreachable!("mutation helper returns command"),
        }

        assert_eq!(
            validate_editor_packet(&packet, &validation_context()).expect_err("version is stale"),
            EditorProtocolValidationError::StaleVersion
        );
    }

    #[test]
    fn validation_rejects_malformed_auth() {
        let packet = EditorProtocolPacket::Handshake {
            packet: EditorHandshakePacket {
                header: header(3, EditorTargetKind::Server, 3, None),
                payload: EditorHandshakePayload::Auth {
                    auth: EditorAuth {
                        token_proof: Vec::new(),
                        nonce_response: vec![1],
                    },
                },
            },
        };

        assert_eq!(
            validate_editor_packet(&packet, &validation_context())
                .expect_err("empty token proof is malformed"),
            EditorProtocolValidationError::MalformedAuth
        );
    }

    #[test]
    fn validation_rejects_missing_capability() {
        let packet = mutation_command_packet(EditorWorldRevision(0));
        let mut context = validation_context();
        context
            .granted_capabilities
            .retain(|capability| *capability != EditorCapability::MutateEntities);

        assert_eq!(
            validate_editor_packet(&packet, &context).expect_err("mutation needs capability"),
            EditorProtocolValidationError::MissingCapability
        );
    }

    #[test]
    fn validation_rejects_stale_world_revision() {
        let packet = mutation_command_packet(EditorWorldRevision(1));
        let mut context = validation_context();
        context.world_revision = EditorWorldRevision(2);

        assert_eq!(
            validate_editor_packet(&packet, &context).expect_err("base revision is stale"),
            EditorProtocolValidationError::StaleRevision
        );
    }

    #[test]
    fn validation_rejects_oversize_payload() {
        let mut packet = mutation_command_packet(EditorWorldRevision(0));
        match &mut packet {
            EditorProtocolPacket::Command { packet } => {
                packet.header.size_budget = EditorSizeBudget { max_bytes: 8 };
            }
            _ => unreachable!("mutation helper returns command"),
        }

        assert_eq!(
            validate_editor_packet(&packet, &validation_context()).expect_err("budget is tiny"),
            EditorProtocolValidationError::OversizePayload
        );
    }

    #[test]
    fn validation_rejects_unknown_component_kind() {
        let packet = mutation_command_packet(EditorWorldRevision(0));
        let mut context = validation_context();
        context.component_schemas.clear();

        assert_eq!(
            validate_editor_packet(&packet, &context).expect_err("component is unknown"),
            EditorProtocolValidationError::UnknownComponentKind
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
        assert!(streams.contains(&EditorDiagnosticStream::ConnectedClients));
        assert!(streams.contains(&EditorDiagnosticStream::ClientFrameTime));
        assert!(streams.contains(&EditorDiagnosticStream::GpuSampleStatus));
        assert!(streams.contains(&EditorDiagnosticStream::SolariRadianceCachePressure));
        assert!(streams.contains(&EditorDiagnosticStream::AvianPhysicsStepTiming));
    }

    #[test]
    fn target_specific_default_diagnostics_do_not_cross_runtime_roles() {
        let server = default_server_editor_diagnostic_subscriptions();
        let client = default_client_editor_diagnostic_subscriptions();

        assert!(server.contains(&EditorDiagnosticStream::ServerTickDuration));
        assert!(server.contains(&EditorDiagnosticStream::StreamChunks));
        assert!(server.contains(&EditorDiagnosticStream::AvianControllerDiagnostics));
        assert!(!server.contains(&EditorDiagnosticStream::GpuSampleStatus));

        assert!(client.contains(&EditorDiagnosticStream::ClientFps));
        assert!(client.contains(&EditorDiagnosticStream::WorldStreamApplyCost));
        assert!(client.contains(&EditorDiagnosticStream::RenderRecovery));
        assert!(!client.contains(&EditorDiagnosticStream::ConnectedClients));
    }
}
