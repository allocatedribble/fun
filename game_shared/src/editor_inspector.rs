use crate::{
    CURRENT_EDITOR_PROTOCOL_VERSION, EDITOR_PROTOCOL_VERSION, EDITOR_WIRE_ENVELOPE_HEADER_LEN,
    EditorAuth, EditorAuthRequired, EditorBindMode, EditorBuildId, EditorCapability,
    EditorComponentSchema, EditorControlConfig, EditorHandshakePacket, EditorHandshakePayload,
    EditorPacketHeader, EditorProtocolPacket, EditorProtocolValidationContext, EditorRequestId,
    EditorSchemaRevision, EditorSessionId, EditorSizeBudget, EditorTargetKind, EditorWelcome,
    EditorWorldRevision, PacketSequence, decode_editor_packet, editor_wire_envelope_len,
    encode_editor_packet, parse_editor_capability, validate_editor_packet,
};
use ring::hmac;
use std::{
    env, fmt,
    io::{self, Read, Write},
    net::{IpAddr, TcpListener, TcpStream, ToSocketAddrs},
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
        let listener_auth = auth.clone();
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
                        .name("fun-editor-inspector-connection".to_string())
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
                    "first packet must be EditorHello".to_string(),
                ));
            }
        },
        _ => {
            return Err(EditorInspectorServiceError::Protocol(
                "first packet must be a handshake packet".to_string(),
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
                    "expected EditorAuth after AuthRequired".to_string(),
                ));
            }
        },
        _ => {
            return Err(EditorInspectorServiceError::Protocol(
                "auth packet must be a handshake packet".to_string(),
            ));
        }
    };
    validate_auth_packet(&editor_auth, &auth, &hello.nonce, &runtime_nonce)?;

    let granted_capabilities = negotiated_capabilities(
        &hello.requested_capabilities,
        &auth.allowed_capabilities,
        &config.granted_capabilities,
    );
    let welcome = EditorProtocolPacket::Handshake {
        packet: EditorHandshakePacket {
            header: EditorPacketHeader::new(
                EditorRequestId(2),
                config.control.target_kind,
                PacketSequence(2),
                Some(config.world_revision),
                EditorSizeBudget {
                    max_bytes: 16 * 1024,
                },
            ),
            payload: EditorHandshakePayload::Welcome {
                welcome: EditorWelcome {
                    granted_capabilities,
                    target_build_id: config.target_build_id,
                    world_revision: config.world_revision,
                    tick_rate_hz: config.tick_rate_hz,
                    schema_revision: config.schema_revision,
                    diagnostic_schema_revision: config.diagnostic_schema_revision,
                },
            },
        },
    };
    write_editor_protocol_packet(&mut stream, &welcome)
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
    let mut context = EditorProtocolValidationContext {
        protocol_version: CURRENT_EDITOR_PROTOCOL_VERSION,
        granted_capabilities: auth.allowed_capabilities.clone(),
        world_revision: config.world_revision,
        max_payload_bytes: 256 * 1024,
        component_schemas: config.component_schemas.clone(),
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
