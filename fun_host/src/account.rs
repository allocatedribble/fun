use std::{
    sync::{Mutex, OnceLock},
    time::Duration,
};

use reqwest::blocking::Client;
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use zeroize::Zeroize;

#[derive(Debug, Clone)]
struct BackendSession {
    backend_base_url: String,
    session_token: SecretString,
    profile: AccountProfileSummary,
    ticket: Option<BackendAuthTicket>,
    _raw_ticket: Option<SecretString>,
}

static BACKEND_SESSION: OnceLock<Mutex<Option<BackendSession>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandResult<T> {
    pub ok: bool,
    pub value: Option<T>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    pub code: String,
    pub level: EventLevel,
    pub message: String,
    pub target_path: Option<String>,
    pub hosted_instance_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthCapability {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountProfileSummary {
    pub subject: String,
    pub display_name: String,
    pub handle: String,
    pub avatar_url: String,
    pub authorized: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BackendAccountMode {
    Login,
    Register,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendAccountTicketLoginRequest {
    pub email: String,
    pub password: String,
    pub mode: BackendAccountMode,
    pub display_name: Option<String>,
    pub audience: String,
    pub requested_capabilities: Vec<AuthCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendAuthTicketRequest {
    pub audience: String,
    pub requested_capabilities: Vec<AuthCapability>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendAuthTicket {
    pub audience: String,
    pub backend_base_url: String,
    pub subject: String,
    pub profile: AccountProfileSummary,
    pub capabilities: Vec<AuthCapability>,
    pub expires_unix_ms: u64,
    pub ticket_redacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendAuthSessionState {
    pub authenticated: bool,
    pub backend_base_url: String,
    pub profile: Option<AccountProfileSummary>,
    pub ticket: Option<BackendAuthTicket>,
}

#[derive(Deserialize)]
struct RequestEnvelope<T> {
    request: T,
}

#[derive(Deserialize)]
struct BackendAccountTicketPayload {
    email: String,
    password: String,
    mode: Option<BackendAccountMode>,
    display_name: Option<String>,
    audience: String,
    requested_capabilities: Vec<AuthCapability>,
}

impl BackendAccountTicketPayload {
    fn into_request(self, default_mode: BackendAccountMode) -> BackendAccountTicketLoginRequest {
        BackendAccountTicketLoginRequest {
            email: self.email,
            password: self.password,
            mode: self.mode.unwrap_or(default_mode),
            display_name: self.display_name,
            audience: self.audience,
            requested_capabilities: self.requested_capabilities,
        }
    }
}

#[derive(Serialize)]
struct AccountRequestBody<'a> {
    email: &'a str,
    password: &'a str,
    display_name: Option<&'a str>,
    audience: &'a str,
    requested_capabilities: &'a [AuthCapability],
}

#[derive(Deserialize)]
struct AuthResponseBody {
    profile: AccountProfileSummary,
    session_token: Option<String>,
    ticket: Option<BackendAuthTicketBody>,
    verification_required: bool,
}

#[derive(Serialize)]
struct TicketRequestBody<'a> {
    session_token: &'a str,
    audience: &'a str,
    requested_capabilities: &'a [AuthCapability],
}

#[derive(Serialize)]
struct LogoutRequestBody<'a> {
    session_token: &'a str,
}

#[derive(Debug, Clone, Deserialize)]
struct BackendAuthTicketBody {
    ticket_id: String,
    audience: String,
    backend_base_url: String,
    subject: String,
    profile: AccountProfileSummary,
    capabilities: Vec<AuthCapability>,
    expires_unix_ms: u64,
}

impl BackendAuthTicketBody {
    fn summary(&self) -> BackendAuthTicket {
        BackendAuthTicket {
            audience: self.audience.clone(),
            backend_base_url: self.backend_base_url.clone(),
            subject: self.subject.clone(),
            profile: self.profile.clone(),
            capabilities: self.capabilities.clone(),
            expires_unix_ms: self.expires_unix_ms,
            ticket_redacted: true,
        }
    }

    fn into_raw_ticket(self) -> SecretString {
        SecretString::from(self.ticket_id)
    }
}

#[derive(Deserialize)]
struct ErrorResponseBody {
    error: String,
    message: String,
}

pub fn request_backend_login_payload(payload_json: &[u8]) -> CommandResult<BackendAuthTicket> {
    decode_account_payload(payload_json, BackendAccountMode::Login)
        .map(request_backend_account_ticket)
        .unwrap_or_else(payload_error)
}

pub fn request_backend_register_payload(payload_json: &[u8]) -> CommandResult<BackendAuthTicket> {
    decode_account_payload(payload_json, BackendAccountMode::Register)
        .map(request_backend_account_ticket)
        .unwrap_or_else(payload_error)
}

pub fn request_backend_account_ticket_payload(
    payload_json: &[u8],
) -> CommandResult<BackendAuthTicket> {
    decode_account_payload(payload_json, BackendAccountMode::Login)
        .map(request_backend_account_ticket)
        .unwrap_or_else(payload_error)
}

pub fn request_backend_auth_ticket_payload(
    payload_json: &[u8],
) -> CommandResult<BackendAuthTicket> {
    decode_payload::<BackendAuthTicketRequest>(payload_json)
        .map(request_backend_auth_ticket)
        .unwrap_or_else(payload_error)
}

pub fn request_backend_account_ticket(
    request: BackendAccountTicketLoginRequest,
) -> CommandResult<BackendAuthTicket> {
    let BackendAccountTicketLoginRequest {
        email,
        password,
        mode,
        display_name,
        audience,
        requested_capabilities,
    } = request;
    let base_url = backend_base_url();
    let endpoint = match mode {
        BackendAccountMode::Login => "api/account/login",
        BackendAccountMode::Register => "api/account/register",
    };
    let audience = normalized_audience(&audience);
    let password = SecretString::from(password);
    let body = AccountRequestBody {
        email: email.trim(),
        password: password.expose_secret(),
        display_name: display_name.as_deref(),
        audience: &audience,
        requested_capabilities: &requested_capabilities,
    };

    let response = post_json::<_, AuthResponseBody>(&base_url, endpoint, &body);
    drop(password);
    match response {
        Ok(mut auth) => {
            if auth.verification_required || auth.ticket.is_none() || auth.session_token.is_none() {
                if let Some(mut session_token) = auth.session_token.take() {
                    session_token.zeroize();
                }
                return failure(
                    "auth.account.verification_required",
                    "Email verification is required before the backend issues editor tickets.",
                    None,
                    Some("fun-backend".to_owned()),
                );
            }
            let Some(ticket_body) = auth.ticket.take() else {
                return failure(
                    "auth.account.ticket_missing",
                    "Backend did not return a scoped ticket.",
                    None,
                    Some("fun-backend".to_owned()),
                );
            };
            let Some(mut session_token) = auth.session_token.take() else {
                return failure(
                    "auth.account.session_missing",
                    "Backend did not return a session token.",
                    None,
                    Some("fun-backend".to_owned()),
                );
            };
            let session_secret = SecretString::from(std::mem::take(&mut session_token));
            session_token.zeroize();
            let ticket = ticket_body.summary();
            replace_session(BackendSession {
                backend_base_url: ticket.backend_base_url.clone(),
                session_token: session_secret,
                profile: auth.profile,
                ticket: Some(ticket.clone()),
                _raw_ticket: Some(ticket_body.into_raw_ticket()),
            });
            CommandResult {
                ok: true,
                value: Some(ticket),
                diagnostics: Vec::new(),
            }
        }
        Err(diagnostics) => command_error(diagnostics),
    }
}

pub fn request_backend_auth_ticket(
    request: BackendAuthTicketRequest,
) -> CommandResult<BackendAuthTicket> {
    let Some(session) = session_snapshot() else {
        return failure(
            "auth.account.session_required",
            "Login is required before refreshing a backend ticket.",
            None,
            Some("fun-backend".to_owned()),
        );
    };
    let audience = normalized_audience(&request.audience);
    let body = TicketRequestBody {
        session_token: session.session_token.expose_secret(),
        audience: &audience,
        requested_capabilities: &request.requested_capabilities,
    };
    match post_json::<_, BackendAuthTicketBody>(&session.backend_base_url, "api/auth/ticket", &body)
    {
        Ok(ticket_body) => {
            let ticket = ticket_body.summary();
            replace_session(BackendSession {
                backend_base_url: ticket.backend_base_url.clone(),
                session_token: session.session_token,
                profile: ticket.profile.clone(),
                ticket: Some(ticket.clone()),
                _raw_ticket: Some(ticket_body.into_raw_ticket()),
            });
            CommandResult {
                ok: true,
                value: Some(ticket),
                diagnostics: Vec::new(),
            }
        }
        Err(diagnostics) => command_error(diagnostics),
    }
}

pub fn backend_auth_session() -> BackendAuthSessionState {
    session_snapshot().map_or_else(
        || BackendAuthSessionState {
            authenticated: false,
            backend_base_url: backend_base_url(),
            profile: None,
            ticket: None,
        },
        |session| BackendAuthSessionState {
            authenticated: true,
            backend_base_url: session.backend_base_url,
            profile: Some(session.profile),
            ticket: session.ticket,
        },
    )
}

pub fn logout_backend_account() -> CommandResult<BackendAuthSessionState> {
    let Some(session) = session_snapshot() else {
        return CommandResult {
            ok: true,
            value: Some(backend_auth_session()),
            diagnostics: Vec::new(),
        };
    };
    let body = LogoutRequestBody {
        session_token: session.session_token.expose_secret(),
    };
    let result =
        post_json::<_, serde_json::Value>(&session.backend_base_url, "api/auth/logout", &body);
    replace_session_none();
    match result {
        Ok(_) => CommandResult {
            ok: true,
            value: Some(backend_auth_session()),
            diagnostics: Vec::new(),
        },
        Err(diagnostics) => CommandResult {
            ok: false,
            value: Some(backend_auth_session()),
            diagnostics,
        },
    }
}

fn post_json<T, U>(base_url: &str, path: &str, body: &T) -> Result<U, Vec<Diagnostic>>
where
    T: Serialize,
    U: for<'de> Deserialize<'de>,
{
    let client = Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|_| {
            failure::<()>(
                "auth.account.client_failed",
                "Could not initialize the backend HTTP client.",
                None,
                Some("fun-backend".to_owned()),
            )
            .diagnostics
        })?;
    let url = format!("{}/{}", base_url.trim_end_matches('/'), path);
    let response = client.post(url).json(body).send().map_err(|_| {
        failure::<()>(
            "auth.account.request_failed",
            "Backend account request failed.",
            None,
            Some("fun-backend".to_owned()),
        )
        .diagnostics
    })?;
    let status = response.status();
    if status.is_success() {
        return response.json::<U>().map_err(|_| {
            failure::<()>(
                "auth.account.response_invalid",
                "Backend account response was not valid.",
                None,
                Some("fun-backend".to_owned()),
            )
            .diagnostics
        });
    }
    let error = response
        .json::<ErrorResponseBody>()
        .unwrap_or(ErrorResponseBody {
            error: "auth.account.rejected".to_owned(),
            message: "Backend account request was rejected.".to_owned(),
        });
    let diagnostic = failure::<()>(
        error.error.as_str(),
        error.message,
        None,
        Some("fun-backend".to_owned()),
    );
    Err(diagnostic.diagnostics)
}

fn decode_account_payload(
    payload_json: &[u8],
    default_mode: BackendAccountMode,
) -> Result<BackendAccountTicketLoginRequest, serde_json::Error> {
    decode_payload::<BackendAccountTicketPayload>(payload_json)
        .map(|payload| payload.into_request(default_mode))
}

fn decode_payload<T>(payload_json: &[u8]) -> Result<T, serde_json::Error>
where
    T: DeserializeOwned,
{
    serde_json::from_slice::<RequestEnvelope<T>>(payload_json)
        .map(|envelope| envelope.request)
        .or_else(|_| serde_json::from_slice::<T>(payload_json))
}

fn backend_base_url() -> String {
    std::env::var("FUN_BACKEND_BASE_URL")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "http://127.0.0.1:8787".to_owned())
        .trim_end_matches('/')
        .to_owned()
}

fn normalized_audience(audience: &str) -> String {
    let trimmed = audience.trim();
    if trimmed.is_empty() {
        "fun_editor".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn payload_error<T>(_error: serde_json::Error) -> CommandResult<T> {
    failure(
        "host.command.payload_invalid",
        "Host command payload did not match the expected account schema.",
        None,
        Some("fun_host".to_owned()),
    )
}

fn failure<T>(
    code: &str,
    message: impl Into<String>,
    target_path: Option<String>,
    hosted_instance_id: Option<String>,
) -> CommandResult<T> {
    CommandResult {
        ok: false,
        value: None,
        diagnostics: vec![Diagnostic {
            code: code.to_owned(),
            level: EventLevel::Error,
            message: message.into(),
            target_path,
            hosted_instance_id,
        }],
    }
}

fn command_error<T>(diagnostics: Vec<Diagnostic>) -> CommandResult<T> {
    CommandResult {
        ok: false,
        value: None,
        diagnostics,
    }
}

fn session_store() -> &'static Mutex<Option<BackendSession>> {
    BACKEND_SESSION.get_or_init(|| Mutex::new(None))
}

fn session_snapshot() -> Option<BackendSession> {
    session_store()
        .lock()
        .ok()
        .and_then(|session| session.clone())
}

fn replace_session(session: BackendSession) {
    if let Ok(mut cache) = session_store().lock() {
        *cache = Some(session);
    }
}

fn replace_session_none() {
    if let Ok(mut cache) = session_store().lock() {
        *cache = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_ticket() -> BackendAuthTicket {
        BackendAuthTicket {
            audience: String::from("fun_editor"),
            backend_base_url: String::from("http://127.0.0.1:8787"),
            subject: String::from("subject-redacted"),
            profile: AccountProfileSummary {
                subject: String::from("subject-redacted"),
                display_name: String::from("Local Operator"),
                handle: String::from("local"),
                avatar_url: String::from("fun://avatar/local"),
                authorized: true,
            },
            capabilities: vec![AuthCapability::ReadEntities],
            expires_unix_ms: 1,
            ticket_redacted: true,
        }
    }

    #[test]
    fn backend_auth_session_state_serializes_no_raw_tokens() {
        let state = BackendAuthSessionState {
            authenticated: true,
            backend_base_url: String::from("http://127.0.0.1:8787"),
            profile: Some(sample_ticket().profile),
            ticket: Some(sample_ticket()),
        };

        let json = serde_json::to_string(&state).expect("session serializes");

        assert!(!json.contains("ticket_id"));
        assert!(!json.contains("session_token"));
        assert!(json.contains("ticket_redacted"));
    }

    #[test]
    fn account_login_payload_accepts_direct_or_legacy_wrapped_request() {
        let direct = br#"{"email":"operator@example.test","password":"not-secret","audience":"fun_editor","requested_capabilities":["read_entities"]}"#;
        let wrapped = br#"{"request":{"email":"operator@example.test","password":"not-secret","mode":"register","display_name":"Operator","audience":"fun_editor","requested_capabilities":["read_entities"]}}"#;

        let direct = decode_account_payload(direct, BackendAccountMode::Login)
            .expect("direct payload decodes");
        let wrapped = decode_account_payload(wrapped, BackendAccountMode::Login)
            .expect("wrapped payload decodes");

        assert_eq!(direct.mode, BackendAccountMode::Login);
        assert_eq!(wrapped.mode, BackendAccountMode::Register);
        assert_eq!(wrapped.display_name.as_deref(), Some("Operator"));
    }
}
