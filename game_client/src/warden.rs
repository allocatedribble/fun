use bevy::{app::AppExit, prelude::*};
use fun_warden_client::integrity::verify_current_process_integrity;
use fun_warden_core::{ExecutableIntegrityManifest, IntegrityStatus};
use fun_warden_protocol::{
    FUN_WARDEN_CHALLENGE_ID_ENV, FUN_WARDEN_ENABLED_ENV, FUN_WARDEN_MODE_ENV,
    FUN_WARDEN_SESSION_ID_ENV, WardenPolicyMode,
};
use tracing::{debug, info, warn};

pub const WARDEN_CLIENT_DIAGNOSTIC_TARGET: &str = "fun::warden::client";

const MAX_WARDEN_SESSION_ID_BYTES: usize = 64;
const MAX_WARDEN_CHALLENGE_ID_BYTES: usize = 80;
const WARDEN_HEARTBEAT_SECONDS: f32 = 5.0;
const WARDEN_INTEGRITY_RECHECK_SECONDS: f32 = 30.0;

pub struct ClientWardenPlugin;

impl Plugin for ClientWardenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WardenClientStatus>()
            .init_resource::<WardenServiceHeartbeatTimer>()
            .init_resource::<WardenIntegrityRecheckTimer>()
            .add_systems(Startup, initialize_warden_client)
            .add_systems(
                Update,
                (
                    warden_service_heartbeat,
                    low_frequency_integrity_recheck,
                    apply_warden_policy_change,
                ),
            );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardenClientConfig {
    pub enabled: bool,
    pub session_id: Option<String>,
    pub challenge_id: Option<String>,
    pub mode: WardenPolicyMode,
}

impl WardenClientConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let enabled =
            std::env::var(FUN_WARDEN_ENABLED_ENV).is_ok_and(|value| env_flag_value(&value));
        let session_id = std::env::var(FUN_WARDEN_SESSION_ID_ENV)
            .ok()
            .and_then(|value| bounded_env_reference(&value, MAX_WARDEN_SESSION_ID_BYTES));
        let challenge_id = std::env::var(FUN_WARDEN_CHALLENGE_ID_ENV)
            .ok()
            .and_then(|value| bounded_env_reference(&value, MAX_WARDEN_CHALLENGE_ID_BYTES));
        let mode = std::env::var(FUN_WARDEN_MODE_ENV)
            .ok()
            .and_then(|value| parse_policy_mode(&value))
            .unwrap_or(WardenPolicyMode::Observe);
        Self {
            enabled,
            session_id,
            challenge_id,
            mode,
        }
    }

    #[must_use]
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut enabled = false;
        let mut session_id = None;
        let mut challenge_id = None;
        let mut mode = WardenPolicyMode::Observe;

        for (key, value) in pairs {
            match key {
                FUN_WARDEN_ENABLED_ENV => enabled = env_flag_value(value),
                FUN_WARDEN_SESSION_ID_ENV => {
                    session_id = bounded_env_reference(value, MAX_WARDEN_SESSION_ID_BYTES);
                }
                FUN_WARDEN_CHALLENGE_ID_ENV => {
                    challenge_id = bounded_env_reference(value, MAX_WARDEN_CHALLENGE_ID_BYTES);
                }
                FUN_WARDEN_MODE_ENV => {
                    mode = parse_policy_mode(value).unwrap_or(WardenPolicyMode::Observe);
                }
                _ => {}
            }
        }

        Self {
            enabled,
            session_id,
            challenge_id,
            mode,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenClientServiceState {
    Disabled,
    PendingService,
    Connected,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenClientBackendDecision {
    Pending,
    Allow,
    ObserveOnly,
    DenyMatchmaking,
    DenySession,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenClientFinding {
    Disabled,
    ManifestReferenceUnavailable,
    CurrentProcessIntegrityPassed,
    CurrentProcessIntegrityFailed,
    CurrentProcessIntegrityUnsupported,
    ServiceConnectionPending,
    EnforcementDenied,
}

#[derive(Debug, Resource)]
pub struct VerifiedWardenIntegrityManifest(pub ExecutableIntegrityManifest);

#[derive(Debug, Resource)]
pub struct WardenClientStatus {
    pub config: WardenClientConfig,
    pub service_state: WardenClientServiceState,
    pub integrity_status: IntegrityStatus,
    pub finding: WardenClientFinding,
    pub backend_decision: WardenClientBackendDecision,
    pub heartbeat_count: u64,
    pub exit_requested: bool,
}

impl Default for WardenClientStatus {
    fn default() -> Self {
        let config = WardenClientConfig::from_env();
        let enabled = config.enabled;
        Self {
            config,
            service_state: if enabled {
                WardenClientServiceState::PendingService
            } else {
                WardenClientServiceState::Disabled
            },
            integrity_status: IntegrityStatus::Unsupported,
            finding: if enabled {
                WardenClientFinding::ServiceConnectionPending
            } else {
                WardenClientFinding::Disabled
            },
            backend_decision: WardenClientBackendDecision::Pending,
            heartbeat_count: 0,
            exit_requested: false,
        }
    }
}

#[derive(Debug, Resource)]
struct WardenServiceHeartbeatTimer(Timer);

impl Default for WardenServiceHeartbeatTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            WARDEN_HEARTBEAT_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

#[derive(Debug, Resource)]
struct WardenIntegrityRecheckTimer(Timer);

impl Default for WardenIntegrityRecheckTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            WARDEN_INTEGRITY_RECHECK_SECONDS,
            TimerMode::Repeating,
        ))
    }
}

fn initialize_warden_client(
    mut status: ResMut<WardenClientStatus>,
    manifest: Option<Res<VerifiedWardenIntegrityManifest>>,
) {
    if !status.config.enabled {
        debug!(
            target: WARDEN_CLIENT_DIAGNOSTIC_TARGET,
            "Warden client integration disabled"
        );
        return;
    }

    verify_current_process(&mut status, manifest.as_deref());
    status.service_state = WardenClientServiceState::PendingService;
    info!(
        target: WARDEN_CLIENT_DIAGNOSTIC_TARGET,
        mode = ?status.config.mode,
        has_session_id = status.config.session_id.is_some(),
        has_challenge_id = status.config.challenge_id.is_some(),
        integrity_status = ?status.integrity_status,
        "initialized Warden client status"
    );
}

fn warden_service_heartbeat(
    time: Res<Time>,
    mut timer: ResMut<WardenServiceHeartbeatTimer>,
    mut status: ResMut<WardenClientStatus>,
) {
    if !status.config.enabled {
        return;
    }
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    status.heartbeat_count = status.heartbeat_count.saturating_add(1);
    debug!(
        target: WARDEN_CLIENT_DIAGNOSTIC_TARGET,
        heartbeat_count = status.heartbeat_count,
        service_state = ?status.service_state,
        "Warden service heartbeat tick"
    );
}

fn low_frequency_integrity_recheck(
    time: Res<Time>,
    mut timer: ResMut<WardenIntegrityRecheckTimer>,
    mut status: ResMut<WardenClientStatus>,
    manifest: Option<Res<VerifiedWardenIntegrityManifest>>,
) {
    if !status.config.enabled {
        return;
    }
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    verify_current_process(&mut status, manifest.as_deref());
}

fn apply_warden_policy_change(
    mut status: ResMut<WardenClientStatus>,
    mut exit: MessageWriter<AppExit>,
) {
    if !status.config.enabled || status.exit_requested {
        return;
    }
    if status.backend_decision != WardenClientBackendDecision::DenySession {
        return;
    }

    status.finding = WardenClientFinding::EnforcementDenied;
    status.exit_requested = true;
    warn!(
        target: WARDEN_CLIENT_DIAGNOSTIC_TARGET,
        mode = ?status.config.mode,
        "Warden policy ended this protected session"
    );
    exit.write(AppExit::Success);
}

fn verify_current_process(
    status: &mut WardenClientStatus,
    manifest: Option<&VerifiedWardenIntegrityManifest>,
) {
    let Some(manifest) = manifest else {
        status.integrity_status = IntegrityStatus::Unsupported;
        status.finding = WardenClientFinding::ManifestReferenceUnavailable;
        return;
    };

    match verify_current_process_integrity(&manifest.0) {
        Ok(verdict) => {
            status.integrity_status = verdict.status;
            status.finding = match verdict.status {
                IntegrityStatus::Passed => WardenClientFinding::CurrentProcessIntegrityPassed,
                IntegrityStatus::Failed => WardenClientFinding::CurrentProcessIntegrityFailed,
                IntegrityStatus::Unsupported => {
                    WardenClientFinding::CurrentProcessIntegrityUnsupported
                }
            };
        }
        Err(_) => {
            status.integrity_status = IntegrityStatus::Failed;
            status.finding = WardenClientFinding::CurrentProcessIntegrityFailed;
        }
    }
}

fn env_flag_value(value: &str) -> bool {
    value.eq_ignore_ascii_case("1")
        || value.eq_ignore_ascii_case("true")
        || value.eq_ignore_ascii_case("yes")
        || value.eq_ignore_ascii_case("on")
}

fn bounded_env_reference(value: &str, max_len: usize) -> Option<String> {
    if value.is_empty() || value.len() > max_len {
        return None;
    }
    if value
        .bytes()
        .any(|byte| !byte.is_ascii() || byte.is_ascii_control())
    {
        return None;
    }
    Some(String::from(value))
}

fn parse_policy_mode(value: &str) -> Option<WardenPolicyMode> {
    if value.eq_ignore_ascii_case("observe") {
        return Some(WardenPolicyMode::Observe);
    }
    if value.eq_ignore_ascii_case("protect") {
        return Some(WardenPolicyMode::Protect);
    }
    if value.eq_ignore_ascii_case("enforce_candidate")
        || value.eq_ignore_ascii_case("enforce-candidate")
    {
        return Some(WardenPolicyMode::EnforceCandidate);
    }
    if value.eq_ignore_ascii_case("enforce") {
        return Some(WardenPolicyMode::Enforce);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{WardenClientConfig, bounded_env_reference, parse_policy_mode};
    use fun_warden_protocol::{
        FUN_WARDEN_CHALLENGE_ID_ENV, FUN_WARDEN_ENABLED_ENV, FUN_WARDEN_MODE_ENV,
        FUN_WARDEN_SESSION_ID_ENV,
    };

    #[test]
    fn warden_env_config_reads_only_non_secret_references() {
        let config = WardenClientConfig::from_pairs([
            (FUN_WARDEN_ENABLED_ENV, "1"),
            (FUN_WARDEN_SESSION_ID_ENV, "session-ref"),
            (FUN_WARDEN_CHALLENGE_ID_ENV, "challenge-ref"),
            (FUN_WARDEN_MODE_ENV, "protect"),
            ("FUN_WARDEN_TOKEN", "must-not-be-read"),
        ]);

        assert!(config.enabled);
        assert_eq!(config.session_id.as_deref(), Some("session-ref"));
        assert_eq!(config.challenge_id.as_deref(), Some("challenge-ref"));
        assert_eq!(config.mode, fun_warden_protocol::WardenPolicyMode::Protect);
    }

    #[test]
    fn warden_env_rejects_control_text_and_oversized_references() {
        assert_eq!(bounded_env_reference("bad\n", 16), None);
        assert_eq!(bounded_env_reference("0123456789", 4), None);
        assert_eq!(
            bounded_env_reference("opaque", 16),
            Some(String::from("opaque"))
        );
    }

    #[test]
    fn warden_policy_mode_parser_defaults_unknown_to_none() {
        assert_eq!(
            parse_policy_mode("enforce-candidate"),
            Some(fun_warden_protocol::WardenPolicyMode::EnforceCandidate)
        );
        assert_eq!(parse_policy_mode("unknown"), None);
    }
}
