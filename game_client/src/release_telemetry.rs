use std::{
    collections::VecDeque,
    fs,
    panic::PanicHookInfo,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use fun_telemetry_core::{
    CanonicalTelemetryFileKind, ClientCrashEvidenceInput, FrameReadOptions, FrameWriteOptions,
    RuntimeBundleMetadata, RuntimeTelemetrySnapshot, StaticEventKindId, TelemetryCoreError,
    TelemetryImportance, TelemetrySubsystem, build_client_crash_evidence_bundle,
    bundle_from_runtime_snapshot, classify_panic_message, digest_bytes, read_bundle_from_path,
    validate_canonical_telemetry_path, write_bundle_to_path,
};

static RELEASE_CRASH_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);

pub const RELEASE_TELEMETRY_BUDGET_CLASS: i32 =
    fun_telemetry_core::enum_values::BUDGET_CLASS_SAMPLED_RUNTIME;
pub const RELEASE_TELEMETRY_RETENTION_CLASS: i32 =
    fun_telemetry_core::enum_values::RETENTION_CLASS_SUMMARIZE_THEN_DISCARD_RAW;
pub const RELEASE_CRASH_TELEMETRY_BUDGET_CLASS: i32 =
    fun_telemetry_core::enum_values::BUDGET_CLASS_FAILURE_CAPTURE;
pub const RELEASE_CRASH_TELEMETRY_RETENTION_CLASS: i32 =
    fun_telemetry_core::enum_values::RETENTION_CLASS_KEEP_FAILURE_EVIDENCE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientTelemetryUploadState {
    Collecting,
    Encoding,
    Compressing,
    QueuedLocal,
    Uploading,
    Accepted,
    RetryableFailure,
    ServerRejectedSchema,
    ServerRejectedBudget,
    ServerRejectedRedaction,
    DroppedByRetention,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientTelemetryServerResponse {
    Accepted,
    RetryableFailure,
    RejectedSchema,
    RejectedBudget,
    RejectedRedaction,
}

pub trait ClientTelemetryUploader {
    fn upload_bundle(&mut self, compressed_frame: &[u8]) -> ClientTelemetryServerResponse;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientCrashHookInstallStatus {
    Installed,
    AlreadyInstalled,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClientCrashHookMetadata {
    pub build_id: u32,
    pub version_id: u32,
    pub platform_id: u32,
    pub subsystem: TelemetrySubsystem,
}

impl ClientCrashHookMetadata {
    pub fn from_env() -> Self {
        Self {
            build_id: env_u32("FUN_CLIENT_BUILD_ID").unwrap_or(1),
            version_id: env_u32("FUN_CLIENT_VERSION_ID").unwrap_or(1),
            platform_id: env_u32("FUN_CLIENT_PLATFORM_ID").unwrap_or(platform_id()),
            subsystem: TelemetrySubsystem::FUN_CRASH_HANDLER,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientTelemetrySpoolConfig {
    pub spool_dir: PathBuf,
    pub max_spool_bytes: u64,
    pub max_queue_len: usize,
    pub max_retry_count: u8,
}

impl ClientTelemetrySpoolConfig {
    pub fn release_default(spool_dir: PathBuf) -> Self {
        Self {
            spool_dir,
            max_spool_bytes: 32 * 1024 * 1024,
            max_queue_len: 64,
            max_retry_count: 3,
        }
    }

    pub fn from_env_or_default() -> Self {
        let spool_dir = std::env::var_os("FUN_CLIENT_TELEMETRY_SPOOL_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(default_spool_dir);
        Self::release_default(spool_dir)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ClientTelemetrySpoolImportance {
    Routine,
    CrashEvidence,
}

impl ClientTelemetrySpoolImportance {
    const fn can_prune_before_upload(self) -> bool {
        matches!(self, Self::Routine)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientTelemetrySpoolArtifact {
    pub path: PathBuf,
    pub bytes: u64,
    pub retry_count: u8,
    pub state: ClientTelemetryUploadState,
    importance: ClientTelemetrySpoolImportance,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ClientTelemetryUploadSummary {
    pub accepted: u32,
    pub retryable_failures: u32,
    pub server_rejected: u32,
    pub dropped_by_retention: u32,
    pub last_state: Option<ClientTelemetryUploadState>,
}

pub struct ClientReleaseTelemetryPipeline {
    config: ClientTelemetrySpoolConfig,
    queue: VecDeque<ClientTelemetrySpoolArtifact>,
    next_sequence: u64,
    state: ClientTelemetryUploadState,
}

impl ClientReleaseTelemetryPipeline {
    pub fn new(config: ClientTelemetrySpoolConfig) -> Result<Self, TelemetryCoreError> {
        fs::create_dir_all(&config.spool_dir).map_err(|source| TelemetryCoreError::Io {
            operation: "creating client telemetry spool directory",
            source,
        })?;
        Ok(Self {
            config,
            queue: VecDeque::new(),
            next_sequence: 0,
            state: ClientTelemetryUploadState::Collecting,
        })
    }

    pub const fn state(&self) -> ClientTelemetryUploadState {
        self.state
    }

    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    pub fn queued_artifacts(&self) -> impl Iterator<Item = &ClientTelemetrySpoolArtifact> {
        self.queue.iter()
    }

    pub fn spool_release_session_snapshot(
        &mut self,
        snapshot: &RuntimeTelemetrySnapshot,
        metadata: RuntimeBundleMetadata,
    ) -> Result<ClientTelemetryUploadState, TelemetryCoreError> {
        let bundle = bundle_from_runtime_snapshot(snapshot, metadata)?;
        self.spool_bundle("session", &bundle, ClientTelemetrySpoolImportance::Routine)
    }

    pub fn spool_crash_evidence(
        &mut self,
        input: &ClientCrashEvidenceInput<'_>,
    ) -> Result<ClientTelemetryUploadState, TelemetryCoreError> {
        let bundle = build_client_crash_evidence_bundle(input)?;
        self.spool_bundle(
            "crash",
            &bundle,
            ClientTelemetrySpoolImportance::CrashEvidence,
        )
    }

    pub fn load_spooled_artifacts(&mut self) -> Result<usize, TelemetryCoreError> {
        let mut paths = Vec::new();
        for entry in
            fs::read_dir(&self.config.spool_dir).map_err(|source| TelemetryCoreError::Io {
                operation: "reading client telemetry spool directory",
                source,
            })?
        {
            let entry = entry.map_err(|source| TelemetryCoreError::Io {
                operation: "reading client telemetry spool entry",
                source,
            })?;
            let path = entry.path();
            if validate_canonical_telemetry_path(&path, CanonicalTelemetryFileKind::Bundle).is_ok()
            {
                paths.push(path);
            }
        }
        paths.sort();
        let mut loaded = 0;
        for path in paths {
            if self.queue.iter().any(|artifact| artifact.path == path) {
                continue;
            }
            let bytes = file_len(&path)?;
            self.queue.push_back(ClientTelemetrySpoolArtifact {
                path,
                bytes,
                retry_count: 0,
                state: ClientTelemetryUploadState::QueuedLocal,
                importance: ClientTelemetrySpoolImportance::CrashEvidence,
            });
            loaded += 1;
        }
        Ok(loaded)
    }

    pub fn upload_pending(
        &mut self,
        uploader: &mut impl ClientTelemetryUploader,
    ) -> Result<ClientTelemetryUploadSummary, TelemetryCoreError> {
        if self.queue.is_empty() {
            self.load_spooled_artifacts()?;
        }
        let mut summary = ClientTelemetryUploadSummary::default();
        while let Some(front) = self.queue.front_mut() {
            if front.retry_count >= self.config.max_retry_count {
                self.state = ClientTelemetryUploadState::RetryableFailure;
                summary.retryable_failures = summary.retryable_failures.saturating_add(1);
                summary.last_state = Some(self.state);
                break;
            }
            front.state = ClientTelemetryUploadState::Uploading;
            self.state = ClientTelemetryUploadState::Uploading;
            let payload = fs::read(&front.path).map_err(|source| TelemetryCoreError::Io {
                operation: "reading queued client telemetry bundle",
                source,
            })?;
            match uploader.upload_bundle(&payload) {
                ClientTelemetryServerResponse::Accepted => {
                    let path = front.path.clone();
                    remove_file_if_exists(&path)?;
                    let _ = self.queue.pop_front();
                    self.state = ClientTelemetryUploadState::Accepted;
                    summary.accepted = summary.accepted.saturating_add(1);
                    summary.last_state = Some(self.state);
                }
                ClientTelemetryServerResponse::RetryableFailure => {
                    front.retry_count = front.retry_count.saturating_add(1);
                    front.state = ClientTelemetryUploadState::RetryableFailure;
                    self.state = ClientTelemetryUploadState::RetryableFailure;
                    summary.retryable_failures = summary.retryable_failures.saturating_add(1);
                    summary.last_state = Some(self.state);
                    break;
                }
                ClientTelemetryServerResponse::RejectedSchema => {
                    front.state = ClientTelemetryUploadState::ServerRejectedSchema;
                    self.state = ClientTelemetryUploadState::ServerRejectedSchema;
                    summary.server_rejected = summary.server_rejected.saturating_add(1);
                    summary.last_state = Some(self.state);
                    let _ = self.queue.pop_front();
                }
                ClientTelemetryServerResponse::RejectedBudget => {
                    front.state = ClientTelemetryUploadState::ServerRejectedBudget;
                    self.state = ClientTelemetryUploadState::ServerRejectedBudget;
                    summary.server_rejected = summary.server_rejected.saturating_add(1);
                    summary.last_state = Some(self.state);
                    let _ = self.queue.pop_front();
                }
                ClientTelemetryServerResponse::RejectedRedaction => {
                    front.state = ClientTelemetryUploadState::ServerRejectedRedaction;
                    self.state = ClientTelemetryUploadState::ServerRejectedRedaction;
                    summary.server_rejected = summary.server_rejected.saturating_add(1);
                    summary.last_state = Some(self.state);
                    let _ = self.queue.pop_front();
                }
            }
        }
        Ok(summary)
    }

    pub fn read_queued_bundle(
        artifact: &ClientTelemetrySpoolArtifact,
    ) -> Result<fun_telemetry_core::telemetry_v1::TelemetryBundle, TelemetryCoreError> {
        read_bundle_from_path(&artifact.path, FrameReadOptions::default())
    }

    fn spool_bundle(
        &mut self,
        stem: &str,
        bundle: &fun_telemetry_core::telemetry_v1::TelemetryBundle,
        importance: ClientTelemetrySpoolImportance,
    ) -> Result<ClientTelemetryUploadState, TelemetryCoreError> {
        if self.queue.len() >= self.config.max_queue_len && importance.can_prune_before_upload() {
            self.state = ClientTelemetryUploadState::DroppedByRetention;
            return Ok(self.state);
        }
        self.state = ClientTelemetryUploadState::Encoding;
        let path = self.next_path(stem);
        validate_canonical_telemetry_path(&path, CanonicalTelemetryFileKind::Bundle)?;
        self.state = ClientTelemetryUploadState::Compressing;
        write_bundle_to_path(&path, bundle, FrameWriteOptions::default())?;
        let bytes = file_len(&path)?;
        let artifact = ClientTelemetrySpoolArtifact {
            path,
            bytes,
            retry_count: 0,
            state: ClientTelemetryUploadState::QueuedLocal,
            importance,
        };
        self.queue.push_back(artifact);
        self.enforce_queue_limit()?;
        self.enforce_spool_limit()?;
        self.state = self
            .queue
            .back()
            .map(|artifact| artifact.state)
            .unwrap_or(ClientTelemetryUploadState::DroppedByRetention);
        Ok(self.state)
    }

    fn next_path(&mut self, stem: &str) -> PathBuf {
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.config
            .spool_dir
            .join(format!("{stem}-{}.funpb.zst", self.next_sequence))
    }

    fn enforce_queue_limit(&mut self) -> Result<(), TelemetryCoreError> {
        while self.queue.len() > self.config.max_queue_len {
            let Some(index) = self
                .queue
                .iter()
                .position(|artifact| artifact.importance.can_prune_before_upload())
            else {
                let last = self.queue.pop_back();
                if let Some(artifact) = last {
                    remove_file_if_exists(&artifact.path)?;
                }
                self.state = ClientTelemetryUploadState::DroppedByRetention;
                return Ok(());
            };
            if let Some(artifact) = self.queue.remove(index) {
                remove_file_if_exists(&artifact.path)?;
            }
        }
        Ok(())
    }

    fn enforce_spool_limit(&mut self) -> Result<(), TelemetryCoreError> {
        let mut bytes = self.spool_bytes();
        while bytes > self.config.max_spool_bytes {
            let Some(index) = self
                .queue
                .iter()
                .position(|artifact| artifact.importance.can_prune_before_upload())
            else {
                let last = self.queue.pop_back();
                if let Some(artifact) = last {
                    remove_file_if_exists(&artifact.path)?;
                }
                self.state = ClientTelemetryUploadState::DroppedByRetention;
                return Ok(());
            };
            if let Some(artifact) = self.queue.remove(index) {
                bytes = bytes.saturating_sub(artifact.bytes);
                remove_file_if_exists(&artifact.path)?;
            }
        }
        Ok(())
    }

    fn spool_bytes(&self) -> u64 {
        self.queue
            .iter()
            .map(|artifact| artifact.bytes)
            .fold(0_u64, u64::saturating_add)
    }
}

#[cfg(all(feature = "release-telemetry", feature = "crash-telemetry"))]
pub fn install_release_panic_hook_from_env() -> ClientCrashHookInstallStatus {
    install_release_panic_hook(
        ClientTelemetrySpoolConfig::from_env_or_default(),
        ClientCrashHookMetadata::from_env(),
    )
}

pub fn install_release_panic_hook(
    config: ClientTelemetrySpoolConfig,
    metadata: ClientCrashHookMetadata,
) -> ClientCrashHookInstallStatus {
    if RELEASE_CRASH_HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return ClientCrashHookInstallStatus::AlreadyInstalled;
    }
    std::panic::set_hook(Box::new(move |info| {
        let panic_message = panic_message(info);
        let line = info.location().map(|location| location.line()).unwrap_or(0);
        let _ = spool_minimal_crash_fallback_with_line(&config, metadata, panic_message, &[], line);
    }));
    ClientCrashHookInstallStatus::Installed
}

pub fn spool_minimal_crash_fallback(
    config: &ClientTelemetrySpoolConfig,
    metadata: ClientCrashHookMetadata,
    panic_message: Option<&str>,
    last_static_event_kind_ids: &[StaticEventKindId],
) -> Result<ClientTelemetryUploadState, TelemetryCoreError> {
    spool_minimal_crash_fallback_with_line(
        config,
        metadata,
        panic_message,
        last_static_event_kind_ids,
        0,
    )
}

fn spool_minimal_crash_fallback_with_line(
    config: &ClientTelemetrySpoolConfig,
    metadata: ClientCrashHookMetadata,
    panic_message: Option<&str>,
    last_static_event_kind_ids: &[StaticEventKindId],
    line: u32,
) -> Result<ClientTelemetryUploadState, TelemetryCoreError> {
    let now = unix_now_ms();
    let classification = classify_panic_message(panic_message);
    let signature = crash_signature_digest(metadata, classification.id(), line);
    let input = ClientCrashEvidenceInput {
        created_unix_ms: now,
        session_start_unix_ms: now,
        crash_unix_ms: now,
        build_id: metadata.build_id,
        version_id: metadata.version_id,
        platform_id: metadata.platform_id,
        subsystem: metadata.subsystem,
        crash_signature_digest: signature,
        error_count: 1,
        warn_count: 0,
        info_count: 0,
        debug_count: 0,
        trace_count: 0,
        last_static_event_kind_ids,
        recent_counters: &[],
        recent_gauges: &[],
        panic_message,
        minidump_reference: None,
        minidump_reference_allowed: false,
    };
    let mut pipeline = ClientReleaseTelemetryPipeline::new(config.clone())?;
    pipeline.spool_crash_evidence(&input)
}

fn file_len(path: &Path) -> Result<u64, TelemetryCoreError> {
    Ok(fs::metadata(path)
        .map_err(|source| TelemetryCoreError::Io {
            operation: "reading client telemetry bundle metadata",
            source,
        })?
        .len())
}

fn remove_file_if_exists(path: &Path) -> Result<(), TelemetryCoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(TelemetryCoreError::Io {
            operation: "removing queued client telemetry bundle",
            source,
        }),
    }
}

pub const RELEASE_TELEMETRY_BACKGROUND_STAGES: [ClientTelemetryUploadState; 5] = [
    ClientTelemetryUploadState::Encoding,
    ClientTelemetryUploadState::Compressing,
    ClientTelemetryUploadState::QueuedLocal,
    ClientTelemetryUploadState::Uploading,
    ClientTelemetryUploadState::Accepted,
];

pub const HOT_PATH_IMPORTANCE: TelemetryImportance = TelemetryImportance::Normal;

fn panic_message<'a>(info: &'a PanicHookInfo<'a>) -> Option<&'a str> {
    info.payload()
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
}

fn crash_signature_digest(
    metadata: ClientCrashHookMetadata,
    classification_id: u32,
    line: u32,
) -> [u8; fun_telemetry_core::DIGEST_BYTES_BLAKE3_256] {
    let mut seed = [0_u8; 20];
    seed[0..4].copy_from_slice(&metadata.build_id.to_le_bytes());
    seed[4..8].copy_from_slice(&metadata.version_id.to_le_bytes());
    seed[8..12].copy_from_slice(&metadata.platform_id.to_le_bytes());
    seed[12..16].copy_from_slice(&classification_id.to_le_bytes());
    seed[16..20].copy_from_slice(&line.to_le_bytes());
    digest_bytes(&seed)
}

fn unix_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or(1)
}

fn default_spool_dir() -> PathBuf {
    std::env::temp_dir().join("project-fun-client-telemetry")
}

fn env_u32(name: &str) -> Option<u32> {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse::<u32>().ok())
}

const fn platform_id() -> u32 {
    if cfg!(target_os = "windows") {
        1
    } else if cfg!(target_os = "linux") {
        2
    } else if cfg!(target_os = "macos") {
        3
    } else {
        255
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_telemetry_core::{
        BundleLimits, CaptureMode, CounterSample, RuntimeTelemetrySummary, StaticEventKindId,
        StaticMetricId, TelemetrySubsystem, UNIT_COUNT, digest_bytes,
    };
    use std::sync::atomic::{AtomicU32, Ordering};

    struct FakeUploader {
        response: ClientTelemetryServerResponse,
        calls: AtomicU32,
    }

    impl FakeUploader {
        const fn new(response: ClientTelemetryServerResponse) -> Self {
            Self {
                response,
                calls: AtomicU32::new(0),
            }
        }

        fn calls(&self) -> u32 {
            self.calls.load(Ordering::SeqCst)
        }
    }

    impl ClientTelemetryUploader for FakeUploader {
        fn upload_bundle(&mut self, compressed_frame: &[u8]) -> ClientTelemetryServerResponse {
            assert!(!compressed_frame.is_empty());
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.response
        }
    }

    fn temp_spool(name: &str) -> Result<PathBuf, TelemetryCoreError> {
        let path = std::env::temp_dir().join(format!(
            "fun-client-release-telemetry-{name}-{}",
            std::process::id()
        ));
        match fs::remove_dir_all(&path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(TelemetryCoreError::Io {
                    operation: "resetting test telemetry spool",
                    source,
                });
            }
        }
        fs::create_dir_all(&path).map_err(|source| TelemetryCoreError::Io {
            operation: "creating test telemetry spool",
            source,
        })?;
        Ok(path)
    }

    fn release_snapshot() -> RuntimeTelemetrySnapshot {
        RuntimeTelemetrySnapshot {
            mode: CaptureMode::Summary,
            started_unix_ms: 1_800_000_000_000,
            finished_unix_ms: 1_800_000_000_010,
            summary: RuntimeTelemetrySummary {
                info_count: 1,
                ..RuntimeTelemetrySummary::default()
            },
            counters: vec![CounterSample {
                subsystem: TelemetrySubsystem::FUN_RENDER,
                metric_id: StaticMetricId::new(1),
                value: 120,
                unit_id: UNIT_COUNT,
            }],
            gauges: Vec::new(),
            histograms: Vec::new(),
            top_n: Vec::new(),
            events: Vec::new(),
            string_table: Vec::new(),
            estimated_bytes: 64,
        }
    }

    fn crash_input<'a>(ids: &'a [StaticEventKindId]) -> ClientCrashEvidenceInput<'a> {
        ClientCrashEvidenceInput {
            created_unix_ms: 1_800_000_000_020,
            session_start_unix_ms: 1_800_000_000_000,
            crash_unix_ms: 1_800_000_000_021,
            build_id: 1,
            version_id: 1,
            platform_id: 1,
            subsystem: TelemetrySubsystem::FUN_RENDER,
            crash_signature_digest: digest_bytes(b"game-client-crash"),
            error_count: 1,
            warn_count: 2,
            info_count: 3,
            debug_count: 0,
            trace_count: 0,
            last_static_event_kind_ids: ids,
            recent_counters: &[],
            recent_gauges: &[],
            panic_message: Some("panic contained raw request body"),
            minidump_reference: None,
            minidump_reference_allowed: false,
        }
    }

    #[test]
    fn normal_release_session_emits_compressed_bundle() {
        let spool = temp_spool("normal-session").expect("spool");
        let mut pipeline =
            ClientReleaseTelemetryPipeline::new(ClientTelemetrySpoolConfig::release_default(spool))
                .expect("pipeline");
        let snapshot = release_snapshot();
        let state = pipeline
            .spool_release_session_snapshot(&snapshot, RuntimeBundleMetadata::default())
            .expect("spooled");
        assert_eq!(state, ClientTelemetryUploadState::QueuedLocal);
        let artifact = pipeline.queued_artifacts().next().expect("artifact");
        let bundle =
            ClientReleaseTelemetryPipeline::read_queued_bundle(artifact).expect("read bundle");
        assert!(
            bundle
                .header
                .as_ref()
                .expect("header")
                .compression
                .is_some()
        );
    }

    #[test]
    fn crash_writes_compressed_evidence_bundle() {
        let spool = temp_spool("crash-evidence").expect("spool");
        let mut pipeline =
            ClientReleaseTelemetryPipeline::new(ClientTelemetrySpoolConfig::release_default(spool))
                .expect("pipeline");
        let ids = [StaticEventKindId::new(7)];
        let state = pipeline
            .spool_crash_evidence(&crash_input(&ids))
            .expect("crash spooled");
        assert_eq!(state, ClientTelemetryUploadState::QueuedLocal);
        let artifact = pipeline.queued_artifacts().next().expect("artifact");
        let bundle =
            ClientReleaseTelemetryPipeline::read_queued_bundle(artifact).expect("read bundle");
        let header = bundle.header.as_ref().expect("header");
        assert_eq!(
            header.artifact_kind,
            fun_telemetry_core::enum_values::ARTIFACT_KIND_CLIENT_CRASH_EVIDENCE
        );
        let encoded = fun_telemetry_core::encode_retained_bundle(&bundle, BundleLimits::default())
            .expect("encoded retained");
        assert!(!encoded.is_empty());
    }

    #[test]
    fn restart_uploads_spooled_crash_evidence() {
        let spool = temp_spool("restart-upload").expect("spool");
        let ids = [StaticEventKindId::new(8)];
        {
            let mut pipeline = ClientReleaseTelemetryPipeline::new(
                ClientTelemetrySpoolConfig::release_default(spool.clone()),
            )
            .expect("pipeline");
            pipeline
                .spool_crash_evidence(&crash_input(&ids))
                .expect("crash spooled");
        }

        let mut restarted =
            ClientReleaseTelemetryPipeline::new(ClientTelemetrySpoolConfig::release_default(spool))
                .expect("restarted");
        let mut uploader = FakeUploader::new(ClientTelemetryServerResponse::Accepted);
        let summary = restarted.upload_pending(&mut uploader).expect("uploaded");
        assert_eq!(summary.accepted, 1);
        assert_eq!(uploader.calls(), 1);
        assert_eq!(restarted.queue_len(), 0);
    }

    #[test]
    fn server_unavailable_keeps_spool_for_retry() {
        let spool = temp_spool("retry").expect("spool");
        let ids = [StaticEventKindId::new(9)];
        let mut pipeline =
            ClientReleaseTelemetryPipeline::new(ClientTelemetrySpoolConfig::release_default(spool))
                .expect("pipeline");
        pipeline
            .spool_crash_evidence(&crash_input(&ids))
            .expect("crash spooled");
        let mut uploader = FakeUploader::new(ClientTelemetryServerResponse::RetryableFailure);
        let summary = pipeline.upload_pending(&mut uploader).expect("retryable");
        assert_eq!(summary.retryable_failures, 1);
        assert_eq!(pipeline.queue_len(), 1);
        assert_eq!(
            pipeline.state(),
            ClientTelemetryUploadState::RetryableFailure
        );
    }

    #[test]
    fn spool_overflow_prunes_routine_before_crash_evidence() {
        let spool = temp_spool("overflow").expect("spool");
        let mut config = ClientTelemetrySpoolConfig::release_default(spool);
        config.max_queue_len = 1;
        let mut pipeline = ClientReleaseTelemetryPipeline::new(config).expect("pipeline");
        pipeline
            .spool_release_session_snapshot(&release_snapshot(), RuntimeBundleMetadata::default())
            .expect("routine spooled");
        let ids = [StaticEventKindId::new(10)];
        pipeline
            .spool_crash_evidence(&crash_input(&ids))
            .expect("crash spooled");
        assert_eq!(pipeline.queue_len(), 1);
        let artifact = pipeline.queued_artifacts().next().expect("artifact");
        let bundle =
            ClientReleaseTelemetryPipeline::read_queued_bundle(artifact).expect("read bundle");
        assert_eq!(
            bundle.header.as_ref().expect("header").artifact_kind,
            fun_telemetry_core::enum_values::ARTIFACT_KIND_CLIENT_CRASH_EVIDENCE
        );
    }

    #[test]
    fn upload_is_not_called_while_spooling_hot_path_snapshot() {
        let spool = temp_spool("hot-path").expect("spool");
        let mut pipeline =
            ClientReleaseTelemetryPipeline::new(ClientTelemetrySpoolConfig::release_default(spool))
                .expect("pipeline");
        let uploader = FakeUploader::new(ClientTelemetryServerResponse::Accepted);
        pipeline
            .spool_release_session_snapshot(&release_snapshot(), RuntimeBundleMetadata::default())
            .expect("session spooled");
        assert_eq!(uploader.calls(), 0);
        assert_eq!(HOT_PATH_IMPORTANCE, TelemetryImportance::Normal);
    }

    #[test]
    fn raw_panic_text_is_redacted_before_spool() {
        let spool = temp_spool("panic-redaction").expect("spool");
        let mut pipeline =
            ClientReleaseTelemetryPipeline::new(ClientTelemetrySpoolConfig::release_default(spool))
                .expect("pipeline");
        let ids = [StaticEventKindId::new(11)];
        pipeline
            .spool_crash_evidence(&crash_input(&ids))
            .expect("crash spooled");
        let artifact = pipeline.queued_artifacts().next().expect("artifact");
        let bundle =
            ClientReleaseTelemetryPipeline::read_queued_bundle(artifact).expect("read bundle");
        let header = bundle.header.as_ref().expect("header");
        assert!(
            !header
                .string_table
                .iter()
                .any(|entry| entry.contains("request body"))
        );
    }

    #[test]
    fn minimal_crash_fallback_survives_restart() {
        let spool = temp_spool("minimal-fallback").expect("spool");
        let config = ClientTelemetrySpoolConfig::release_default(spool.clone());
        let metadata = ClientCrashHookMetadata {
            build_id: 11,
            version_id: 12,
            platform_id: 13,
            subsystem: TelemetrySubsystem::FUN_RENDER,
        };
        let state = spool_minimal_crash_fallback(
            &config,
            metadata,
            Some("panic included password value"),
            &[StaticEventKindId::new(77)],
        )
        .expect("fallback spooled");
        assert_eq!(state, ClientTelemetryUploadState::QueuedLocal);

        let mut restarted =
            ClientReleaseTelemetryPipeline::new(ClientTelemetrySpoolConfig::release_default(spool))
                .expect("restarted");
        assert_eq!(restarted.load_spooled_artifacts().expect("loaded"), 1);
        let artifact = restarted.queued_artifacts().next().expect("artifact");
        let bundle =
            ClientReleaseTelemetryPipeline::read_queued_bundle(artifact).expect("read bundle");
        let header = bundle.header.as_ref().expect("header");
        assert!(
            !header
                .string_table
                .iter()
                .any(|entry| entry.contains("password value"))
        );
    }
}
