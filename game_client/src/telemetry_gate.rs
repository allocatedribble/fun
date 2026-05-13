#![allow(dead_code)]

#[cfg(all(not(debug_assertions), feature = "debug-diagnostics"))]
compile_error!("game_client debug-diagnostics requires debug assertions");
#[cfg(all(not(debug_assertions), feature = "debug-benchmarks"))]
compile_error!("game_client debug-benchmarks requires debug assertions");
#[cfg(all(not(debug_assertions), feature = "local-debug-captures"))]
compile_error!("game_client local-debug-captures requires debug assertions");
#[cfg(all(feature = "release-telemetry", not(feature = "client-telemetry")))]
compile_error!("game_client release telemetry requires client-telemetry");
#[cfg(all(feature = "release-telemetry", not(feature = "crash-telemetry")))]
compile_error!("game_client release telemetry requires crash-telemetry");
#[cfg(all(feature = "release-telemetry", not(feature = "telemetry-upload")))]
compile_error!("game_client release telemetry requires telemetry-upload");
#[cfg(all(feature = "release-telemetry", not(feature = "telemetry-retention")))]
compile_error!("game_client release telemetry requires telemetry-retention");

#[cfg(feature = "client-telemetry")]
use fun_ecs::Resource;

#[cfg(feature = "client-telemetry")]
pub const CLIENT_TELEMETRY_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "crash-telemetry")]
pub const CRASH_UPLOADER_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "telemetry-upload")]
pub const TELEMETRY_UPLOAD_SPOOL_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "telemetry-retention")]
pub const TELEMETRY_RETENTION_METADATA_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "debug-diagnostics")]
pub const DEBUG_DIAGNOSTICS_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "debug-benchmarks")]
pub const DEBUG_BENCHMARKS_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "local-debug-captures")]
pub const LOCAL_DEBUG_CAPTURES_SYMBOL_PRESENT: bool = true;

pub const RELEASE_TELEMETRY_FEATURE_PRESENT: bool = cfg!(feature = "release-telemetry");
pub const DEBUG_DIAGNOSTICS_FEATURE_PRESENT: bool = cfg!(feature = "debug-diagnostics");
pub const DEBUG_BENCHMARKS_FEATURE_PRESENT: bool = cfg!(feature = "debug-benchmarks");
pub const LOCAL_DEBUG_CAPTURES_FEATURE_PRESENT: bool = cfg!(feature = "local-debug-captures");

const RELEASE_TELEMETRY_ACTIVE: bool = cfg!(feature = "release-telemetry");
const DEBUG_DIAGNOSTICS_ACTIVE: bool = cfg!(all(
    feature = "debug-diagnostics",
    not(feature = "release-telemetry")
));
const DEBUG_BENCHMARKS_ACTIVE: bool = cfg!(all(
    feature = "debug-benchmarks",
    not(feature = "release-telemetry")
));
const LOCAL_DEBUG_CAPTURES_ACTIVE: bool = cfg!(all(
    feature = "local-debug-captures",
    not(feature = "release-telemetry")
));
const TELEMETRY_RENDERED_VIEWS_ACTIVE: bool = cfg!(all(
    feature = "telemetry-rendered-views",
    not(feature = "release-telemetry")
));

#[cfg(feature = "client-telemetry")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Resource)]
pub struct ClientTelemetryFeatureContract {
    pub profile: TelemetryProfileMarker,
    pub compact_tracing_capture: bool,
    pub compressed_bundle_writer: bool,
    pub upload_or_spool: bool,
    pub crash_capture: bool,
    pub retention_metadata: bool,
    pub rendered_views: bool,
    pub debug_diagnostics: bool,
    pub debug_benchmarks: bool,
    pub local_debug_captures: bool,
    pub source_project: i32,
    pub artifact_kind: i32,
    pub budget_class: i32,
    pub retention_class: i32,
    pub redaction_class: i32,
    pub trust_boundary: i32,
    pub max_bundle_bytes: u32,
}

#[cfg(feature = "crash-telemetry")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CrashTelemetryUploaderContract {
    pub durable_after_restart: bool,
    pub compressed_bundle_writer: bool,
    pub retention_metadata: bool,
    pub redaction_class: i32,
    pub trust_boundary: i32,
}

#[cfg(feature = "client-telemetry")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TelemetryProfileMarker {
    Release,
    Debug,
}

#[cfg(feature = "client-telemetry")]
pub fn client_telemetry_contract() -> ClientTelemetryFeatureContract {
    let metadata = fun_telemetry_core::metadata_for_subsystem(
        fun_telemetry_core::TelemetrySubsystem::FUN_CRASH_HANDLER,
    )
    .unwrap_or_default();
    ClientTelemetryFeatureContract {
        profile: if RELEASE_TELEMETRY_ACTIVE {
            TelemetryProfileMarker::Release
        } else {
            TelemetryProfileMarker::Debug
        },
        compact_tracing_capture: true,
        compressed_bundle_writer: true,
        upload_or_spool: cfg!(feature = "telemetry-upload"),
        crash_capture: cfg!(feature = "crash-telemetry"),
        retention_metadata: cfg!(feature = "telemetry-retention"),
        rendered_views: TELEMETRY_RENDERED_VIEWS_ACTIVE,
        debug_diagnostics: DEBUG_DIAGNOSTICS_ACTIVE,
        debug_benchmarks: DEBUG_BENCHMARKS_ACTIVE,
        local_debug_captures: LOCAL_DEBUG_CAPTURES_ACTIVE,
        source_project: metadata.source_project,
        artifact_kind: metadata.artifact_kind,
        budget_class: if RELEASE_TELEMETRY_ACTIVE {
            fun_telemetry_core::enum_values::BUDGET_CLASS_SAMPLED_RUNTIME
        } else {
            fun_telemetry_core::enum_values::BUDGET_CLASS_TARGETED_TRACE
        },
        retention_class: metadata.retention_class,
        redaction_class: metadata.redaction_class,
        trust_boundary: metadata.trust_boundary,
        max_bundle_bytes: metadata.max_bundle_bytes,
    }
}

#[cfg(feature = "client-telemetry")]
pub fn canonical_runtime_writer(
    started_unix_ms: u64,
) -> fun_telemetry_core::ProjectTelemetryFunnel {
    if RELEASE_TELEMETRY_ACTIVE {
        fun_telemetry_core::ProjectTelemetryFunnel::release(started_unix_ms)
    } else {
        fun_telemetry_core::ProjectTelemetryFunnel::debug_local(started_unix_ms)
    }
}

#[cfg(feature = "client-telemetry")]
pub fn canonical_frame_options() -> fun_telemetry_core::FrameWriteOptions {
    fun_telemetry_core::FrameWriteOptions::default()
}

#[cfg(feature = "crash-telemetry")]
pub fn crash_uploader_contract() -> CrashTelemetryUploaderContract {
    let metadata = fun_telemetry_core::metadata_for_subsystem(
        fun_telemetry_core::TelemetrySubsystem::FUN_CRASH_HANDLER,
    )
    .unwrap_or_default();
    CrashTelemetryUploaderContract {
        durable_after_restart: true,
        compressed_bundle_writer: true,
        retention_metadata: cfg!(feature = "telemetry-retention"),
        redaction_class: metadata.redaction_class,
        trust_boundary: metadata.trust_boundary,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "release-telemetry")]
    #[test]
    fn release_client_symbols_present() {
        use super::*;

        assert!(std::hint::black_box(RELEASE_TELEMETRY_FEATURE_PRESENT));
        assert!(std::hint::black_box(CLIENT_TELEMETRY_SYMBOL_PRESENT));
        assert!(std::hint::black_box(CRASH_UPLOADER_SYMBOL_PRESENT));
        assert!(std::hint::black_box(TELEMETRY_UPLOAD_SPOOL_SYMBOL_PRESENT));
        assert!(std::hint::black_box(
            TELEMETRY_RETENTION_METADATA_SYMBOL_PRESENT
        ));

        let contract = client_telemetry_contract();
        assert_eq!(contract.profile, TelemetryProfileMarker::Release);
        assert!(contract.compact_tracing_capture);
        assert!(contract.compressed_bundle_writer);
        assert!(contract.upload_or_spool);
        assert!(contract.crash_capture);
        assert!(contract.retention_metadata);
        assert!(!contract.rendered_views);
        assert!(!contract.debug_diagnostics);
        assert!(!contract.debug_benchmarks);
        assert!(!contract.local_debug_captures);
        assert_ne!(contract.budget_class, 0);
        assert_ne!(contract.retention_class, 0);
        assert_ne!(contract.redaction_class, 0);
        assert_ne!(contract.trust_boundary, 0);

        let frame_options = canonical_frame_options();
        assert_eq!(
            frame_options.compression_profile,
            fun_telemetry_core::CompressionProfile::Normal
        );

        let funnel = canonical_runtime_writer(1_800_000_000_000);
        assert_eq!(
            funnel.config().profile,
            fun_telemetry_core::TelemetryBuildProfile::Release
        );

        let crash_contract = crash_uploader_contract();
        assert!(crash_contract.durable_after_restart);
        assert!(crash_contract.compressed_bundle_writer);
        assert!(crash_contract.retention_metadata);
    }

    #[cfg(all(
        feature = "debug-diagnostics",
        feature = "debug-benchmarks",
        feature = "local-debug-captures",
        not(feature = "release-telemetry")
    ))]
    #[test]
    fn debug_client_symbols_present() {
        use super::*;

        assert!(!std::hint::black_box(RELEASE_TELEMETRY_FEATURE_PRESENT));
        assert!(std::hint::black_box(CLIENT_TELEMETRY_SYMBOL_PRESENT));
        assert!(std::hint::black_box(DEBUG_DIAGNOSTICS_SYMBOL_PRESENT));
        assert!(std::hint::black_box(DEBUG_BENCHMARKS_SYMBOL_PRESENT));
        assert!(std::hint::black_box(LOCAL_DEBUG_CAPTURES_SYMBOL_PRESENT));
        assert!(std::hint::black_box(
            TELEMETRY_RETENTION_METADATA_SYMBOL_PRESENT
        ));

        let contract = client_telemetry_contract();
        assert_eq!(contract.profile, TelemetryProfileMarker::Debug);
        assert!(contract.compact_tracing_capture);
        assert!(contract.compressed_bundle_writer);
        assert!(!contract.upload_or_spool);
        assert!(!contract.crash_capture);
        assert!(contract.retention_metadata);
        assert!(contract.rendered_views);
        assert!(contract.debug_diagnostics);
        assert!(contract.debug_benchmarks);
        assert!(contract.local_debug_captures);
        assert_eq!(
            contract.budget_class,
            fun_telemetry_core::enum_values::BUDGET_CLASS_TARGETED_TRACE
        );
        assert_ne!(contract.max_bundle_bytes, 0);

        let funnel = canonical_runtime_writer(1_800_000_000_000);
        assert_eq!(
            funnel.config().profile,
            fun_telemetry_core::TelemetryBuildProfile::Debug
        );
    }
}
