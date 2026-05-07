#![allow(dead_code)]

#[cfg(all(feature = "release-telemetry", feature = "debug-diagnostics"))]
compile_error!("game_server release telemetry cannot be built with debug-diagnostics");
#[cfg(all(feature = "release-telemetry", feature = "debug-benchmarks"))]
compile_error!("game_server release telemetry cannot be built with debug-benchmarks");
#[cfg(all(feature = "release-telemetry", feature = "local-debug-captures"))]
compile_error!("game_server release telemetry cannot be built with local-debug-captures");
#[cfg(all(feature = "release-telemetry", feature = "diagnostics"))]
compile_error!("game_server release telemetry cannot be built with diagnostics");
#[cfg(all(feature = "release-telemetry", feature = "telemetry-rendered-views"))]
compile_error!("game_server release telemetry cannot be built with telemetry-rendered-views");
#[cfg(all(not(debug_assertions), feature = "debug-diagnostics"))]
compile_error!("game_server debug-diagnostics requires debug assertions");
#[cfg(all(not(debug_assertions), feature = "debug-benchmarks"))]
compile_error!("game_server debug-benchmarks requires debug assertions");
#[cfg(all(not(debug_assertions), feature = "local-debug-captures"))]
compile_error!("game_server local-debug-captures requires debug assertions");
#[cfg(all(feature = "release-telemetry", not(feature = "server-ingest")))]
compile_error!("game_server release telemetry requires server-ingest");
#[cfg(all(feature = "release-telemetry", not(feature = "telemetry-retention")))]
compile_error!("game_server release telemetry requires telemetry-retention");

#[cfg(feature = "server-ingest")]
use bevy::prelude::Resource;

#[cfg(feature = "server-ingest")]
pub const SERVER_INGEST_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "telemetry-retention")]
pub const SERVER_RETENTION_METADATA_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "debug-diagnostics")]
pub const SERVER_DEBUG_DIAGNOSTICS_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "debug-benchmarks")]
pub const SERVER_DEBUG_BENCHMARKS_SYMBOL_PRESENT: bool = true;
#[cfg(feature = "local-debug-captures")]
pub const SERVER_LOCAL_DEBUG_CAPTURES_SYMBOL_PRESENT: bool = true;

pub const SERVER_RELEASE_TELEMETRY_FEATURE_PRESENT: bool = cfg!(feature = "release-telemetry");
pub const SERVER_DEBUG_ONLY_ROUTES_PRESENT: bool = cfg!(any(
    feature = "debug-diagnostics",
    feature = "debug-benchmarks",
    feature = "local-debug-captures",
    feature = "diagnostics"
));
pub const SERVER_DEBUG_ONLY_ROUTES_ABSENT_IN_RELEASE: bool =
    cfg!(feature = "release-telemetry") && !SERVER_DEBUG_ONLY_ROUTES_PRESENT;

#[cfg(feature = "server-ingest")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Resource)]
pub struct ServerTelemetryFeatureContract {
    pub profile: ServerTelemetryProfileMarker,
    pub ingest: bool,
    pub compressed_bundle_reader: bool,
    pub retention_metadata: bool,
    pub rendered_views: bool,
    pub debug_only_routes: bool,
    pub source_project: i32,
    pub artifact_kind: i32,
    pub budget_class: i32,
    pub retention_class: i32,
    pub redaction_class: i32,
    pub trust_boundary: i32,
    pub max_bundle_bytes: u32,
}

#[cfg(feature = "server-ingest")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerTelemetryProfileMarker {
    Release,
    Debug,
}

#[cfg(feature = "server-ingest")]
pub fn server_telemetry_contract() -> ServerTelemetryFeatureContract {
    let metadata =
        fun_telemetry_core::metadata_for_subsystem(fun_telemetry_core::TelemetrySubsystem::FUN_NET)
            .unwrap_or_default();
    ServerTelemetryFeatureContract {
        profile: if cfg!(feature = "release-telemetry") {
            ServerTelemetryProfileMarker::Release
        } else {
            ServerTelemetryProfileMarker::Debug
        },
        ingest: true,
        compressed_bundle_reader: true,
        retention_metadata: cfg!(feature = "telemetry-retention"),
        rendered_views: cfg!(feature = "telemetry-rendered-views"),
        debug_only_routes: SERVER_DEBUG_ONLY_ROUTES_PRESENT,
        source_project: metadata.source_project,
        artifact_kind: metadata.artifact_kind,
        budget_class: if cfg!(feature = "release-telemetry") {
            fun_telemetry_core::enum_values::BUDGET_CLASS_HOT_PATH_COUNTERS
        } else {
            fun_telemetry_core::enum_values::BUDGET_CLASS_TARGETED_TRACE
        },
        retention_class: metadata.retention_class,
        redaction_class: metadata.redaction_class,
        trust_boundary: metadata.trust_boundary,
        max_bundle_bytes: metadata.max_bundle_bytes,
    }
}

#[cfg(feature = "server-ingest")]
pub fn server_ingest_frame_options() -> fun_telemetry_core::FrameReadOptions {
    fun_telemetry_core::FrameReadOptions::default()
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "release-telemetry")]
    #[test]
    fn server_release_symbols_present() {
        use super::*;

        assert!(SERVER_RELEASE_TELEMETRY_FEATURE_PRESENT);
        assert!(SERVER_INGEST_SYMBOL_PRESENT);
        assert!(SERVER_RETENTION_METADATA_SYMBOL_PRESENT);
        assert!(SERVER_DEBUG_ONLY_ROUTES_ABSENT_IN_RELEASE);

        let contract = server_telemetry_contract();
        assert_eq!(contract.profile, ServerTelemetryProfileMarker::Release);
        assert!(contract.ingest);
        assert!(contract.compressed_bundle_reader);
        assert!(contract.retention_metadata);
        assert!(!contract.rendered_views);
        assert!(!contract.debug_only_routes);
        assert_ne!(contract.budget_class, 0);
        assert_ne!(contract.retention_class, 0);
        assert_ne!(contract.redaction_class, 0);
        assert_ne!(contract.trust_boundary, 0);

        let frame_options = server_ingest_frame_options();
        assert!(!frame_options.compatibility_mode);
    }

    #[cfg(all(feature = "debug-diagnostics", feature = "local-debug-captures"))]
    #[test]
    fn server_debug_symbols_present() {
        use super::*;

        assert!(!SERVER_RELEASE_TELEMETRY_FEATURE_PRESENT);
        assert!(SERVER_INGEST_SYMBOL_PRESENT);
        assert!(SERVER_DEBUG_DIAGNOSTICS_SYMBOL_PRESENT);
        assert!(SERVER_LOCAL_DEBUG_CAPTURES_SYMBOL_PRESENT);

        let contract = server_telemetry_contract();
        assert_eq!(contract.profile, ServerTelemetryProfileMarker::Debug);
        assert!(contract.ingest);
        assert!(contract.compressed_bundle_reader);
        assert!(contract.rendered_views);
        assert!(contract.debug_only_routes);
        assert_ne!(contract.budget_class, 0);
        assert_ne!(contract.max_bundle_bytes, 0);
    }
}
