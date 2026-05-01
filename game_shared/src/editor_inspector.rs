use crate::{
    CURRENT_EDITOR_PROTOCOL_VERSION, EDITOR_PROTOCOL_VERSION, EDITOR_WIRE_ENVELOPE_HEADER_LEN,
    EditorAuth, EditorAuthRequired, EditorBindMode, EditorBuildId, EditorCapability,
    EditorCommandPacket, EditorCommandPayload, EditorComponentSchema, EditorControlConfig,
    EditorDiagnosticBatch, EditorDiagnosticEvent, EditorDiagnosticPacket, EditorEntityPage,
    EditorEntityQuery, EditorEntityRow, EditorEventPacket, EditorEventPayload,
    EditorHandshakePacket, EditorHandshakePayload, EditorMutationAck, EditorMutationStatus,
    EditorMutationTransaction, EditorPacketHeader, EditorPageCursor, EditorProtocolPacket,
    EditorProtocolValidationContext, EditorRequestId, EditorSchemaRevision, EditorSessionId,
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
    sync::{Arc, Mutex},
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
const EDITOR_MUTATION_RESULT_TIMEOUT: Duration = Duration::from_secs(2);

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
    pending_mutations: VecDeque<EditorMutationTransaction>,
    mutation_results: VecDeque<EditorInspectorMutationResult>,
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
            pending_mutations: VecDeque::new(),
            mutation_results: VecDeque::new(),
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
            snapshot.editor_attached = attached;
            snapshot.diagnostics = if attached {
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

    pub fn queue_mutation(&self, transaction: EditorMutationTransaction) {
        if let Ok(mut snapshot) = self.inner.lock() {
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
    let mut callback_started = false;
    if let Some(control_addr) = control_addr {
        let callback_config = config.clone();
        let callback_auth = auth.clone();
        thread::Builder::new()
            .name(format!(
                "fun-editor-{:?}-callback",
                config.control.target_kind.runtime_class()
            ))
            .spawn(move || {
                if let Ok(stream) = TcpStream::connect(control_addr) {
                    let _ =
                        handle_runtime_editor_connection(stream, callback_config, callback_auth);
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
        thread::Builder::new()
            .name(format!(
                "fun-editor-{:?}-inspector",
                config.control.target_kind.runtime_class()
            ))
            .spawn(move || {
                for stream in listener.incoming().flatten() {
                    let config = listener_config.clone();
                    let auth = listener_auth.clone();
                    let _ = thread::Builder::new()
                        .name("fun-editor-inspector-connection".to_owned())
                        .spawn(move || {
                            let _ = handle_runtime_editor_connection(stream, config, auth);
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

fn handle_runtime_editor_connection(
    mut stream: TcpStream,
    config: EditorInspectorServiceConfig,
    auth: RuntimeEditorAuth,
) -> Result<(), EditorInspectorServiceError> {
    stream
        .set_read_timeout(Some(Duration::from_secs(8)))
        .map_err(io_error)?;
    stream
        .set_write_timeout(Some(Duration::from_secs(8)))
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
                    granted_capabilities,
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
    let result = serve_authenticated_editor_connection(&mut stream, &config, &auth);
    config.runtime_state.mark_editor_attached(false);
    result
}

fn serve_authenticated_editor_connection(
    stream: &mut TcpStream,
    config: &EditorInspectorServiceConfig,
    auth: &RuntimeEditorAuth,
) -> Result<(), EditorInspectorServiceError> {
    stream.set_read_timeout(None).map_err(io_error)?;
    loop {
        let packet = match read_editor_protocol_packet(stream) {
            Ok(packet) => packet,
            Err(EditorInspectorServiceError::Io(_)) => return Ok(()),
            Err(error) => return Err(error),
        };
        let context = validation_context(config, auth);
        validate_editor_packet(&packet, &context)
            .map_err(|error| EditorInspectorServiceError::Protocol(format!("{error:?}")))?;

        let EditorProtocolPacket::Command { packet } = packet else {
            continue;
        };
        handle_editor_command_packet(stream, config, packet)?;
    }
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
        EditorCommandPayload::SubscribeDiagnostics { .. } => {
            let batch = config.runtime_state.drain_diagnostics(256);
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
        EditorCommandPayload::Exec { .. } | EditorCommandPayload::Persist { .. } => Ok(()),
    }
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
    if packet.token_proof != expected_proof {
        return Err(EditorInspectorServiceError::InvalidTokenProof);
    }

    let expected_response = editor_auth_nonce_response(
        auth.token.as_bytes(),
        auth.session_id,
        editor_nonce,
        runtime_nonce,
    );
    if packet.nonce_response != expected_response {
        return Err(EditorInspectorServiceError::InvalidNonceResponse);
    }
    Ok(())
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
    let metadata = config.runtime_state.metadata();
    let mut context = EditorProtocolValidationContext {
        protocol_version: CURRENT_EDITOR_PROTOCOL_VERSION,
        granted_capabilities: auth.allowed_capabilities.clone(),
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
}
