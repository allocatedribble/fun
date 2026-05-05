use std::collections::VecDeque;

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
use fun_warden_core::{
    ExecutableIntegrityManifest, IntegrityStatus, ProtectedProtectionProfile, ProtectionLevel,
};
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
const WARDEN_HANDLER_ISLAND_PEER_CHECKS_PER_FRAME: u8 = 2;
const WARDEN_VM_UNLOCK_CHECKS_PER_FRAME: u8 = 2;
const MAX_WARDEN_COMPACT_EVIDENCE_RECORDS: usize = 16;
const MAX_WARDEN_EVIDENCE_FLUSH_PER_FRAME: u8 = 4;
const FUN_WARDEN_SERVICE_DECISION_ENV: &str = "FUN_WARDEN_SERVICE_DECISION";
const FUN_WARDEN_POLICY_EPOCH_ENV: &str = "FUN_WARDEN_POLICY_EPOCH";
const FUN_WARDEN_DEVICE_ATTESTATION_STATUS_ENV: &str = "FUN_WARDEN_DEVICE_ATTESTATION_STATUS";
const FUN_WARDEN_LEGACY_MODE_COMPAT_ENV: &str = "FUN_WARDEN_LEGACY_MODE_COMPAT";
const FUN_WARDEN_VM_UNLOCK_MATERIAL_READY_ENV: &str = "FUN_WARDEN_VM_UNLOCK_MATERIAL_READY";
const FUN_WARDEN_HARDWARE_RUN_PROOF_READY_ENV: &str = "FUN_WARDEN_HARDWARE_RUN_PROOF_READY";
const FUN_WARDEN_DEVICE_VARIANT_TICKET_READY_ENV: &str = "FUN_WARDEN_DEVICE_VARIANT_TICKET_READY";
const FUN_WARDEN_SEALED_PROGRAM_AUTHENTICATION_STATUS_ENV: &str =
    "FUN_WARDEN_SEALED_PROGRAM_AUTHENTICATION_STATUS";
const WARDEN_RUNTIME_ENV_KEYS: [&str; 13] = [
    FUN_WARDEN_ENABLED_ENV,
    FUN_WARDEN_SESSION_ID_ENV,
    FUN_WARDEN_CHALLENGE_ID_ENV,
    FUN_WARDEN_MODE_ENV,
    FUN_WARDEN_LEGACY_MODE_COMPAT_ENV,
    FUN_WARDEN_VM_UNLOCK_MATERIAL_READY_ENV,
    FUN_WARDEN_HARDWARE_RUN_PROOF_READY_ENV,
    FUN_WARDEN_DEVICE_VARIANT_TICKET_READY_ENV,
    FUN_WARDEN_SEALED_PROGRAM_AUTHENTICATION_STATUS_ENV,
    fun_warden_protocol::FUN_WARDEN_PROTECTED_PROFILE_ENV,
    fun_warden_protocol::FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV,
    fun_warden_protocol::FUN_WARDEN_PROTECTED_INTEGRITY_STATUS_ENV,
    fun_warden_protocol::FUN_WARDEN_PROTECTED_UNLOCK_REQUIRED_ENV,
];

pub struct ClientWardenPlugin;

impl Plugin for ClientWardenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WardenClientStatus>()
            .init_resource::<WardenProtectedRuntimeStatus>()
            .init_resource::<WardenProtectedServiceReportOutbox>()
            .init_resource::<WardenHandlerIslandPeerCheckState>()
            .init_resource::<WardenVmPackageReadiness>()
            .init_resource::<WardenVmProgramUnlockState>()
            .init_resource::<WardenProtectedCallEvidenceBuffer>()
            .init_resource::<WardenVmUnlockDiagnosticBuffer>()
            .init_resource::<WardenRedactedEvidenceFlushState>()
            .init_resource::<WardenRedactedUnlockDiagnosticFlushState>()
            .init_resource::<WardenServiceHeartbeatTimer>()
            .init_resource::<WardenServicePolicyPollTimer>()
            .init_resource::<WardenIntegrityRecheckTimer>()
            .add_systems(Startup, initialize_warden_client)
            .add_systems(
                PreUpdate,
                (
                    bounded_handler_island_peer_checks,
                    check_warden_vm_package_readiness,
                    verify_warden_vm_unlock_material_availability,
                    run_bounded_warden_vm_unlock_checks,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    warden_service_heartbeat,
                    poll_warden_service_policy_update,
                    low_frequency_integrity_recheck,
                    report_protected_status_to_service,
                    apply_warden_policy_change,
                ),
            )
            .add_systems(PostUpdate, collect_compact_protected_call_evidence)
            .add_systems(PostUpdate, collect_warden_unlock_denial_evidence)
            .add_systems(
                Last,
                (
                    flush_redacted_warden_evidence_within_budget,
                    flush_redacted_warden_unlock_diagnostics_within_budget,
                ),
            );
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WardenClientConfig {
    pub enabled: bool,
    pub session_id: Option<String>,
    pub challenge_id: Option<String>,
    pub protection_mode: ProtectionLevel,
    pub mode_parse_fallback: Option<WardenModeParseFallback>,
    pub protected_runtime: SharedProtectedRuntimeConfig,
    pub vm_unlock_material_ready: bool,
    pub hardware_run_proof_ready: bool,
    pub device_variant_ticket_ready: bool,
    pub sealed_program_authentication_failed: bool,
}

impl WardenClientConfig {
    #[must_use]
    pub fn from_env() -> Self {
        let pairs = WARDEN_RUNTIME_ENV_KEYS
            .iter()
            .filter_map(|key| std::env::var(key).ok().map(|value| (*key, value)))
            .collect::<Vec<_>>();
        Self::from_pairs(pairs)
    }

    #[must_use]
    pub fn from_pairs<K, V>(pairs: impl IntoIterator<Item = (K, V)>) -> Self
    where
        K: AsRef<str>,
        V: AsRef<str>,
    {
        let pairs = pairs.into_iter().collect::<Vec<_>>();
        let mut enabled_flag = false;
        let mut session_id = None;
        let mut challenge_id = None;
        let mut mode_value = None;
        let mut legacy_mode_compat = false;
        let mut vm_unlock_material_ready = false;
        let mut hardware_run_proof_ready = false;
        let mut device_variant_ticket_ready = false;
        let mut sealed_program_authentication_failed = false;

        for (key, value) in &pairs {
            match key.as_ref() {
                FUN_WARDEN_ENABLED_ENV => enabled_flag = env_flag_value(value.as_ref()),
                FUN_WARDEN_SESSION_ID_ENV => {
                    session_id = bounded_env_reference(value.as_ref(), MAX_WARDEN_SESSION_ID_BYTES);
                }
                FUN_WARDEN_CHALLENGE_ID_ENV => {
                    challenge_id =
                        bounded_env_reference(value.as_ref(), MAX_WARDEN_CHALLENGE_ID_BYTES);
                }
                FUN_WARDEN_MODE_ENV => {
                    mode_value = Some(value.as_ref());
                }
                FUN_WARDEN_LEGACY_MODE_COMPAT_ENV => {
                    legacy_mode_compat = env_flag_value(value.as_ref());
                }
                FUN_WARDEN_VM_UNLOCK_MATERIAL_READY_ENV => {
                    vm_unlock_material_ready = env_flag_value(value.as_ref());
                }
                FUN_WARDEN_HARDWARE_RUN_PROOF_READY_ENV => {
                    hardware_run_proof_ready = env_flag_value(value.as_ref());
                }
                FUN_WARDEN_DEVICE_VARIANT_TICKET_READY_ENV => {
                    device_variant_ticket_ready = env_flag_value(value.as_ref());
                }
                FUN_WARDEN_SEALED_PROGRAM_AUTHENTICATION_STATUS_ENV => {
                    sealed_program_authentication_failed =
                        sealed_program_authentication_status_failed(value.as_ref());
                }
                _ => {}
            }
        }
        let parsed_mode = parse_warden_protection_mode(
            mode_value,
            enabled_flag,
            legacy_mode_compat && legacy_mode_compat_allowed(),
        );
        let enabled = parsed_mode.protection_mode != ProtectionLevel::None;
        let internal_policy_mode = parsed_mode.protection_mode.internal_policy_mode();
        let runtime_pairs = normalized_protected_runtime_pairs(&pairs, parsed_mode.protection_mode);
        let protected_runtime = SharedProtectedRuntimeConfig::from_pairs(runtime_pairs)
            .unwrap_or_else(|_| disabled_protected_runtime_config(internal_policy_mode));

        Self {
            enabled,
            session_id,
            challenge_id,
            protection_mode: parsed_mode.protection_mode,
            mode_parse_fallback: parsed_mode.fallback,
            protected_runtime,
            vm_unlock_material_ready,
            hardware_run_proof_ready,
            device_variant_ticket_ready,
            sealed_program_authentication_failed,
        }
    }

    #[must_use]
    pub const fn internal_policy_mode(&self) -> WardenPolicyMode {
        self.protection_mode.internal_policy_mode()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenModeParseFallback {
    MissingModeEnabledAsHidden,
    MalformedModeHidden,
    LegacyModeCompat,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenVmPackageReadinessState {
    Disabled,
    ObservationOnly,
    Ready,
    MissingProtectedBundle,
    RuntimeUnavailable,
    UnlockMaterialMissing,
    HardwareProofMissing,
    TicketMissing,
    SealedProgramAuthenticationFailed,
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

#[derive(Debug, Resource, Default)]
pub struct WardenHandlerIslandPeerCheckState {
    pub total_checks: u64,
    pub last_frame_budget: u8,
    pub last_protection_mode: Option<ProtectionLevel>,
}

#[derive(Debug, Resource)]
pub struct WardenVmPackageReadiness {
    pub state: WardenVmPackageReadinessState,
    pub check_count: u64,
    pub last_protection_mode: ProtectionLevel,
}

impl Default for WardenVmPackageReadiness {
    fn default() -> Self {
        Self {
            state: WardenVmPackageReadinessState::Disabled,
            check_count: 0,
            last_protection_mode: ProtectionLevel::None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenVmUnlockFailureClass {
    UnlockMaterialMissing,
    HardwareProofMissing,
    TicketMissing,
    SealedProgramAuthenticationFailed,
}

#[derive(Debug, Resource, Default)]
pub struct WardenVmProgramUnlockState {
    pub unlocked_function_count: u16,
    pub denied_function_count: u16,
    pub last_unlock_failure: Option<WardenVmUnlockFailureClass>,
    pub last_policy_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WardenCompactEvidenceReason {
    StartupSnapshot,
    IntegrityChanged,
    BackendDecisionChanged,
    VmPackageReadinessChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WardenCompactProtectedCallEvidence {
    pub sequence: u64,
    pub reason: WardenCompactEvidenceReason,
    pub protection_mode: ProtectionLevel,
    pub integrity_status: IntegrityStatus,
    pub loader_verdict: WardenProtectedLoaderVerdict,
    pub backend_decision: WardenClientBackendDecision,
    pub vm_package_readiness: WardenVmPackageReadinessState,
}

#[derive(Debug, Resource)]
pub struct WardenProtectedCallEvidenceBuffer {
    pub records: VecDeque<WardenCompactProtectedCallEvidence>,
    pub next_sequence: u64,
    last_snapshot: Option<WardenCompactProtectedCallEvidenceSnapshot>,
}

impl Default for WardenProtectedCallEvidenceBuffer {
    fn default() -> Self {
        Self {
            records: VecDeque::with_capacity(MAX_WARDEN_COMPACT_EVIDENCE_RECORDS),
            next_sequence: 0,
            last_snapshot: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WardenCompactUnlockDenialEvidence {
    pub sequence: u64,
    pub protection_mode: ProtectionLevel,
    pub readiness: WardenVmPackageReadinessState,
    pub failure: WardenVmUnlockFailureClass,
    pub policy_epoch: u64,
}

#[derive(Debug, Resource)]
pub struct WardenVmUnlockDiagnosticBuffer {
    pub records: VecDeque<WardenCompactUnlockDenialEvidence>,
    pub next_sequence: u64,
    last_snapshot: Option<WardenUnlockDiagnosticSnapshot>,
}

impl Default for WardenVmUnlockDiagnosticBuffer {
    fn default() -> Self {
        Self {
            records: VecDeque::with_capacity(MAX_WARDEN_COMPACT_EVIDENCE_RECORDS),
            next_sequence: 0,
            last_snapshot: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WardenUnlockDiagnosticSnapshot {
    protection_mode: ProtectionLevel,
    readiness: WardenVmPackageReadinessState,
    failure: WardenVmUnlockFailureClass,
    policy_epoch: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WardenCompactProtectedCallEvidenceSnapshot {
    protection_mode: ProtectionLevel,
    integrity_status: IntegrityStatus,
    loader_verdict: WardenProtectedLoaderVerdict,
    backend_decision: WardenClientBackendDecision,
    vm_package_readiness: WardenVmPackageReadinessState,
}

#[derive(Debug, Resource, Default)]
pub struct WardenRedactedEvidenceFlushState {
    pub total_flushed: u64,
    pub last_flush_count: u8,
    pub deferred_due_to_budget: u64,
}

#[derive(Debug, Resource, Default)]
pub struct WardenRedactedUnlockDiagnosticFlushState {
    pub total_flushed: u64,
    pub last_flush_count: u8,
    pub deferred_due_to_budget: u64,
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
        let config = SharedProtectedRuntimeConfig::from_env().unwrap_or_else(|_| {
            disabled_protected_runtime_config(ProtectionLevel::None.internal_policy_mode())
        });
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
        protection_mode = ?status.config.protection_mode,
        mode_parse_fallback = ?status.config.mode_parse_fallback,
        protected_profile = ?protected_status.profile,
        loader_verdict = ?protected_status.loader_verdict,
        has_session_id = status.config.session_id.is_some(),
        has_challenge_id = status.config.challenge_id.is_some(),
        integrity_status = ?status.integrity_status,
        "initialized Warden client status"
    );
}

fn bounded_handler_island_peer_checks(
    status: Res<WardenClientStatus>,
    mut peer_checks: ResMut<WardenHandlerIslandPeerCheckState>,
) {
    if !status.config.enabled || status.config.protection_mode == ProtectionLevel::None {
        peer_checks.last_frame_budget = 0;
        peer_checks.last_protection_mode = Some(ProtectionLevel::None);
        return;
    }

    let budget = match status.config.protection_mode {
        ProtectionLevel::None => 0,
        ProtectionLevel::Hidden => 1,
        ProtectionLevel::Protected => WARDEN_HANDLER_ISLAND_PEER_CHECKS_PER_FRAME,
    };
    peer_checks.last_frame_budget = budget;
    peer_checks.last_protection_mode = Some(status.config.protection_mode);
    peer_checks.total_checks = peer_checks.total_checks.saturating_add(u64::from(budget));
}

fn check_warden_vm_package_readiness(
    status: Res<WardenClientStatus>,
    protected_status: Res<WardenProtectedRuntimeStatus>,
    mut readiness: ResMut<WardenVmPackageReadiness>,
) {
    let next_state =
        if !status.config.enabled || status.config.protection_mode == ProtectionLevel::None {
            WardenVmPackageReadinessState::Disabled
        } else {
            match status.config.protection_mode {
                ProtectionLevel::None => WardenVmPackageReadinessState::Disabled,
                ProtectionLevel::Hidden => WardenVmPackageReadinessState::ObservationOnly,
                ProtectionLevel::Protected => {
                    if protected_status.profile.is_none()
                        || status
                            .config
                            .protected_runtime
                            .protected_bundle_digest
                            .is_none()
                    {
                        WardenVmPackageReadinessState::MissingProtectedBundle
                    } else if matches!(
                        protected_status.loader_verdict,
                        WardenProtectedLoaderVerdict::Failed
                            | WardenProtectedLoaderVerdict::Unsupported
                    ) {
                        WardenVmPackageReadinessState::RuntimeUnavailable
                    } else {
                        WardenVmPackageReadinessState::Ready
                    }
                }
            }
        };

    readiness.state = next_state;
    readiness.last_protection_mode = status.config.protection_mode;
    readiness.check_count = readiness.check_count.saturating_add(1);
}

fn verify_warden_vm_unlock_material_availability(
    status: Res<WardenClientStatus>,
    protected_status: Res<WardenProtectedRuntimeStatus>,
    mut readiness: ResMut<WardenVmPackageReadiness>,
    mut unlock_state: ResMut<WardenVmProgramUnlockState>,
) {
    unlock_state.last_policy_epoch = status.last_policy_epoch;
    if !status.config.enabled || status.config.protection_mode == ProtectionLevel::None {
        readiness.state = WardenVmPackageReadinessState::Disabled;
        unlock_state.last_unlock_failure = None;
        return;
    }

    let Some(failure) = warden_vm_unlock_failure(&status, &protected_status, readiness.state)
    else {
        unlock_state.last_unlock_failure = None;
        return;
    };
    readiness.state = readiness_state_for_unlock_failure(failure);
    unlock_state.last_unlock_failure = Some(failure);
}

fn run_bounded_warden_vm_unlock_checks(
    status: Res<WardenClientStatus>,
    readiness: Res<WardenVmPackageReadiness>,
    mut unlock_state: ResMut<WardenVmProgramUnlockState>,
) {
    if !status.config.enabled || status.config.protection_mode == ProtectionLevel::None {
        unlock_state.unlocked_function_count = 0;
        unlock_state.denied_function_count = 0;
        unlock_state.last_unlock_failure = None;
        unlock_state.last_policy_epoch = status.last_policy_epoch;
        return;
    }

    let budget = match status.config.protection_mode {
        ProtectionLevel::None => 0,
        ProtectionLevel::Hidden => 1,
        ProtectionLevel::Protected => WARDEN_VM_UNLOCK_CHECKS_PER_FRAME,
    };
    unlock_state.last_policy_epoch = status.last_policy_epoch;
    if readiness.state == WardenVmPackageReadinessState::Ready {
        unlock_state.unlocked_function_count = u16::from(budget);
        unlock_state.denied_function_count = 0;
        unlock_state.last_unlock_failure = None;
    } else if let Some(failure) = warden_vm_unlock_failure_for_readiness(readiness.state) {
        unlock_state.unlocked_function_count = 0;
        unlock_state.denied_function_count = u16::from(budget);
        unlock_state.last_unlock_failure = Some(failure);
    } else {
        unlock_state.unlocked_function_count = 0;
        unlock_state.denied_function_count = 0;
    }
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
        protection_mode = ?status.config.protection_mode,
        "Warden policy ended this protected session"
    );
    exit.write(AppExit::Success);
}

fn collect_compact_protected_call_evidence(
    status: Res<WardenClientStatus>,
    protected_status: Res<WardenProtectedRuntimeStatus>,
    readiness: Res<WardenVmPackageReadiness>,
    mut buffer: ResMut<WardenProtectedCallEvidenceBuffer>,
) {
    if !status.config.enabled || status.config.protection_mode == ProtectionLevel::None {
        return;
    }

    let snapshot = WardenCompactProtectedCallEvidenceSnapshot {
        protection_mode: status.config.protection_mode,
        integrity_status: status.integrity_status,
        loader_verdict: protected_status.loader_verdict,
        backend_decision: status.last_admission_decision,
        vm_package_readiness: readiness.state,
    };
    let reason = match buffer.last_snapshot {
        None => WardenCompactEvidenceReason::StartupSnapshot,
        Some(previous) if previous.vm_package_readiness != snapshot.vm_package_readiness => {
            WardenCompactEvidenceReason::VmPackageReadinessChanged
        }
        Some(previous) if previous.backend_decision != snapshot.backend_decision => {
            WardenCompactEvidenceReason::BackendDecisionChanged
        }
        Some(previous)
            if previous.integrity_status != snapshot.integrity_status
                || previous.loader_verdict != snapshot.loader_verdict =>
        {
            WardenCompactEvidenceReason::IntegrityChanged
        }
        Some(_) => return,
    };

    if buffer.records.len() == MAX_WARDEN_COMPACT_EVIDENCE_RECORDS {
        buffer.records.pop_front();
    }
    let evidence = WardenCompactProtectedCallEvidence {
        sequence: buffer.next_sequence,
        reason,
        protection_mode: snapshot.protection_mode,
        integrity_status: snapshot.integrity_status,
        loader_verdict: snapshot.loader_verdict,
        backend_decision: snapshot.backend_decision,
        vm_package_readiness: snapshot.vm_package_readiness,
    };
    buffer.next_sequence = buffer.next_sequence.saturating_add(1);
    buffer.last_snapshot = Some(snapshot);
    buffer.records.push_back(evidence);
}

fn collect_warden_unlock_denial_evidence(
    status: Res<WardenClientStatus>,
    readiness: Res<WardenVmPackageReadiness>,
    unlock_state: Res<WardenVmProgramUnlockState>,
    mut buffer: ResMut<WardenVmUnlockDiagnosticBuffer>,
) {
    if !status.config.enabled || status.config.protection_mode == ProtectionLevel::None {
        return;
    }
    let Some(failure) = unlock_state.last_unlock_failure else {
        return;
    };

    let snapshot = WardenUnlockDiagnosticSnapshot {
        protection_mode: status.config.protection_mode,
        readiness: readiness.state,
        failure,
        policy_epoch: unlock_state.last_policy_epoch,
    };
    if buffer
        .last_snapshot
        .is_some_and(|previous| previous == snapshot)
    {
        return;
    }
    if buffer.records.len() == MAX_WARDEN_COMPACT_EVIDENCE_RECORDS {
        buffer.records.pop_front();
    }
    let evidence = WardenCompactUnlockDenialEvidence {
        sequence: buffer.next_sequence,
        protection_mode: snapshot.protection_mode,
        readiness: snapshot.readiness,
        failure: snapshot.failure,
        policy_epoch: snapshot.policy_epoch,
    };
    buffer.next_sequence = buffer.next_sequence.saturating_add(1);
    buffer.last_snapshot = Some(snapshot);
    buffer.records.push_back(evidence);
}

fn flush_redacted_warden_evidence_within_budget(
    mut buffer: ResMut<WardenProtectedCallEvidenceBuffer>,
    mut flush_state: ResMut<WardenRedactedEvidenceFlushState>,
) {
    let mut flushed = 0_u8;
    while flushed < MAX_WARDEN_EVIDENCE_FLUSH_PER_FRAME && buffer.records.pop_front().is_some() {
        flushed = flushed.saturating_add(1);
    }
    flush_state.last_flush_count = flushed;
    flush_state.total_flushed = flush_state.total_flushed.saturating_add(u64::from(flushed));
    flush_state.deferred_due_to_budget = flush_state
        .deferred_due_to_budget
        .saturating_add(buffer.records.len() as u64);
}

fn flush_redacted_warden_unlock_diagnostics_within_budget(
    mut buffer: ResMut<WardenVmUnlockDiagnosticBuffer>,
    mut flush_state: ResMut<WardenRedactedUnlockDiagnosticFlushState>,
) {
    let mut flushed = 0_u8;
    while flushed < MAX_WARDEN_EVIDENCE_FLUSH_PER_FRAME && buffer.records.pop_front().is_some() {
        flushed = flushed.saturating_add(1);
    }
    flush_state.last_flush_count = flushed;
    flush_state.total_flushed = flush_state.total_flushed.saturating_add(u64::from(flushed));
    flush_state.deferred_due_to_budget = flush_state
        .deferred_due_to_budget
        .saturating_add(buffer.records.len() as u64);
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

fn sealed_program_authentication_status_failed(value: &str) -> bool {
    value.eq_ignore_ascii_case("failed")
        || value.eq_ignore_ascii_case("failure")
        || value.eq_ignore_ascii_case("auth_failed")
        || value.eq_ignore_ascii_case("authentication_failed")
        || value.eq_ignore_ascii_case("sealed_program_authentication_failed")
}

fn warden_vm_unlock_failure(
    status: &WardenClientStatus,
    protected_status: &WardenProtectedRuntimeStatus,
    package_readiness: WardenVmPackageReadinessState,
) -> Option<WardenVmUnlockFailureClass> {
    if package_readiness != WardenVmPackageReadinessState::Ready
        && package_readiness != WardenVmPackageReadinessState::ObservationOnly
    {
        return None;
    }
    if !warden_vm_unlock_required(status) {
        return None;
    }
    if matches!(
        protected_status.backend_challenge_binding_status,
        WardenBackendChallengeBindingStatus::Missing
            | WardenBackendChallengeBindingStatus::Stale
            | WardenBackendChallengeBindingStatus::Replayed
    ) {
        return Some(WardenVmUnlockFailureClass::UnlockMaterialMissing);
    }
    if !status.config.vm_unlock_material_ready {
        return Some(WardenVmUnlockFailureClass::UnlockMaterialMissing);
    }
    if !status.config.device_variant_ticket_ready {
        return Some(WardenVmUnlockFailureClass::TicketMissing);
    }
    if !status.config.hardware_run_proof_ready
        || status.device_attestation_status != ClientAttestationStatus::Passed
    {
        return Some(WardenVmUnlockFailureClass::HardwareProofMissing);
    }
    if status.config.sealed_program_authentication_failed {
        return Some(WardenVmUnlockFailureClass::SealedProgramAuthenticationFailed);
    }
    None
}

fn warden_vm_unlock_required(status: &WardenClientStatus) -> bool {
    match status.config.protection_mode {
        ProtectionLevel::None => false,
        ProtectionLevel::Hidden => status.config.protected_runtime.server_keyed_unlock_required,
        ProtectionLevel::Protected => true,
    }
}

const fn readiness_state_for_unlock_failure(
    failure: WardenVmUnlockFailureClass,
) -> WardenVmPackageReadinessState {
    match failure {
        WardenVmUnlockFailureClass::UnlockMaterialMissing => {
            WardenVmPackageReadinessState::UnlockMaterialMissing
        }
        WardenVmUnlockFailureClass::HardwareProofMissing => {
            WardenVmPackageReadinessState::HardwareProofMissing
        }
        WardenVmUnlockFailureClass::TicketMissing => WardenVmPackageReadinessState::TicketMissing,
        WardenVmUnlockFailureClass::SealedProgramAuthenticationFailed => {
            WardenVmPackageReadinessState::SealedProgramAuthenticationFailed
        }
    }
}

const fn warden_vm_unlock_failure_for_readiness(
    state: WardenVmPackageReadinessState,
) -> Option<WardenVmUnlockFailureClass> {
    match state {
        WardenVmPackageReadinessState::UnlockMaterialMissing => {
            Some(WardenVmUnlockFailureClass::UnlockMaterialMissing)
        }
        WardenVmPackageReadinessState::HardwareProofMissing => {
            Some(WardenVmUnlockFailureClass::HardwareProofMissing)
        }
        WardenVmPackageReadinessState::TicketMissing => {
            Some(WardenVmUnlockFailureClass::TicketMissing)
        }
        WardenVmPackageReadinessState::SealedProgramAuthenticationFailed => {
            Some(WardenVmUnlockFailureClass::SealedProgramAuthenticationFailed)
        }
        _ => None,
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ParsedWardenProtectionMode {
    protection_mode: ProtectionLevel,
    fallback: Option<WardenModeParseFallback>,
}

fn parse_warden_protection_mode(
    value: Option<&str>,
    enabled_flag: bool,
    legacy_mode_compat: bool,
) -> ParsedWardenProtectionMode {
    let Some(value) = value else {
        return ParsedWardenProtectionMode {
            protection_mode: if enabled_flag {
                ProtectionLevel::Hidden
            } else {
                ProtectionLevel::None
            },
            fallback: enabled_flag.then_some(WardenModeParseFallback::MissingModeEnabledAsHidden),
        };
    };
    if let Ok(mode) = ProtectionLevel::parse_public_label(value) {
        return ParsedWardenProtectionMode {
            protection_mode: mode,
            fallback: None,
        };
    }
    if legacy_mode_compat && let Some(mode) = parse_legacy_warden_mode(value) {
        return ParsedWardenProtectionMode {
            protection_mode: mode,
            fallback: Some(WardenModeParseFallback::LegacyModeCompat),
        };
    }
    ParsedWardenProtectionMode {
        protection_mode: ProtectionLevel::Hidden,
        fallback: Some(WardenModeParseFallback::MalformedModeHidden),
    }
}

fn parse_legacy_warden_mode(value: &str) -> Option<ProtectionLevel> {
    if value.eq_ignore_ascii_case("observe") || value.eq_ignore_ascii_case("passive") {
        return Some(ProtectionLevel::Hidden);
    }
    if value.eq_ignore_ascii_case("protect") || value.eq_ignore_ascii_case("monitor") {
        return Some(ProtectionLevel::Hidden);
    }
    if value.eq_ignore_ascii_case("enforce_candidate")
        || value.eq_ignore_ascii_case("enforce-candidate")
        || value.eq_ignore_ascii_case("enforce")
        || value.eq_ignore_ascii_case("full")
    {
        return Some(ProtectionLevel::Protected);
    }
    None
}

fn legacy_mode_compat_allowed() -> bool {
    cfg!(debug_assertions)
}

fn normalized_protected_runtime_pairs<K, V>(
    pairs: &[(K, V)],
    protection_mode: ProtectionLevel,
) -> Vec<(&str, String)>
where
    K: AsRef<str>,
    V: AsRef<str>,
{
    let mut normalized = Vec::with_capacity(pairs.len().saturating_add(1));
    let mut inserted_mode = false;
    for (key, value) in pairs {
        let key = key.as_ref();
        if key == FUN_WARDEN_MODE_ENV {
            normalized.push((key, String::from(protection_mode.public_label())));
            inserted_mode = true;
        } else if key != FUN_WARDEN_ENABLED_ENV
            && key != FUN_WARDEN_SESSION_ID_ENV
            && key != FUN_WARDEN_CHALLENGE_ID_ENV
            && key != FUN_WARDEN_LEGACY_MODE_COMPAT_ENV
        {
            normalized.push((key, String::from(value.as_ref())));
        }
    }
    if !inserted_mode {
        normalized.push((
            FUN_WARDEN_MODE_ENV,
            String::from(protection_mode.public_label()),
        ));
    }
    normalized
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
        ClientWardenPlugin, FUN_WARDEN_DEVICE_VARIANT_TICKET_READY_ENV,
        FUN_WARDEN_HARDWARE_RUN_PROOF_READY_ENV, FUN_WARDEN_LEGACY_MODE_COMPAT_ENV,
        FUN_WARDEN_SEALED_PROGRAM_AUTHENTICATION_STATUS_ENV,
        FUN_WARDEN_VM_UNLOCK_MATERIAL_READY_ENV, WARDEN_HEARTBEAT_SECONDS,
        WARDEN_INTEGRITY_RECHECK_SECONDS, WardenClientBackendDecision, WardenClientConfig,
        WardenClientFinding, WardenClientServiceState, WardenClientStatus,
        WardenCompactProtectedCallEvidence, WardenCompactUnlockDenialEvidence,
        WardenHandlerIslandPeerCheckState, WardenIntegrityRecheckTimer, WardenModeParseFallback,
        WardenProtectedCallEvidenceBuffer, WardenRedactedEvidenceFlushState,
        WardenRedactedUnlockDiagnosticFlushState, WardenVmPackageReadiness,
        WardenVmPackageReadinessState, WardenVmProgramUnlockState, WardenVmUnlockDiagnosticBuffer,
        WardenVmUnlockFailureClass, apply_service_policy_update, bounded_env_reference,
        legacy_mode_compat_allowed, ticket_id_from_session_reference,
        warden_policy_poll_interval_seconds, warden_service_policy_update_from_pairs,
    };
    use bevy::prelude::*;
    use fun_warden_core::{
        ExecutionMode, ProtectionLevel, WardenConfigVmProgramGate, WardenFunctionInflationProfile,
        WardenJsonConfig, WardenVmProgramUnlockClass,
    };
    use fun_warden_protocol::{
        ClientAttestationStatus, Digest32, FUN_WARDEN_CHALLENGE_ID_ENV, FUN_WARDEN_ENABLED_ENV,
        FUN_WARDEN_MODE_ENV, FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV,
        FUN_WARDEN_PROTECTED_INTEGRITY_STATUS_ENV, FUN_WARDEN_PROTECTED_PROFILE_ENV,
        FUN_WARDEN_PROTECTED_UNLOCK_REQUIRED_ENV, FUN_WARDEN_SESSION_ID_ENV, TicketId16,
    };

    const GAME_CLIENT_WARDEN_JSON: &str = include_str!("../warden.json");

    #[test]
    fn warden_env_config_reads_only_non_secret_references() {
        let config = WardenClientConfig::from_pairs([
            (FUN_WARDEN_ENABLED_ENV, "1"),
            (FUN_WARDEN_SESSION_ID_ENV, "session-ref"),
            (FUN_WARDEN_CHALLENGE_ID_ENV, "challenge-ref"),
            (FUN_WARDEN_MODE_ENV, "Hidden"),
            (FUN_WARDEN_VM_UNLOCK_MATERIAL_READY_ENV, "1"),
            (FUN_WARDEN_HARDWARE_RUN_PROOF_READY_ENV, "1"),
            (FUN_WARDEN_DEVICE_VARIANT_TICKET_READY_ENV, "1"),
            (
                FUN_WARDEN_SEALED_PROGRAM_AUTHENTICATION_STATUS_ENV,
                "passed",
            ),
            (FUN_WARDEN_PROTECTED_PROFILE_ENV, "Hidden"),
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
        assert_eq!(config.protection_mode, ProtectionLevel::Hidden);
        assert_eq!(
            config.internal_policy_mode(),
            fun_warden_protocol::WardenPolicyMode::Protect
        );
        assert_eq!(
            config.protected_runtime.profile,
            Some(fun_warden_core::ProtectedProtectionProfile::Standard)
        );
        assert_eq!(
            config.protected_runtime.protected_bundle_digest,
            Some(Digest32([7; 32]))
        );
        assert!(config.vm_unlock_material_ready);
        assert!(config.hardware_run_proof_ready);
        assert!(config.device_variant_ticket_ready);
        assert!(!config.sealed_program_authentication_failed);
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
            (FUN_WARDEN_MODE_ENV, "Hidden"),
            (FUN_WARDEN_PROTECTED_PROFILE_ENV, "Protected"),
            (FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV, "not-a-digest"),
        ]);

        assert!(config.enabled);
        assert_eq!(config.protection_mode, ProtectionLevel::Hidden);
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
    fn warden_public_mode_parser_accepts_only_none_hidden_protected() {
        let none = WardenClientConfig::from_pairs([(FUN_WARDEN_MODE_ENV, "None")]);
        assert!(!none.enabled);
        assert_eq!(none.protection_mode, ProtectionLevel::None);
        assert_eq!(
            none.protected_runtime.enforcement_mode,
            fun_warden_protocol::WardenPolicyMode::Observe
        );

        let hidden = WardenClientConfig::from_pairs([(FUN_WARDEN_MODE_ENV, "Hidden")]);
        assert!(hidden.enabled);
        assert_eq!(hidden.protection_mode, ProtectionLevel::Hidden);
        assert_eq!(
            hidden.protected_runtime.enforcement_mode,
            fun_warden_protocol::WardenPolicyMode::Protect
        );

        let protected = WardenClientConfig::from_pairs([(FUN_WARDEN_MODE_ENV, "Protected")]);
        assert!(protected.enabled);
        assert_eq!(protected.protection_mode, ProtectionLevel::Protected);
        assert_eq!(
            protected.protected_runtime.enforcement_mode,
            fun_warden_protocol::WardenPolicyMode::Enforce
        );
    }

    #[test]
    fn game_client_warden_json_matches_client_plugin_contract() {
        let config: WardenJsonConfig =
            serde_json::from_str(GAME_CLIENT_WARDEN_JSON).expect("game_client warden.json");

        assert_eq!(config.validate(), Ok(()));
        assert_eq!(config.protection_level(), ProtectionLevel::Protected);
        assert_eq!(config.package.name, "game_client");
        assert_eq!(config.package.profile, "game_client");
        assert_eq!(
            config.allowed_execution_tiers(),
            vec![
                ExecutionMode::Interpreter,
                ExecutionMode::QuickenedInterpreter,
                ExecutionMode::SuperInstructionInterpreter,
                ExecutionMode::AotStub,
            ]
        );
        assert!(!config.execution_modes().allows(ExecutionMode::DynamicJit));
        assert_eq!(
            config.vm_program_gate,
            WardenConfigVmProgramGate::protected_default()
        );
        assert_eq!(
            config.vm_program_gate.default_unlock,
            WardenVmProgramUnlockClass::PerSession
        );
        assert!(config.vm_program_gate.require_hardware_proof);
        assert!(config.vm_program_gate.require_warden_ticket);
        assert!(config.vm_program_gate.seal_function_bytecode);
        assert!(config.function_inflation.enabled);
        assert_eq!(
            config.function_inflation.profile,
            WardenFunctionInflationProfile::Large
        );
        assert!(config.function_inflation.preserve_benchmark_budget);
        assert_eq!(
            config.bevy.integrity_recheck_seconds as f32,
            WARDEN_INTEGRITY_RECHECK_SECONDS
        );
        assert_eq!(
            config.bevy.service_heartbeat_seconds as f32,
            WARDEN_HEARTBEAT_SECONDS
        );
        assert_eq!(config.bevy.frame_budget_us, 300);
    }

    #[test]
    fn game_client_warden_json_uses_only_public_protection_vocabulary() {
        let config: serde_json::Value =
            serde_json::from_str(GAME_CLIENT_WARDEN_JSON).expect("game_client warden.json value");

        assert_eq!(config["protection"], "Protected");
        for legacy in [
            "observe",
            "protect",
            "enforce",
            "enforce_candidate",
            "light",
            "standard",
            "ranked",
            "research",
        ] {
            assert!(
                !GAME_CLIENT_WARDEN_JSON.contains(&format!("\"{legacy}\"")),
                "game_client warden.json must not contain legacy public label {legacy}"
            );
        }
    }

    #[test]
    fn client_warden_mode_parser_accepts_only_none_hidden_protected() {
        for (label, expected, enabled) in [
            ("None", ProtectionLevel::None, false),
            ("Hidden", ProtectionLevel::Hidden, true),
            ("Protected", ProtectionLevel::Protected, true),
        ] {
            let parsed = WardenClientConfig::from_pairs([(FUN_WARDEN_MODE_ENV, label)]);
            assert_eq!(parsed.protection_mode, expected);
            assert_eq!(parsed.enabled, enabled);
            assert_eq!(parsed.mode_parse_fallback, None);
        }

        for legacy in [
            "observe",
            "Observe",
            "protect",
            "Protect",
            "enforce",
            "Enforce",
            "enforce_candidate",
            "EnforceCandidate",
        ] {
            let parsed = WardenClientConfig::from_pairs([(FUN_WARDEN_MODE_ENV, legacy)]);
            assert_eq!(parsed.protection_mode, ProtectionLevel::Hidden);
            assert_eq!(
                parsed.mode_parse_fallback,
                Some(WardenModeParseFallback::MalformedModeHidden)
            );
        }
    }

    #[test]
    fn warden_legacy_mode_compat_requires_explicit_debug_flag() {
        let legacy_without_compat =
            WardenClientConfig::from_pairs([(FUN_WARDEN_MODE_ENV, "protect")]);

        assert_eq!(
            legacy_without_compat.protection_mode,
            ProtectionLevel::Hidden
        );
        assert_eq!(
            legacy_without_compat.mode_parse_fallback,
            Some(WardenModeParseFallback::MalformedModeHidden)
        );

        let legacy_with_compat = WardenClientConfig::from_pairs([
            (FUN_WARDEN_MODE_ENV, "enforce"),
            (FUN_WARDEN_LEGACY_MODE_COMPAT_ENV, "1"),
        ]);
        if legacy_mode_compat_allowed() {
            assert_eq!(
                legacy_with_compat.protection_mode,
                ProtectionLevel::Protected
            );
            assert_eq!(
                legacy_with_compat.mode_parse_fallback,
                Some(WardenModeParseFallback::LegacyModeCompat)
            );
        } else {
            assert_eq!(legacy_with_compat.protection_mode, ProtectionLevel::Hidden);
            assert_eq!(
                legacy_with_compat.mode_parse_fallback,
                Some(WardenModeParseFallback::MalformedModeHidden)
            );
        }
    }

    #[test]
    fn missing_mode_uses_none_unless_legacy_enabled_flag_is_set() {
        let disabled = WardenClientConfig::from_pairs(std::iter::empty::<(&str, &str)>());
        assert!(!disabled.enabled);
        assert_eq!(disabled.protection_mode, ProtectionLevel::None);
        assert_eq!(disabled.mode_parse_fallback, None);

        let legacy_enabled = WardenClientConfig::from_pairs([(FUN_WARDEN_ENABLED_ENV, "1")]);
        assert!(legacy_enabled.enabled);
        assert_eq!(legacy_enabled.protection_mode, ProtectionLevel::Hidden);
        assert_eq!(
            legacy_enabled.mode_parse_fallback,
            Some(WardenModeParseFallback::MissingModeEnabledAsHidden)
        );
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
    fn hidden_mode_does_not_set_blocked_by_warden_locally() {
        let mut status = enabled_status();
        status.config = WardenClientConfig::from_pairs([(FUN_WARDEN_MODE_ENV, "Hidden")]);
        let update = warden_service_policy_update_from_pairs(
            [
                (
                    "FUN_WARDEN_SERVICE_DECISION",
                    "quarantine_to_untrusted_pool",
                ),
                ("FUN_WARDEN_POLICY_EPOCH", "8"),
                ("FUN_WARDEN_DEVICE_ATTESTATION_STATUS", "failed"),
            ],
            0,
        )
        .expect("policy update");

        apply_service_policy_update(&mut status, update);

        assert_eq!(status.config.protection_mode, ProtectionLevel::Hidden);
        assert_eq!(
            status.last_admission_decision,
            WardenClientBackendDecision::QuarantineToUntrustedPool
        );
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
    fn protected_deny_session_requests_clean_app_exit() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        {
            let mut status = app.world_mut().resource_mut::<WardenClientStatus>();
            status.config = WardenClientConfig::from_pairs([(FUN_WARDEN_MODE_ENV, "Protected")]);
            status.last_admission_decision = WardenClientBackendDecision::DenySession;
        }

        app.update();

        let status = app.world().resource::<WardenClientStatus>();
        assert_eq!(status.config.protection_mode, ProtectionLevel::Protected);
        assert_eq!(status.finding, WardenClientFinding::EnforcementDenied);
        assert!(status.blocked_by_warden);
        assert!(status.exit_requested);
    }

    #[test]
    fn service_policy_poll_interval_is_low_frequency_and_jittered() {
        assert_eq!(warden_policy_poll_interval_seconds(0), 5.0);
        assert_eq!(warden_policy_poll_interval_seconds(10_000), 15.0);
        let interval = warden_policy_poll_interval_seconds(4_321);
        assert!((5.0..=15.0).contains(&interval));
    }

    #[test]
    fn service_policy_poll_remains_low_frequency() {
        assert_eq!(warden_policy_poll_interval_seconds(0), 5.0);
        assert_eq!(warden_policy_poll_interval_seconds(10_000), 15.0);
        assert!(warden_policy_poll_interval_seconds(999) >= 5.0);
        assert!(warden_policy_poll_interval_seconds(999) <= 15.0);
    }

    #[test]
    fn integrity_recheck_respects_budget() {
        let timer = WardenIntegrityRecheckTimer::default();

        assert_eq!(
            timer.0.duration().as_secs_f32(),
            WARDEN_INTEGRITY_RECHECK_SECONDS
        );
    }

    #[test]
    fn warden_plugin_runs_split_schedule_work_with_bounded_frame_budget() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        app.world_mut().resource_mut::<WardenClientStatus>().config =
            WardenClientConfig::from_pairs([
                (FUN_WARDEN_MODE_ENV, "Hidden"),
                (FUN_WARDEN_PROTECTED_PROFILE_ENV, "Hidden"),
            ]);

        app.update();

        let peer_checks = app.world().resource::<WardenHandlerIslandPeerCheckState>();
        assert_eq!(peer_checks.last_frame_budget, 1);
        assert_eq!(peer_checks.total_checks, 1);
        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::ObservationOnly
        );
        let flush_state = app.world().resource::<WardenRedactedEvidenceFlushState>();
        assert_eq!(flush_state.last_flush_count, 1);
        assert_eq!(flush_state.total_flushed, 1);
        assert!(
            app.world()
                .resource::<WardenProtectedCallEvidenceBuffer>()
                .records
                .is_empty()
        );
    }

    #[test]
    fn protected_unlock_readiness_requires_material_ticket_and_hardware_proof() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        app.world_mut().resource_mut::<WardenClientStatus>().config =
            protected_config_with_unlock_flags(false, false, false, false);

        app.update();

        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::UnlockMaterialMissing
        );
        let unlock_state = app.world().resource::<WardenVmProgramUnlockState>();
        assert_eq!(
            unlock_state.last_unlock_failure,
            Some(WardenVmUnlockFailureClass::UnlockMaterialMissing)
        );
        assert_eq!(unlock_state.unlocked_function_count, 0);
        assert_eq!(unlock_state.denied_function_count, 2);

        app.world_mut().resource_mut::<WardenClientStatus>().config =
            protected_config_with_unlock_flags(true, false, false, false);
        app.update();

        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::TicketMissing
        );
        let unlock_state = app.world().resource::<WardenVmProgramUnlockState>();
        assert_eq!(
            unlock_state.last_unlock_failure,
            Some(WardenVmUnlockFailureClass::TicketMissing)
        );

        {
            let mut status = app.world_mut().resource_mut::<WardenClientStatus>();
            status.config = protected_config_with_unlock_flags(true, true, true, false);
            status.device_attestation_status = ClientAttestationStatus::Passed;
        }
        app.update();

        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(readiness.state, WardenVmPackageReadinessState::Ready);
        let unlock_state = app.world().resource::<WardenVmProgramUnlockState>();
        assert_eq!(unlock_state.last_unlock_failure, None);
        assert_eq!(unlock_state.unlocked_function_count, 2);
        assert_eq!(unlock_state.denied_function_count, 0);
    }

    #[test]
    fn protected_mode_reports_ticket_missing_as_vm_readiness_failure() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        {
            let mut status = app.world_mut().resource_mut::<WardenClientStatus>();
            status.config = protected_config_with_unlock_flags(true, false, true, false);
            status.device_attestation_status = ClientAttestationStatus::Passed;
        }

        app.update();

        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::TicketMissing
        );
        let unlock_state = app.world().resource::<WardenVmProgramUnlockState>();
        assert_eq!(
            unlock_state.last_unlock_failure,
            Some(WardenVmUnlockFailureClass::TicketMissing)
        );
        assert_eq!(unlock_state.unlocked_function_count, 0);
        assert_eq!(unlock_state.denied_function_count, 2);
    }

    #[test]
    fn protected_mode_reports_hardware_proof_missing_as_vm_readiness_failure() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        {
            let mut status = app.world_mut().resource_mut::<WardenClientStatus>();
            status.config = protected_config_with_unlock_flags(true, true, false, false);
            status.device_attestation_status = ClientAttestationStatus::Passed;
        }

        app.update();

        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::HardwareProofMissing
        );
        let unlock_state = app.world().resource::<WardenVmProgramUnlockState>();
        assert_eq!(
            unlock_state.last_unlock_failure,
            Some(WardenVmUnlockFailureClass::HardwareProofMissing)
        );
        assert_eq!(unlock_state.unlocked_function_count, 0);
        assert_eq!(unlock_state.denied_function_count, 2);
    }

    #[test]
    fn hidden_mode_reports_unlock_missing_without_local_block() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        app.world_mut().resource_mut::<WardenClientStatus>().config =
            WardenClientConfig::from_pairs([
                (FUN_WARDEN_MODE_ENV, "Hidden"),
                (FUN_WARDEN_PROTECTED_PROFILE_ENV, "Hidden"),
                (
                    FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV,
                    "0808080808080808080808080808080808080808080808080808080808080808",
                ),
                (FUN_WARDEN_PROTECTED_INTEGRITY_STATUS_ENV, "passed"),
                (FUN_WARDEN_PROTECTED_UNLOCK_REQUIRED_ENV, "1"),
                (FUN_WARDEN_VM_UNLOCK_MATERIAL_READY_ENV, "1"),
            ]);

        app.update();

        let status = app.world().resource::<WardenClientStatus>();
        assert_eq!(status.config.protection_mode, ProtectionLevel::Hidden);
        assert!(!status.blocked_by_warden);
        assert!(!status.exit_requested);
        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::TicketMissing
        );
        let unlock_state = app.world().resource::<WardenVmProgramUnlockState>();
        assert_eq!(
            unlock_state.last_unlock_failure,
            Some(WardenVmUnlockFailureClass::TicketMissing)
        );
        assert_eq!(unlock_state.denied_function_count, 1);
    }

    #[test]
    fn hidden_mode_reports_unlock_failure_without_blocking_gameplay() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        app.world_mut().resource_mut::<WardenClientStatus>().config =
            hidden_config_requiring_unlock_with_ticket_missing();

        app.update();

        let status = app.world().resource::<WardenClientStatus>();
        assert_eq!(status.config.protection_mode, ProtectionLevel::Hidden);
        assert!(!status.blocked_by_warden);
        assert!(!status.exit_requested);
        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::TicketMissing
        );
        let unlock_state = app.world().resource::<WardenVmProgramUnlockState>();
        assert_eq!(
            unlock_state.last_unlock_failure,
            Some(WardenVmUnlockFailureClass::TicketMissing)
        );
        assert_eq!(unlock_state.unlocked_function_count, 0);
        assert_eq!(unlock_state.denied_function_count, 1);
        let flush_state = app
            .world()
            .resource::<WardenRedactedUnlockDiagnosticFlushState>();
        assert_eq!(flush_state.total_flushed, 1);
    }

    #[test]
    fn protected_mode_blocks_only_protected_boundary_on_unlock_failure() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        {
            let mut status = app.world_mut().resource_mut::<WardenClientStatus>();
            status.config = protected_config_with_unlock_flags(true, false, true, false);
            status.device_attestation_status = ClientAttestationStatus::Passed;
        }

        app.update();

        let status = app.world().resource::<WardenClientStatus>();
        assert_eq!(status.config.protection_mode, ProtectionLevel::Protected);
        assert_eq!(
            status.last_admission_decision,
            WardenClientBackendDecision::Pending
        );
        assert_ne!(status.finding, WardenClientFinding::EnforcementDenied);
        assert!(!status.blocked_by_warden);
        assert!(!status.exit_requested);
        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::TicketMissing
        );
        let unlock_state = app.world().resource::<WardenVmProgramUnlockState>();
        assert_eq!(unlock_state.unlocked_function_count, 0);
        assert_eq!(unlock_state.denied_function_count, 2);
    }

    #[test]
    fn protected_unlock_authentication_failure_collects_redacted_diagnostic() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(ClientWardenPlugin);
        {
            let mut status = app.world_mut().resource_mut::<WardenClientStatus>();
            status.config = protected_config_with_unlock_flags(true, true, true, true);
            status.device_attestation_status = ClientAttestationStatus::Passed;
            status.last_policy_epoch = 42;
        }

        app.update();

        let readiness = app.world().resource::<WardenVmPackageReadiness>();
        assert_eq!(
            readiness.state,
            WardenVmPackageReadinessState::SealedProgramAuthenticationFailed
        );
        let flush_state = app
            .world()
            .resource::<WardenRedactedUnlockDiagnosticFlushState>();
        assert_eq!(flush_state.last_flush_count, 1);
        assert_eq!(flush_state.total_flushed, 1);
        assert!(
            app.world()
                .resource::<WardenVmUnlockDiagnosticBuffer>()
                .records
                .is_empty()
        );
    }

    #[test]
    fn redacted_unlock_evidence_flushes_within_frame_budget() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<WardenVmUnlockDiagnosticBuffer>()
            .init_resource::<WardenRedactedUnlockDiagnosticFlushState>()
            .add_systems(
                Last,
                super::flush_redacted_warden_unlock_diagnostics_within_budget,
            );
        let frame_budget = super::MAX_WARDEN_EVIDENCE_FLUSH_PER_FRAME;
        {
            let mut buffer = app
                .world_mut()
                .resource_mut::<WardenVmUnlockDiagnosticBuffer>();
            for sequence in 0..u64::from(frame_budget.saturating_add(2)) {
                buffer.records.push_back(WardenCompactUnlockDenialEvidence {
                    sequence,
                    protection_mode: ProtectionLevel::Protected,
                    readiness: WardenVmPackageReadinessState::TicketMissing,
                    failure: WardenVmUnlockFailureClass::TicketMissing,
                    policy_epoch: 99,
                });
            }
        }

        app.update();

        let flush_state = app
            .world()
            .resource::<WardenRedactedUnlockDiagnosticFlushState>();
        assert_eq!(flush_state.last_flush_count, frame_budget);
        assert_eq!(flush_state.total_flushed, u64::from(frame_budget));
        assert_eq!(flush_state.deferred_due_to_budget, 2);
        let buffer = app.world().resource::<WardenVmUnlockDiagnosticBuffer>();
        assert_eq!(buffer.records.len(), 2);
    }

    #[test]
    fn compact_evidence_records_only_redacted_state_classes() {
        let evidence = WardenCompactProtectedCallEvidence {
            sequence: 7,
            reason: super::WardenCompactEvidenceReason::StartupSnapshot,
            protection_mode: ProtectionLevel::Protected,
            integrity_status: fun_warden_core::IntegrityStatus::Failed,
            loader_verdict: super::WardenProtectedLoaderVerdict::Failed,
            backend_decision: WardenClientBackendDecision::DenySession,
            vm_package_readiness: WardenVmPackageReadinessState::RuntimeUnavailable,
        };
        let debug = format!("{evidence:?}");

        for forbidden in [
            "ticket_id",
            "session_id",
            "token",
            "account",
            "device",
            "hardware",
            "serial",
            "bytecode",
        ] {
            assert!(!debug.to_ascii_lowercase().contains(forbidden));
        }
    }

    #[test]
    fn unlock_diagnostic_records_only_redacted_state_classes() {
        let evidence = WardenCompactUnlockDenialEvidence {
            sequence: 3,
            protection_mode: ProtectionLevel::Protected,
            readiness: WardenVmPackageReadinessState::HardwareProofMissing,
            failure: WardenVmUnlockFailureClass::HardwareProofMissing,
            policy_epoch: 11,
        };
        let debug = format!("{evidence:?}");

        for forbidden in [
            "ticket_id",
            "session_id",
            "token",
            "account",
            "hardware_id",
            "serial",
            "bytecode",
            "signature",
        ] {
            assert!(!debug.to_ascii_lowercase().contains(forbidden));
        }
    }

    fn enabled_status() -> WardenClientStatus {
        let mut status = WardenClientStatus::default();
        status.config.enabled = true;
        status.service_state = WardenClientServiceState::PendingService;
        status
    }

    fn hidden_config_requiring_unlock_with_ticket_missing() -> WardenClientConfig {
        WardenClientConfig::from_pairs([
            (FUN_WARDEN_MODE_ENV, "Hidden"),
            (FUN_WARDEN_PROTECTED_PROFILE_ENV, "Hidden"),
            (
                FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV,
                "0808080808080808080808080808080808080808080808080808080808080808",
            ),
            (FUN_WARDEN_PROTECTED_INTEGRITY_STATUS_ENV, "passed"),
            (FUN_WARDEN_PROTECTED_UNLOCK_REQUIRED_ENV, "1"),
            (FUN_WARDEN_VM_UNLOCK_MATERIAL_READY_ENV, "1"),
        ])
    }

    fn protected_config_with_unlock_flags(
        unlock_material_ready: bool,
        ticket_ready: bool,
        hardware_proof_ready: bool,
        sealed_auth_failed: bool,
    ) -> WardenClientConfig {
        WardenClientConfig::from_pairs([
            (FUN_WARDEN_MODE_ENV, "Protected"),
            (FUN_WARDEN_PROTECTED_PROFILE_ENV, "Protected"),
            (
                FUN_WARDEN_PROTECTED_BUNDLE_DIGEST_ENV,
                "0909090909090909090909090909090909090909090909090909090909090909",
            ),
            (FUN_WARDEN_PROTECTED_INTEGRITY_STATUS_ENV, "passed"),
            (FUN_WARDEN_PROTECTED_UNLOCK_REQUIRED_ENV, "1"),
            (
                FUN_WARDEN_VM_UNLOCK_MATERIAL_READY_ENV,
                flag_label(unlock_material_ready),
            ),
            (
                FUN_WARDEN_DEVICE_VARIANT_TICKET_READY_ENV,
                flag_label(ticket_ready),
            ),
            (
                FUN_WARDEN_HARDWARE_RUN_PROOF_READY_ENV,
                flag_label(hardware_proof_ready),
            ),
            (
                FUN_WARDEN_SEALED_PROGRAM_AUTHENTICATION_STATUS_ENV,
                if sealed_auth_failed {
                    "failed"
                } else {
                    "passed"
                },
            ),
        ])
    }

    const fn flag_label(value: bool) -> &'static str {
        if value { "1" } else { "0" }
    }
}
