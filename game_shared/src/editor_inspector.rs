use crate::{
    CURRENT_EDITOR_PROTOCOL_VERSION, EDITOR_PROTOCOL_VERSION, EDITOR_WIRE_ENVELOPE_HEADER_LEN,
    EditorAuditEvent, EditorAuth, EditorAuthRequired, EditorBindMode, EditorBuildId,
    EditorCapability, EditorCommandPacket, EditorCommandPayload, EditorComponentSchema,
    EditorControlConfig, EditorDiagnosticBatch, EditorDiagnosticEvent, EditorDiagnosticPacket,
    EditorDiagnosticStream, EditorDiagnosticSubscription, EditorEntityPage, EditorEntityQuery,
    EditorEntityRow, EditorEventPacket, EditorEventPayload, EditorExecResult, EditorExecStatus,
    EditorHandshakePacket, EditorHandshakePayload, EditorMutationAck, EditorMutationStatus,
    EditorMutationTransaction, EditorPacketHeader, EditorPageCursor, EditorPersistenceAck,
    EditorPersistenceStatus, EditorProtocolPacket, EditorProtocolValidationContext,
    EditorRequestId, EditorRuntimeControlCommand, EditorSchemaRevision, EditorSessionId,
    EditorSizeBudget, EditorTargetKind, EditorTransactionId, EditorWelcome, EditorWorldRevision,
    PacketSequence, RuntimeDiagnosticSinks, decode_editor_packet, editor_wire_envelope_len,
    encode_editor_packet, parse_editor_capability, validate_editor_packet,
};
use ring::hmac;
use std::{
    collections::VecDeque,
    env, fmt,
    io::{self, Read, Write},
    net::{IpAddr, TcpListener, TcpStream, ToSocketAddrs},
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const EDITOR_AUTH_TOKEN_ENV: &str = "FUN_EDITOR_AUTH_TOKEN";
const EDITOR_SESSION_ID_ENV: &str = "FUN_EDITOR_SESSION_ID";
const EDITOR_AUTH_EXPIRES_ENV: &str = "FUN_EDITOR_AUTH_EXPIRES_UNIX_MS";
const EDITOR_ALLOWED_CAPS_ENV: &str = "FUN_EDITOR_ALLOWED_CAPS";
const EDITOR_CONTROL_ADDR_ENV: &str = "FUN_EDITOR_CONTROL_ADDR";
const EDITOR_BIND_ENABLE_ENV: &str = "FUN_EDITOR_ENABLE_INSPECTOR_BIND";
const EDITOR_REMOTE_ENABLE_ENV: &str = "FUN_EDITOR_ENABLE_REMOTE_INSPECTOR";
const EDITOR_SERVER_BIND_ENABLE_ENV: &str = "FUN_EDITOR_ENABLE_SERVER_INSPECTOR_BIND";
const EDITOR_CLIENT_BIND_ENABLE_ENV: &str = "FUN_EDITOR_ENABLE_CLIENT_INSPECTOR_BIND";
const EDITOR_SERVER_BIND_ADDR_ENV: &str = "FUN_EDITOR_SERVER_INSPECTOR_ADDR";
const EDITOR_CLIENT_BIND_ADDR_ENV: &str = "FUN_EDITOR_CLIENT_INSPECTOR_ADDR";
const EDITOR_MAX_CONNECTIONS_ENV: &str = "FUN_EDITOR_INSPECTOR_MAX_CONNECTIONS";
const EDITOR_IDLE_TIMEOUT_MS_ENV: &str = "FUN_EDITOR_INSPECTOR_IDLE_TIMEOUT_MS";
const EDITOR_MUTATION_RESULT_TIMEOUT: Duration = Duration::from_secs(2);
const EDITOR_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(8);
const DEFAULT_EDITOR_AUTHENTICATED_IDLE_TIMEOUT: Duration = Duration::from_secs(30);
const DEFAULT_MAX_EDITOR_INSPECTOR_CONNECTIONS: usize = 8;
const HARD_MAX_EDITOR_INSPECTOR_CONNECTIONS: usize = 64;
const MAX_EDITOR_ENVELOPE_BYTES: usize = 64 * 1024;
const MAX_PENDING_EDITOR_MUTATIONS: usize = 128;
const MAX_PENDING_MUTATION_RESULTS: usize = 128;
const MAX_PENDING_RUNTIME_CONTROLS: usize = 64;

#[derive(Debug, Clone)]
pub struct EditorInspectorServiceConfig {
    pub control: EditorControlConfig,
    pub project_id: String,
    pub target_build_id: EditorBuildId,
    pub world_revision: EditorWorldRevision,
    pub tick_rate_hz: u32,
    pub schema_revision: EditorSchemaRevision,
    pub diagnostic_schema_revision: EditorSchemaRevision,
    pub granted_capabilities: Vec<EditorCapability>,
    pub component_schemas: Vec<EditorComponentSchema>,
    pub runtime_state: EditorInspectorRuntimeState,
}

impl EditorInspectorServiceConfig {
    #[must_use]
    pub fn local_development(
        target_kind: EditorTargetKind,
        project_id: impl Into<String>,
        granted_capabilities: Vec<EditorCapability>,
    ) -> Self {
        Self {
            control: EditorControlConfig::local_development(target_kind),
            project_id: project_id.into(),
            target_build_id: EditorBuildId(1),
            world_revision: EditorWorldRevision(0),
            tick_rate_hz: 60,
            schema_revision: EditorSchemaRevision(1),
            diagnostic_schema_revision: EditorSchemaRevision(1),
            granted_capabilities,
            component_schemas: Vec::new(),
            runtime_state: EditorInspectorRuntimeState::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditorInspectorServiceError {
    Disabled,
    AuthUnavailable(&'static str),
    AuthExpired,
    InvalidSession,
    InvalidTokenProof,
    InvalidNonceResponse,
    InvalidTarget,
    ConnectionLimitExceeded,
    Protocol(String),
    Io(String),
    RemoteBindDisabled,
}

impl fmt::Display for EditorInspectorServiceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Disabled => f.write_str("editor inspector is disabled"),
            Self::AuthUnavailable(name) => write!(f, "missing editor auth environment `{name}`"),
            Self::AuthExpired => f.write_str("editor auth session has expired"),
            Self::InvalidSession => f.write_str("editor session id did not match runtime auth"),
            Self::InvalidTokenProof => f.write_str("editor auth token proof was invalid"),
            Self::InvalidNonceResponse => f.write_str("editor auth nonce response was invalid"),
            Self::InvalidTarget => f.write_str("editor handshake target did not match runtime"),
            Self::ConnectionLimitExceeded => {
                f.write_str("editor inspector connection limit exceeded")
            }
            Self::Protocol(error) => write!(f, "editor protocol error: {error}"),
            Self::Io(error) => write!(f, "editor inspector IO error: {error}"),
            Self::RemoteBindDisabled => {
                f.write_str("remote editor inspector bind requires explicit remote enable")
            }
        }
    }
}

impl std::error::Error for EditorInspectorServiceError {}

#[derive(Debug, Clone)]
pub struct EditorInspectorServiceSummary {
    pub callback_started: bool,
    pub bind_addr: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EditorInspectorRuntimeState {
    inner: Arc<Mutex<EditorInspectorSnapshot>>,
}

#[derive(Debug, Clone)]
pub struct EditorInspectorSnapshot {
    pub world_revision: EditorWorldRevision,
    pub schema_revision: EditorSchemaRevision,
    pub diagnostic_schema_revision: EditorSchemaRevision,
    pub component_schemas: Vec<EditorComponentSchema>,
    pub entities: Vec<EditorEntityRow>,
    pub diagnostics: RuntimeDiagnosticSinks,
    pub editor_attached: bool,
    editor_attachment_count: usize,
    pending_mutations: VecDeque<EditorMutationTransaction>,
    mutation_results: VecDeque<EditorInspectorMutationResult>,
    pending_runtime_controls: VecDeque<EditorRuntimeControlCommand>,
}

#[derive(Debug, Clone)]
struct EditorInspectorMutationResult {
    transaction_id: EditorTransactionId,
    world_revision: EditorWorldRevision,
    events: Vec<EditorEventPayload>,
}

impl Default for EditorInspectorRuntimeState {
    fn default() -> Self {
        Self {
            inner: Arc::new(Mutex::new(EditorInspectorSnapshot::default())),
        }
    }
}

impl Default for EditorInspectorSnapshot {
    fn default() -> Self {
        Self {
            world_revision: EditorWorldRevision(0),
            schema_revision: EditorSchemaRevision(1),
            diagnostic_schema_revision: EditorSchemaRevision(1),
            component_schemas: Vec::new(),
            entities: Vec::new(),
            diagnostics: RuntimeDiagnosticSinks::disabled(),
            editor_attached: false,
            editor_attachment_count: 0,
            pending_mutations: VecDeque::new(),
            mutation_results: VecDeque::new(),
            pending_runtime_controls: VecDeque::new(),
        }
    }
}

impl EditorInspectorRuntimeState {
    pub fn replace_entities(
        &self,
        world_revision: EditorWorldRevision,
        schema_revision: EditorSchemaRevision,
        component_schemas: Vec<EditorComponentSchema>,
        mut entities: Vec<EditorEntityRow>,
    ) {
        entities.sort_by_key(|row| row.entity);
        if let Ok(mut snapshot) = self.inner.lock() {
            snapshot.world_revision = world_revision;
            snapshot.schema_revision = schema_revision;
            snapshot.component_schemas = component_schemas;
            snapshot.entities = entities;
        }
    }

    pub fn set_diagnostic_schema_revision(&self, revision: EditorSchemaRevision) {
        if let Ok(mut snapshot) = self.inner.lock() {
            snapshot.diagnostic_schema_revision = revision;
        }
    }

    #[must_use]
    pub fn editor_attached(&self) -> bool {
        self.inner
            .lock()
            .map(|snapshot| snapshot.editor_attached)
            .unwrap_or(false)
    }

    pub fn mark_editor_attached(&self, attached: bool) {
        if let Ok(mut snapshot) = self.inner.lock() {
            if attached {
                snapshot.editor_attachment_count =
                    snapshot.editor_attachment_count.saturating_add(1);
            } else {
                snapshot.editor_attachment_count =
                    snapshot.editor_attachment_count.saturating_sub(1);
            }
            let now_attached = snapshot.editor_attachment_count > 0;
            if snapshot.editor_attached == now_attached {
                return;
            }
            snapshot.editor_attached = now_attached;
            snapshot.diagnostics = if now_attached {
                RuntimeDiagnosticSinks::editor_attached(512, 512)
            } else {
                RuntimeDiagnosticSinks::disabled()
            };
        }
    }

    pub fn emit_diagnostic(&self, packet: crate::DiagnosticPacket) {
        if let Ok(mut snapshot) = self.inner.lock()
            && snapshot.editor_attached
        {
            let _ = snapshot.diagnostics.emit(packet);
        }
    }

    #[must_use]
    pub fn query_entities(
        &self,
        request_id: EditorRequestId,
        query: &EditorEntityQuery,
    ) -> EditorEntityPage {
        let Ok(snapshot) = self.inner.lock() else {
            return EditorEntityPage {
                request_id,
                world_revision: EditorWorldRevision(0),
                cursor: None,
                rows: Vec::new(),
                has_more: false,
            };
        };

        let start = query.cursor.map_or(0usize, |cursor| cursor.0 as usize);
        let limit = usize::from(query.limit.max(1));
        let filtered = snapshot
            .entities
            .iter()
            .filter(|row| {
                if query.entity.is_some_and(|entity| row.entity != entity) {
                    return false;
                }
                query.component_filter.is_none_or(|component| {
                    row.components
                        .iter()
                        .any(|value| value.component_kind == component)
                })
            })
            .collect::<Vec<_>>();
        let end = start.saturating_add(limit).min(filtered.len());
        let rows = if start >= filtered.len() {
            Vec::new()
        } else {
            filtered[start..end]
                .iter()
                .map(|row| {
                    let mut row = (*row).clone();
                    if !query.include_components {
                        row.components.clear();
                    }
                    row
                })
                .collect()
        };

        EditorEntityPage {
            request_id,
            world_revision: snapshot.world_revision,
            cursor: (end < filtered.len()).then_some(EditorPageCursor(end as u64)),
            rows,
            has_more: end < filtered.len(),
        }
    }

    #[must_use]
    pub fn drain_diagnostics(&self, max_packets: usize) -> EditorDiagnosticBatch {
        let Ok(mut snapshot) = self.inner.lock() else {
            return EditorDiagnosticBatch {
                world_revision: EditorWorldRevision(0),
                packets: Vec::new(),
            };
        };
        let world_revision = snapshot.world_revision;
        let packets = snapshot.diagnostics.editor_live_stream.drain(max_packets);
        EditorDiagnosticBatch {
            world_revision,
            packets,
        }
    }

    #[must_use]
    pub fn drain_diagnostics_for_subscription(
        &self,
        subscription: &EditorDiagnosticSubscription,
        max_packets: usize,
    ) -> EditorDiagnosticBatch {
        let Ok(mut snapshot) = self.inner.lock() else {
            return EditorDiagnosticBatch {
                world_revision: EditorWorldRevision(0),
                packets: Vec::new(),
            };
        };
        let world_revision = snapshot.world_revision;
        let source_packets = if let Some(sequence) = subscription.since_sequence {
            snapshot
                .diagnostics
                .ring
                .packets_since(Some(crate::DiagnosticSequence(sequence.0.into())))
        } else {
            snapshot.diagnostics.editor_live_stream.drain(usize::MAX)
        };
        let packets = filter_diagnostic_packets(source_packets, &subscription.streams, max_packets);
        EditorDiagnosticBatch {
            world_revision,
            packets,
        }
    }

    pub fn queue_mutation(&self, transaction: EditorMutationTransaction) {
        if let Ok(mut snapshot) = self.inner.lock() {
            if snapshot.pending_mutations.len() >= MAX_PENDING_EDITOR_MUTATIONS {
                let _ = snapshot.pending_mutations.pop_front();
            }
            snapshot.pending_mutations.push_back(transaction);
        }
    }

    #[must_use]
    pub fn drain_mutations(&self, max_transactions: usize) -> Vec<EditorMutationTransaction> {
        let Ok(mut snapshot) = self.inner.lock() else {
            return Vec::new();
        };
        let take = max_transactions.min(snapshot.pending_mutations.len());
        snapshot.pending_mutations.drain(..take).collect()
    }

    pub fn complete_mutation(
        &self,
        transaction_id: EditorTransactionId,
        world_revision: EditorWorldRevision,
        events: Vec<EditorEventPayload>,
    ) {
        if let Ok(mut snapshot) = self.inner.lock() {
            if snapshot.mutation_results.len() >= MAX_PENDING_MUTATION_RESULTS {
                let _ = snapshot.mutation_results.pop_front();
            }
            snapshot
                .mutation_results
                .push_back(EditorInspectorMutationResult {
                    transaction_id,
                    world_revision,
                    events,
                });
        }
    }

    #[must_use]
    fn take_mutation_result(
        &self,
        transaction_id: EditorTransactionId,
    ) -> Option<EditorInspectorMutationResult> {
        let mut snapshot = self.inner.lock().ok()?;
        let index = snapshot
            .mutation_results
            .iter()
            .position(|result| result.transaction_id == transaction_id)?;
        snapshot.mutation_results.remove(index)
    }

    pub fn queue_runtime_control(&self, command: EditorRuntimeControlCommand) {
        if let Ok(mut snapshot) = self.inner.lock() {
            if snapshot.pending_runtime_controls.len() >= MAX_PENDING_RUNTIME_CONTROLS {
                let _ = snapshot.pending_runtime_controls.pop_front();
            }
            snapshot.pending_runtime_controls.push_back(command);
        }
    }

    #[must_use]
    pub fn drain_runtime_controls(&self, max_commands: usize) -> Vec<EditorRuntimeControlCommand> {
        let Ok(mut snapshot) = self.inner.lock() else {
            return Vec::new();
        };
        let take = max_commands.min(snapshot.pending_runtime_controls.len());
        snapshot.pending_runtime_controls.drain(..take).collect()
    }

    #[must_use]
    pub fn metadata(&self) -> EditorInspectorSnapshotMetadata {
        self.inner
            .lock()
            .map(|snapshot| EditorInspectorSnapshotMetadata {
                world_revision: snapshot.world_revision,
                schema_revision: snapshot.schema_revision,
                diagnostic_schema_revision: snapshot.diagnostic_schema_revision,
                component_schemas: snapshot.component_schemas.clone(),
            })
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Default)]
pub struct EditorInspectorSnapshotMetadata {
    pub world_revision: EditorWorldRevision,
    pub schema_revision: EditorSchemaRevision,
    pub diagnostic_schema_revision: EditorSchemaRevision,
    pub component_schemas: Vec<EditorComponentSchema>,
}

pub fn spawn_editor_inspector_service(
    config: EditorInspectorServiceConfig,
) -> Result<EditorInspectorServiceSummary, EditorInspectorServiceError> {
    if !config.control.enabled {
        return Ok(EditorInspectorServiceSummary {
            callback_started: false,
            bind_addr: None,
        });
    }

    let control_addr = env::var(EDITOR_CONTROL_ADDR_ENV).ok();
    let bind_enabled = inspector_bind_enabled(config.control.target_kind);
    if control_addr.is_none() && !bind_enabled {
        return Ok(EditorInspectorServiceSummary {
            callback_started: false,
            bind_addr: None,
        });
    }

    let auth = RuntimeEditorAuth::from_env()?;
    let connection_limiter = EditorConnectionLimiter::from_env();
    let mut callback_started = false;
    if let Some(control_addr) = control_addr {
        let callback_config = config.clone();
        let callback_auth = auth.clone();
        let callback_limiter = connection_limiter.clone();
        thread::Builder::new()
            .name(format!(
                "fun-editor-{:?}-callback",
                config.control.target_kind.runtime_class()
            ))
            .spawn(move || {
                if let Ok(stream) = TcpStream::connect(control_addr) {
                    let _ = handle_limited_runtime_editor_connection(
                        stream,
                        callback_config,
                        callback_auth,
                        callback_limiter,
                    );
                }
            })
            .map_err(io_error)?;
        callback_started = true;
    }

    let bind_addr = if bind_enabled {
        let bind_addr = inspector_bind_addr(&config);
        validate_bind_addr(&bind_addr, config.control.bind_mode)?;
        let listener = TcpListener::bind(&bind_addr).map_err(io_error)?;
        let local_addr = listener.local_addr().map_err(io_error)?.to_string();
        let listener_config = config.clone();
        let listener_auth = auth;
        let listener_limiter = connection_limiter;
        thread::Builder::new()
            .name(format!(
                "fun-editor-{:?}-inspector",
                config.control.target_kind.runtime_class()
            ))
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let Some(connection_guard) = listener_limiter.try_acquire() else {
                        let _ = stream.shutdown(std::net::Shutdown::Both);
                        continue;
                    };
                    let config = listener_config.clone();
                    let auth = listener_auth.clone();
                    let _ = thread::Builder::new()
                        .name("fun-editor-inspector-connection".to_owned())
                        .spawn(move || {
                            let _ = handle_runtime_editor_connection(
                                stream,
                                config,
                                auth,
                                connection_guard,
                            );
                        });
                }
            })
            .map_err(io_error)?;
        Some(local_addr)
    } else {
        None
    };

    Ok(EditorInspectorServiceSummary {
        callback_started,
        bind_addr,
    })
}

pub fn editor_auth_token_proof(
    token: &[u8],
    session_id: EditorSessionId,
    editor_nonce: &[u8],
    runtime_nonce: &[u8],
) -> Vec<u8> {
    hmac_digest(
        token,
        b"fun-editor-auth-token-proof-v1",
        session_id,
        editor_nonce,
        runtime_nonce,
    )
}

pub fn editor_auth_nonce_response(
    token: &[u8],
    session_id: EditorSessionId,
    editor_nonce: &[u8],
    runtime_nonce: &[u8],
) -> Vec<u8> {
    hmac_digest(
        token,
        b"fun-editor-auth-nonce-response-v1",
        session_id,
        runtime_nonce,
        editor_nonce,
    )
}

fn hmac_digest(
    token: &[u8],
    label: &[u8],
    session_id: EditorSessionId,
    first_nonce: &[u8],
    second_nonce: &[u8],
) -> Vec<u8> {
    let key = hmac::Key::new(hmac::HMAC_SHA256, token);
    let mut context = hmac::Context::with_key(&key);
    context.update(label);
    context.update(&session_id.0.to_le_bytes());
    context.update(first_nonce);
    context.update(second_nonce);
    context.sign().as_ref().to_vec()
}

#[derive(Debug, Clone)]
struct EditorConnectionLimiter {
    active: Arc<AtomicUsize>,
    max_connections: usize,
}

impl EditorConnectionLimiter {
    fn from_env() -> Self {
        let max_connections = env::var(EDITOR_MAX_CONNECTIONS_ENV)
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .map(|value| value.clamp(1, HARD_MAX_EDITOR_INSPECTOR_CONNECTIONS))
            .unwrap_or(DEFAULT_MAX_EDITOR_INSPECTOR_CONNECTIONS);
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            max_connections,
        }
    }

    fn try_acquire(&self) -> Option<EditorConnectionGuard> {
        let mut observed = self.active.load(Ordering::Acquire);
        loop {
            if observed >= self.max_connections {
                return None;
            }
            match self.active.compare_exchange_weak(
                observed,
                observed + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    return Some(EditorConnectionGuard {
                        active: Arc::clone(&self.active),
                    });
                }
                Err(actual) => observed = actual,
            }
        }
    }
}

#[derive(Debug)]
struct EditorConnectionGuard {
    active: Arc<AtomicUsize>,
}

impl Drop for EditorConnectionGuard {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

fn handle_limited_runtime_editor_connection(
    stream: TcpStream,
    config: EditorInspectorServiceConfig,
    auth: RuntimeEditorAuth,
    limiter: EditorConnectionLimiter,
) -> Result<(), EditorInspectorServiceError> {
    let Some(connection_guard) = limiter.try_acquire() else {
        let _ = stream.shutdown(std::net::Shutdown::Both);
        return Err(EditorInspectorServiceError::ConnectionLimitExceeded);
    };
    handle_runtime_editor_connection(stream, config, auth, connection_guard)
}

fn handle_runtime_editor_connection(
    mut stream: TcpStream,
    config: EditorInspectorServiceConfig,
    auth: RuntimeEditorAuth,
    _connection_guard: EditorConnectionGuard,
) -> Result<(), EditorInspectorServiceError> {
    stream
        .set_read_timeout(Some(EDITOR_HANDSHAKE_TIMEOUT))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(EDITOR_HANDSHAKE_TIMEOUT))
        .map_err(io_error)?;

    let hello_packet = read_editor_protocol_packet(&mut stream)?;
    let context = validation_context(&config, &auth);
    validate_editor_packet(&hello_packet, &context)
        .map_err(|error| EditorInspectorServiceError::Protocol(format!("{error:?}")))?;
    let hello = match hello_packet {
        EditorProtocolPacket::Handshake { packet } => match packet.payload {
            EditorHandshakePayload::Hello { hello } => hello,
            _ => {
                return Err(EditorInspectorServiceError::Protocol(
                    "first packet must be EditorHello".to_owned(),
                ));
            }
        },
        _ => {
            return Err(EditorInspectorServiceError::Protocol(
                "first packet must be a handshake packet".to_owned(),
            ));
        }
    };

    if hello.target_kind.runtime_class() != config.control.target_kind.runtime_class() {
        return Err(EditorInspectorServiceError::InvalidTarget);
    }
    if hello.session_id != auth.session_id {
        return Err(EditorInspectorServiceError::InvalidSession);
    }
    if auth.expired() {
        return Err(EditorInspectorServiceError::AuthExpired);
    }

    let runtime_nonce = random_nonce()?;
    let auth_required = EditorProtocolPacket::Handshake {
        packet: EditorHandshakePacket {
            header: EditorPacketHeader::new(
                EditorRequestId(1),
                config.control.target_kind,
                PacketSequence(1),
                Some(config.world_revision),
                EditorSizeBudget { max_bytes: 4096 },
            ),
            payload: EditorHandshakePayload::AuthRequired {
                required: EditorAuthRequired {
                    protocol_version: EDITOR_PROTOCOL_VERSION,
                    target_kind: config.control.target_kind,
                    nonce: runtime_nonce.clone(),
                },
            },
        },
    };
    write_editor_protocol_packet(&mut stream, &auth_required)?;

    let auth_packet = read_editor_protocol_packet(&mut stream)?;
    validate_editor_packet(&auth_packet, &context)
        .map_err(|error| EditorInspectorServiceError::Protocol(format!("{error:?}")))?;
    let editor_auth = match auth_packet {
        EditorProtocolPacket::Handshake { packet } => match packet.payload {
            EditorHandshakePayload::Auth { auth } => auth,
            _ => {
                return Err(EditorInspectorServiceError::Protocol(
                    "expected EditorAuth after AuthRequired".to_owned(),
                ));
            }
        },
        _ => {
            return Err(EditorInspectorServiceError::Protocol(
                "auth packet must be a handshake packet".to_owned(),
            ));
        }
    };
    validate_auth_packet(&editor_auth, &auth, &hello.nonce, &runtime_nonce)?;

    let granted_capabilities = negotiated_capabilities(
        &hello.requested_capabilities,
        &auth.allowed_capabilities,
        &config.granted_capabilities,
    );
    let metadata = config.runtime_state.metadata();
    let welcome = EditorProtocolPacket::Handshake {
        packet: EditorHandshakePacket {
            header: EditorPacketHeader::new(
                EditorRequestId(2),
                config.control.target_kind,
                PacketSequence(2),
                Some(metadata.world_revision),
                EditorSizeBudget {
                    max_bytes: 16 * 1024,
                },
            ),
            payload: EditorHandshakePayload::Welcome {
                welcome: EditorWelcome {
                    granted_capabilities: granted_capabilities.clone(),
                    target_build_id: config.target_build_id,
                    world_revision: metadata.world_revision,
                    tick_rate_hz: config.tick_rate_hz,
                    schema_revision: metadata.schema_revision,
                    diagnostic_schema_revision: metadata.diagnostic_schema_revision,
                    component_schemas: metadata.component_schemas,
                },
            },
        },
    };
    write_editor_protocol_packet(&mut stream, &welcome)?;

    config.runtime_state.mark_editor_attached(true);
    let result = serve_authenticated_editor_connection(&mut stream, &config, granted_capabilities);
    config.runtime_state.mark_editor_attached(false);
    result
}

fn serve_authenticated_editor_connection(
    stream: &mut TcpStream,
    config: &EditorInspectorServiceConfig,
    granted_capabilities: Vec<EditorCapability>,
) -> Result<(), EditorInspectorServiceError> {
    stream
        .set_read_timeout(Some(authenticated_idle_timeout_from_env()))
        .map_err(io_error)?;
    loop {
        let packet = match read_editor_protocol_packet(stream) {
            Ok(packet) => packet,
            Err(EditorInspectorServiceError::Io(_)) => return Ok(()),
            Err(error) => return Err(error),
        };
        let context = validation_context_with_capabilities(config, granted_capabilities.clone());
        validate_editor_packet(&packet, &context)
            .map_err(|error| EditorInspectorServiceError::Protocol(format!("{error:?}")))?;

        let EditorProtocolPacket::Command { packet } = packet else {
            continue;
        };
        handle_editor_command_packet(stream, config, packet)?;
    }
}

fn authenticated_idle_timeout_from_env() -> Duration {
    env::var(EDITOR_IDLE_TIMEOUT_MS_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_millis)
        .filter(|duration| !duration.is_zero())
        .unwrap_or(DEFAULT_EDITOR_AUTHENTICATED_IDLE_TIMEOUT)
}

fn handle_editor_command_packet(
    stream: &mut TcpStream,
    config: &EditorInspectorServiceConfig,
    packet: EditorCommandPacket,
) -> Result<(), EditorInspectorServiceError> {
    match packet.payload {
        EditorCommandPayload::QueryEntities { query } => {
            let page = config
                .runtime_state
                .query_entities(packet.header.request_id, &query);
            write_editor_protocol_packet(
                stream,
                &EditorProtocolPacket::Event {
                    packet: EditorEventPacket {
                        header: response_header(&packet.header, page.world_revision),
                        payload: EditorEventPayload::EntityPage { page },
                    },
                },
            )
        }
        EditorCommandPayload::SubscribeDiagnostics { subscription } => {
            let batch = config
                .runtime_state
                .drain_diagnostics_for_subscription(&subscription, 256);
            write_editor_protocol_packet(
                stream,
                &EditorProtocolPacket::Diagnostic {
                    packet: EditorDiagnosticPacket {
                        header: response_header(&packet.header, batch.world_revision),
                        batch,
                    },
                },
            )
        }
        EditorCommandPayload::Ping => {
            let world_revision = config.runtime_state.metadata().world_revision;
            write_editor_protocol_packet(
                stream,
                &EditorProtocolPacket::Event {
                    packet: EditorEventPacket {
                        header: response_header(&packet.header, world_revision),
                        payload: EditorEventPayload::Pong { world_revision },
                    },
                },
            )
        }
        EditorCommandPayload::Mutate { transaction } => {
            let transaction_id = transaction.transaction_id;
            config.runtime_state.queue_mutation(transaction);
            let result = wait_for_mutation_result(config, transaction_id)
                .unwrap_or_else(|| mutation_timeout_result(&packet.header, transaction_id));
            for event in result.events {
                write_editor_protocol_packet(
                    stream,
                    &EditorProtocolPacket::Event {
                        packet: EditorEventPacket {
                            header: response_header(&packet.header, result.world_revision),
                            payload: event,
                        },
                    },
                )?;
            }
            Ok(())
        }
        EditorCommandPayload::RuntimeControl { command } => {
            let command_id = command.command_id();
            config.runtime_state.queue_runtime_control(command);
            let world_revision = config.runtime_state.metadata().world_revision;
            write_editor_protocol_packet(
                stream,
                &EditorProtocolPacket::Event {
                    packet: EditorEventPacket {
                        header: response_header(&packet.header, world_revision),
                        payload: EditorEventPayload::Audit {
                            event: EditorAuditEvent {
                                sequence: packet.header.sequence,
                                capability: EditorCapability::ControlRuntime,
                                target_kind: packet.header.target,
                                message: format!(
                                    "queued authenticated runtime control command `{command_id}`"
                                ),
                            },
                        },
                    },
                },
            )
        }
        EditorCommandPayload::Exec { request } => {
            let world_revision = config.runtime_state.metadata().world_revision;
            write_editor_protocol_packet(
                stream,
                &EditorProtocolPacket::Event {
                    packet: EditorEventPacket {
                        header: response_header(&packet.header, world_revision),
                        payload: EditorEventPayload::ExecResult {
                            result: EditorExecResult {
                                header: response_header(&request.header, world_revision),
                                request_id: request.request_id,
                                status: EditorExecStatus::Rejected,
                                stdout_events: Vec::new(),
                                diagnostic_events: Vec::new(),
                                mutations: Vec::new(),
                                duration_ns: 0,
                                error: Some(
                                    "runtime execution is not implemented for this target"
                                        .to_owned(),
                                ),
                            },
                        },
                    },
                },
            )
        }
        EditorCommandPayload::Persist { request } => {
            let world_revision = config.runtime_state.metadata().world_revision;
            write_editor_protocol_packet(
                stream,
                &EditorProtocolPacket::Event {
                    packet: EditorEventPacket {
                        header: response_header(&packet.header, world_revision),
                        payload: EditorEventPayload::PersistenceAck {
                            ack: EditorPersistenceAck {
                                header: response_header(&request.header, world_revision),
                                transaction_id: request.transaction_id,
                                status: EditorPersistenceStatus::Rejected,
                                world_revision,
                                diagnostics: vec![mutation_diagnostic_event(
                                    "persistence_not_implemented",
                                    request.transaction_id,
                                    world_revision,
                                    "runtime persistence is not implemented for this target",
                                    crate::DiagnosticLevel::Warn,
                                )],
                            },
                        },
                    },
                },
            )
        }
    }
}

fn filter_diagnostic_packets(
    packets: Vec<crate::RecordedDiagnosticPacket>,
    streams: &[EditorDiagnosticStream],
    max_packets: usize,
) -> Vec<crate::RecordedDiagnosticPacket> {
    let mut filtered = Vec::with_capacity(max_packets.min(packets.len()));
    if streams.is_empty() || max_packets == 0 {
        return filtered;
    }

    for packet in packets {
        if diagnostic_packet_matches_streams(&packet.packet, streams) {
            filtered.push(packet);
            if filtered.len() == max_packets {
                break;
            }
        }
    }
    filtered
}

fn diagnostic_packet_matches_streams(
    packet: &crate::DiagnosticPacket,
    streams: &[EditorDiagnosticStream],
) -> bool {
    streams
        .iter()
        .copied()
        .any(|stream| diagnostic_packet_matches_stream(packet, stream))
}

fn diagnostic_packet_matches_stream(
    packet: &crate::DiagnosticPacket,
    stream: EditorDiagnosticStream,
) -> bool {
    match packet {
        crate::DiagnosticPacket::Event { event } => {
            event
                .fields
                .iter()
                .filter(|field| field.name.eq_ignore_ascii_case("stream"))
                .any(|field| match &field.value {
                    crate::DiagnosticValue::Text { value } => {
                        diagnostic_label_matches_stream(value, stream)
                    }
                    _ => false,
                })
                || diagnostic_label_matches_stream(&event.target, stream)
                || event.fields.iter().any(|field| match &field.value {
                    crate::DiagnosticValue::Text { value } => {
                        diagnostic_label_matches_stream(value, stream)
                    }
                    _ => false,
                })
        }
        crate::DiagnosticPacket::Counter { counter } => {
            diagnostic_label_matches_stream(&counter.name, stream)
                || diagnostic_label_matches_stream(&counter.target, stream)
        }
        crate::DiagnosticPacket::Span { span } => {
            diagnostic_label_matches_stream(&span.name, stream)
                || diagnostic_label_matches_stream(&span.target, stream)
        }
        crate::DiagnosticPacket::FrameProfile { profile } => {
            matches!(stream, EditorDiagnosticStream::FrameProfilerSummary)
                || profile.rows.iter().any(|row| {
                    diagnostic_label_matches_stream(&row.name, stream)
                        || diagnostic_label_matches_stream(&row.target, stream)
                })
        }
    }
}

fn diagnostic_label_matches_stream(label: &str, stream: EditorDiagnosticStream) -> bool {
    normalized_label_matches(label, diagnostic_stream_key(stream))
        || diagnostic_stream_aliases(stream)
            .iter()
            .any(|alias| normalized_label_matches(label, alias))
}

fn diagnostic_stream_key(stream: EditorDiagnosticStream) -> &'static str {
    match stream {
        EditorDiagnosticStream::ServerTickDuration => "servertickduration",
        EditorDiagnosticStream::ServerTickBudget => "servertickbudget",
        EditorDiagnosticStream::ConnectedClients => "connectedclients",
        EditorDiagnosticStream::NetworkChannelPressure => "networkchannelpressure",
        EditorDiagnosticStream::PacketBytes => "packetbytes",
        EditorDiagnosticStream::WorldRevision => "worldrevision",
        EditorDiagnosticStream::MutationCost => "mutationcost",
        EditorDiagnosticStream::SnapshotBudget => "snapshotbudget",
        EditorDiagnosticStream::SnapshotBudgetExhaustion => "snapshotbudgetexhaustion",
        EditorDiagnosticStream::RelevanceDecisions => "relevancedecisions",
        EditorDiagnosticStream::WorldStreamRevisions => "worldstreamrevisions",
        EditorDiagnosticStream::StreamChunks => "streamchunks",
        EditorDiagnosticStream::MutationTransactions => "mutationtransactions",
        EditorDiagnosticStream::ClientFps => "clientfps",
        EditorDiagnosticStream::ClientFrameNs => "clientframens",
        EditorDiagnosticStream::ClientFrameTime => "clientframetime",
        EditorDiagnosticStream::FrameProfilerSummary => "frameprofilersummary",
        EditorDiagnosticStream::GpuSampleStatus => "gpusamplestatus",
        EditorDiagnosticStream::RenderCpuMaterialCounters => "rendercpumaterialcounters",
        EditorDiagnosticStream::MeshletPathCounts => "meshletpathcounts",
        EditorDiagnosticStream::ScheduleHeatmap => "scheduleheatmap",
        EditorDiagnosticStream::RenderPathCounts => "renderpathcounts",
        EditorDiagnosticStream::SolariTimings => "solaritimings",
        EditorDiagnosticStream::SolariBudget => "solaribudget",
        EditorDiagnosticStream::SolariQueue => "solariqueue",
        EditorDiagnosticStream::SolariRadianceCachePressure => "solariradiancecachepressure",
        EditorDiagnosticStream::RenderRecovery => "renderrecovery",
        EditorDiagnosticStream::CameraCount => "cameracount",
        EditorDiagnosticStream::WorldStreamApplyCost => "worldstreamapplycost",
        EditorDiagnosticStream::AvianPhysicsStepTiming => "avianphysicssteptiming",
        EditorDiagnosticStream::AvianCollisionDiagnostics => "aviancollisiondiagnostics",
        EditorDiagnosticStream::AvianControllerDiagnostics => "aviancontrollerdiagnostics",
        EditorDiagnosticStream::AiPresentation => "aipresentation",
        EditorDiagnosticStream::AiLodDebug => "ailoddebug",
        EditorDiagnosticStream::SquadAiDebug => "squadaidebug",
    }
}

fn diagnostic_stream_aliases(stream: EditorDiagnosticStream) -> &'static [&'static str] {
    match stream {
        EditorDiagnosticStream::ClientFps => &["fps"],
        EditorDiagnosticStream::FrameProfilerSummary => &["frameprofile", "profile"],
        EditorDiagnosticStream::GpuSampleStatus => &["gpusample"],
        EditorDiagnosticStream::MutationTransactions => &["mutationtransaction"],
        EditorDiagnosticStream::PacketBytes => &["packet"],
        EditorDiagnosticStream::StreamChunks => &["worldstream", "chunk"],
        EditorDiagnosticStream::WorldStreamRevisions => &["worldstreamrevision"],
        EditorDiagnosticStream::AiPresentation => &["ai"],
        EditorDiagnosticStream::AiLodDebug => &["ailod"],
        EditorDiagnosticStream::SquadAiDebug => &["squad"],
        _ => &[],
    }
}

fn normalized_label_matches(label: &str, expected: &str) -> bool {
    let mut expected = expected.bytes();
    for byte in label.bytes() {
        if !byte.is_ascii_alphanumeric() {
            continue;
        }
        if expected.next() != Some(byte.to_ascii_lowercase()) {
            return false;
        }
    }
    expected.next().is_none()
}

fn wait_for_mutation_result(
    config: &EditorInspectorServiceConfig,
    transaction_id: EditorTransactionId,
) -> Option<EditorInspectorMutationResult> {
    let started = std::time::Instant::now();
    while started.elapsed() < EDITOR_MUTATION_RESULT_TIMEOUT {
        if let Some(result) = config.runtime_state.take_mutation_result(transaction_id) {
            return Some(result);
        }
        thread::sleep(Duration::from_millis(5));
    }
    None
}

fn mutation_timeout_result(
    request: &EditorPacketHeader,
    transaction_id: EditorTransactionId,
) -> EditorInspectorMutationResult {
    let world_revision = request.base_world_revision.unwrap_or_default();
    let diagnostic = mutation_diagnostic_event(
        "mutation_apply_timeout",
        transaction_id,
        world_revision,
        "runtime did not apply editor mutation before the inspector timeout",
        crate::DiagnosticLevel::Warn,
    );
    let ack = EditorMutationAck {
        header: response_header(request, world_revision),
        transaction_id,
        status: EditorMutationStatus::Failed,
        world_revision,
        applied_ops: 0,
        conflict_count: 0,
        diagnostics: vec![diagnostic],
    };

    EditorInspectorMutationResult {
        transaction_id,
        world_revision,
        events: vec![EditorEventPayload::MutationAck { ack }],
    }
}

fn mutation_diagnostic_event(
    code: &str,
    transaction_id: EditorTransactionId,
    world_revision: EditorWorldRevision,
    message: &str,
    level: crate::DiagnosticLevel,
) -> EditorDiagnosticEvent {
    crate::DiagnosticEvent {
        target: "server".to_owned(),
        level,
        timestamp_or_tick: crate::DiagnosticTimestampOrTick::TimestampNs { ns: unix_ns() },
        fields: vec![
            crate::DiagnosticField::text("stream", "mutation_transactions"),
            crate::DiagnosticField::text("code", code),
            crate::DiagnosticField::u64("transaction_id", transaction_id.0),
            crate::DiagnosticField::u64("world_revision", world_revision.0),
            crate::DiagnosticField::text("message", message),
        ],
        source: crate::DiagnosticSource::static_location(file!(), line!(), module_path!()),
        frame_index: None,
        span_id: None,
    }
}

fn response_header(
    request: &EditorPacketHeader,
    world_revision: EditorWorldRevision,
) -> EditorPacketHeader {
    EditorPacketHeader::new(
        request.request_id,
        request.target,
        PacketSequence(request.sequence.0.saturating_add(1)),
        Some(world_revision),
        EditorSizeBudget {
            max_bytes: request.size_budget.max_bytes,
        },
    )
}

fn validate_auth_packet(
    packet: &EditorAuth,
    auth: &RuntimeEditorAuth,
    editor_nonce: &[u8],
    runtime_nonce: &[u8],
) -> Result<(), EditorInspectorServiceError> {
    let expected_proof = editor_auth_token_proof(
        auth.token.as_bytes(),
        auth.session_id,
        editor_nonce,
        runtime_nonce,
    );
    if !constant_time_bytes_eq(&packet.token_proof, &expected_proof) {
        return Err(EditorInspectorServiceError::InvalidTokenProof);
    }

    let expected_response = editor_auth_nonce_response(
        auth.token.as_bytes(),
        auth.session_id,
        editor_nonce,
        runtime_nonce,
    );
    if !constant_time_bytes_eq(&packet.nonce_response, &expected_response) {
        return Err(EditorInspectorServiceError::InvalidNonceResponse);
    }
    Ok(())
}

fn constant_time_bytes_eq(left: &[u8], right: &[u8]) -> bool {
    let max_len = left.len().max(right.len());
    let mut diff = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or(0);
        let right_byte = right.get(index).copied().unwrap_or(0);
        diff |= usize::from(left_byte ^ right_byte);
    }
    diff == 0
}

fn negotiated_capabilities(
    requested: &[EditorCapability],
    auth_allowed: &[EditorCapability],
    runtime_allowed: &[EditorCapability],
) -> Vec<EditorCapability> {
    requested
        .iter()
        .copied()
        .filter(|capability| auth_allowed.contains(capability))
        .filter(|capability| runtime_allowed.contains(capability))
        .collect()
}

fn validation_context(
    config: &EditorInspectorServiceConfig,
    auth: &RuntimeEditorAuth,
) -> EditorProtocolValidationContext {
    validation_context_with_capabilities(config, auth.allowed_capabilities.clone())
}

fn validation_context_with_capabilities(
    config: &EditorInspectorServiceConfig,
    granted_capabilities: Vec<EditorCapability>,
) -> EditorProtocolValidationContext {
    let metadata = config.runtime_state.metadata();
    let mut context = EditorProtocolValidationContext {
        protocol_version: CURRENT_EDITOR_PROTOCOL_VERSION,
        granted_capabilities,
        world_revision: metadata.world_revision,
        max_payload_bytes: 256 * 1024,
        component_schemas: metadata.component_schemas,
    };
    if context.component_schemas.is_empty() {
        context.max_payload_bytes = 64 * 1024;
    }
    context
}

fn read_editor_protocol_packet(
    stream: &mut TcpStream,
) -> Result<EditorProtocolPacket, EditorInspectorServiceError> {
    let mut header = [0_u8; EDITOR_WIRE_ENVELOPE_HEADER_LEN];
    stream.read_exact(&mut header).map_err(io_error)?;
    let envelope_len = editor_wire_envelope_len(&header)
        .map_err(|error| EditorInspectorServiceError::Protocol(format!("{error:?}")))?;
    if envelope_len > MAX_EDITOR_ENVELOPE_BYTES {
        return Err(EditorInspectorServiceError::Protocol(
            "PayloadTooLarge".to_owned(),
        ));
    }
    let payload_len = envelope_len.saturating_sub(EDITOR_WIRE_ENVELOPE_HEADER_LEN);
    let mut bytes = Vec::with_capacity(envelope_len);
    bytes.extend_from_slice(&header);
    bytes.resize(envelope_len, 0);
    stream
        .read_exact(&mut bytes[EDITOR_WIRE_ENVELOPE_HEADER_LEN..][..payload_len])
        .map_err(io_error)?;
    decode_editor_packet(&bytes)
        .map_err(|error| EditorInspectorServiceError::Protocol(format!("{error:?}")))
}

fn write_editor_protocol_packet(
    stream: &mut TcpStream,
    packet: &EditorProtocolPacket,
) -> Result<(), EditorInspectorServiceError> {
    let bytes = encode_editor_packet(packet)
        .map_err(|error| EditorInspectorServiceError::Protocol(format!("{error:?}")))?;
    stream.write_all(&bytes).map_err(io_error)
}

#[derive(Debug, Clone)]
struct RuntimeEditorAuth {
    session_id: EditorSessionId,
    token: String,
    expires_unix_ms: u64,
    allowed_capabilities: Vec<EditorCapability>,
}

impl RuntimeEditorAuth {
    fn from_env() -> Result<Self, EditorInspectorServiceError> {
        let session_id = env::var(EDITOR_SESSION_ID_ENV)
            .map_err(|_| EditorInspectorServiceError::AuthUnavailable(EDITOR_SESSION_ID_ENV))?
            .parse::<u64>()
            .map(EditorSessionId)
            .map_err(|_| EditorInspectorServiceError::InvalidSession)?;
        let token = env::var(EDITOR_AUTH_TOKEN_ENV)
            .map_err(|_| EditorInspectorServiceError::AuthUnavailable(EDITOR_AUTH_TOKEN_ENV))?;
        let expires_unix_ms = env::var(EDITOR_AUTH_EXPIRES_ENV)
            .map_err(|_| EditorInspectorServiceError::AuthUnavailable(EDITOR_AUTH_EXPIRES_ENV))?
            .parse::<u64>()
            .map_err(|_| EditorInspectorServiceError::AuthExpired)?;
        let allowed_capabilities = env::var(EDITOR_ALLOWED_CAPS_ENV)
            .map_err(|_| EditorInspectorServiceError::AuthUnavailable(EDITOR_ALLOWED_CAPS_ENV))?
            .split(',')
            .filter_map(parse_editor_capability)
            .collect::<Vec<_>>();

        let auth = Self {
            session_id,
            token,
            expires_unix_ms,
            allowed_capabilities,
        };
        if auth.expired() {
            Err(EditorInspectorServiceError::AuthExpired)
        } else {
            Ok(auth)
        }
    }

    fn expired(&self) -> bool {
        unix_ms() >= self.expires_unix_ms
    }
}

fn inspector_bind_enabled(target_kind: EditorTargetKind) -> bool {
    env::var_os(EDITOR_BIND_ENABLE_ENV).is_some()
        || match target_kind.runtime_class() {
            EditorTargetKind::Server => env::var_os(EDITOR_SERVER_BIND_ENABLE_ENV).is_some(),
            EditorTargetKind::Client | EditorTargetKind::ClientId(_) => {
                env::var_os(EDITOR_CLIENT_BIND_ENABLE_ENV).is_some()
            }
        }
}

fn inspector_bind_addr(config: &EditorInspectorServiceConfig) -> String {
    match config.control.target_kind.runtime_class() {
        EditorTargetKind::Server => env::var(EDITOR_SERVER_BIND_ADDR_ENV)
            .unwrap_or_else(|_| config.control.bind_addr.clone()),
        EditorTargetKind::Client | EditorTargetKind::ClientId(_) => {
            env::var(EDITOR_CLIENT_BIND_ADDR_ENV)
                .unwrap_or_else(|_| config.control.bind_addr.clone())
        }
    }
}

fn validate_bind_addr(
    addr: &str,
    bind_mode: EditorBindMode,
) -> Result<(), EditorInspectorServiceError> {
    let remote_enabled = env::var_os(EDITOR_REMOTE_ENABLE_ENV).is_some()
        || matches!(bind_mode, EditorBindMode::ExplicitRemote);
    let mut addrs = addr.to_socket_addrs().map_err(io_error)?;
    let Some(socket_addr) = addrs.next() else {
        return Err(EditorInspectorServiceError::Io(format!(
            "could not resolve bind addr `{addr}`"
        )));
    };
    if is_loopback(socket_addr.ip()) || remote_enabled {
        Ok(())
    } else {
        Err(EditorInspectorServiceError::RemoteBindDisabled)
    }
}

fn is_loopback(addr: IpAddr) -> bool {
    match addr {
        IpAddr::V4(addr) => addr.is_loopback(),
        IpAddr::V6(addr) => addr.is_loopback(),
    }
}

fn random_nonce() -> Result<Vec<u8>, EditorInspectorServiceError> {
    let mut nonce = vec![0_u8; 32];
    getrandom::fill(&mut nonce).map_err(|error| {
        EditorInspectorServiceError::Io(format!("could not create runtime auth nonce: {error}"))
    })?;
    Ok(nonce)
}

fn unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn unix_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn io_error(error: io::Error) -> EditorInspectorServiceError {
    EditorInspectorServiceError::Io(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_proof_changes_with_runtime_nonce() {
        let session = EditorSessionId(7);
        let first = editor_auth_token_proof(b"token", session, b"editor", b"runtime-a");
        let second = editor_auth_token_proof(b"token", session, b"editor", b"runtime-b");

        assert_ne!(first, second);
        assert_eq!(
            first,
            editor_auth_token_proof(b"token", session, b"editor", b"runtime-a")
        );
    }

    #[test]
    fn negotiated_capabilities_require_request_auth_and_runtime_grant() {
        let granted = negotiated_capabilities(
            &[
                EditorCapability::ReadEntities,
                EditorCapability::ExecuteServerCode,
            ],
            &[
                EditorCapability::ReadEntities,
                EditorCapability::ExecuteServerCode,
            ],
            &[EditorCapability::ReadEntities],
        );

        assert_eq!(granted, vec![EditorCapability::ReadEntities]);
    }

    #[test]
    fn post_handshake_validation_uses_negotiated_capabilities() {
        let config = EditorInspectorServiceConfig::local_development(
            EditorTargetKind::Server,
            "fun",
            vec![EditorCapability::ReadEntities],
        );
        config.runtime_state.replace_entities(
            EditorWorldRevision(1),
            EditorSchemaRevision(1),
            Vec::new(),
            Vec::new(),
        );
        let context =
            validation_context_with_capabilities(&config, vec![EditorCapability::ReadEntities]);
        let packet = EditorProtocolPacket::Command {
            packet: EditorCommandPacket {
                header: EditorPacketHeader::new(
                    EditorRequestId(7),
                    EditorTargetKind::Server,
                    PacketSequence(7),
                    Some(EditorWorldRevision(1)),
                    EditorSizeBudget { max_bytes: 4096 },
                ),
                payload: EditorCommandPayload::RuntimeControl {
                    command: EditorRuntimeControlCommand::SimulationResume,
                },
            },
        };

        let error = validate_editor_packet(&packet, &context)
            .expect_err("runtime did not negotiate ControlRuntime");

        assert_eq!(
            error,
            crate::EditorProtocolValidationError::MissingCapability
        );
    }

    #[test]
    fn auth_proof_comparison_rejects_wrong_length_and_wrong_token() {
        let session = EditorSessionId(7);
        let proof = editor_auth_token_proof(b"token", session, b"editor", b"runtime-a");
        let mut wrong_token = editor_auth_token_proof(b"other", session, b"editor", b"runtime-a");
        wrong_token[0] ^= 0xff;
        let short = &proof[..proof.len() - 1];

        assert!(constant_time_bytes_eq(&proof, &proof));
        assert!(!constant_time_bytes_eq(&proof, &wrong_token));
        assert!(!constant_time_bytes_eq(&proof, short));
    }

    #[test]
    fn runtime_control_commands_are_queued_for_runtime_systems() {
        let state = EditorInspectorRuntimeState::default();
        state.queue_runtime_control(EditorRuntimeControlCommand::InputSetOwner {
            owner: crate::EditorInputOwner::Editor,
        });
        state.queue_runtime_control(EditorRuntimeControlCommand::WindowSetEmbedded {
            embedded: true,
        });

        assert_eq!(
            state.drain_runtime_controls(1),
            vec![EditorRuntimeControlCommand::InputSetOwner {
                owner: crate::EditorInputOwner::Editor
            }]
        );
        assert_eq!(
            state.drain_runtime_controls(8),
            vec![EditorRuntimeControlCommand::WindowSetEmbedded { embedded: true }]
        );
        assert!(state.drain_runtime_controls(8).is_empty());
    }

    #[test]
    fn editor_attachment_state_is_reference_counted() {
        let state = EditorInspectorRuntimeState::default();

        state.mark_editor_attached(true);
        state.mark_editor_attached(true);
        assert!(state.editor_attached());

        state.mark_editor_attached(false);
        assert!(state.editor_attached());

        state.mark_editor_attached(false);
        assert!(!state.editor_attached());
    }

    #[test]
    fn runtime_control_queue_drops_oldest_when_bounded() {
        let state = EditorInspectorRuntimeState::default();
        for index in 0..(MAX_PENDING_RUNTIME_CONTROLS + 1) {
            state.queue_runtime_control(EditorRuntimeControlCommand::WindowSetEmbedded {
                embedded: index % 2 == 0,
            });
        }

        let queued = state.drain_runtime_controls(MAX_PENDING_RUNTIME_CONTROLS + 1);

        assert_eq!(queued.len(), MAX_PENDING_RUNTIME_CONTROLS);
    }

    #[test]
    fn inspector_connection_limiter_rejects_over_capacity() {
        let limiter = EditorConnectionLimiter {
            active: Arc::new(AtomicUsize::new(0)),
            max_connections: 1,
        };

        let first = limiter.try_acquire();
        assert!(first.is_some());
        assert!(limiter.try_acquire().is_none());
        drop(first);
        assert!(limiter.try_acquire().is_some());
    }

    #[test]
    fn diagnostics_subscription_replays_since_sequence_and_filters_streams() {
        let state = EditorInspectorRuntimeState::default();
        state.mark_editor_attached(true);
        state.emit_diagnostic(crate::DiagnosticPacket::Event {
            event: crate::DiagnosticEvent {
                target: "fun::server".to_owned(),
                level: crate::DiagnosticLevel::Info,
                timestamp_or_tick: crate::DiagnosticTimestampOrTick::Tick { tick: 1 },
                fields: vec![crate::DiagnosticField::text(
                    "stream",
                    "mutation_transactions",
                )],
                source: crate::DiagnosticSource::static_location(file!(), line!(), module_path!()),
                frame_index: None,
                span_id: None,
            },
        });
        state.emit_diagnostic(crate::DiagnosticPacket::Event {
            event: crate::DiagnosticEvent {
                target: "fun::server".to_owned(),
                level: crate::DiagnosticLevel::Info,
                timestamp_or_tick: crate::DiagnosticTimestampOrTick::Tick { tick: 2 },
                fields: vec![crate::DiagnosticField::text("stream", "stream_chunks")],
                source: crate::DiagnosticSource::static_location(file!(), line!(), module_path!()),
                frame_index: None,
                span_id: None,
            },
        });
        state.emit_diagnostic(crate::DiagnosticPacket::Event {
            event: crate::DiagnosticEvent {
                target: "fun::server".to_owned(),
                level: crate::DiagnosticLevel::Info,
                timestamp_or_tick: crate::DiagnosticTimestampOrTick::Tick { tick: 3 },
                fields: vec![crate::DiagnosticField::text(
                    "stream",
                    "mutation_transactions",
                )],
                source: crate::DiagnosticSource::static_location(file!(), line!(), module_path!()),
                frame_index: None,
                span_id: None,
            },
        });

        let batch = state.drain_diagnostics_for_subscription(
            &EditorDiagnosticSubscription {
                streams: vec![EditorDiagnosticStream::MutationTransactions],
                since_sequence: Some(PacketSequence(1)),
            },
            8,
        );

        assert_eq!(
            batch
                .packets
                .iter()
                .map(|packet| packet.sequence)
                .collect::<Vec<_>>(),
            vec![crate::DiagnosticSequence(3)]
        );
    }

    #[test]
    fn diagnostics_subscription_live_batch_honors_streams() {
        let state = EditorInspectorRuntimeState::default();
        state.mark_editor_attached(true);
        state.emit_diagnostic(crate::DiagnosticPacket::Counter {
            counter: crate::DiagnosticCounter {
                target: "fun::render".to_owned(),
                name: "client_fps".to_owned(),
                value: crate::DiagnosticValue::U64 { value: 60 },
                unit: crate::DiagnosticUnit::Count,
                window: crate::DiagnosticWindow {
                    sample_count: 1,
                    duration_ns: 16_000_000,
                },
            },
        });
        state.emit_diagnostic(crate::DiagnosticPacket::Counter {
            counter: crate::DiagnosticCounter {
                target: "fun::server".to_owned(),
                name: "connected_clients".to_owned(),
                value: crate::DiagnosticValue::U64 { value: 1 },
                unit: crate::DiagnosticUnit::Count,
                window: crate::DiagnosticWindow {
                    sample_count: 1,
                    duration_ns: 16_000_000,
                },
            },
        });

        let batch = state.drain_diagnostics_for_subscription(
            &EditorDiagnosticSubscription {
                streams: vec![EditorDiagnosticStream::ConnectedClients],
                since_sequence: None,
            },
            8,
        );

        assert_eq!(batch.packets.len(), 1);
        assert_eq!(batch.packets[0].sequence, crate::DiagnosticSequence(2));
    }
}
