use std::{
    fmt::Write as _,
    fs, io,
    path::{Path, PathBuf},
};

use fun_telemetry_core::{
    BundleHeaderInput, CompressionProfile, FrameReadOptions, FrameWriteOptions,
    TelemetryBundleBuilder, digest_bytes, enum_values, read_bundle_from_path,
    telemetry_v1 as telemetry, write_bundle_to_path,
};
use serde::Serialize;

use crate::settings::{
    FrameGenerationSetting, GraphicsBackendSetting, RendererBenchmarkReproSettings,
    RendererCapabilityFacts, RendererRuntimeMode, RendererSettingsRequest,
    RendererSettingsSelection, resolve_renderer_settings,
};

pub const RENDERER_BENCHMARK_SCHEMA: &str = "fun.renderer.benchmark.v1";
pub const RENDERER_BENCHMARK_SCHEMA_VERSION: u16 = 1;
pub const RENDERER_BENCHMARK_CANONICAL_SCHEMA: &str = "fun.renderer.benchmark.summary.v1";
pub const RENDERER_BENCHMARK_CANONICAL_EXTENSION: &str = ".funpb.sum.zst";
pub const RENDERER_BENCHMARK_ARTIFACT_ENV: &str = "FUN_RENDERER_BENCHMARK_ARTIFACT";
pub const RENDERER_BENCHMARK_SCENE_COUNT: usize = 14;
pub const RENDERER_PERF_GATE_COUNT: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkSceneKind {
    ClearPresent,
    StaticScene,
    NativeUiComposition,
    Dx12VulkanParity,
    UploadStress,
    PipelineWarmupHotLoop,
    VirtualGeometryStress,
    DynamicGeometryStress,
    ProceduralInvalidation,
    ManyLightStress,
    VirtualShadowStress,
    GiReflectionScene,
    UpscalingScene,
    FgEligibilityPacing,
}

impl BenchmarkSceneKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ClearPresent => "clear_present",
            Self::StaticScene => "static_scene",
            Self::NativeUiComposition => "native_ui_composition",
            Self::Dx12VulkanParity => "dx12_vulkan_parity",
            Self::UploadStress => "upload_stress",
            Self::PipelineWarmupHotLoop => "pipeline_warmup_hot_loop",
            Self::VirtualGeometryStress => "virtual_geometry_stress",
            Self::DynamicGeometryStress => "dynamic_geometry_stress",
            Self::ProceduralInvalidation => "procedural_invalidation",
            Self::ManyLightStress => "many_light_stress",
            Self::VirtualShadowStress => "virtual_shadow_stress",
            Self::GiReflectionScene => "gi_reflection_scene",
            Self::UpscalingScene => "upscaling_scene",
            Self::FgEligibilityPacing => "fg_eligibility_pacing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BenchmarkSceneDescriptor {
    pub stable_id: &'static str,
    pub kind: BenchmarkSceneKind,
    pub display_name: &'static str,
    pub requires_native_ui_gpu_transport: bool,
    pub requires_backend_truth: bool,
    pub requires_upscaler_boundary: bool,
    pub requires_frame_generation_boundary: bool,
    pub stress_scene: bool,
    pub p95_p99_gate_required: bool,
}

impl BenchmarkSceneDescriptor {
    #[must_use]
    pub const fn new(
        stable_id: &'static str,
        kind: BenchmarkSceneKind,
        display_name: &'static str,
    ) -> Self {
        Self {
            stable_id,
            kind,
            display_name,
            requires_native_ui_gpu_transport: matches!(
                kind,
                BenchmarkSceneKind::NativeUiComposition
            ),
            requires_backend_truth: matches!(kind, BenchmarkSceneKind::Dx12VulkanParity),
            requires_upscaler_boundary: matches!(
                kind,
                BenchmarkSceneKind::UpscalingScene | BenchmarkSceneKind::FgEligibilityPacing
            ),
            requires_frame_generation_boundary: matches!(
                kind,
                BenchmarkSceneKind::FgEligibilityPacing
            ),
            stress_scene: matches!(
                kind,
                BenchmarkSceneKind::UploadStress
                    | BenchmarkSceneKind::PipelineWarmupHotLoop
                    | BenchmarkSceneKind::VirtualGeometryStress
                    | BenchmarkSceneKind::DynamicGeometryStress
                    | BenchmarkSceneKind::ProceduralInvalidation
                    | BenchmarkSceneKind::ManyLightStress
                    | BenchmarkSceneKind::VirtualShadowStress
                    | BenchmarkSceneKind::GiReflectionScene
            ),
            p95_p99_gate_required: true,
        }
    }
}

pub const RENDERER_BENCHMARK_SCENES: [BenchmarkSceneDescriptor; RENDERER_BENCHMARK_SCENE_COUNT] = [
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.clear_present",
        BenchmarkSceneKind::ClearPresent,
        "minimal clear/present",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.static_scene",
        BenchmarkSceneKind::StaticScene,
        "static scene",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.native_ui_composition",
        BenchmarkSceneKind::NativeUiComposition,
        "NATIVE_UI UI composition",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.dx12_vulkan_parity",
        BenchmarkSceneKind::Dx12VulkanParity,
        "DX12/Vulkan parity",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.upload_stress",
        BenchmarkSceneKind::UploadStress,
        "upload stress",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.pipeline_warmup_hot_loop",
        BenchmarkSceneKind::PipelineWarmupHotLoop,
        "pipeline warmup/hot-loop",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.virtual_geometry_stress",
        BenchmarkSceneKind::VirtualGeometryStress,
        "virtual geometry stress",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.dynamic_geometry_stress",
        BenchmarkSceneKind::DynamicGeometryStress,
        "dynamic geometry stress",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.procedural_invalidation",
        BenchmarkSceneKind::ProceduralInvalidation,
        "procedural invalidation",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.many_light_stress",
        BenchmarkSceneKind::ManyLightStress,
        "many-light stress",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.virtual_shadow_stress",
        BenchmarkSceneKind::VirtualShadowStress,
        "virtual shadow stress",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.gi_reflection_scene",
        BenchmarkSceneKind::GiReflectionScene,
        "GI/reflection scene",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.upscaling_scene",
        BenchmarkSceneKind::UpscalingScene,
        "upscaling scene",
    ),
    BenchmarkSceneDescriptor::new(
        "renderer.benchmark.scene.fg_eligibility_pacing",
        BenchmarkSceneKind::FgEligibilityPacing,
        "FG eligibility/pacing scene",
    ),
];

#[must_use]
pub fn renderer_benchmark_scene(stable_id: &str) -> Option<&'static BenchmarkSceneDescriptor> {
    RENDERER_BENCHMARK_SCENES
        .iter()
        .find(|scene| scene.stable_id == stable_id)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct FrameTimeMetrics {
    pub cpu_frame_time_p50_us: u32,
    pub cpu_frame_time_p95_us: u32,
    pub cpu_frame_time_p99_us: u32,
    pub gpu_frame_time_p50_us: u32,
    pub gpu_frame_time_p95_us: u32,
    pub gpu_frame_time_p99_us: u32,
}

impl FrameTimeMetrics {
    #[must_use]
    pub const fn stable_144hz_sample() -> Self {
        Self {
            cpu_frame_time_p50_us: 4_800,
            cpu_frame_time_p95_us: 6_300,
            cpu_frame_time_p99_us: 6_700,
            gpu_frame_time_p50_us: 4_100,
            gpu_frame_time_p95_us: 6_000,
            gpu_frame_time_p99_us: 6_500,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct PassTimingSample {
    pub pass_label: &'static str,
    pub category: &'static str,
    pub p50_us: u32,
    pub p95_us: u32,
    pub p99_us: u32,
}

impl PassTimingSample {
    #[must_use]
    pub const fn new(
        pass_label: &'static str,
        category: &'static str,
        p50_us: u32,
        p95_us: u32,
        p99_us: u32,
    ) -> Self {
        Self {
            pass_label,
            category,
            p50_us,
            p95_us,
            p99_us,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererBenchmarkMetrics {
    pub frame_time: FrameTimeMetrics,
    pub pass_timings: Vec<PassTimingSample>,
    pub upload_bytes: u64,
    pub allocation_count: u32,
    pub runtime_pipeline_creation_count: u32,
    pub page_faults: u32,
    pub evictions: u32,
    pub visible_cluster_count: u32,
    pub drawn_cluster_count: u32,
    pub light_count: u32,
    pub candidate_count: u32,
    pub shadow_pages_refreshed: u32,
    pub gi_cache_occupancy: u32,
    pub native_ui_import_latency_us: u32,
    pub native_ui_composite_latency_us: u32,
    pub native_ui_cpu_fallback_attempts: u32,
    pub upscaler_time_us: u32,
    pub fg_generated_count: u32,
    pub fg_presented_count: u32,
    pub product_bevy_ui_dependency_detected: bool,
    pub fallback_reasons: Vec<&'static str>,
}

impl RendererBenchmarkMetrics {
    #[must_use]
    pub fn clean_smoke() -> Self {
        Self {
            frame_time: FrameTimeMetrics::stable_144hz_sample(),
            pass_timings: vec![
                PassTimingSample::new("fun_renderer.pass.clear", "clear", 40, 50, 55),
                PassTimingSample::new("fun_renderer.pass.compose", "ui_composite", 70, 90, 100),
            ],
            upload_bytes: 0,
            allocation_count: 0,
            runtime_pipeline_creation_count: 0,
            page_faults: 0,
            evictions: 0,
            visible_cluster_count: 0,
            drawn_cluster_count: 0,
            light_count: 0,
            candidate_count: 0,
            shadow_pages_refreshed: 0,
            gi_cache_occupancy: 0,
            native_ui_import_latency_us: 0,
            native_ui_composite_latency_us: 0,
            native_ui_cpu_fallback_attempts: 0,
            upscaler_time_us: 0,
            fg_generated_count: 0,
            fg_presented_count: 0,
            product_bevy_ui_dependency_detected: false,
            fallback_reasons: Vec::new(),
        }
    }

    #[must_use]
    pub fn with_page_pressure(mut self, page_faults: u32, evictions: u32) -> Self {
        self.page_faults = page_faults;
        self.evictions = evictions;
        self
    }
}

impl Default for RendererBenchmarkMetrics {
    fn default() -> Self {
        Self::clean_smoke()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererBenchmarkGitRevisions {
    pub root_revision: String,
    pub fun_revision: String,
    pub bevy_revision: String,
}

impl RendererBenchmarkGitRevisions {
    #[must_use]
    pub fn unknown() -> Self {
        Self {
            root_revision: String::from("unknown"),
            fun_revision: String::from("unknown"),
            bevy_revision: String::from("unknown"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererBenchmarkCaptureRefs {
    pub screenshot_path: Option<String>,
    pub gpu_capture_path: Option<String>,
}

impl RendererBenchmarkCaptureRefs {
    #[must_use]
    pub const fn none() -> Self {
        Self {
            screenshot_path: None,
            gpu_capture_path: None,
        }
    }
}

impl Default for RendererBenchmarkCaptureRefs {
    fn default() -> Self {
        Self::none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererPerformanceClaim {
    None,
    Improved,
    Regressed,
    MovedComplexity,
}

impl RendererPerformanceClaim {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Improved => "improved",
            Self::Regressed => "regressed",
            Self::MovedComplexity => "moved_complexity",
        }
    }

    #[must_use]
    pub const fn requires_artifact(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererBenchmarkEvidence {
    pub performance_claim: RendererPerformanceClaim,
    pub artifact_present: bool,
}

impl RendererBenchmarkEvidence {
    #[must_use]
    pub const fn no_claim() -> Self {
        Self {
            performance_claim: RendererPerformanceClaim::None,
            artifact_present: false,
        }
    }

    #[must_use]
    pub const fn claim_with_artifact(claim: RendererPerformanceClaim) -> Self {
        Self {
            performance_claim: claim,
            artifact_present: true,
        }
    }
}

impl Default for RendererBenchmarkEvidence {
    fn default() -> Self {
        Self::no_claim()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererBenchmarkRunIdentity {
    pub git_revisions: RendererBenchmarkGitRevisions,
    pub feature_flags: crate::settings::RendererCompiledFeatureSupport,
    pub scene_id: &'static str,
    pub captures: RendererBenchmarkCaptureRefs,
}

impl RendererBenchmarkRunIdentity {
    #[must_use]
    pub fn unknown(scene: &'static BenchmarkSceneDescriptor) -> Self {
        Self {
            git_revisions: RendererBenchmarkGitRevisions::unknown(),
            feature_flags: crate::settings::RendererCompiledFeatureSupport::COMPILED,
            scene_id: scene.stable_id,
            captures: RendererBenchmarkCaptureRefs::none(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RendererPerfGateConfig {
    pub runtime_pipeline_creation_allowance: u32,
    pub page_fault_storm_limit: u32,
}

impl RendererPerfGateConfig {
    pub const STRICT: Self = Self {
        runtime_pipeline_creation_allowance: 0,
        page_fault_storm_limit: 512,
    };
}

impl Default for RendererPerfGateConfig {
    fn default() -> Self {
        Self::STRICT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RendererPerfGateKind {
    NoUnexpectedRuntimePipelineCreation,
    NoSilentBackendFallback,
    NoProductCpuNativeUiFallback,
    NoUnboundedPageFaultStorm,
    NoUnsupportedFrameGeneration,
    NoHiddenBevyUiProductDependency,
    NoPerformanceClaimWithoutArtifact,
}

impl RendererPerfGateKind {
    pub const ALL: [Self; RENDERER_PERF_GATE_COUNT] = [
        Self::NoUnexpectedRuntimePipelineCreation,
        Self::NoSilentBackendFallback,
        Self::NoProductCpuNativeUiFallback,
        Self::NoUnboundedPageFaultStorm,
        Self::NoUnsupportedFrameGeneration,
        Self::NoHiddenBevyUiProductDependency,
        Self::NoPerformanceClaimWithoutArtifact,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoUnexpectedRuntimePipelineCreation => "no_unexpected_runtime_pipeline_creation",
            Self::NoSilentBackendFallback => "no_silent_backend_fallback",
            Self::NoProductCpuNativeUiFallback => "no_product_cpu_native_ui_fallback",
            Self::NoUnboundedPageFaultStorm => "no_unbounded_page_fault_storm",
            Self::NoUnsupportedFrameGeneration => "no_unsupported_frame_generation",
            Self::NoHiddenBevyUiProductDependency => "no_hidden_bevy_ui_product_dependency",
            Self::NoPerformanceClaimWithoutArtifact => "no_performance_claim_without_artifact",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererPerfGateResult {
    pub kind: RendererPerfGateKind,
    pub passed: bool,
    pub measured: u64,
    pub limit: Option<u64>,
    pub reason: &'static str,
}

impl RendererPerfGateResult {
    #[must_use]
    pub const fn pass(kind: RendererPerfGateKind, measured: u64, reason: &'static str) -> Self {
        Self {
            kind,
            passed: true,
            measured,
            limit: None,
            reason,
        }
    }

    #[must_use]
    pub const fn pass_with_limit(
        kind: RendererPerfGateKind,
        measured: u64,
        limit: u64,
        reason: &'static str,
    ) -> Self {
        Self {
            kind,
            passed: true,
            measured,
            limit: Some(limit),
            reason,
        }
    }

    #[must_use]
    pub const fn fail(
        kind: RendererPerfGateKind,
        measured: u64,
        limit: Option<u64>,
        reason: &'static str,
    ) -> Self {
        Self {
            kind,
            passed: false,
            measured,
            limit,
            reason,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererPerfGateReport {
    pub passed: bool,
    pub results: Vec<RendererPerfGateResult>,
}

impl RendererPerfGateReport {
    #[must_use]
    pub fn from_results(results: Vec<RendererPerfGateResult>) -> Self {
        Self {
            passed: results.iter().all(|result| result.passed),
            results,
        }
    }

    #[must_use]
    pub fn failed_gate(&self, kind: RendererPerfGateKind) -> Option<&RendererPerfGateResult> {
        self.results
            .iter()
            .find(|result| result.kind == kind && !result.passed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererBenchmarkArtifact {
    pub schema: &'static str,
    pub schema_version: u16,
    pub scene: BenchmarkSceneDescriptor,
    pub active_settings: RendererBenchmarkReproSettings,
    pub capability_report: RendererCapabilityFacts,
    pub git_revisions: RendererBenchmarkGitRevisions,
    pub feature_flags: crate::settings::RendererCompiledFeatureSupport,
    pub metrics: RendererBenchmarkMetrics,
    pub captures: RendererBenchmarkCaptureRefs,
    pub performance_claim: RendererPerformanceClaim,
    pub perf_gates: RendererPerfGateReport,
}

impl RendererBenchmarkArtifact {
    #[must_use]
    pub fn from_selection(
        scene: &'static BenchmarkSceneDescriptor,
        selection: &RendererSettingsSelection,
        metrics: RendererBenchmarkMetrics,
        identity: RendererBenchmarkRunIdentity,
        evidence: RendererBenchmarkEvidence,
        gate_config: RendererPerfGateConfig,
    ) -> Self {
        let perf_gates = evaluate_perf_gates(
            scene,
            selection.benchmark_repro,
            selection.capabilities,
            &metrics,
            evidence.performance_claim,
            evidence.artifact_present,
            gate_config,
        );

        Self {
            schema: RENDERER_BENCHMARK_SCHEMA,
            schema_version: RENDERER_BENCHMARK_SCHEMA_VERSION,
            scene: *scene,
            active_settings: selection.benchmark_repro,
            capability_report: selection.capabilities,
            git_revisions: identity.git_revisions,
            feature_flags: identity.feature_flags,
            metrics,
            captures: identity.captures,
            performance_claim: evidence.performance_claim,
            perf_gates,
        }
    }

    #[must_use]
    pub fn synthetic(
        scene: &'static BenchmarkSceneDescriptor,
        capabilities: RendererCapabilityFacts,
        metrics: RendererBenchmarkMetrics,
    ) -> Self {
        let request = RendererSettingsRequest::baseline();
        let selection = resolve_renderer_settings(request, capabilities);
        Self::from_selection(
            scene,
            &selection,
            metrics,
            RendererBenchmarkRunIdentity::unknown(scene),
            RendererBenchmarkEvidence::claim_with_artifact(
                RendererPerformanceClaim::MovedComplexity,
            ),
            RendererPerfGateConfig::STRICT,
        )
    }
}

#[must_use]
pub fn evaluate_perf_gates(
    scene: &BenchmarkSceneDescriptor,
    settings: RendererBenchmarkReproSettings,
    capabilities: RendererCapabilityFacts,
    metrics: &RendererBenchmarkMetrics,
    performance_claim: RendererPerformanceClaim,
    artifact_present: bool,
    config: RendererPerfGateConfig,
) -> RendererPerfGateReport {
    let mut results = Vec::with_capacity(RENDERER_PERF_GATE_COUNT);

    results.push(runtime_pipeline_gate(metrics, config));
    results.push(backend_fallback_gate(settings, capabilities, metrics));
    results.push(product_native_ui_gate(scene, capabilities, metrics));
    results.push(page_fault_gate(metrics, config));
    results.push(frame_generation_gate(settings, capabilities, metrics));
    results.push(bevy_ui_gate(metrics));
    results.push(performance_claim_gate(performance_claim, artifact_present));

    RendererPerfGateReport::from_results(results)
}

fn runtime_pipeline_gate(
    metrics: &RendererBenchmarkMetrics,
    config: RendererPerfGateConfig,
) -> RendererPerfGateResult {
    let measured = u64::from(metrics.runtime_pipeline_creation_count);
    let limit = u64::from(config.runtime_pipeline_creation_allowance);
    if measured > limit {
        return RendererPerfGateResult::fail(
            RendererPerfGateKind::NoUnexpectedRuntimePipelineCreation,
            measured,
            Some(limit),
            "runtime_pipeline_creation_count_exceeded",
        );
    }
    RendererPerfGateResult::pass_with_limit(
        RendererPerfGateKind::NoUnexpectedRuntimePipelineCreation,
        measured,
        limit,
        "runtime_pipeline_creation_count_within_gate",
    )
}

fn backend_fallback_gate(
    settings: RendererBenchmarkReproSettings,
    capabilities: RendererCapabilityFacts,
    metrics: &RendererBenchmarkMetrics,
) -> RendererPerfGateResult {
    let explicit_mismatch = match settings.graphics_backend {
        GraphicsBackendSetting::Auto => false,
        requested => requested != capabilities.actual_backend,
    };
    let fallback_reason = metrics.fallback_reasons.iter().any(|reason| {
        matches!(
            *reason,
            "silent_backend_fallback" | "actual_backend_mismatch" | "fallback_backend_selected"
        )
    });

    if explicit_mismatch || fallback_reason {
        return RendererPerfGateResult::fail(
            RendererPerfGateKind::NoSilentBackendFallback,
            1,
            None,
            "selected_backend_does_not_match_actual_backend",
        );
    }
    RendererPerfGateResult::pass(
        RendererPerfGateKind::NoSilentBackendFallback,
        0,
        "backend_truth_reported_without_fallback",
    )
}

fn product_native_ui_gate(
    scene: &BenchmarkSceneDescriptor,
    capabilities: RendererCapabilityFacts,
    metrics: &RendererBenchmarkMetrics,
) -> RendererPerfGateResult {
    let product_native_ui_required = scene.requires_native_ui_gpu_transport
        && capabilities.runtime_mode == RendererRuntimeMode::Product;
    let fallback_reason = metrics.fallback_reasons.iter().any(|reason| {
        matches!(
            *reason,
            "native_ui_cpu_fallback" | "cpu_on_paint_upload" | "native_ui_transport_cpu_upload"
        )
    });
    let attempted = metrics.native_ui_cpu_fallback_attempts > 0;
    let unavailable = product_native_ui_required && !capabilities.native_ui_gpu_transport_available;

    if attempted || fallback_reason || unavailable {
        return RendererPerfGateResult::fail(
            RendererPerfGateKind::NoProductCpuNativeUiFallback,
            u64::from(metrics.native_ui_cpu_fallback_attempts),
            None,
            "product_native_ui_gpu_transport_not_clean",
        );
    }

    RendererPerfGateResult::pass(
        RendererPerfGateKind::NoProductCpuNativeUiFallback,
        0,
        "product_native_ui_gpu_transport_clean_or_not_required",
    )
}

fn page_fault_gate(
    metrics: &RendererBenchmarkMetrics,
    config: RendererPerfGateConfig,
) -> RendererPerfGateResult {
    let measured = u64::from(metrics.page_faults);
    let limit = u64::from(config.page_fault_storm_limit);
    if measured > limit {
        return RendererPerfGateResult::fail(
            RendererPerfGateKind::NoUnboundedPageFaultStorm,
            measured,
            Some(limit),
            "page_fault_storm_detected",
        );
    }
    RendererPerfGateResult::pass_with_limit(
        RendererPerfGateKind::NoUnboundedPageFaultStorm,
        measured,
        limit,
        "page_faults_within_budget",
    )
}

fn frame_generation_gate(
    settings: RendererBenchmarkReproSettings,
    capabilities: RendererCapabilityFacts,
    metrics: &RendererBenchmarkMetrics,
) -> RendererPerfGateResult {
    let explicit_vendor_fg = matches!(
        settings.frame_generation,
        FrameGenerationSetting::DlssFrameGeneration | FrameGenerationSetting::FsrFrameGeneration
    );
    let generated = metrics.fg_generated_count > 0 || metrics.fg_presented_count > 0;
    let unsupported = !capabilities.frame_generation.ready()
        || capabilities.runtime_mode.blocks_default_frame_generation();

    if (explicit_vendor_fg || generated) && unsupported {
        return RendererPerfGateResult::fail(
            RendererPerfGateKind::NoUnsupportedFrameGeneration,
            u64::from(metrics.fg_generated_count),
            None,
            "frame_generation_enabled_without_required_capabilities",
        );
    }

    RendererPerfGateResult::pass(
        RendererPerfGateKind::NoUnsupportedFrameGeneration,
        u64::from(metrics.fg_generated_count),
        "frame_generation_disabled_or_supported",
    )
}

fn bevy_ui_gate(metrics: &RendererBenchmarkMetrics) -> RendererPerfGateResult {
    if metrics.product_bevy_ui_dependency_detected {
        return RendererPerfGateResult::fail(
            RendererPerfGateKind::NoHiddenBevyUiProductDependency,
            1,
            None,
            "product_bevy_ui_dependency_detected",
        );
    }
    RendererPerfGateResult::pass(
        RendererPerfGateKind::NoHiddenBevyUiProductDependency,
        0,
        "no_product_bevy_ui_dependency_detected",
    )
}

fn performance_claim_gate(
    claim: RendererPerformanceClaim,
    artifact_present: bool,
) -> RendererPerfGateResult {
    if claim.requires_artifact() && !artifact_present {
        return RendererPerfGateResult::fail(
            RendererPerfGateKind::NoPerformanceClaimWithoutArtifact,
            1,
            None,
            "performance_claim_requires_fun_data_bundle_artifact",
        );
    }
    RendererPerfGateResult::pass(
        RendererPerfGateKind::NoPerformanceClaimWithoutArtifact,
        0,
        "performance_claim_has_artifact_or_no_claim",
    )
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RendererBenchmarkMarkdownSummary {
    pub schema: &'static str,
    pub schema_version: u16,
    pub content: String,
}

impl RendererBenchmarkMarkdownSummary {
    #[must_use]
    pub fn from_artifact(artifact: &RendererBenchmarkArtifact) -> Self {
        let mut content = String::new();
        let _ = writeln!(content, "# Renderer Benchmark Summary");
        let _ = writeln!(content);
        let _ = writeln!(content, "schema: `{}`", artifact.schema);
        let _ = writeln!(content, "schema_version: `{}`", artifact.schema_version);
        let _ = writeln!(content, "scene_id: `{}`", artifact.scene.stable_id);
        let _ = writeln!(content, "scene_kind: `{}`", artifact.scene.kind.as_str());
        let _ = writeln!(
            content,
            "performance_claim: `{}`",
            artifact.performance_claim.as_str()
        );
        let _ = writeln!(
            content,
            "perf_gates_passed: `{}`",
            artifact.perf_gates.passed
        );
        let _ = writeln!(content);
        let _ = writeln!(content, "## Frame Time");
        let _ = writeln!(
            content,
            "| metric | p50 us | p95 us | p99 us |\n| --- | ---: | ---: | ---: |"
        );
        let frame = artifact.metrics.frame_time;
        let _ = writeln!(
            content,
            "| cpu_frame_time | {} | {} | {} |",
            frame.cpu_frame_time_p50_us, frame.cpu_frame_time_p95_us, frame.cpu_frame_time_p99_us
        );
        let _ = writeln!(
            content,
            "| gpu_frame_time | {} | {} | {} |",
            frame.gpu_frame_time_p50_us, frame.gpu_frame_time_p95_us, frame.gpu_frame_time_p99_us
        );
        let _ = writeln!(content);
        let _ = writeln!(content, "## Key Metrics");
        let _ = writeln!(
            content,
            "| metric | value |\n| --- | ---: |\n| upload_bytes | {} |\n| allocation_count | {} |\n| runtime_pipeline_creation_count | {} |\n| page_faults | {} |\n| evictions | {} |\n| visible_cluster_count | {} |\n| drawn_cluster_count | {} |\n| light_count | {} |\n| candidate_count | {} |\n| shadow_pages_refreshed | {} |\n| gi_cache_occupancy | {} |\n| native_ui_import_latency_us | {} |\n| native_ui_composite_latency_us | {} |\n| upscaler_time_us | {} |\n| fg_generated_count | {} |\n| fg_presented_count | {} |",
            artifact.metrics.upload_bytes,
            artifact.metrics.allocation_count,
            artifact.metrics.runtime_pipeline_creation_count,
            artifact.metrics.page_faults,
            artifact.metrics.evictions,
            artifact.metrics.visible_cluster_count,
            artifact.metrics.drawn_cluster_count,
            artifact.metrics.light_count,
            artifact.metrics.candidate_count,
            artifact.metrics.shadow_pages_refreshed,
            artifact.metrics.gi_cache_occupancy,
            artifact.metrics.native_ui_import_latency_us,
            artifact.metrics.native_ui_composite_latency_us,
            artifact.metrics.upscaler_time_us,
            artifact.metrics.fg_generated_count,
            artifact.metrics.fg_presented_count
        );
        let _ = writeln!(content);
        let _ = writeln!(content, "## Active Settings");
        let settings = artifact.active_settings;
        let _ = writeln!(
            content,
            "runtime_backend=`{}` graphics_backend=`{}` quality=`{}` upscaler=`{}` frame_generation=`{}`",
            settings.runtime_backend.as_str(),
            settings.graphics_backend.as_str(),
            settings.quality_preset.as_str(),
            settings.upscaler.as_str(),
            settings.frame_generation.as_str()
        );
        let _ = writeln!(
            content,
            "page_pool_pages=`{}` shadow_page_budget=`{}` light_candidate_budget=`{}` gi_cache_update_budget=`{}`",
            settings.page_pool_pages,
            settings.shadow_page_budget,
            settings.light_candidate_budget,
            settings.gi_cache_update_budget
        );
        let _ = writeln!(content);
        let _ = writeln!(content, "## Capability Report");
        let capabilities = artifact.capability_report;
        let _ = writeln!(
            content,
            "actual_backend=`{}` vendor=`{}` runtime_mode=`{}` native_ui_gpu_transport_available=`{}` fg_ready=`{}`",
            capabilities.actual_backend.as_str(),
            capabilities.adapter_vendor.as_str(),
            capabilities.runtime_mode.as_str(),
            capabilities.native_ui_gpu_transport_available,
            capabilities.frame_generation.ready()
        );
        let _ = writeln!(content);
        let _ = writeln!(content, "## Perf Gates");
        let _ = writeln!(content, "| gate | status | measured | limit | reason |");
        let _ = writeln!(content, "| --- | --- | ---: | ---: | --- |");
        for result in &artifact.perf_gates.results {
            let status = if result.passed { "pass" } else { "fail" };
            let limit = result
                .limit
                .map_or_else(|| String::from("-"), |value| value.to_string());
            let _ = writeln!(
                content,
                "| `{}` | {} | {} | {} | `{}` |",
                result.kind.as_str(),
                status,
                result.measured,
                limit,
                result.reason
            );
        }

        Self {
            schema: RENDERER_BENCHMARK_SCHEMA,
            schema_version: RENDERER_BENCHMARK_SCHEMA_VERSION,
            content,
        }
    }
}

pub fn write_renderer_benchmark_artifacts(
    requested_path: impl AsRef<Path>,
    artifact: &RendererBenchmarkArtifact,
) -> io::Result<PathBuf> {
    let bundle_path = canonical_renderer_benchmark_path(requested_path.as_ref());
    if let Some(parent) = bundle_path.parent() {
        fs::create_dir_all(parent)?;
    }
    write_renderer_benchmark_bundle(&bundle_path, artifact)?;

    let json_view_path = bundle_path.with_extension("compatibility-view.json");
    let generated_view = RendererBenchmarkGeneratedView {
        view_status: "generated_compatibility_view",
        canonical_source: format!("{}", bundle_path.display()),
        artifact,
    };
    let json = serde_json::to_vec_pretty(&generated_view).map_err(io::Error::other)?;
    fs::write(json_view_path, json)?;

    let markdown_path = bundle_path.with_extension("compatibility-view.md");
    let summary = RendererBenchmarkMarkdownSummary::from_artifact(artifact);
    fs::write(&markdown_path, summary.content.as_bytes())?;
    Ok(bundle_path)
}

#[derive(Serialize)]
struct RendererBenchmarkGeneratedView<'a> {
    view_status: &'static str,
    canonical_source: String,
    artifact: &'a RendererBenchmarkArtifact,
}

fn canonical_renderer_benchmark_path(requested_path: &Path) -> PathBuf {
    if requested_path
        .to_string_lossy()
        .ends_with(RENDERER_BENCHMARK_CANONICAL_EXTENSION)
    {
        return requested_path.to_path_buf();
    }
    let mut path = requested_path.to_path_buf();
    path.set_extension("funpb.sum.zst");
    path
}

fn write_renderer_benchmark_bundle(
    bundle_path: &Path,
    artifact: &RendererBenchmarkArtifact,
) -> io::Result<()> {
    let artifact_bytes = serde_json::to_vec(artifact).map_err(io::Error::other)?;
    let artifact_digest = digest_bytes(&artifact_bytes);
    let now_ms = current_unix_ms()?;
    let failed_gate_count = artifact
        .perf_gates
        .results
        .iter()
        .filter(|result| !result.passed)
        .count();
    let severity = if failed_gate_count == 0 {
        enum_values::SEVERITY_INFO
    } else {
        enum_values::SEVERITY_ERROR
    };
    let retention_class = if failed_gate_count == 0 {
        enum_values::RETENTION_CLASS_KEEP_SUMMARY
    } else {
        enum_values::RETENTION_CLASS_KEEP_FAILURE_EVIDENCE
    };
    let mut builder = TelemetryBundleBuilder::new(BundleHeaderInput {
        created_unix_ms: now_ms,
        start_unix_ms: now_ms,
        end_unix_ms: now_ms,
        source_project: enum_values::SOURCE_PROJECT_FUN,
        artifact_kind: enum_values::ARTIFACT_KIND_BENCHMARK_RUN,
        budget_class: enum_values::BUDGET_CLASS_BENCHMARK_CAPTURE,
        retention_class,
        redaction_class: enum_values::REDACTION_CLASS_OPERATIONAL,
        trust_boundary: enum_values::TRUST_BOUNDARY_LOCAL_TOOL,
        artifact_digest,
        max_raw_bytes: 0,
        max_summary_bytes: 128 * 1024,
        max_bundle_bytes: 512 * 1024,
    });
    let schema_name_id = builder
        .intern(RENDERER_BENCHMARK_CANONICAL_SCHEMA)
        .map_err(io::Error::other)?;
    let producer_id = builder
        .intern("fun-renderer.benchmark")
        .map_err(io::Error::other)?;
    let scene_id = builder
        .intern(artifact.scene.stable_id)
        .map_err(io::Error::other)?;
    let unit_count_id = builder.intern("count").map_err(io::Error::other)?;
    let unit_bytes_id = builder.intern("bytes").map_err(io::Error::other)?;
    builder.add_counter_snapshot(
        enum_values::STABLE_SUBSYSTEM_FUN_RENDER,
        81_001,
        artifact.perf_gates.results.len() as u64,
        unit_count_id,
    );
    builder.add_counter_snapshot(
        enum_values::STABLE_SUBSYSTEM_FUN_RENDER,
        81_002,
        failed_gate_count as u64,
        unit_count_id,
    );
    builder.add_counter_snapshot(
        enum_values::STABLE_SUBSYSTEM_FUN_RENDER,
        81_003,
        artifact.metrics.upload_bytes,
        unit_bytes_id,
    );
    builder.add_counter_snapshot(
        enum_values::STABLE_SUBSYSTEM_FUN_RENDER,
        81_004,
        u64::from(artifact.metrics.runtime_pipeline_creation_count),
        unit_count_id,
    );
    builder.add_counter_snapshot(
        enum_values::STABLE_SUBSYSTEM_FUN_RENDER,
        81_005,
        u64::from(artifact.metrics.page_faults),
        unit_count_id,
    );
    builder.add_counter_snapshot(
        enum_values::STABLE_SUBSYSTEM_FUN_RENDER,
        81_006,
        0,
        unit_count_id,
    );
    builder.add_summary(schema_name_id, scene_id, severity);
    builder
        .set_retention_policy(
            if failed_gate_count == 0 { 55 } else { 85 },
            if failed_gate_count == 0 {
                enum_values::IMPORTANCE_CLASS_NORMAL
            } else {
                enum_values::IMPORTANCE_CLASS_HIGH
            },
            true,
            false,
            0,
            0,
        )
        .map_err(io::Error::other)?;
    builder
        .set_severity_summary(telemetry::SeveritySummary {
            max_severity: severity,
            error_count: if failed_gate_count == 0 { 0 } else { 1 },
            warn_count: 0,
            info_count: if failed_gate_count == 0 { 1 } else { 0 },
            debug_count: 0,
            trace_count: 0,
        })
        .map_err(io::Error::other)?;
    let mut bundle = builder.build().map_err(io::Error::other)?;
    if let Some(header) = &mut bundle.header {
        header.schema_name_id = schema_name_id;
        header.producer_id = producer_id;
        if let Some(lineage) = &mut header.lineage {
            lineage.source_digest.push(artifact_digest.to_vec());
            lineage.producer_id.push(producer_id);
        }
    }
    write_bundle_to_path(
        bundle_path,
        &bundle,
        FrameWriteOptions {
            compression_profile: CompressionProfile::ColdCompaction,
            ..FrameWriteOptions::default()
        },
    )
    .map_err(io::Error::other)?;
    read_bundle_from_path(bundle_path, FrameReadOptions::default()).map_err(io::Error::other)?;
    Ok(())
}

fn current_unix_ms() -> io::Result<u64> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(io::Error::other)?;
    u64::try_from(duration.as_millis()).map_err(io::Error::other)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn scene(kind: BenchmarkSceneKind) -> &'static BenchmarkSceneDescriptor {
        RENDERER_BENCHMARK_SCENES
            .iter()
            .find(|scene| scene.kind == kind)
            .expect("benchmark scene kind should be registered")
    }

    fn selection_for(mut capabilities: RendererCapabilityFacts) -> RendererSettingsSelection {
        capabilities.native_ui_gpu_transport_available = true;
        resolve_renderer_settings(RendererSettingsRequest::baseline(), capabilities)
    }

    #[test]
    fn benchmark_scene_catalog_contains_required_pass21_scenes() {
        assert_eq!(
            RENDERER_BENCHMARK_SCENES.len(),
            RENDERER_BENCHMARK_SCENE_COUNT
        );
        for kind in [
            BenchmarkSceneKind::ClearPresent,
            BenchmarkSceneKind::StaticScene,
            BenchmarkSceneKind::NativeUiComposition,
            BenchmarkSceneKind::Dx12VulkanParity,
            BenchmarkSceneKind::UploadStress,
            BenchmarkSceneKind::PipelineWarmupHotLoop,
            BenchmarkSceneKind::VirtualGeometryStress,
            BenchmarkSceneKind::DynamicGeometryStress,
            BenchmarkSceneKind::ProceduralInvalidation,
            BenchmarkSceneKind::ManyLightStress,
            BenchmarkSceneKind::VirtualShadowStress,
            BenchmarkSceneKind::GiReflectionScene,
            BenchmarkSceneKind::UpscalingScene,
            BenchmarkSceneKind::FgEligibilityPacing,
        ] {
            assert!(
                RENDERER_BENCHMARK_SCENES
                    .iter()
                    .any(|scene| scene.kind == kind)
            );
        }
        assert!(scene(BenchmarkSceneKind::NativeUiComposition).requires_native_ui_gpu_transport);
        assert!(scene(BenchmarkSceneKind::Dx12VulkanParity).requires_backend_truth);
        assert!(scene(BenchmarkSceneKind::FgEligibilityPacing).requires_frame_generation_boundary);
        assert!(renderer_benchmark_scene("renderer.benchmark.scene.many_light_stress").is_some());
    }

    #[test]
    fn perf_gates_pass_for_clean_benchmark_artifact() {
        let capabilities = RendererCapabilityFacts::high_end_dx12_nvidia();
        let selection = selection_for(capabilities);
        let artifact = RendererBenchmarkArtifact::from_selection(
            scene(BenchmarkSceneKind::ClearPresent),
            &selection,
            RendererBenchmarkMetrics::clean_smoke(),
            RendererBenchmarkRunIdentity::unknown(scene(BenchmarkSceneKind::ClearPresent)),
            RendererBenchmarkEvidence::claim_with_artifact(RendererPerformanceClaim::Improved),
            RendererPerfGateConfig::STRICT,
        );

        assert!(artifact.perf_gates.passed);
        assert_eq!(artifact.perf_gates.results.len(), RENDERER_PERF_GATE_COUNT);
    }

    #[test]
    fn hard_gates_fail_for_each_forbidden_state() {
        let capabilities = RendererCapabilityFacts::high_end_dx12_nvidia();
        let selection = selection_for(capabilities);
        let settings = selection.benchmark_repro;

        let mut metrics = RendererBenchmarkMetrics::clean_smoke();
        metrics.runtime_pipeline_creation_count = 1;
        assert!(
            evaluate_perf_gates(
                scene(BenchmarkSceneKind::PipelineWarmupHotLoop),
                settings,
                capabilities,
                &metrics,
                RendererPerformanceClaim::None,
                false,
                RendererPerfGateConfig::STRICT,
            )
            .failed_gate(RendererPerfGateKind::NoUnexpectedRuntimePipelineCreation)
            .is_some()
        );

        let mut metrics = RendererBenchmarkMetrics::clean_smoke();
        metrics.fallback_reasons.push("actual_backend_mismatch");
        assert!(
            evaluate_perf_gates(
                scene(BenchmarkSceneKind::Dx12VulkanParity),
                settings,
                capabilities,
                &metrics,
                RendererPerformanceClaim::None,
                false,
                RendererPerfGateConfig::STRICT,
            )
            .failed_gate(RendererPerfGateKind::NoSilentBackendFallback)
            .is_some()
        );

        let mut metrics = RendererBenchmarkMetrics::clean_smoke();
        metrics.native_ui_cpu_fallback_attempts = 1;
        assert!(
            evaluate_perf_gates(
                scene(BenchmarkSceneKind::NativeUiComposition),
                settings,
                capabilities,
                &metrics,
                RendererPerformanceClaim::None,
                false,
                RendererPerfGateConfig::STRICT,
            )
            .failed_gate(RendererPerfGateKind::NoProductCpuNativeUiFallback)
            .is_some()
        );

        let metrics = RendererBenchmarkMetrics::clean_smoke().with_page_pressure(900, 5);
        assert!(
            evaluate_perf_gates(
                scene(BenchmarkSceneKind::VirtualGeometryStress),
                settings,
                capabilities,
                &metrics,
                RendererPerformanceClaim::None,
                false,
                RendererPerfGateConfig::STRICT,
            )
            .failed_gate(RendererPerfGateKind::NoUnboundedPageFaultStorm)
            .is_some()
        );

        let mut unsupported_fg = RendererCapabilityFacts::minimal();
        unsupported_fg.native_ui_gpu_transport_available = true;
        let mut fg_settings = settings;
        fg_settings.frame_generation = FrameGenerationSetting::DlssFrameGeneration;
        assert!(
            evaluate_perf_gates(
                scene(BenchmarkSceneKind::FgEligibilityPacing),
                fg_settings,
                unsupported_fg,
                &RendererBenchmarkMetrics::clean_smoke(),
                RendererPerformanceClaim::None,
                false,
                RendererPerfGateConfig::STRICT,
            )
            .failed_gate(RendererPerfGateKind::NoUnsupportedFrameGeneration)
            .is_some()
        );

        let mut metrics = RendererBenchmarkMetrics::clean_smoke();
        metrics.product_bevy_ui_dependency_detected = true;
        assert!(
            evaluate_perf_gates(
                scene(BenchmarkSceneKind::StaticScene),
                settings,
                capabilities,
                &metrics,
                RendererPerformanceClaim::None,
                false,
                RendererPerfGateConfig::STRICT,
            )
            .failed_gate(RendererPerfGateKind::NoHiddenBevyUiProductDependency)
            .is_some()
        );

        assert!(
            evaluate_perf_gates(
                scene(BenchmarkSceneKind::ClearPresent),
                settings,
                capabilities,
                &RendererBenchmarkMetrics::clean_smoke(),
                RendererPerformanceClaim::Improved,
                false,
                RendererPerfGateConfig::STRICT,
            )
            .failed_gate(RendererPerfGateKind::NoPerformanceClaimWithoutArtifact)
            .is_some()
        );
    }

    #[test]
    fn benchmark_artifact_serializes_metrics_settings_capabilities_and_gates() {
        let capabilities = RendererCapabilityFacts::high_end_dx12_nvidia();
        let artifact = RendererBenchmarkArtifact::synthetic(
            scene(BenchmarkSceneKind::ManyLightStress),
            capabilities,
            RendererBenchmarkMetrics {
                light_count: 12_000,
                candidate_count: 64_000,
                shadow_pages_refreshed: 128,
                ..RendererBenchmarkMetrics::clean_smoke()
            },
        );

        let json = serde_json::to_value(&artifact).expect("artifact should serialize to JSON");
        assert_eq!(
            json["schema"],
            Value::String(RENDERER_BENCHMARK_SCHEMA.into())
        );
        assert_eq!(
            json["scene"]["stable_id"],
            Value::String("renderer.benchmark.scene.many_light_stress".into())
        );
        assert_eq!(json["metrics"]["light_count"], Value::from(12_000));
        assert_eq!(json["metrics"]["candidate_count"], Value::from(64_000));
        assert_eq!(
            json["active_settings"]["quality_preset"],
            Value::String("baseline".into())
        );
        assert_eq!(
            json["capability_report"]["actual_backend"],
            Value::String("dx12".into())
        );
        assert_eq!(json["perf_gates"]["passed"], Value::Bool(true));

        let summary = RendererBenchmarkMarkdownSummary::from_artifact(&artifact);
        assert!(summary.content.contains("p95 us"));
        assert!(summary.content.contains("runtime_pipeline_creation_count"));
        assert!(summary.content.contains("actual_backend=`dx12`"));
        assert!(
            summary
                .content
                .contains("no_performance_claim_without_artifact")
        );
    }

    #[test]
    fn benchmark_artifacts_record_fun_data_bundle_views_and_gate_status() {
        let capabilities = RendererCapabilityFacts::high_end_dx12_nvidia();
        let artifact = RendererBenchmarkArtifact::synthetic(
            scene(BenchmarkSceneKind::PipelineWarmupHotLoop),
            capabilities,
            RendererBenchmarkMetrics::clean_smoke(),
        );
        assert!(artifact.perf_gates.passed);

        if let Some(path) = std::env::var_os(RENDERER_BENCHMARK_ARTIFACT_ENV) {
            let bundle = write_renderer_benchmark_artifacts(&path, &artifact)
                .expect("renderer benchmark artifact should be writable");
            assert!(
                bundle
                    .to_string_lossy()
                    .ends_with(RENDERER_BENCHMARK_CANONICAL_EXTENSION)
            );
            assert!(bundle.exists());
            assert!(bundle.with_extension("compatibility-view.json").exists());
            assert!(bundle.with_extension("compatibility-view.md").exists());
        }
    }
}
