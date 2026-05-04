use bevy::{app::AppExit, prelude::*};
use fun_warden_client::{
    integrity::verify_current_process_integrity,
    runtime::{
        WARDEN_CLIENT_DIAGNOSTIC_TARGET, WardenBackendChallengeBindingStatus,
        WardenProtectedLoaderVerdict, WardenProtectedRuntimeConfig as SharedProtectedRuntimeConfig,
        WardenProtectedRuntimeStatus as SharedProtectedRuntimeStatus,
        protected_region_status_report, redacted_protected_runtime_diagnostics_json,
    },
};
use fun_warden_core::{ExecutableIntegrityManifest, IntegrityStatus, ProtectedProtectionProfile};
use fun_warden_protocol::{
    ClientAttestationStatus, FUN_WARDEN_CHALLENGE_ID_ENV, FUN_WARDEN_ENABLED_ENV,
    FUN_WARDEN_MODE_ENV, FUN_WARDEN_SESSION_ID_ENV, ProtectedRegionStatusReport, TicketId16,
    WardenPolicyMode,
};
use tracing::{debug, info, warn};

const MAX_WARDEN_SESSION_ID_BYTES: usize = 64;
const MAX_WARDEN_CHALLENGE_ID_BYTES: usize = 80;
const WARDEN_HEARTBEAT_SECONDS: f32 = 5.0;
const WARDEN_INTEGRITY_RECHECK_SECONDS: f32 = 30.0;
const WARDEN_SERVICE_POLICY_POLL_MIN_SECONDS: f32 = 5.0;
const WARDEN_SERVICE_POLICY_POLL_JITTER_SECONDS: f32 = 10.0;
const FUN_WARDEN_SERVICE_DECISION_ENV: &str = "FUN_WARDEN_SERVICE_DECISION";
const FUN_WARDEN_POLICY_EPOCH_ENV: &str = "FUN_WARDEN_POLICY_EPOCH";
const FUN_WARDEN_DEVICE_ATTESTATION_STATUS_ENV: &str = "FUN_WARDEN_DEVICE_ATTESTATION_STATUS";

pub struct ClientWardenPlugin;

impl Plugin for ClientWardenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WardenClientStatus>()
            .init_resource::<WardenProtectedRuntimeStatus>()
            .init_resource::<WardenProtectedServiceReportOutbox>()
            .init_resource::<WardenServiceHeartbeatTimer>()
            .init_resource::<WardenServicePolicyPollTimer>()
            .init_resource::<WardenIntegrityRecheckTimer>()
            .add_systems(Startup, initialize_warden_client)
            .add_systems(
                Update,
                (
                    warden_service_heartbeat,
                    poll_warden_service_policy_update,
                    low_frequency_integrity_recheck,
                    report_protected_status_to_service,
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
    pub protected_runtime: SharedProtectedRuntimeConfig,
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
        let protected_runtime = SharedProtectedRuntimeConfig::from_env()
            .unwrap_or_else(|_| disabled_protected_runtime_config(mode));
        Self {
            enabled,
            session_id,
            challenge_id,
            mode,
            protected_runtime,
        }
    }

    #[must_use]
    pub fn from_pairs<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut enabled = false;
        let mut session_id = None;
        let mut challenge_id = None;
        let mut mode = WardenPolicyMode::Observe;
        let pairs = pairs.into_iter().collect::<Vec<_>>();

        for (key, value) in &pairs {
            match *key {
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
        let protected_runtime = SharedProtectedRuntimeConfig::from_pairs(pairs)
            .unwrap_or_else(|_| disabled_protected_runtime_config(mode));

        Self {
            enabled,
            session_id,
            challenge_id,
            mode,
            protected_runtime,
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
    QuarantineToUntrustedPool,
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
    pub device_attestation_status: ClientAttestationStatus,
    pub finding: WardenClientFinding,
    pub backend_decision: WardenClientBackendDecision,
    pub last_admission_decision: WardenClientBackendDecision,
    pub last_policy_epoch: u64,
    pub blocked_by_warden: bool,
    pub heartbeat_count: u64,
    pub exit_requested: bool,
}

#[derive(Debug, Resource)]
pub struct WardenProtectedRuntimeStatus {
    pub profile: Option<ProtectedProtectionProfile>,
    pub loader_verdict: WardenProtectedLoaderVerdict,
    pub integrity_mesh_status: IntegrityStatus,
    pub last_check_coarse_timestamp_ms: u64,
    pub backend_challenge_binding_status: WardenBackendChallengeBindingStatus,
    pub enforcement_mode: WardenPolicyMode,
}

#[derive(Debug, Resource, Default)]
pub struct WardenProtectedServiceReportOutbox {
    pub last_report: Option<ProtectedRegionStatusReport>,
    pub report_count: u64,
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
            device_attestation_status: ClientAttestationStatus::Unsupported,
            finding: if enabled {
                WardenClientFinding::ServiceConnectionPending
            } else {
                WardenClientFinding::Disabled
            },
            backend_decision: WardenClientBackendDecision::Pending,
            last_admission_decision: WardenClientBackendDecision::Pending,
            last_policy_epoch: 0,
            blocked_by_warden: false,
            heartbeat_count: 0,
            exit_requested: false,
        }
    }
}

impl Default for WardenProtectedRuntimeStatus {
    fn default() -> Self {
        let config = SharedProtectedRuntimeConfig::from_env()
            .unwrap_or_else(|_| disabled_protected_runtime_config(WardenPolicyMode::Observe));
        SharedProtectedRuntimeStatus::from_config(config).into()
    }
}

impl From<SharedProtectedRuntimeStatus> for WardenProtectedRuntimeStatus {
    fn from(status: SharedProtectedRuntimeStatus) -> Self {
        Self {
            profile: status.profile,
            loader_verdict: status.loader_verdict,
            integrity_mesh_status: status.integrity_mesh_status,
            last_check_coarse_timestamp_ms: status.last_check_coarse_timestamp_ms,
            backend_challenge_binding_status: status.backend_challenge_binding_status,
            enforcement_mode: status.enforcement_mode,
        }
    }
}

impl From<&WardenProtectedRuntimeStatus> for SharedProtectedRuntimeStatus {
    fn from(status: &WardenProtectedRuntimeStatus) -> Self {
        Self {
            profile: status.profile,
            loader_verdict: status.loader_verdict,
            integrity_mesh_status: status.integrity_mesh_status,
            last_check_coarse_timestamp_ms: status.last_check_coarse_timestamp_ms,
            backend_challenge_binding_status: status.backend_challenge_binding_status,
            enforcement_mode: status.enforcement_mode,
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
struct WardenServicePolicyPollTimer(Timer);

impl Default for WardenServicePolicyPollTimer {
    fn default() -> Self {
        Self(Timer::from_seconds(
            warden_policy_poll_interval_seconds(coarse_now_ms()),
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
    mut protected_status: ResMut<WardenProtectedRuntimeStatus>,
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
    *protected_status =
        SharedProtectedRuntimeStatus::from_config(status.config.protected_runtime).into();
    status.service_state = WardenClientServiceState::PendingService;
    info!(
        target: WARDEN_CLIENT_DIAGNOSTIC_TARGET,
        mode = ?status.config.mode,
        protected_profile = ?protected_status.profile,
        loader_verdict = ?protected_status.loader_verdict,
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

fn poll_warden_service_policy_update(
    time: Res<Time>,
    mut timer: ResMut<WardenServicePolicyPollTimer>,
    mut status: ResMut<WardenClientStatus>,
) {
    if !status.config.enabled {
        return;
    }
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    let Some(update) = warden_service_policy_update_from_env(status.last_policy_epoch) else {
        return;
    };
    apply_service_policy_update(&mut status, update);
}

fn low_frequency_integrity_recheck(
    time: Res<Time>,
    mut timer: ResMut<WardenIntegrityRecheckTimer>,
    mut status: ResMut<WardenClientStatus>,
    mut protected_status: ResMut<WardenProtectedRuntimeStatus>,
    manifest: Option<Res<VerifiedWardenIntegrityManifest>>,
) {
    if !status.config.enabled {
        return;
    }
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }

    verify_current_process(&mut status, manifest.as_deref());
    protected_status.integrity_mesh_status = status.integrity_status;
    protected_status.last_check_coarse_timestamp_ms = coarse_now_ms();
}

fn report_protected_status_to_service(
    status: Res<WardenClientStatus>,
    protected_status: Res<WardenProtectedRuntimeStatus>,
    mut outbox: ResMut<WardenProtectedServiceReportOutbox>,
) {
    if !status.config.enabled || !protected_status.is_changed() {
        return;
    }
    let Some(ticket_id) = status
        .config
        .session_id
        .as_deref()
        .and_then(ticket_id_from_session_reference)
    else {
        return;
    };
    let shared_status = SharedProtectedRuntimeStatus::from(&*protected_status);
    let region_failure_count = u32::from(
        shared_status.integrity_mesh_status == IntegrityStatus::Failed
            || shared_status.loader_verdict == WardenProtectedLoaderVerdict::Failed,
    );
    let Ok(report) = protected_region_status_report(
        ticket_id,
        status.config.protected_runtime,
        shared_status,
        region_failure_count,
    ) else {
        return;
    };

    outbox.last_report = Some(report);
    outbox.report_count = outbox.report_count.saturating_add(1);
    if let Ok(json) =
        redacted_protected_runtime_diagnostics_json(status.config.protected_runtime, shared_status)
    {
        debug!(
            target: WARDEN_CLIENT_DIAGNOSTIC_TARGET,
            protected_runtime = %json,
            "queued compact Warden protected status report"
        );
    }
}

fn apply_warden_policy_change(
    mut status: ResMut<WardenClientStatus>,
    mut exit: MessageWriter<AppExit>,
) {
    if !status.config.enabled || status.exit_requested {
        return;
    }
    if status.last_admission_decision != WardenClientBackendDecision::DenySession {
        return;
    }

    status.finding = WardenClientFinding::EnforcementDenied;
    status.exit_requested = true;
    status.blocked_by_warden = true;
    warn!(
        target: WARDEN_CLIENT_DIAGNOSTIC_TARGET,
        mode = ?status.config.mode,
        "Warden policy ended this protected session"
    );
    exit.write(AppExit::Success);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WardenServicePolicyUpdate {
    device_attestation_status: ClientAttestationStatus,
    admission_decision: WardenClientBackendDecision,
    policy_epoch: u64,
}

fn apply_service_policy_update(status: &mut WardenClientStatus, update: WardenServicePolicyUpdate) {
    if update.policy_epoch <= status.last_policy_epoch {
        return;
    }
    status.service_state = WardenClientServiceState::Connected;
    status.device_attestation_status = update.device_attestation_status;
    status.backend_decision = update.admission_decision;
    status.last_admission_decision = update.admission_decision;
    status.last_policy_epoch = update.policy_epoch;
    status.blocked_by_warden =
        update.admission_decision == WardenClientBackendDecision::DenySession;
    if update.admission_decision == WardenClientBackendDecision::DenySession {
        status.finding = WardenClientFinding::EnforcementDenied;
    }
    debug!(
        target: WARDEN_CLIENT_DIAGNOSTIC_TARGET,
        policy_epoch = status.last_policy_epoch,
        admission_decision = ?status.last_admission_decision,
        device_attestation_status = ?status.device_attestation_status,
        "applied Warden service policy update"
    );
}

fn warden_service_policy_update_from_env(current_epoch: u64) -> Option<WardenServicePolicyUpdate> {
    let pairs = [
        (
            FUN_WARDEN_SERVICE_DECISION_ENV,
            std::env::var(FUN_WARDEN_SERVICE_DECISION_ENV).ok(),
        ),
        (
            FUN_WARDEN_POLICY_EPOCH_ENV,
            std::env::var(FUN_WARDEN_POLICY_EPOCH_ENV).ok(),
        ),
        (
            FUN_WARDEN_DEVICE_ATTESTATION_STATUS_ENV,
            std::env::var(FUN_WARDEN_DEVICE_ATTESTATION_STATUS_ENV).ok(),
        ),
    ];
    warden_service_policy_update_from_pairs(
        pairs
            .iter()
            .filter_map(|(name, value)| value.as_deref().map(|value| (*name, value))),
        current_epoch,
    )
}

fn warden_service_policy_update_from_pairs<'a>(
    pairs: impl IntoIterator<Item = (&'a str, &'a str)>,
    current_epoch: u64,
) -> Option<WardenServicePolicyUpdate> {
    let mut admission_decision = None;
    let mut policy_epoch = None;
    let mut device_attestation_status = ClientAttestationStatus::Unsupported;
    for (key, value) in pairs {
        match key {
            FUN_WARDEN_SERVICE_DECISION_ENV => {
                admission_decision = parse_service_admission_decision(value);
            }
            FUN_WARDEN_POLICY_EPOCH_ENV => {
                policy_epoch = value.parse::<u64>().ok();
            }
            FUN_WARDEN_DEVICE_ATTESTATION_STATUS_ENV => {
                device_attestation_status = parse_device_attestation_status(value)
                    .unwrap_or(ClientAttestationStatus::Unsupported);
            }
            _ => {}
        }
    }
    let admission_decision = admission_decision?;
    let policy_epoch = policy_epoch.unwrap_or_else(|| current_epoch.saturating_add(1));
    if policy_epoch <= current_epoch {
        return None;
    }
    Some(WardenServicePolicyUpdate {
        device_attestation_status,
        admission_decision,
        policy_epoch,
    })
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

const fn disabled_protected_runtime_config(
    enforcement_mode: WardenPolicyMode,
) -> SharedProtectedRuntimeConfig {
    SharedProtectedRuntimeConfig {
        profile: None,
        protected_bundle_digest: None,
        loader_integrity_status: IntegrityStatus::Unsupported,
        server_keyed_unlock_required: false,
        enforcement_mode,
    }
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

fn parse_service_admission_decision(value: &str) -> Option<WardenClientBackendDecision> {
    if value.eq_ignore_ascii_case("allow") {
        return Some(WardenClientBackendDecision::Allow);
    }
    if value.eq_ignore_ascii_case("observe") || value.eq_ignore_ascii_case("observe_only") {
        return Some(WardenClientBackendDecision::ObserveOnly);
    }
    if value.eq_ignore_ascii_case("deny_matchmaking")
        || value.eq_ignore_ascii_case("deny-matchmaking")
    {
        return Some(WardenClientBackendDecision::DenyMatchmaking);
    }
    if value.eq_ignore_ascii_case("deny_session") || value.eq_ignore_ascii_case("deny-session") {
        return Some(WardenClientBackendDecision::DenySession);
    }
    if value.eq_ignore_ascii_case("quarantine")
        || value.eq_ignore_ascii_case("quarantine_to_untrusted_pool")
        || value.eq_ignore_ascii_case("quarantine-to-untrusted-pool")
    {
        return Some(WardenClientBackendDecision::QuarantineToUntrustedPool);
    }
    None
}

fn parse_device_attestation_status(value: &str) -> Option<ClientAttestationStatus> {
    if value.eq_ignore_ascii_case("passed") {
        return Some(ClientAttestationStatus::Passed);
    }
    if value.eq_ignore_ascii_case("failed") {
        return Some(ClientAttestationStatus::Failed);
    }
    if value.eq_ignore_ascii_case("unsupported") {
        return Some(ClientAttestationStatus::Unsupported);
    }
    None
}

fn ticket_id_from_session_reference(value: &str) -> Option<TicketId16> {
    if value.len() != 32 {
        return None;
    }
    let mut bytes = [0_u8; 16];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let high = hex_nibble(chunk[0])?;
        let low = hex_nibble(chunk[1])?;
        bytes[index] = (high << 4) | low;
    }
    Some(TicketId16(bytes))
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn warden_policy_poll_interval_seconds(coarse_seed_ms: u64) -> f32 {
    let jitter_window_ms = (WARDEN_SERVICE_POLICY_POLL_JITTER_SECONDS * 1000.0) as u64;
    let jitter_millis = (coarse_seed_ms % jitter_window_ms.saturating_add(1)) as f32;
    WARDEN_SERVICE_POLICY_POLL_MIN_SECONDS + jitter_millis / 1000.0
}

fn coarse_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::{
        WardenClientBackendDecision, WardenClientConfig, WardenClientFinding,
        WardenClientServiceState, WardenClientStatus, apply_service_policy_update,
        bounded_env_reference, parse_policy_mode, ticket_id_from_session_reference,
        warden_policy_poll_interval_seconds, warden_service_policy_update_from_pairs,
    };
    use fun_warden_protocol::{
        ClientAttestationStatus, Digest32, FUN_WARDEN_CHALLENGE_ID_ENV, FUN_WARDEN_ENABLED_ENV,
        FUN_WARDEN_MODE_ENV, FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV,
        FUN_WARDEN_PROTECTED_INTEGRITY_STATUS_ENV, FUN_WARDEN_PROTECTED_PROFILE_ENV,
        FUN_WARDEN_PROTECTED_UNLOCK_REQUIRED_ENV, FUN_WARDEN_SESSION_ID_ENV, TicketId16,
    };

    #[test]
    fn warden_env_config_reads_only_non_secret_references() {
        let config = WardenClientConfig::from_pairs([
            (FUN_WARDEN_ENABLED_ENV, "1"),
            (FUN_WARDEN_SESSION_ID_ENV, "session-ref"),
            (FUN_WARDEN_CHALLENGE_ID_ENV, "challenge-ref"),
            (FUN_WARDEN_MODE_ENV, "protect"),
            (FUN_WARDEN_PROTECTED_PROFILE_ENV, "standard"),
            (
                FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV,
                "0707070707070707070707070707070707070707070707070707070707070707",
            ),
            (FUN_WARDEN_PROTECTED_INTEGRITY_STATUS_ENV, "passed"),
            (FUN_WARDEN_PROTECTED_UNLOCK_REQUIRED_ENV, "0"),
            ("FUN_WARDEN_TOKEN", "must-not-be-read"),
            ("FUN_WARDEN_HARDWARE_ID", "must-not-be-read"),
        ]);

        assert!(config.enabled);
        assert_eq!(config.session_id.as_deref(), Some("session-ref"));
        assert_eq!(config.challenge_id.as_deref(), Some("challenge-ref"));
        assert_eq!(config.mode, fun_warden_protocol::WardenPolicyMode::Protect);
        assert_eq!(
            config.protected_runtime.profile,
            Some(fun_warden_core::ProtectedProtectionProfile::Standard)
        );
        assert_eq!(
            config.protected_runtime.protected_bundle_digest,
            Some(Digest32([7; 32]))
        );
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
    fn invalid_protected_runtime_env_fails_closed_for_client_config() {
        let config = WardenClientConfig::from_pairs([
            (FUN_WARDEN_ENABLED_ENV, "1"),
            (FUN_WARDEN_MODE_ENV, "protect"),
            (FUN_WARDEN_PROTECTED_PROFILE_ENV, "ranked"),
            (FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV, "not-a-digest"),
        ]);

        assert!(config.enabled);
        assert_eq!(config.mode, fun_warden_protocol::WardenPolicyMode::Protect);
        assert_eq!(config.protected_runtime.profile, None);
        assert_eq!(config.protected_runtime.protected_bundle_digest, None);
        assert_eq!(
            config.protected_runtime.loader_integrity_status,
            fun_warden_core::IntegrityStatus::Unsupported
        );
        assert!(!config.protected_runtime.server_keyed_unlock_required);
        assert_eq!(
            config.protected_runtime.enforcement_mode,
            fun_warden_protocol::WardenPolicyMode::Protect
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

    #[test]
    fn session_reference_decodes_ticket_id_without_accepting_other_text() {
        assert_eq!(
            ticket_id_from_session_reference("09090909090909090909090909090909"),
            Some(TicketId16([9; 16]))
        );
        assert_eq!(ticket_id_from_session_reference("session-ref"), None);
        assert_eq!(
            ticket_id_from_session_reference("zz090909090909090909090909090909"),
            None
        );
    }

    #[test]
    fn service_policy_update_tracks_quarantine_without_session_exit() {
        let mut status = enabled_status();
        let update = warden_service_policy_update_from_pairs(
            [
                (
                    "FUN_WARDEN_SERVICE_DECISION",
                    "quarantine_to_untrusted_pool",
                ),
                ("FUN_WARDEN_POLICY_EPOCH", "7"),
                ("FUN_WARDEN_DEVICE_ATTESTATION_STATUS", "passed"),
            ],
            0,
        )
        .expect("policy update");

        apply_service_policy_update(&mut status, update);

        assert_eq!(status.service_state, WardenClientServiceState::Connected);
        assert_eq!(
            status.last_admission_decision,
            WardenClientBackendDecision::QuarantineToUntrustedPool
        );
        assert_eq!(
            status.device_attestation_status,
            ClientAttestationStatus::Passed
        );
        assert_eq!(status.last_policy_epoch, 7);
        assert!(!status.blocked_by_warden);
        assert!(!status.exit_requested);
    }

    #[test]
    fn service_policy_update_marks_deny_session_as_warden_blocked() {
        let mut status = enabled_status();
        let update = warden_service_policy_update_from_pairs(
            [
                ("FUN_WARDEN_SERVICE_DECISION", "deny_session"),
                ("FUN_WARDEN_POLICY_EPOCH", "2"),
                ("FUN_WARDEN_DEVICE_ATTESTATION_STATUS", "failed"),
            ],
            0,
        )
        .expect("policy update");

        apply_service_policy_update(&mut status, update);

        assert_eq!(
            status.last_admission_decision,
            WardenClientBackendDecision::DenySession
        );
        assert_eq!(
            status.device_attestation_status,
            ClientAttestationStatus::Failed
        );
        assert!(status.blocked_by_warden);
        assert_eq!(status.finding, WardenClientFinding::EnforcementDenied);
    }

    #[test]
    fn service_policy_poll_interval_is_low_frequency_and_jittered() {
        assert_eq!(warden_policy_poll_interval_seconds(0), 5.0);
        assert_eq!(warden_policy_poll_interval_seconds(10_000), 15.0);
        let interval = warden_policy_poll_interval_seconds(4_321);
        assert!((5.0..=15.0).contains(&interval));
    }

    fn enabled_status() -> WardenClientStatus {
        let mut status = WardenClientStatus::default();
        status.config.enabled = true;
        status.service_state = WardenClientServiceState::PendingService;
        status
    }
}
