//! V2-3A and V2-3C: renderer schedule contract.
//!
//! Maps the renderer's typed [`FrameGraphPassRole`] and Lux's typed
//! [`LuxResourceIntentKind`] into the canonical cross-subsystem schedule
//! contract declared by `fun_scheduler_types` (Path B of the V2 plan,
//! see `docs/scheduler/naming.md`).
//!
//! - [`pass_role_to_task_class`] is the V2-3A pass-role mapping:
//!   `LuxUploadLightBuffers` / `LuxClusterLights` / shadow setup /
//!   reservoir setup / GI-cache update → `TaskClass::RenderPrepare`;
//!   `LuxDirectLighting` / GI / reflection / denoise / volumetric →
//!   `TaskClass::RenderRecord`; `Compose` / `Present` / final
//!   output transform → `TaskClass::FrameCritical`; debug overlays /
//!   diagnostics readback → `TaskClass::Diagnostic`.
//! - [`resource_intent_to_budget_origin`] is the V2-3A resource-intent
//!   mapping: every `LuxResourceIntentKind` lands on a stable
//!   [`fun_scheduler_types::budget::BudgetOrigin`] — debug buffers on
//!   `DiagnosticCapture`, everything else on `RendererPageBatch`.
//! - [`RendererSchedulePlanV1`] is the V2-3C in-memory plan struct.
//!   Persistence is explicitly deferred to a later pass that routes
//!   through compressed protobuf bundles per the root `AGENTS.md`
//!   telemetry standard.
//! - V2-3D scope guard: this module is *contract only*. It does not
//!   execute work, does not change render order, does not run a
//!   stealing scheduler, and does not alter the frame graph executor.
//!   The goal is to make renderer work *schedulable*, not *rescheduled*.

use core::fmt;
use core::marker::PhantomData;

use fun_scheduler_types::budget::{BudgetOrigin, Deadline, TaskBudget};
use fun_scheduler_types::class::{TaskClass, TaskPriority};
use fun_scheduler_types::schedule::{
    ScheduleBudget, ScheduleDeadline, ScheduleDomain, ScheduleLane,
};
use fun_scheduler_types::work_graph::{
    DeterministicDescriptor, GraphInvariantError, WorkEdge, WorkGraph, WorkGraphId, WorkNode,
    WorkNodeId, WorkNodeLiveness, WorkPhase,
};

use crate::frame_graph::FrameGraphPassRole;

pub const LUX_RESOURCE_INTENT_KIND_COUNT: usize = 22;

/// Renderer-side resource intent kind used for scheduler budgeting.
///
/// This is a value contract, not a lighting-crate dependency. The
/// lighting pipeline may emit equivalent tags, but scheduler-visible
/// renderer work must stay buildable without pulling scene or backend
/// integration crates into the core schedule lane.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxResourceIntentKind {
    LightBuffer,
    LightIndexBuffer,
    ClusterGrid,
    ReservoirBuffer,
    ShadowRequestBuffer,
    ShadowAtlas,
    VirtualShadowPageTable,
    VoxelShadowPageTable,
    VoxelTerrainSdfPool,
    SurfaceCache,
    RadianceCache,
    VoxelTerrainRadianceClipmap,
    VoxelCanopyOpacityClipmap,
    StormExtinctionClipmap,
    ProbeCache,
    ReflectionTraceBuffer,
    DenoiseHistory,
    VolumetricFroxelDensity,
    VolumetricFroxelScattering,
    VolumetricHistory,
    IntegratedFog,
    LuxDebugBuffer,
}

impl LuxResourceIntentKind {
    pub const ALL: [Self; LUX_RESOURCE_INTENT_KIND_COUNT] = [
        Self::LightBuffer,
        Self::LightIndexBuffer,
        Self::ClusterGrid,
        Self::ReservoirBuffer,
        Self::ShadowRequestBuffer,
        Self::ShadowAtlas,
        Self::VirtualShadowPageTable,
        Self::VoxelShadowPageTable,
        Self::VoxelTerrainSdfPool,
        Self::SurfaceCache,
        Self::RadianceCache,
        Self::VoxelTerrainRadianceClipmap,
        Self::VoxelCanopyOpacityClipmap,
        Self::StormExtinctionClipmap,
        Self::ProbeCache,
        Self::ReflectionTraceBuffer,
        Self::DenoiseHistory,
        Self::VolumetricFroxelDensity,
        Self::VolumetricFroxelScattering,
        Self::VolumetricHistory,
        Self::IntegratedFog,
        Self::LuxDebugBuffer,
    ];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LightBuffer => "light_buffer",
            Self::LightIndexBuffer => "light_index_buffer",
            Self::ClusterGrid => "cluster_grid",
            Self::ReservoirBuffer => "reservoir_buffer",
            Self::ShadowRequestBuffer => "shadow_request_buffer",
            Self::ShadowAtlas => "shadow_atlas",
            Self::VirtualShadowPageTable => "virtual_shadow_page_table",
            Self::VoxelShadowPageTable => "voxel_shadow_page_table",
            Self::VoxelTerrainSdfPool => "voxel_terrain_sdf_pool",
            Self::SurfaceCache => "surface_cache",
            Self::RadianceCache => "radiance_cache",
            Self::VoxelTerrainRadianceClipmap => "voxel_terrain_radiance_clipmap",
            Self::VoxelCanopyOpacityClipmap => "voxel_canopy_opacity_clipmap",
            Self::StormExtinctionClipmap => "storm_extinction_clipmap",
            Self::ProbeCache => "probe_cache",
            Self::ReflectionTraceBuffer => "reflection_trace_buffer",
            Self::DenoiseHistory => "denoise_history",
            Self::VolumetricFroxelDensity => "volumetric_froxel_density",
            Self::VolumetricFroxelScattering => "volumetric_froxel_scattering",
            Self::VolumetricHistory => "volumetric_history",
            Self::IntegratedFog => "integrated_fog",
            Self::LuxDebugBuffer => "lux_debug_buffer",
        }
    }
}

/// V2-3C reason code: renderer pass scheduled on the present-critical
/// lane.
///
/// Centralized in `fun_scheduler_types::reason` (canonical home).
/// This `pub const` alias preserves the renderer-side import path so
/// existing consumers don't need to retarget.
pub const REASON_RENDERER_PRESENT_CRITICAL: &str =
    fun_scheduler_types::reason::RENDERER_PRESENT_CRITICAL;

/// V2-3C reason code: renderer pass scheduled on the diagnostics lane.
pub const REASON_RENDERER_DIAGNOSTIC_OVERLAY: &str =
    fun_scheduler_types::reason::RENDERER_DIAGNOSTIC_OVERLAY;

/// V2-3C reason code: renderer pass scheduled as preparation work
/// (extract / upload / setup).
pub const REASON_RENDERER_PREPARE_PASS: &str = fun_scheduler_types::reason::RENDERER_PREPARE_PASS;

/// V2-3C reason code: renderer pass scheduled as command-buffer
/// recording.
pub const REASON_RENDERER_RECORD_PASS: &str = fun_scheduler_types::reason::RENDERER_RECORD_PASS;

/// V2-3A pass-role → task-class mapping. Maps a renderer
/// [`FrameGraphPassRole`] onto the canonical
/// [`TaskClass`] taxonomy from `fun_scheduler_types`.
///
/// The classifier is total over every [`FrameGraphPassRole`] variant.
/// Lux roles split by their typed order key: roles in the upload /
/// cluster / reservoir-setup / shadow-setup / GI-cache-update band
/// land on `RenderPrepare`; lighting / GI-trace / reflection /
/// denoise / volumetric record bands land on `RenderRecord`; the
/// debug overlay role lands on `Diagnostic`.
#[must_use]
pub const fn pass_role_to_task_class(role: FrameGraphPassRole) -> TaskClass {
    use FrameGraphPassRole as P;
    match role {
        // Present-critical: the path that gates display.
        P::Compose | P::Present | P::PostProcessFinalOutputTransform => TaskClass::FrameCritical,

        // Diagnostics-only roles: debug overlays and readbacks.
        P::PostProcessDebugOverlay | P::DiagnosticsReadback | P::LuxDebugOverlay => {
            TaskClass::Diagnostic
        }

        // Render preparation: scene setup, virtual feedback, Lux upload /
        // cluster / reservoir-setup / shadow-setup / GI-cache-update.
        P::Clear
        | P::StaticScenePlaceholder
        | P::VirtualResourceFeedback
        | P::LuxUploadLightBuffers
        | P::LuxClusterLights
        | P::LuxReservoirTemporalReuse
        | P::LuxReservoirSpatialReuse
        | P::LuxShadowRequests
        | P::LuxVirtualShadowPages
        | P::LuxVirtualShadowFilter
        | P::LuxVoxelShadowDemandMark
        | P::LuxVoxelShadowPageBuild
        // Pass C7.2 — typed cloud shadow project / filter
        // / register-layer roles are typed shadow-setup
        // work that runs before the typed render-record
        // band consumes them.
        | P::LuxCloudShadowProject
        | P::LuxCloudShadowFilter
        | P::LuxCloudShadowRegisterLayer
        | P::LuxGiCacheUpdate
        | P::LuxVoxelRadianceClipmapUpdate => TaskClass::RenderPrepare,

        // Command-buffer recording: imports, upscaler / frame-generation
        // boundary, post-process chain (minus the final-output transform
        // and debug overlay), and the actual Lux rendering passes
        // (direct lighting, GI trace, reflection trace, denoise,
        // volumetric record band). Pass V2.5 added the granular HDR
        // post-pipeline roles (exposure histogram / adapt, bloom
        // prefilter / downsample / upsample / composite); they all
        // belong on `RenderRecord` like the umbrella `PostProcessExposure`
        // / `PostProcessBloom` roles they refine.
        P::NativeUiGpuImport
        | P::UiImportPlaceholder
        | P::UpscaleBoundary
        | P::FrameGenerationBoundary
        | P::PostProcessExposure
        | P::PostProcessBloom
        | P::PostProcessExposureHistogram
        | P::PostProcessExposureAdapt
        | P::PostProcessBloomPrefilter
        | P::PostProcessBloomDownsample
        | P::PostProcessBloomUpsample
        | P::PostProcessBloomComposite
        | P::PostProcessToneMapping
        | P::PostProcessColorGradingLut
        | P::PostProcessSharpening
        | P::LuxDirectLighting
        | P::LuxVoxelSdfDistantShadowResolve
        | P::LuxGiTrace
        | P::LuxReflectionTrace
        | P::LuxVoxelTerrainAoResolve
        | P::LuxDenoise
        | P::LuxVolumetricFogInject
        | P::LuxStormExtinctionInject
        | P::LuxVoxelCanopyTransmittanceInject
        | P::LuxVolumetricLightInject
        | P::LuxVolumetricTemporalReproject
        | P::LuxVolumetricIntegrate
        | P::LuxVolumetricComposite => TaskClass::RenderRecord,
    }
}

/// V2-3A resource-intent → budget-origin mapping. Maps a Lux
/// [`LuxResourceIntentKind`] onto a canonical
/// [`BudgetOrigin`] from `fun_scheduler_types`.
///
/// Every renderer-allocated Lux resource belongs to a renderer page
/// batch budget except the debug buffer, which belongs to the
/// diagnostic capture budget so debug staging never starves
/// production work.
#[must_use]
pub const fn resource_intent_to_budget_origin(kind: LuxResourceIntentKind) -> BudgetOrigin {
    use LuxResourceIntentKind as K;
    match kind {
        K::LuxDebugBuffer => BudgetOrigin::DiagnosticCapture,
        K::LightBuffer
        | K::LightIndexBuffer
        | K::ClusterGrid
        | K::ReservoirBuffer
        | K::ShadowRequestBuffer
        | K::ShadowAtlas
        | K::VirtualShadowPageTable
        | K::VoxelShadowPageTable
        | K::VoxelTerrainSdfPool
        | K::SurfaceCache
        | K::RadianceCache
        | K::VoxelTerrainRadianceClipmap
        | K::VoxelCanopyOpacityClipmap
        | K::StormExtinctionClipmap
        | K::ProbeCache
        | K::ReflectionTraceBuffer
        | K::DenoiseHistory
        | K::VolumetricFroxelDensity
        | K::VolumetricFroxelScattering
        | K::VolumetricHistory
        | K::IntegratedFog => BudgetOrigin::RendererPageBatch,
    }
}

/// Derive the canonical reason code for a plan that covers the given
/// [`FrameGraphPassRole`]. The reason code is the explanatory label
/// for *why* the plan landed on its [`TaskClass`].
#[must_use]
pub const fn derive_reason_code(role: FrameGraphPassRole) -> &'static str {
    match pass_role_to_task_class(role) {
        TaskClass::FrameCritical => REASON_RENDERER_PRESENT_CRITICAL,
        TaskClass::Diagnostic => REASON_RENDERER_DIAGNOSTIC_OVERLAY,
        TaskClass::RenderPrepare => REASON_RENDERER_PREPARE_PASS,
        TaskClass::RenderRecord => REASON_RENDERER_RECORD_PASS,
        _ => REASON_RENDERER_RECORD_PASS,
    }
}

/// V2-3C renderer schedule plan, V1 in-memory record.
///
/// One `RendererSchedulePlanV1` is produced per renderer pass-role
/// admission decision. The plan is in-memory only; persistence is
/// explicitly deferred per the V2-3D scope guard. The [`fmt::Display`]
/// impl is for CLI / test-fixture rendering.
#[derive(Clone, Debug)]
pub struct RendererSchedulePlanV1 {
    /// Monotonic frame index within a session.
    pub frame_index: u64,
    /// The typed frame-graph pass role this plan covers.
    pub render_phase: FrameGraphPassRole,
    /// Number of Lux passes encompassed by this plan record.
    pub lux_pass_count: u32,
    /// Number of Lux resource intents encompassed by this plan record.
    pub resource_intent_count: u32,
    /// Estimated CPU microseconds for the planned work.
    pub estimated_cpu_us: u32,
    /// Estimated upload bytes for the planned work.
    pub estimated_upload_bytes: u64,
    /// Canonical [`TaskClass`] the renderer requests for this plan.
    pub schedule_lane: TaskClass,
    /// Canonical task budget envelope.
    pub budget: TaskBudget,
    /// Absolute monotonic deadline. Use [`Deadline::NEVER`] for plans
    /// without an explicit deadline.
    pub deadline: Deadline,
    /// Stable explanatory label for *why* the plan landed on its
    /// [`Self::schedule_lane`]. Always a `&'static str` constant from
    /// the renderer reason-code set or a re-export thereof.
    pub reason_code: &'static str,
}

impl RendererSchedulePlanV1 {
    /// Construct a plan from a [`FrameGraphPassRole`] plus the caller's
    /// pass / intent counts and cost estimates. The schedule lane and
    /// reason code are derived from the role; the budget and deadline
    /// default to unbounded.
    #[must_use]
    pub const fn for_role(
        frame_index: u64,
        render_phase: FrameGraphPassRole,
        lux_pass_count: u32,
        resource_intent_count: u32,
        estimated_cpu_us: u32,
        estimated_upload_bytes: u64,
    ) -> Self {
        Self {
            frame_index,
            render_phase,
            lux_pass_count,
            resource_intent_count,
            estimated_cpu_us,
            estimated_upload_bytes,
            schedule_lane: pass_role_to_task_class(render_phase),
            budget: TaskBudget::UNBOUNDED,
            deadline: Deadline::NEVER,
            reason_code: derive_reason_code(render_phase),
        }
    }

    /// Attach an explicit budget envelope to the plan.
    #[must_use]
    pub const fn with_budget(mut self, budget: TaskBudget) -> Self {
        self.budget = budget;
        self
    }

    /// Attach an explicit deadline to the plan.
    #[must_use]
    pub const fn with_deadline(mut self, deadline: Deadline) -> Self {
        self.deadline = deadline;
        self
    }
}

impl fmt::Display for RendererSchedulePlanV1 {
    /// Compact text rendering for CLI / test-fixture use. Per the V2-3D
    /// scope guard, this rendering is a view only; canonical telemetry
    /// must route through a compressed protobuf bundle, not this
    /// `Display` output.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "renderer plan frame={} role={} lane={} reason={} \
             lux_passes={} resources={} cpu_us={} upload_bytes={}",
            self.frame_index,
            self.render_phase.as_str(),
            self.schedule_lane.label(),
            self.reason_code,
            self.lux_pass_count,
            self.resource_intent_count,
            self.estimated_cpu_us,
            self.estimated_upload_bytes,
        )
    }
}

// ============================================================================
// FS-10 — Renderer work-graph scheduling
//
// Adds typed renderer graph phases, node classes, Lux-specific node
// kinds, a renderer work descriptor that the FS-8 `WorkGraph<W>`
// kernel can carry, a renderer-side typed schedule report, and the
// mapping helpers the scheduler uses to route work onto FS-7
// `ScheduleLane`s.
//
// **Scope guard (FS-10 D)**: this module remains contract only — it
// does not change render order, does not run a stealing scheduler,
// does not invent graph semantics, does not leak backend handles
// (wgpu / DX12 / Vulkan / Metal) into scheduling decisions, and
// does not alter the frame graph executor.
// ============================================================================

/// FS-10 renderer-graph phase taxonomy. 11 phases covering the
/// renderer's CPU side of the frame pipeline from extraction to
/// post-frame diagnostics.
///
/// Each phase corresponds to one or more [`RendererNodeClass`]
/// entries; the scheduler maps the phase to a canonical
/// [`ScheduleLane`] via [`phase_to_schedule_lane`].
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[repr(u8)]
#[non_exhaustive]
pub enum RendererGraphPhase {
    /// FUN ECS to render-world extraction.
    RenderExtract = 0,
    /// Asset prep (texture decompression, mesh upload prep, shader
    /// preflight).
    RenderPrepareAssets = 1,
    /// Per-frame scene prep (visibility, GPU scene buffer fill).
    RenderPrepareScene = 2,
    /// Lux plan generation (light selection, shadow request build).
    RenderLuxPlan = 3,
    /// Frame graph + Lux graph build.
    RenderGraphBuild = 4,
    /// Graph validation (resource lifetimes, barrier coverage).
    RenderGraphValidate = 5,
    /// Graph compile (pipeline materialization, descriptor build).
    RenderGraphCompile = 6,
    /// Command-buffer recording.
    RenderRecord = 7,
    /// Queue submission.
    RenderSubmit = 8,
    /// Swapchain present + frame-pacing.
    RenderPresent = 9,
    /// Post-frame diagnostics flush.
    RendererDiagnosticsFlush = 10,
}

impl RendererGraphPhase {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RenderExtract => "render_extract",
            Self::RenderPrepareAssets => "render_prepare_assets",
            Self::RenderPrepareScene => "render_prepare_scene",
            Self::RenderLuxPlan => "render_lux_plan",
            Self::RenderGraphBuild => "render_graph_build",
            Self::RenderGraphValidate => "render_graph_validate",
            Self::RenderGraphCompile => "render_graph_compile",
            Self::RenderRecord => "render_record",
            Self::RenderSubmit => "render_submit",
            Self::RenderPresent => "render_present",
            Self::RendererDiagnosticsFlush => "renderer_diagnostics_flush",
        }
    }

    /// Every variant in canonical execution order.
    #[inline]
    pub const fn all() -> &'static [Self] {
        &[
            Self::RenderExtract,
            Self::RenderPrepareAssets,
            Self::RenderPrepareScene,
            Self::RenderLuxPlan,
            Self::RenderGraphBuild,
            Self::RenderGraphValidate,
            Self::RenderGraphCompile,
            Self::RenderRecord,
            Self::RenderSubmit,
            Self::RenderPresent,
            Self::RendererDiagnosticsFlush,
        ]
    }

    /// Map this phase onto an FS-7 [`ScheduleLane`]. The mapping is
    /// intentional and stable:
    ///
    /// - `RenderPresent` and `RenderSubmit` land on
    ///   [`ScheduleLane::RenderPresent`] / [`ScheduleLane::RenderSubmit`]
    ///   for the present-critical path.
    /// - `RenderRecord` and `RenderPrepareScene` /
    ///   `RenderPrepareAssets` / `RenderLuxPlan` land on the
    ///   matching prepare/record lanes.
    /// - `RenderGraphBuild` / `RenderGraphValidate` /
    ///   `RenderGraphCompile` land on
    ///   [`ScheduleLane::RenderGraphCompile`] — these are CPU-bound
    ///   prep that can share runtime workers with other domains.
    /// - `RenderExtract` lands on [`ScheduleLane::RenderExtract`].
    /// - `RendererDiagnosticsFlush` lands on
    ///   [`ScheduleLane::DiagnosticsLowPriority`] so it sheds first
    ///   under frame pressure.
    #[inline]
    pub const fn to_schedule_lane(self) -> ScheduleLane {
        match self {
            Self::RenderExtract => ScheduleLane::RenderExtract,
            Self::RenderPrepareAssets => ScheduleLane::ResourceBackground,
            Self::RenderPrepareScene => ScheduleLane::RenderPrepare,
            Self::RenderLuxPlan => ScheduleLane::RenderPrepare,
            Self::RenderGraphBuild => ScheduleLane::RenderGraphCompile,
            Self::RenderGraphValidate => ScheduleLane::RenderGraphCompile,
            Self::RenderGraphCompile => ScheduleLane::RenderGraphCompile,
            Self::RenderRecord => ScheduleLane::RenderRecord,
            Self::RenderSubmit => ScheduleLane::RenderSubmit,
            Self::RenderPresent => ScheduleLane::RenderPresent,
            Self::RendererDiagnosticsFlush => ScheduleLane::DiagnosticsLowPriority,
        }
    }
}

/// Tier-5 renderer work taxonomy. These are the scheduler-visible
/// work shapes the renderer may emit for a frame; backend handles
/// and subsystem-specific queues stay out of the descriptor.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[repr(u8)]
#[non_exhaustive]
pub enum RendererWorkKind {
    /// Extract active views.
    ExtractViews = 0,
    /// Extract renderable entities.
    ExtractRenderables = 1,
    /// Extract renderer-visible lights.
    ExtractLights = 2,
    /// Extract native UI surfaces.
    ExtractUiSurfaces = 3,
    /// Prepare one GPU-scene chunk.
    PrepareGpuSceneChunk = 4,
    /// Prepare one material chunk.
    PrepareMaterialChunk = 5,
    /// Prepare one mesh chunk.
    PrepareMeshChunk = 6,
    /// Prepare one upload batch.
    PrepareUploadBatch = 7,
    /// Reflect shader metadata.
    ShaderReflect = 8,
    /// Warm a graphics or compute pipeline.
    PipelineWarmup = 9,
    /// Cull one visibility chunk.
    VisibilityCullChunk = 10,
    /// Build one light cluster range.
    ClusterBuild = 11,
    /// Resolve one virtual-geometry page job.
    VirtualGeometryPageResolve = 12,
    /// Resolve one virtual-shadow page job.
    VirtualShadowPageResolve = 13,
    /// Build the frame graph.
    FrameGraphBuild = 14,
    /// Compile the frame graph.
    FrameGraphCompile = 15,
    /// Record a render pass.
    RecordPass = 16,
    /// Record a copy/import pass.
    RecordCopyPass = 17,
    /// Record a compute pass.
    RecordComputePass = 18,
    /// Submit the queue.
    SubmitQueue = 19,
    /// Present the surface.
    PresentSurface = 20,
    /// Import ECS-derived render artifacts into renderer realization work.
    ImportEcsRenderArtifacts = 21,
    /// Retire or shed optional GPU artifact realizations.
    RetireGpuArtifacts = 22,
    /// Compose a native UI packet.
    NativeUiPacketCompose = 23,
    /// Capture renderer diagnostics.
    DiagnosticCapture = 24,
}

impl RendererWorkKind {
    /// Stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ExtractViews => "extract_views",
            Self::ExtractRenderables => "extract_renderables",
            Self::ExtractLights => "extract_lights",
            Self::ExtractUiSurfaces => "extract_ui_surfaces",
            Self::PrepareGpuSceneChunk => "prepare_gpu_scene_chunk",
            Self::PrepareMaterialChunk => "prepare_material_chunk",
            Self::PrepareMeshChunk => "prepare_mesh_chunk",
            Self::PrepareUploadBatch => "prepare_upload_batch",
            Self::ShaderReflect => "shader_reflect",
            Self::PipelineWarmup => "pipeline_warmup",
            Self::VisibilityCullChunk => "visibility_cull_chunk",
            Self::ClusterBuild => "cluster_build",
            Self::VirtualGeometryPageResolve => "virtual_geometry_page_resolve",
            Self::VirtualShadowPageResolve => "virtual_shadow_page_resolve",
            Self::FrameGraphBuild => "frame_graph_build",
            Self::FrameGraphCompile => "frame_graph_compile",
            Self::RecordPass => "record_pass",
            Self::RecordCopyPass => "record_copy_pass",
            Self::RecordComputePass => "record_compute_pass",
            Self::SubmitQueue => "submit_queue",
            Self::PresentSurface => "present_surface",
            Self::ImportEcsRenderArtifacts => "import_ecs_render_artifacts",
            Self::RetireGpuArtifacts => "retire_gpu_artifacts",
            Self::NativeUiPacketCompose => "native_ui_packet_compose",
            Self::DiagnosticCapture => "diagnostic_capture",
        }
    }

    /// Every variant in declaration order.
    #[inline]
    pub const fn all() -> &'static [Self] {
        &[
            Self::ExtractViews,
            Self::ExtractRenderables,
            Self::ExtractLights,
            Self::ExtractUiSurfaces,
            Self::PrepareGpuSceneChunk,
            Self::PrepareMaterialChunk,
            Self::PrepareMeshChunk,
            Self::PrepareUploadBatch,
            Self::ShaderReflect,
            Self::PipelineWarmup,
            Self::VisibilityCullChunk,
            Self::ClusterBuild,
            Self::VirtualGeometryPageResolve,
            Self::VirtualShadowPageResolve,
            Self::FrameGraphBuild,
            Self::FrameGraphCompile,
            Self::RecordPass,
            Self::RecordCopyPass,
            Self::RecordComputePass,
            Self::SubmitQueue,
            Self::PresentSurface,
            Self::ImportEcsRenderArtifacts,
            Self::RetireGpuArtifacts,
            Self::NativeUiPacketCompose,
            Self::DiagnosticCapture,
        ]
    }

    /// Coarse renderer phase.
    #[inline]
    pub const fn phase(self) -> RendererGraphPhase {
        match self {
            Self::ExtractViews
            | Self::ExtractRenderables
            | Self::ExtractLights
            | Self::ExtractUiSurfaces => RendererGraphPhase::RenderExtract,
            Self::PrepareMaterialChunk
            | Self::PrepareMeshChunk
            | Self::PrepareUploadBatch
            | Self::ShaderReflect
            | Self::PipelineWarmup
            | Self::RetireGpuArtifacts => RendererGraphPhase::RenderPrepareAssets,
            Self::PrepareGpuSceneChunk
            | Self::VisibilityCullChunk
            | Self::VirtualGeometryPageResolve
            | Self::ImportEcsRenderArtifacts
            | Self::NativeUiPacketCompose => RendererGraphPhase::RenderPrepareScene,
            Self::ClusterBuild | Self::VirtualShadowPageResolve => {
                RendererGraphPhase::RenderLuxPlan
            }
            Self::FrameGraphBuild => RendererGraphPhase::RenderGraphBuild,
            Self::FrameGraphCompile => RendererGraphPhase::RenderGraphCompile,
            Self::RecordPass | Self::RecordCopyPass | Self::RecordComputePass => {
                RendererGraphPhase::RenderRecord
            }
            Self::SubmitQueue => RendererGraphPhase::RenderSubmit,
            Self::PresentSurface => RendererGraphPhase::RenderPresent,
            Self::DiagnosticCapture => RendererGraphPhase::RendererDiagnosticsFlush,
        }
    }

    /// Legacy node class projection used by existing reports.
    #[inline]
    pub const fn node_class(self) -> RendererNodeClass {
        match self {
            Self::ExtractViews
            | Self::ExtractRenderables
            | Self::ExtractLights
            | Self::ExtractUiSurfaces => RendererNodeClass::Extraction,
            Self::PrepareMaterialChunk
            | Self::PrepareMeshChunk
            | Self::PrepareUploadBatch
            | Self::ShaderReflect
            | Self::PipelineWarmup => RendererNodeClass::AssetPrepare,
            Self::PrepareGpuSceneChunk
            | Self::VisibilityCullChunk
            | Self::VirtualGeometryPageResolve
            | Self::ImportEcsRenderArtifacts
            | Self::RetireGpuArtifacts => RendererNodeClass::ArtifactRealization,
            Self::ClusterBuild | Self::VirtualShadowPageResolve => RendererNodeClass::LuxGraph,
            Self::FrameGraphBuild | Self::NativeUiPacketCompose => RendererNodeClass::PacketBuild,
            Self::FrameGraphCompile => RendererNodeClass::PassCompile,
            Self::RecordPass | Self::RecordCopyPass | Self::RecordComputePass => {
                RendererNodeClass::CommandRecord
            }
            Self::SubmitQueue => RendererNodeClass::Submit,
            Self::PresentSurface => RendererNodeClass::Present,
            Self::DiagnosticCapture => RendererNodeClass::Diagnostics,
        }
    }

    /// Whether this work is optional and therefore must never block
    /// present.
    #[inline]
    pub const fn optional_for_present(self) -> bool {
        matches!(
            self,
            Self::DiagnosticCapture
                | Self::PipelineWarmup
                | Self::ShaderReflect
                | Self::RetireGpuArtifacts
        )
    }

    /// Whether this work records a required command buffer pass.
    #[inline]
    pub const fn is_record(self) -> bool {
        matches!(
            self,
            Self::RecordPass | Self::RecordCopyPass | Self::RecordComputePass
        )
    }

    /// Whether this work participates in the submit/present path.
    #[inline]
    pub const fn is_submit_present(self) -> bool {
        matches!(self, Self::SubmitQueue | Self::PresentSurface)
    }
}

/// Static renderer phase proof marker.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct RendererStaticPhaseProof;

impl RendererStaticPhaseProof {
    /// Product renderer phase chain. The typestate frame API below
    /// prevents callers from expressing present-before-submit,
    /// submit-before-record, record-before-compile, compile-before-build,
    /// queue-before-visibility, or prepare-scene-before-extraction.
    pub const PRODUCT: Self = Self;

    /// Canonical static phase chain from extraction through diagnostics.
    #[inline]
    pub const fn chain(&self) -> &'static [RendererWorkKind] {
        &[
            RendererWorkKind::ExtractViews,
            RendererWorkKind::ExtractRenderables,
            RendererWorkKind::ExtractLights,
            RendererWorkKind::ExtractUiSurfaces,
            RendererWorkKind::PrepareMaterialChunk,
            RendererWorkKind::PrepareMeshChunk,
            RendererWorkKind::PrepareUploadBatch,
            RendererWorkKind::PrepareGpuSceneChunk,
            RendererWorkKind::VisibilityCullChunk,
            RendererWorkKind::ClusterBuild,
            RendererWorkKind::FrameGraphBuild,
            RendererWorkKind::FrameGraphCompile,
            RendererWorkKind::RecordPass,
            RendererWorkKind::SubmitQueue,
            RendererWorkKind::PresentSurface,
            RendererWorkKind::RetireGpuArtifacts,
            RendererWorkKind::DiagnosticCapture,
        ]
    }
}

/// Renderer frame typestate wrapper.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct RendererFrame<State> {
    /// Frame index.
    pub frame_index: u64,
    /// View/asset generation.
    pub generation: u64,
    _state: PhantomData<State>,
}

impl<State> RendererFrame<State> {
    #[inline]
    const fn cast<Next>(self) -> RendererFrame<Next> {
        RendererFrame {
            frame_index: self.frame_index,
            generation: self.generation,
            _state: PhantomData,
        }
    }
}

/// Extracted renderer frame state.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Extracted;
/// Prepared renderer frame state.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Prepared;
/// Queued renderer frame state.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Queued;
/// Graph-compiled renderer frame state.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct GraphCompiled;
/// Recorded renderer frame state.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Recorded;
/// Submitted renderer frame state.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Submitted;
/// Presented renderer frame state.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct Presented;

impl RendererFrame<Extracted> {
    /// Construct the first proven frame state after extraction.
    #[must_use]
    pub const fn new(frame_index: u64, generation: u64) -> Self {
        Self {
            frame_index,
            generation,
            _state: PhantomData,
        }
    }

    /// Prepare assets and scene data.
    #[must_use]
    pub const fn prepare(self) -> RendererFrame<Prepared> {
        self.cast()
    }
}

impl RendererFrame<Prepared> {
    /// Complete visibility and queue/frame-graph build prerequisites.
    #[must_use]
    pub const fn queue(self) -> RendererFrame<Queued> {
        self.cast()
    }
}

impl RendererFrame<Queued> {
    /// Compile the frame graph.
    #[must_use]
    pub const fn compile_graph(self) -> RendererFrame<GraphCompiled> {
        self.cast()
    }
}

impl RendererFrame<GraphCompiled> {
    /// Record all required passes.
    #[must_use]
    pub const fn record(self) -> RendererFrame<Recorded> {
        self.cast()
    }
}

impl RendererFrame<Recorded> {
    /// Submit the required queue work.
    #[must_use]
    pub const fn submit(self) -> RendererFrame<Submitted> {
        self.cast()
    }
}

impl RendererFrame<Submitted> {
    /// Present the surface.
    #[must_use]
    pub const fn present(self) -> RendererFrame<Presented> {
        self.cast()
    }
}

/// FS-10 renderer-graph node class taxonomy. 11 classes covering
/// every typed work-node shape the scheduler routes through the
/// renderer.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[repr(u8)]
#[non_exhaustive]
pub enum RendererNodeClass {
    /// ECS → render world extraction node.
    Extraction = 0,
    /// Asset prep node (texture / mesh / shader).
    AssetPrepare = 1,
    /// Render artifact realization node fed by ECS-derived rows.
    ArtifactRealization = 2,
    /// Lux graph build / dispatch node.
    LuxGraph = 3,
    /// Pass compilation node (pipeline materialization).
    PassCompile = 4,
    /// Packet build node (frame-graph packet emission).
    PacketBuild = 5,
    /// Command-buffer recording node.
    CommandRecord = 6,
    /// Queue submission node.
    Submit = 7,
    /// Swapchain present node.
    Present = 8,
    /// Readback node (CPU-side staging consume).
    Readback = 9,
    /// Renderer-side diagnostics emission node.
    Diagnostics = 10,
}

impl RendererNodeClass {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Extraction => "extraction",
            Self::AssetPrepare => "asset_prepare",
            Self::ArtifactRealization => "artifact_realization",
            Self::LuxGraph => "lux_graph",
            Self::PassCompile => "pass_compile",
            Self::PacketBuild => "packet_build",
            Self::CommandRecord => "command_record",
            Self::Submit => "submit",
            Self::Present => "present",
            Self::Readback => "readback",
            Self::Diagnostics => "diagnostics",
        }
    }

    /// Every variant in declaration order.
    #[inline]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Extraction,
            Self::AssetPrepare,
            Self::ArtifactRealization,
            Self::LuxGraph,
            Self::PassCompile,
            Self::PacketBuild,
            Self::CommandRecord,
            Self::Submit,
            Self::Present,
            Self::Readback,
            Self::Diagnostics,
        ]
    }

    /// Map this node class to its primary graph phase. Phase →
    /// class is one-to-many; class → phase is the canonical
    /// inverse picking the most-natural owning phase.
    #[inline]
    pub const fn primary_phase(self) -> RendererGraphPhase {
        match self {
            Self::Extraction => RendererGraphPhase::RenderExtract,
            Self::AssetPrepare => RendererGraphPhase::RenderPrepareAssets,
            Self::ArtifactRealization => RendererGraphPhase::RenderPrepareScene,
            Self::LuxGraph => RendererGraphPhase::RenderLuxPlan,
            Self::PassCompile => RendererGraphPhase::RenderGraphCompile,
            Self::PacketBuild => RendererGraphPhase::RenderGraphBuild,
            Self::CommandRecord => RendererGraphPhase::RenderRecord,
            Self::Submit => RendererGraphPhase::RenderSubmit,
            Self::Present => RendererGraphPhase::RenderPresent,
            Self::Readback => RendererGraphPhase::RendererDiagnosticsFlush,
            Self::Diagnostics => RendererGraphPhase::RendererDiagnosticsFlush,
        }
    }
}

/// FS-10 typed Lux-specific node kinds. 16 kinds covering the
/// canonical Lux pipeline + HDR post path.
///
/// Each kind maps to a representative [`FrameGraphPassRole`] via
/// [`LuxNodeKind::pass_role`]; the canonical Lux ordering is
/// preserved through [`LuxNodeKind::lux_order_key`] (delegates to
/// the typed `FrameGraphPassRole::lux_order_key` for variants the
/// renderer already orders).
///
/// Godrays and Volumetric are intentionally lumped onto
/// representative Lux pass roles
/// (`LuxVolumetricComposite` and `LuxVolumetricIntegrate`
/// respectively); the underlying `FrameGraphPassRole` enum carries
/// the finer 5-variant volumetric breakdown the renderer uses for
/// graph compilation. `LuxNodeKind` is the scheduling-side
/// classification, not the graph-side enumeration.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Ord, PartialOrd)]
#[repr(u8)]
#[non_exhaustive]
pub enum LuxNodeKind {
    /// Upload per-frame light buffers.
    LuxUploadLightBuffers = 0,
    /// Cluster lights into the cluster grid.
    LuxClusterLights = 1,
    /// Direct lighting pass.
    LuxDirectLighting = 2,
    /// Shadow request build.
    LuxShadowRequests = 3,
    /// Virtual shadow page render.
    LuxVirtualShadowPages = 4,
    /// GI + reflection trace.
    LuxGiReflection = 5,
    /// Volumetric inject / integrate / composite (umbrella — see the
    /// finer FS-M1.4E variants `LuxVolumetricFogInject` etc. for the
    /// per-pass shape the post-V2.6 volumetric chain uses).
    LuxVolumetric = 6,
    /// Godrays (volumetric + screen-space fallback).
    LuxGodrays = 7,
    /// Post-process exposure histogram pass.
    PostProcessExposureHistogram = 8,
    /// Post-process exposure adapt pass.
    PostProcessExposureAdapt = 9,
    /// Post-process bloom prefilter pass.
    PostProcessBloomPrefilter = 10,
    /// Post-process bloom downsample pass.
    PostProcessBloomDownsample = 11,
    /// Post-process bloom upsample pass.
    PostProcessBloomUpsample = 12,
    /// Post-process bloom composite pass.
    PostProcessBloomComposite = 13,
    /// Tonemap pass.
    Tonemap = 14,
    /// Final output transform.
    FinalOutput = 15,
    /// FS-M1.4E: typed volumetric fog density injection pass.
    LuxVolumetricFogInject = 16,
    /// FS-M1.4E: typed volumetric light injection pass.
    LuxVolumetricLightInject = 17,
    /// FS-M1.4E: typed volumetric integrate pass.
    LuxVolumetricIntegrate = 18,
    /// FS-M1.4E: typed volumetric composite-back-into-scene pass.
    LuxVolumetricComposite = 19,
    /// FS-M1.4E (Pass C7.2): typed cloud shadow project pass.
    LuxCloudShadowProject = 20,
    /// FS-M1.4E (Pass C7.2): typed cloud shadow filter pass.
    LuxCloudShadowFilter = 21,
    /// FS-M1.4E (Pass C7.2): typed cloud shadow register-layer pass
    /// (publishes the typed transmittance into the Lux aux-layer
    /// slot).
    LuxCloudShadowRegisterLayer = 22,
    /// FS-M1.4E (Pass C7.3): typed cloud shadow sample pack — folds
    /// the projected + filtered cloud transmittance into the packed
    /// sample format the lighting passes consume.
    CloudShadowSamplePack = 23,
}

impl LuxNodeKind {
    /// Return the canonical stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::LuxUploadLightBuffers => "lux_upload_light_buffers",
            Self::LuxClusterLights => "lux_cluster_lights",
            Self::LuxDirectLighting => "lux_direct_lighting",
            Self::LuxShadowRequests => "lux_shadow_requests",
            Self::LuxVirtualShadowPages => "lux_virtual_shadow_pages",
            Self::LuxGiReflection => "lux_gi_reflection",
            Self::LuxVolumetric => "lux_volumetric",
            Self::LuxGodrays => "lux_godrays",
            Self::PostProcessExposureHistogram => "post_process_exposure_histogram",
            Self::PostProcessExposureAdapt => "post_process_exposure_adapt",
            Self::PostProcessBloomPrefilter => "post_process_bloom_prefilter",
            Self::PostProcessBloomDownsample => "post_process_bloom_downsample",
            Self::PostProcessBloomUpsample => "post_process_bloom_upsample",
            Self::PostProcessBloomComposite => "post_process_bloom_composite",
            Self::Tonemap => "tonemap",
            Self::FinalOutput => "final_output",
            Self::LuxVolumetricFogInject => "lux_volumetric_fog_inject",
            Self::LuxVolumetricLightInject => "lux_volumetric_light_inject",
            Self::LuxVolumetricIntegrate => "lux_volumetric_integrate",
            Self::LuxVolumetricComposite => "lux_volumetric_composite",
            Self::LuxCloudShadowProject => "lux_cloud_shadow_project",
            Self::LuxCloudShadowFilter => "lux_cloud_shadow_filter",
            Self::LuxCloudShadowRegisterLayer => "lux_cloud_shadow_register_layer",
            Self::CloudShadowSamplePack => "cloud_shadow_sample_pack",
        }
    }

    /// Every variant in canonical execution order.
    #[inline]
    pub const fn all() -> &'static [Self] {
        &[
            Self::LuxUploadLightBuffers,
            Self::LuxClusterLights,
            Self::LuxShadowRequests,
            Self::LuxVirtualShadowPages,
            Self::LuxCloudShadowProject,
            Self::LuxCloudShadowFilter,
            Self::CloudShadowSamplePack,
            Self::LuxCloudShadowRegisterLayer,
            Self::LuxDirectLighting,
            Self::LuxGiReflection,
            Self::LuxVolumetric,
            Self::LuxVolumetricFogInject,
            Self::LuxVolumetricLightInject,
            Self::LuxVolumetricIntegrate,
            Self::LuxVolumetricComposite,
            Self::LuxGodrays,
            Self::PostProcessExposureHistogram,
            Self::PostProcessExposureAdapt,
            Self::PostProcessBloomPrefilter,
            Self::PostProcessBloomDownsample,
            Self::PostProcessBloomUpsample,
            Self::PostProcessBloomComposite,
            Self::Tonemap,
            Self::FinalOutput,
        ]
    }

    /// Map this Lux node onto a representative [`FrameGraphPassRole`].
    /// The renderer's graph compiler may produce multiple frame-graph
    /// passes for one `LuxNodeKind` (e.g. Volumetric expands to
    /// 5 pass roles); this accessor returns the representative role
    /// the scheduler uses for lane / priority decisions.
    #[inline]
    pub const fn pass_role(self) -> FrameGraphPassRole {
        match self {
            Self::LuxUploadLightBuffers => FrameGraphPassRole::LuxUploadLightBuffers,
            Self::LuxClusterLights => FrameGraphPassRole::LuxClusterLights,
            Self::LuxDirectLighting => FrameGraphPassRole::LuxDirectLighting,
            Self::LuxShadowRequests => FrameGraphPassRole::LuxShadowRequests,
            Self::LuxVirtualShadowPages => FrameGraphPassRole::LuxVirtualShadowPages,
            Self::LuxGiReflection => FrameGraphPassRole::LuxGiTrace,
            Self::LuxVolumetric => FrameGraphPassRole::LuxVolumetricIntegrate,
            Self::LuxGodrays => FrameGraphPassRole::LuxVolumetricComposite,
            Self::PostProcessExposureHistogram => FrameGraphPassRole::PostProcessExposure,
            Self::PostProcessExposureAdapt => FrameGraphPassRole::PostProcessExposure,
            Self::PostProcessBloomPrefilter => FrameGraphPassRole::PostProcessBloom,
            Self::PostProcessBloomDownsample => FrameGraphPassRole::PostProcessBloom,
            Self::PostProcessBloomUpsample => FrameGraphPassRole::PostProcessBloom,
            Self::PostProcessBloomComposite => FrameGraphPassRole::PostProcessBloom,
            Self::Tonemap => FrameGraphPassRole::PostProcessToneMapping,
            Self::FinalOutput => FrameGraphPassRole::PostProcessFinalOutputTransform,
            Self::LuxVolumetricFogInject => FrameGraphPassRole::LuxVolumetricFogInject,
            Self::LuxVolumetricLightInject => FrameGraphPassRole::LuxVolumetricLightInject,
            Self::LuxVolumetricIntegrate => FrameGraphPassRole::LuxVolumetricIntegrate,
            Self::LuxVolumetricComposite => FrameGraphPassRole::LuxVolumetricComposite,
            Self::LuxCloudShadowProject => FrameGraphPassRole::LuxCloudShadowProject,
            Self::LuxCloudShadowFilter => FrameGraphPassRole::LuxCloudShadowFilter,
            Self::LuxCloudShadowRegisterLayer => FrameGraphPassRole::LuxCloudShadowRegisterLayer,
            // Sample pack is renderer-internal scheduling glue; route
            // to the register-layer role for FS-7 lane / priority
            // classification.
            Self::CloudShadowSamplePack => FrameGraphPassRole::LuxCloudShadowRegisterLayer,
        }
    }

    /// Return a stable order key. Lux nodes preserve the renderer's
    /// canonical `lux_order_key` (100–340 range); the HDR post path
    /// follows in the 1000+ range so post always sorts after Lux.
    #[inline]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::LuxUploadLightBuffers => 100,
            Self::LuxClusterLights => 110,
            Self::LuxShadowRequests => 120,
            Self::LuxVirtualShadowPages => 130,
            Self::LuxCloudShadowProject => 140,
            Self::LuxCloudShadowFilter => 145,
            Self::CloudShadowSamplePack => 148,
            Self::LuxCloudShadowRegisterLayer => 150,
            Self::LuxDirectLighting => 200,
            Self::LuxGiReflection => 210,
            Self::LuxVolumetric => 300,
            Self::LuxVolumetricFogInject => 302,
            Self::LuxVolumetricLightInject => 305,
            Self::LuxVolumetricIntegrate => 310,
            Self::LuxVolumetricComposite => 320,
            Self::LuxGodrays => 340,
            Self::PostProcessExposureHistogram => 1000,
            Self::PostProcessExposureAdapt => 1010,
            Self::PostProcessBloomPrefilter => 1100,
            Self::PostProcessBloomDownsample => 1110,
            Self::PostProcessBloomUpsample => 1120,
            Self::PostProcessBloomComposite => 1130,
            Self::Tonemap => 1200,
            Self::FinalOutput => 1300,
        }
    }
}

/// FS-10 renderer work descriptor carried by every
/// [`WorkNode<RendererWork>`] in a [`RendererWorkGraph`].
///
/// `generation` is the caller-supplied view / camera / asset
/// generation key; the scheduler invalidates queued work whose
/// generation no longer matches the current camera/view (cancelled
/// page warmup, stale extraction, etc.). The descriptor carries
/// **no backend handles** — wgpu / DX12 / Vulkan / Metal objects
/// live in the renderer's resource manager, not in scheduling
/// decisions.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct RendererWork {
    /// Tier-5 work kind.
    pub kind: RendererWorkKind,
    /// Typed node class.
    pub class: RendererNodeClass,
    /// Graph phase this node belongs to.
    pub phase: RendererGraphPhase,
    /// Optional [`FrameGraphPassRole`] (Some for graph-compile,
    /// record, packet-build, submit, present; None for extraction /
    /// asset prep / diagnostics).
    pub pass_role: Option<FrameGraphPassRole>,
    /// Optional [`LuxNodeKind`] (Some for Lux-graph / Lux-record
    /// nodes; None for non-Lux phases).
    pub lux_kind: Option<LuxNodeKind>,
    /// Optional [`LuxResourceIntentKind`] for nodes that allocate
    /// or transition a Lux resource.
    pub resource_intent: Option<LuxResourceIntentKind>,
    /// View / camera / asset generation. Stale generations cancel
    /// at admission.
    pub generation: u64,
}

impl RendererWork {
    /// Construct a work descriptor from a Tier-5 work kind.
    #[must_use]
    pub const fn for_kind(kind: RendererWorkKind, generation: u64) -> Self {
        Self {
            kind,
            class: kind.node_class(),
            phase: kind.phase(),
            pass_role: None,
            lux_kind: None,
            resource_intent: None,
            generation,
        }
    }

    /// Construct a work descriptor for an extraction node.
    #[must_use]
    pub const fn extraction(generation: u64) -> Self {
        Self {
            kind: RendererWorkKind::ExtractViews,
            class: RendererNodeClass::Extraction,
            phase: RendererGraphPhase::RenderExtract,
            pass_role: None,
            lux_kind: None,
            resource_intent: None,
            generation,
        }
    }

    /// Construct a work descriptor for a Lux graph node.
    #[must_use]
    pub const fn lux(kind: LuxNodeKind, generation: u64) -> Self {
        let work_kind = lux_node_to_work_kind(kind);
        Self {
            kind: work_kind,
            class: RendererNodeClass::LuxGraph,
            phase: RendererGraphPhase::RenderLuxPlan,
            pass_role: Some(kind.pass_role()),
            lux_kind: Some(kind),
            resource_intent: None,
            generation,
        }
    }

    /// Construct a work descriptor for a present node.
    #[must_use]
    pub const fn present(generation: u64) -> Self {
        Self {
            kind: RendererWorkKind::PresentSurface,
            class: RendererNodeClass::Present,
            phase: RendererGraphPhase::RenderPresent,
            pass_role: Some(FrameGraphPassRole::Present),
            lux_kind: None,
            resource_intent: None,
            generation,
        }
    }

    /// Construct a work descriptor for a submit node.
    #[must_use]
    pub const fn submit(generation: u64) -> Self {
        Self {
            kind: RendererWorkKind::SubmitQueue,
            class: RendererNodeClass::Submit,
            phase: RendererGraphPhase::RenderSubmit,
            pass_role: Some(FrameGraphPassRole::Compose),
            lux_kind: None,
            resource_intent: None,
            generation,
        }
    }

    /// Construct a work descriptor for a diagnostics node.
    #[must_use]
    pub const fn diagnostics(generation: u64) -> Self {
        Self {
            kind: RendererWorkKind::DiagnosticCapture,
            class: RendererNodeClass::Diagnostics,
            phase: RendererGraphPhase::RendererDiagnosticsFlush,
            pass_role: Some(FrameGraphPassRole::DiagnosticsReadback),
            lux_kind: None,
            resource_intent: None,
            generation,
        }
    }
}

#[inline]
const fn lux_node_to_work_kind(kind: LuxNodeKind) -> RendererWorkKind {
    match kind {
        LuxNodeKind::LuxUploadLightBuffers => RendererWorkKind::PrepareUploadBatch,
        LuxNodeKind::LuxClusterLights => RendererWorkKind::ClusterBuild,
        LuxNodeKind::LuxShadowRequests
        | LuxNodeKind::LuxVirtualShadowPages
        | LuxNodeKind::LuxCloudShadowProject
        | LuxNodeKind::LuxCloudShadowFilter
        | LuxNodeKind::LuxCloudShadowRegisterLayer
        | LuxNodeKind::CloudShadowSamplePack => RendererWorkKind::VirtualShadowPageResolve,
        LuxNodeKind::LuxGiReflection
        | LuxNodeKind::LuxVolumetric
        | LuxNodeKind::LuxGodrays
        | LuxNodeKind::LuxVolumetricFogInject
        | LuxNodeKind::LuxVolumetricLightInject
        | LuxNodeKind::LuxVolumetricIntegrate
        | LuxNodeKind::LuxVolumetricComposite
        | LuxNodeKind::PostProcessExposureHistogram
        | LuxNodeKind::PostProcessExposureAdapt
        | LuxNodeKind::PostProcessBloomPrefilter
        | LuxNodeKind::PostProcessBloomDownsample
        | LuxNodeKind::PostProcessBloomUpsample
        | LuxNodeKind::PostProcessBloomComposite => RendererWorkKind::RecordComputePass,
        LuxNodeKind::LuxDirectLighting | LuxNodeKind::Tonemap | LuxNodeKind::FinalOutput => {
            RendererWorkKind::RecordPass
        }
    }
}

/// FS-10 renderer-graph schedule report. 12 metric fields, no
/// backend handles, all primitives.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct RendererGraphScheduleReport {
    /// Frame graph build duration.
    pub graph_build_ns: u64,
    /// Frame graph validation duration.
    pub graph_validation_ns: u64,
    /// Frame graph compile duration.
    pub graph_compile_ns: u64,
    /// Queue wait per [`RendererGraphPhase`] (11 phases).
    pub queue_wait_by_phase_ns: [u64; 11],
    /// Packet build duration.
    pub packet_build_ns: u64,
    /// Command-buffer record duration.
    pub command_record_ns: u64,
    /// Queue submit wait duration.
    pub submit_wait_ns: u64,
    /// Bytes uploaded to virtual-texture pages this frame.
    pub page_upload_bytes: u64,
    /// Number of page warmup units cancelled by generation
    /// invalidation.
    pub cancelled_page_work_count: u32,
    /// Number of page warmup units coalesced into another pending
    /// upload.
    pub coalesced_page_work_count: u32,
    /// Number of frame deadline misses this frame.
    pub frame_deadline_misses: u32,
    /// Render-worker CPU utilization in micro-percent
    /// (`0 ..= 1_000_000`, i.e. `1_000_000 == 100%`).
    pub render_worker_utilization_micros: u32,
}

impl RendererGraphScheduleReport {
    /// Construct an empty report (all zeros).
    #[must_use]
    pub const fn new() -> Self {
        Self {
            graph_build_ns: 0,
            graph_validation_ns: 0,
            graph_compile_ns: 0,
            queue_wait_by_phase_ns: [0; 11],
            packet_build_ns: 0,
            command_record_ns: 0,
            submit_wait_ns: 0,
            page_upload_bytes: 0,
            cancelled_page_work_count: 0,
            coalesced_page_work_count: 0,
            frame_deadline_misses: 0,
            render_worker_utilization_micros: 0,
        }
    }

    /// Record bytes uploaded for a page warmup unit.
    pub fn record_page_upload(&mut self, bytes: u64) {
        self.page_upload_bytes = self.page_upload_bytes.saturating_add(bytes);
    }

    /// Record a page warmup unit cancelled by generation
    /// invalidation. Does **not** touch `page_upload_bytes` — the
    /// scheduler-side cancellation prevents stale uploads from ever
    /// being recorded.
    pub fn record_cancelled_page(&mut self) {
        self.cancelled_page_work_count = self.cancelled_page_work_count.saturating_add(1);
    }

    /// Record a page warmup unit coalesced into another pending
    /// upload.
    pub fn record_coalesced_page(&mut self) {
        self.coalesced_page_work_count = self.coalesced_page_work_count.saturating_add(1);
    }

    /// Record a frame deadline miss.
    pub fn record_deadline_miss(&mut self) {
        self.frame_deadline_misses = self.frame_deadline_misses.saturating_add(1);
    }
}

/// FS-10 renderer work graph type alias — the renderer's
/// instantiation of FS-8's [`WorkGraph<W>`] with a typed
/// [`RendererWork`] descriptor.
pub type RendererWorkGraph = WorkGraph<RendererWork>;

/// Construct a `WorkNode<RendererWork>` for a renderer work
/// descriptor. The node lands on the FS-7 [`ScheduleLane`] that
/// matches the work's phase via
/// [`RendererGraphPhase::to_schedule_lane`].
#[must_use]
pub fn renderer_work_node(id: WorkNodeId, work: RendererWork) -> WorkNode<RendererWork> {
    let lane = work.phase.to_schedule_lane();
    let work_label = work.kind.label();
    let order_key = work.lux_kind.map_or(0, LuxNodeKind::order_key);
    let priority = if matches!(
        work.phase,
        RendererGraphPhase::RenderSubmit | RendererGraphPhase::RenderPresent,
    ) {
        TaskPriority::Critical
    } else if matches!(work.phase, RendererGraphPhase::RendererDiagnosticsFlush) {
        TaskPriority::Idle
    } else {
        TaskPriority::High
    };
    WorkNode::new(
        id,
        ScheduleDomain::Renderer,
        lane,
        WorkPhase::Compute,
        priority,
        ScheduleBudget::UNBOUNDED,
        ScheduleDeadline::Frame,
        work,
    )
    .with_deterministic_descriptor(DeterministicDescriptor::new(
        work_label,
        u64::from(order_key),
    ))
    .with_liveness_contract(renderer_work_liveness(work))
}

/// Construct an empty [`RendererWorkGraph`].
#[must_use]
pub fn renderer_work_graph(graph_id: WorkGraphId) -> RendererWorkGraph {
    use fun_scheduler_types::work_graph::{CommitPolicy, GraphExecutionMode};
    WorkGraph::new(
        graph_id,
        ScheduleDomain::Renderer,
        GraphExecutionMode::SingleThreadDeterministic,
        CommitPolicy::DescriptorOrder,
    )
}

fn renderer_work_liveness(work: RendererWork) -> WorkNodeLiveness {
    let liveness = WorkNodeLiveness::new();
    if matches!(
        work.kind,
        RendererWorkKind::DiagnosticCapture
            | RendererWorkKind::ShaderReflect
            | RendererWorkKind::PipelineWarmup
    ) {
        liveness.with_frame_worker_allowed(false)
    } else {
        liveness
    }
}

/// Renderer-specific liveness rejection.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
#[non_exhaustive]
pub enum RendererGraphLivenessError {
    /// Generic work-graph invariant failed.
    GraphInvariant(GraphInvariantError),
    /// No submit node exists.
    MissingSubmit,
    /// No present node exists.
    MissingPresent,
    /// Present must have exactly one dependency.
    PresentDependencyCount {
        /// Present node.
        present: WorkNodeId,
        /// Dependency count.
        count: usize,
    },
    /// Present must wait on submit only.
    PresentDependencyNotSubmit {
        /// Present node.
        present: WorkNodeId,
        /// Invalid dependency.
        dependency: WorkNodeId,
    },
    /// Submit does not wait on a required record node.
    SubmitMissingRequiredRecord {
        /// Submit node.
        submit: WorkNodeId,
        /// Required record node.
        record: WorkNodeId,
    },
    /// Optional work gates present.
    OptionalWorkBlocksPresent {
        /// Optional node.
        optional: WorkNodeId,
        /// Present node.
        present: WorkNodeId,
    },
    /// A record node waits on present.
    RecordWaitsOnPresent {
        /// Record node.
        record: WorkNodeId,
        /// Present node.
        present: WorkNodeId,
    },
    /// Frame-critical work appears after submit other than present.
    FrameCriticalWorkAfterSubmit {
        /// Submit node.
        submit: WorkNodeId,
        /// Later frame-critical node.
        node: WorkNodeId,
    },
    /// GPU readback/diagnostic capture is on a frame worker.
    GpuReadbackOnFrameWorker {
        /// Node.
        node: WorkNodeId,
    },
    /// Optional page eviction blocks visible/required page admission.
    OptionalPageEvictBlocksAdmit {
        /// Optional eviction node.
        evict: WorkNodeId,
        /// Required admission node.
        admit: WorkNodeId,
    },
}

impl RendererGraphLivenessError {
    /// Stable label.
    #[inline]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GraphInvariant(_) => "graph_invariant",
            Self::MissingSubmit => "missing_submit",
            Self::MissingPresent => "missing_present",
            Self::PresentDependencyCount { .. } => "present_dependency_count",
            Self::PresentDependencyNotSubmit { .. } => "present_dependency_not_submit",
            Self::SubmitMissingRequiredRecord { .. } => "submit_missing_required_record",
            Self::OptionalWorkBlocksPresent { .. } => "optional_work_blocks_present",
            Self::RecordWaitsOnPresent { .. } => "record_waits_on_present",
            Self::FrameCriticalWorkAfterSubmit { .. } => "frame_critical_work_after_submit",
            Self::GpuReadbackOnFrameWorker { .. } => "gpu_readback_on_frame_worker",
            Self::OptionalPageEvictBlocksAdmit { .. } => "optional_page_evict_blocks_admit",
        }
    }
}

impl fmt::Display for RendererGraphLivenessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::GraphInvariant(err) => write!(f, "renderer graph invariant failed: {err}"),
            Self::MissingSubmit => write!(f, "renderer graph has no submit node"),
            Self::MissingPresent => write!(f, "renderer graph has no present node"),
            Self::PresentDependencyCount { present, count } => write!(
                f,
                "present node {} has {} dependencies; expected exactly one submit dependency",
                present.get(),
                count
            ),
            Self::PresentDependencyNotSubmit {
                present,
                dependency,
            } => write!(
                f,
                "present node {} depends on non-submit node {}",
                present.get(),
                dependency.get()
            ),
            Self::SubmitMissingRequiredRecord { submit, record } => write!(
                f,
                "submit node {} does not wait on required record node {}",
                submit.get(),
                record.get()
            ),
            Self::OptionalWorkBlocksPresent { optional, present } => write!(
                f,
                "optional renderer node {} blocks present node {}",
                optional.get(),
                present.get()
            ),
            Self::RecordWaitsOnPresent { record, present } => write!(
                f,
                "record node {} waits on present node {}",
                record.get(),
                present.get()
            ),
            Self::FrameCriticalWorkAfterSubmit { submit, node } => write!(
                f,
                "frame-critical node {} is scheduled after submit node {}",
                node.get(),
                submit.get()
            ),
            Self::GpuReadbackOnFrameWorker { node } => {
                write!(f, "GPU readback node {} is on a frame worker", node.get())
            }
            Self::OptionalPageEvictBlocksAdmit { evict, admit } => write!(
                f,
                "optional page eviction node {} blocks required page admission node {}",
                evict.get(),
                admit.get()
            ),
        }
    }
}

impl From<GraphInvariantError> for RendererGraphLivenessError {
    fn from(value: GraphInvariantError) -> Self {
        Self::GraphInvariant(value)
    }
}

/// Validate renderer-specific liveness invariants on top of the
/// generic scheduler proof.
pub fn validate_renderer_graph_liveness(
    graph: &RendererWorkGraph,
) -> Result<(), RendererGraphLivenessError> {
    let submits = nodes_with_kind(graph, RendererWorkKind::SubmitQueue);
    let presents = nodes_with_kind(graph, RendererWorkKind::PresentSurface);
    if submits.is_empty() {
        return Err(RendererGraphLivenessError::MissingSubmit);
    }
    if presents.is_empty() {
        return Err(RendererGraphLivenessError::MissingPresent);
    }

    for present in &presents {
        let deps = incoming_dependencies(graph, *present);
        if deps.len() != 1 {
            return Err(RendererGraphLivenessError::PresentDependencyCount {
                present: *present,
                count: deps.len(),
            });
        }
        let dependency = deps[0];
        if graph
            .node(dependency)
            .is_none_or(|node| node.work_descriptor.kind != RendererWorkKind::SubmitQueue)
        {
            return Err(RendererGraphLivenessError::PresentDependencyNotSubmit {
                present: *present,
                dependency,
            });
        }
    }

    for node in &graph.nodes {
        if node.work_descriptor.kind == RendererWorkKind::DiagnosticCapture
            && node.lane.default_lane_kind() == fun_scheduler_types::lane::LaneKind::Frame
        {
            return Err(RendererGraphLivenessError::GpuReadbackOnFrameWorker { node: node.id });
        }
    }

    for present in &presents {
        for optional in graph
            .nodes
            .iter()
            .filter(|node| node.work_descriptor.kind.optional_for_present())
        {
            if reaches(graph, optional.id, *present) {
                return Err(RendererGraphLivenessError::OptionalWorkBlocksPresent {
                    optional: optional.id,
                    present: *present,
                });
            }
        }
    }

    for submit in &submits {
        for record in graph
            .nodes
            .iter()
            .filter(|node| node.work_descriptor.kind.is_record())
        {
            if !reaches(graph, record.id, *submit) {
                return Err(RendererGraphLivenessError::SubmitMissingRequiredRecord {
                    submit: *submit,
                    record: record.id,
                });
            }
        }
    }

    for record in graph
        .nodes
        .iter()
        .filter(|node| node.work_descriptor.kind.is_record())
    {
        for present in &presents {
            if reaches(graph, *present, record.id) {
                return Err(RendererGraphLivenessError::RecordWaitsOnPresent {
                    record: record.id,
                    present: *present,
                });
            }
        }
    }

    for submit in &submits {
        for node in &graph.nodes {
            if node.id != *submit
                && node.work_descriptor.kind != RendererWorkKind::PresentSurface
                && node.lane.default_lane_kind() == fun_scheduler_types::lane::LaneKind::Frame
                && reaches(graph, *submit, node.id)
            {
                return Err(RendererGraphLivenessError::FrameCriticalWorkAfterSubmit {
                    submit: *submit,
                    node: node.id,
                });
            }
        }
    }

    for evict in nodes_with_kind(graph, RendererWorkKind::RetireGpuArtifacts) {
        for admit in nodes_with_kind(graph, RendererWorkKind::ImportEcsRenderArtifacts) {
            if reaches(graph, evict, admit) {
                return Err(RendererGraphLivenessError::OptionalPageEvictBlocksAdmit {
                    evict,
                    admit,
                });
            }
        }
    }

    graph.liveness_proof()?;
    Ok(())
}

fn nodes_with_kind(graph: &RendererWorkGraph, kind: RendererWorkKind) -> Vec<WorkNodeId> {
    graph
        .nodes
        .iter()
        .filter_map(|node| (node.work_descriptor.kind == kind).then_some(node.id))
        .collect()
}

fn incoming_dependencies(graph: &RendererWorkGraph, node: WorkNodeId) -> Vec<WorkNodeId> {
    let mut deps = graph
        .node(node)
        .map(|node| node.dependencies.clone())
        .unwrap_or_default();
    for edge in &graph.edges {
        match *edge {
            WorkEdge::Dependency { from, to } if to == node => push_unique(&mut deps, from),
            WorkEdge::ResourceReadAfterWrite { writer, reader, .. } if reader == node => {
                push_unique(&mut deps, writer);
            }
            _ => {}
        }
    }
    deps
}

fn successors(graph: &RendererWorkGraph, node: WorkNodeId) -> Vec<WorkNodeId> {
    let mut out = Vec::new();
    for candidate in &graph.nodes {
        if candidate.dependencies.contains(&node) {
            push_unique(&mut out, candidate.id);
        }
    }
    for edge in &graph.edges {
        match *edge {
            WorkEdge::Dependency { from, to } if from == node => push_unique(&mut out, to),
            WorkEdge::ResourceReadAfterWrite { writer, reader, .. } if writer == node => {
                push_unique(&mut out, reader);
            }
            _ => {}
        }
    }
    out
}

fn reaches(graph: &RendererWorkGraph, from: WorkNodeId, to: WorkNodeId) -> bool {
    let mut stack = vec![from];
    let mut seen = vec![false; graph.node_count()];
    while let Some(node) = stack.pop() {
        if node == to {
            return true;
        }
        let idx = node.get() as usize;
        if idx >= seen.len() || seen[idx] {
            continue;
        }
        seen[idx] = true;
        for successor in successors(graph, node) {
            stack.push(successor);
        }
    }
    false
}

fn push_unique(nodes: &mut Vec<WorkNodeId>, node: WorkNodeId) {
    if !nodes.contains(&node) {
        nodes.push(node);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every [`FrameGraphPassRole`] variant the renderer ships, in
    /// declaration order. Used for exhaustive coverage tests so adding
    /// a new variant without classifying it lights up a compile-time
    /// failure here (the match in `pass_role_to_task_class` is
    /// exhaustive, so a new variant breaks the build first).
    const ALL_ROLES: &[FrameGraphPassRole] = &[
        FrameGraphPassRole::Clear,
        FrameGraphPassRole::StaticScenePlaceholder,
        FrameGraphPassRole::VirtualResourceFeedback,
        FrameGraphPassRole::NativeUiGpuImport,
        FrameGraphPassRole::UiImportPlaceholder,
        FrameGraphPassRole::UpscaleBoundary,
        FrameGraphPassRole::FrameGenerationBoundary,
        FrameGraphPassRole::PostProcessExposure,
        FrameGraphPassRole::PostProcessBloom,
        FrameGraphPassRole::PostProcessExposureHistogram,
        FrameGraphPassRole::PostProcessExposureAdapt,
        FrameGraphPassRole::PostProcessBloomPrefilter,
        FrameGraphPassRole::PostProcessBloomDownsample,
        FrameGraphPassRole::PostProcessBloomUpsample,
        FrameGraphPassRole::PostProcessBloomComposite,
        FrameGraphPassRole::PostProcessToneMapping,
        FrameGraphPassRole::PostProcessColorGradingLut,
        FrameGraphPassRole::PostProcessSharpening,
        FrameGraphPassRole::PostProcessDebugOverlay,
        FrameGraphPassRole::PostProcessFinalOutputTransform,
        FrameGraphPassRole::Compose,
        FrameGraphPassRole::DiagnosticsReadback,
        FrameGraphPassRole::Present,
        FrameGraphPassRole::LuxUploadLightBuffers,
        FrameGraphPassRole::LuxClusterLights,
        FrameGraphPassRole::LuxReservoirTemporalReuse,
        FrameGraphPassRole::LuxReservoirSpatialReuse,
        FrameGraphPassRole::LuxShadowRequests,
        FrameGraphPassRole::LuxVirtualShadowPages,
        FrameGraphPassRole::LuxVirtualShadowFilter,
        FrameGraphPassRole::LuxDirectLighting,
        FrameGraphPassRole::LuxGiTrace,
        FrameGraphPassRole::LuxGiCacheUpdate,
        FrameGraphPassRole::LuxReflectionTrace,
        FrameGraphPassRole::LuxDenoise,
        FrameGraphPassRole::LuxVolumetricFogInject,
        FrameGraphPassRole::LuxVolumetricLightInject,
        FrameGraphPassRole::LuxVolumetricTemporalReproject,
        FrameGraphPassRole::LuxVolumetricIntegrate,
        FrameGraphPassRole::LuxVolumetricComposite,
        FrameGraphPassRole::LuxDebugOverlay,
    ];

    fn is_lux(role: FrameGraphPassRole) -> bool {
        role.is_lux()
    }

    #[test]
    fn every_lux_pass_role_maps_to_one_lane() {
        // V2-3C test 1: every Lux pass role classifies to exactly one
        // canonical `TaskClass`. The mapping function is total
        // (compile-time exhaustive over `FrameGraphPassRole`); this
        // test asserts each Lux variant produces one of the four
        // expected classes and never silently falls through to a
        // background or blocking class.
        for role in ALL_ROLES.iter().copied().filter(|r| is_lux(*r)) {
            let class = pass_role_to_task_class(role);
            assert!(
                matches!(
                    class,
                    TaskClass::FrameCritical
                        | TaskClass::RenderPrepare
                        | TaskClass::RenderRecord
                        | TaskClass::Diagnostic
                ),
                "Lux role {} mapped to unexpected class {}",
                role.as_str(),
                class.label(),
            );
        }
    }

    #[test]
    fn every_lux_resource_intent_maps_to_one_budget_class() {
        // V2-3C test 2: every Lux resource intent kind maps to exactly
        // one `BudgetOrigin`. Exhaustive over `LuxResourceIntentKind::ALL`.
        for kind in LuxResourceIntentKind::ALL {
            let origin = resource_intent_to_budget_origin(kind);
            // Both legal targets are renderer-aligned origins.
            assert!(
                matches!(
                    origin,
                    BudgetOrigin::RendererPageBatch | BudgetOrigin::DiagnosticCapture
                ),
                "intent kind mapped to unexpected origin {}",
                origin.label(),
            );
        }
    }

    #[test]
    fn debug_overlay_work_never_maps_to_frame_critical() {
        // V2-3C test 3: debug overlay work never lands on
        // `FrameCritical`. Three roles carry debug semantics:
        // `PostProcessDebugOverlay`, `DiagnosticsReadback`,
        // `LuxDebugOverlay`. None of them may produce a frame-critical
        // classification.
        for role in [
            FrameGraphPassRole::PostProcessDebugOverlay,
            FrameGraphPassRole::DiagnosticsReadback,
            FrameGraphPassRole::LuxDebugOverlay,
        ] {
            let class = pass_role_to_task_class(role);
            assert_ne!(
                class,
                TaskClass::FrameCritical,
                "debug role {} must not be FrameCritical",
                role.as_str(),
            );
            // Positive assertion: debug roles land on Diagnostic.
            assert_eq!(
                class,
                TaskClass::Diagnostic,
                "{} should be Diagnostic",
                role.as_str()
            );
        }
    }

    #[test]
    fn present_and_record_critical_work_never_maps_to_background() {
        // V2-3C test 4: present-critical and record-critical work
        // never collapse to a background class. The renderer's
        // present-path roles (`Compose`, `Present`,
        // `PostProcessFinalOutputTransform`) must remain
        // `FrameCritical`; the record-band Lux roles must remain
        // `RenderRecord`.
        for role in [
            FrameGraphPassRole::Compose,
            FrameGraphPassRole::Present,
            FrameGraphPassRole::PostProcessFinalOutputTransform,
        ] {
            let class = pass_role_to_task_class(role);
            assert!(
                !matches!(class, TaskClass::Background | TaskClass::Throughput),
                "{} fell into a background class ({})",
                role.as_str(),
                class.label(),
            );
            assert_eq!(class, TaskClass::FrameCritical);
        }
        for role in [
            FrameGraphPassRole::LuxDirectLighting,
            FrameGraphPassRole::LuxGiTrace,
            FrameGraphPassRole::LuxReflectionTrace,
            FrameGraphPassRole::LuxDenoise,
            FrameGraphPassRole::LuxVolumetricFogInject,
            FrameGraphPassRole::LuxVolumetricLightInject,
            FrameGraphPassRole::LuxVolumetricTemporalReproject,
            FrameGraphPassRole::LuxVolumetricIntegrate,
            FrameGraphPassRole::LuxVolumetricComposite,
        ] {
            let class = pass_role_to_task_class(role);
            assert!(
                !matches!(class, TaskClass::Background | TaskClass::Throughput),
                "{} fell into a background class ({})",
                role.as_str(),
                class.label(),
            );
            assert_eq!(class, TaskClass::RenderRecord);
        }
    }

    #[test]
    fn all_mapping_labels_are_stable() {
        // V2-3C test 5: every label this module emits is a stable
        // `&'static str`. Compile-time ascription proves the type for
        // each label source; the runtime check asserts the labels are
        // non-empty and exhaustively cover the variant set.
        let _: &'static str = REASON_RENDERER_PRESENT_CRITICAL;
        let _: &'static str = REASON_RENDERER_DIAGNOSTIC_OVERLAY;
        let _: &'static str = REASON_RENDERER_PREPARE_PASS;
        let _: &'static str = REASON_RENDERER_RECORD_PASS;

        // Every classification produces a stable task-class label.
        for role in ALL_ROLES.iter().copied() {
            let class = pass_role_to_task_class(role);
            let class_label: &'static str = class.label();
            assert!(!class_label.is_empty());
            let reason: &'static str = derive_reason_code(role);
            assert!(!reason.is_empty());
        }

        // Every resource intent produces a stable budget-origin label.
        for kind in LuxResourceIntentKind::ALL {
            let origin = resource_intent_to_budget_origin(kind);
            let origin_label: &'static str = origin.label();
            assert!(!origin_label.is_empty());
        }

        // Plan struct: every field that emits text comes from a stable
        // static source.
        let plan = RendererSchedulePlanV1::for_role(
            42,
            FrameGraphPassRole::LuxDirectLighting,
            3,
            7,
            150,
            4096,
        );
        let _: &'static str = plan.reason_code;
        let _: &'static str = plan.render_phase.as_str();
        let _: &'static str = plan.schedule_lane.label();
        let rendered = format!("{plan}");
        assert!(rendered.contains("lux_direct_lighting"));
        assert!(rendered.contains("render_record"));
        assert!(rendered.contains("renderer_record_pass"));
    }

    #[test]
    fn for_role_classifier_propagates_correct_lane_and_reason() {
        // Lux upload lands on RenderPrepare with the prepare reason.
        let plan = RendererSchedulePlanV1::for_role(
            1,
            FrameGraphPassRole::LuxUploadLightBuffers,
            1,
            1,
            10,
            128,
        );
        assert_eq!(plan.schedule_lane, TaskClass::RenderPrepare);
        assert_eq!(plan.reason_code, REASON_RENDERER_PREPARE_PASS);

        // Present lands on FrameCritical with the present-critical reason.
        let plan = RendererSchedulePlanV1::for_role(2, FrameGraphPassRole::Present, 0, 0, 5, 0);
        assert_eq!(plan.schedule_lane, TaskClass::FrameCritical);
        assert_eq!(plan.reason_code, REASON_RENDERER_PRESENT_CRITICAL);

        // Debug overlay lands on Diagnostic.
        let plan =
            RendererSchedulePlanV1::for_role(3, FrameGraphPassRole::LuxDebugOverlay, 0, 1, 20, 64);
        assert_eq!(plan.schedule_lane, TaskClass::Diagnostic);
        assert_eq!(plan.reason_code, REASON_RENDERER_DIAGNOSTIC_OVERLAY);
    }

    #[test]
    fn plan_builder_chain_keeps_budget_and_deadline_overrides() {
        let plan = RendererSchedulePlanV1::for_role(
            10,
            FrameGraphPassRole::LuxClusterLights,
            2,
            4,
            42,
            512,
        )
        .with_budget(TaskBudget::new().with_cpu_ns(1_000_000))
        .with_deadline(Deadline(123_456));
        assert_eq!(plan.budget.cpu_ns, Some(1_000_000));
        assert_eq!(plan.deadline, Deadline(123_456));
        assert_eq!(plan.schedule_lane, TaskClass::RenderPrepare);
    }

    // ========================================================================
    // FS-10 tests
    // ========================================================================

    #[test]
    fn every_renderer_graph_phase_maps_to_a_schedule_lane() {
        // FS-10 test 1: every renderer graph role maps to a
        // scheduler lane.
        for phase in RendererGraphPhase::all().iter().copied() {
            let lane = phase.to_schedule_lane();
            let _: &'static str = lane.label();
            assert!(
                !lane.label().is_empty(),
                "phase {} lane is empty",
                phase.label()
            );
        }
    }

    #[test]
    fn critical_present_submit_work_cannot_be_downgraded_to_background() {
        // FS-10 test 2: critical present/submit work cannot be
        // downgraded to background. Verify Present and Submit land
        // on their canonical FS-7 lanes (which both route to
        // `LaneKind::Frame` in the runtime kernel) and that no
        // path through the mapping ever produces a background-class
        // lane.
        let present_lane = RendererGraphPhase::RenderPresent.to_schedule_lane();
        let submit_lane = RendererGraphPhase::RenderSubmit.to_schedule_lane();
        assert_eq!(present_lane, ScheduleLane::RenderPresent);
        assert_eq!(submit_lane, ScheduleLane::RenderSubmit);
        // Neither lane may share the diagnostics/background tier.
        for lane in [present_lane, submit_lane] {
            assert_ne!(lane, ScheduleLane::DiagnosticsLowPriority);
            assert_ne!(lane, ScheduleLane::IdlePrefetch);
            assert_ne!(lane, ScheduleLane::ResourceBackground);
        }
    }

    #[test]
    fn diagnostics_sheds_before_render_submit() {
        // FS-10 test 3: diagnostics shed before render submit. The
        // diagnostics phase lands on
        // `ScheduleLane::DiagnosticsLowPriority` which sheds first
        // under tight-budget; the submit phase lands on
        // `ScheduleLane::RenderSubmit` which never sheds. This test
        // pins both ends of the comparison.
        let diag_lane = RendererGraphPhase::RendererDiagnosticsFlush.to_schedule_lane();
        let submit_lane = RendererGraphPhase::RenderSubmit.to_schedule_lane();
        assert_eq!(diag_lane, ScheduleLane::DiagnosticsLowPriority);
        assert_eq!(submit_lane, ScheduleLane::RenderSubmit);
        // The runtime-kernel projection: diagnostics goes to Internal,
        // submit goes to Frame — different worker pools.
        assert_ne!(
            diag_lane.default_lane_kind(),
            submit_lane.default_lane_kind(),
        );
    }

    #[test]
    fn lux_pass_ordering_remains_stable() {
        // FS-10 test 4: Lux pass ordering remains stable. The 8 Lux
        // node kinds preserve the canonical 100–340 order key range
        // from the renderer's existing `FrameGraphPassRole::lux_order_key`.
        let lux_nodes = [
            LuxNodeKind::LuxUploadLightBuffers,
            LuxNodeKind::LuxClusterLights,
            LuxNodeKind::LuxShadowRequests,
            LuxNodeKind::LuxVirtualShadowPages,
            LuxNodeKind::LuxDirectLighting,
            LuxNodeKind::LuxGiReflection,
            LuxNodeKind::LuxVolumetric,
            LuxNodeKind::LuxGodrays,
        ];
        let mut prev: u16 = 0;
        for kind in lux_nodes {
            let key = kind.order_key();
            assert!(
                key > prev,
                "Lux node {} order_key {} not strictly greater than {}",
                kind.label(),
                key,
                prev,
            );
            assert!(key < 1000, "Lux node order keys must be < 1000");
            prev = key;
        }
    }

    #[test]
    fn hdr_post_path_order_remains_stable() {
        // FS-10 test 5: HDR post path order remains stable.
        // Exposure → Bloom → Tonemap → FinalOutput, with all post
        // keys >= 1000 so post sorts after every Lux node.
        let post = [
            LuxNodeKind::PostProcessExposureHistogram,
            LuxNodeKind::PostProcessExposureAdapt,
            LuxNodeKind::PostProcessBloomPrefilter,
            LuxNodeKind::PostProcessBloomDownsample,
            LuxNodeKind::PostProcessBloomUpsample,
            LuxNodeKind::PostProcessBloomComposite,
            LuxNodeKind::Tonemap,
            LuxNodeKind::FinalOutput,
        ];
        let mut prev: u16 = 999; // last Lux key is < 1000
        for kind in post {
            let key = kind.order_key();
            assert!(
                key > prev,
                "post node {} order_key {} not strictly greater than {}",
                kind.label(),
                key,
                prev,
            );
            assert!(key >= 1000, "post node order keys must be >= 1000");
            prev = key;
        }
    }

    #[test]
    fn cancelled_page_work_does_not_record_stale_uploads() {
        // FS-10 test 6: cancelled page work does not record stale
        // uploads. The report's `record_cancelled_page` increments
        // the cancellation counter without touching
        // `page_upload_bytes` — verified by recording one upload,
        // then 3 cancellations, then asserting the upload total is
        // unchanged.
        let mut report = RendererGraphScheduleReport::new();
        report.record_page_upload(4096);
        assert_eq!(report.page_upload_bytes, 4096);
        assert_eq!(report.cancelled_page_work_count, 0);
        report.record_cancelled_page();
        report.record_cancelled_page();
        report.record_cancelled_page();
        assert_eq!(
            report.page_upload_bytes, 4096,
            "cancellations must not record stale uploads",
        );
        assert_eq!(report.cancelled_page_work_count, 3);
    }

    #[test]
    fn renderer_graph_schedule_report_contains_no_backend_handles() {
        // FS-10 test 7: report contains no backend handles. Every
        // field is a fixed-width primitive type — verified by
        // compile-time ascription.
        let report = RendererGraphScheduleReport::new();
        let _: u64 = report.graph_build_ns;
        let _: u64 = report.graph_validation_ns;
        let _: u64 = report.graph_compile_ns;
        let _: [u64; 11] = report.queue_wait_by_phase_ns;
        let _: u64 = report.packet_build_ns;
        let _: u64 = report.command_record_ns;
        let _: u64 = report.submit_wait_ns;
        let _: u64 = report.page_upload_bytes;
        let _: u32 = report.cancelled_page_work_count;
        let _: u32 = report.coalesced_page_work_count;
        let _: u32 = report.frame_deadline_misses;
        let _: u32 = report.render_worker_utilization_micros;
        // Display rendering would only emit numeric literals; no
        // backend handle can reach the report.
    }

    #[test]
    fn renderer_work_graph_topo_orders_extract_prepare_record_submit_present() {
        // Build a minimal renderer-shaped graph through the FS-10
        // type aliases and assert topological order respects the
        // canonical extract → prepare → record → submit → present
        // pipeline.
        let mut g = renderer_work_graph(WorkGraphId(42));
        let extract_id = g.next_node_id();
        g.add_node(renderer_work_node(extract_id, RendererWork::extraction(1)));
        let lux_id = g.next_node_id();
        let lux_work = RendererWork::lux(LuxNodeKind::LuxDirectLighting, 1);
        let mut lux_node = renderer_work_node(lux_id, lux_work);
        lux_node = lux_node.with_dependency(extract_id);
        g.add_node(lux_node);

        let submit_id = g.next_node_id();
        let mut submit_node = renderer_work_node(submit_id, RendererWork::submit(1));
        submit_node = submit_node.with_dependency(lux_id);
        g.add_node(submit_node);

        let present_id = g.next_node_id();
        let mut present_node = renderer_work_node(present_id, RendererWork::present(1));
        present_node = present_node.with_dependency(submit_id);
        g.add_node(present_node);

        let order = g.topological_order().expect("renderer graph topo-sorts");
        assert_eq!(order, vec![extract_id, lux_id, submit_id, present_id]);
        assert_eq!(
            g.node(present_id).expect("present node").lane,
            ScheduleLane::RenderPresent
        );
        assert_eq!(
            g.node(submit_id).expect("submit node").lane,
            ScheduleLane::RenderSubmit
        );
        assert_eq!(
            g.node(extract_id).expect("extract node").lane,
            ScheduleLane::RenderExtract
        );
    }

    #[test]
    fn renderer_work_descriptor_carries_no_handles_by_construction() {
        // The work descriptor type contains only typed enums and
        // a u64 generation — no Box<dyn>, no &mut dyn Backend, no
        // raw pointers. Compile-time ascription on every field.
        let work = RendererWork::lux(LuxNodeKind::LuxClusterLights, 7);
        let _: RendererWorkKind = work.kind;
        let _: RendererNodeClass = work.class;
        let _: RendererGraphPhase = work.phase;
        let _: Option<FrameGraphPassRole> = work.pass_role;
        let _: Option<LuxNodeKind> = work.lux_kind;
        let _: Option<LuxResourceIntentKind> = work.resource_intent;
        let _: u64 = work.generation;
    }

    #[test]
    fn renderer_work_kind_labels_are_unique_and_cover_tier5_taxonomy() {
        let labels: Vec<&'static str> = RendererWorkKind::all()
            .iter()
            .map(|kind| kind.label())
            .collect();
        assert_eq!(labels.len(), 25);
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
        assert!(RendererWorkKind::DiagnosticCapture.optional_for_present());
        assert!(RendererWorkKind::PipelineWarmup.optional_for_present());
        assert!(RendererWorkKind::RecordPass.is_record());
        assert!(RendererWorkKind::SubmitQueue.is_submit_present());
        assert!(RendererWorkKind::PresentSurface.is_submit_present());
    }

    #[test]
    fn renderer_frame_typestate_chain_reaches_presented_only_in_order() {
        let frame: RendererFrame<Presented> = RendererFrame::<Extracted>::new(9, 4)
            .prepare()
            .queue()
            .compile_graph()
            .record()
            .submit()
            .present();
        assert_eq!(frame.frame_index, 9);
        assert_eq!(frame.generation, 4);
    }

    #[test]
    fn renderer_static_phase_chain_pins_submit_before_present() {
        let chain = RendererStaticPhaseProof::PRODUCT.chain();
        let pos = |kind| {
            chain
                .iter()
                .position(|candidate| *candidate == kind)
                .expect("phase in chain")
        };
        assert!(pos(RendererWorkKind::ExtractViews) < pos(RendererWorkKind::PrepareGpuSceneChunk));
        assert!(
            pos(RendererWorkKind::VisibilityCullChunk) < pos(RendererWorkKind::FrameGraphBuild)
        );
        assert!(pos(RendererWorkKind::FrameGraphBuild) < pos(RendererWorkKind::FrameGraphCompile));
        assert!(pos(RendererWorkKind::FrameGraphCompile) < pos(RendererWorkKind::RecordPass));
        assert!(pos(RendererWorkKind::RecordPass) < pos(RendererWorkKind::SubmitQueue));
        assert!(pos(RendererWorkKind::SubmitQueue) < pos(RendererWorkKind::PresentSurface));
    }

    fn add_test_node(g: &mut RendererWorkGraph, work: RendererWork) -> WorkNodeId {
        let id = g.next_node_id();
        g.add_node(renderer_work_node(id, work));
        id
    }

    fn add_test_dep(g: &mut RendererWorkGraph, node: WorkNodeId, dependency: WorkNodeId) {
        let idx = node.get() as usize;
        g.nodes[idx].dependencies.push(dependency);
    }

    fn minimal_live_renderer_graph() -> RendererWorkGraph {
        let mut g = renderer_work_graph(WorkGraphId::new(700));
        let extract = add_test_node(&mut g, RendererWork::extraction(1));
        let record = add_test_node(
            &mut g,
            RendererWork {
                kind: RendererWorkKind::RecordPass,
                class: RendererNodeClass::CommandRecord,
                phase: RendererGraphPhase::RenderRecord,
                pass_role: Some(FrameGraphPassRole::StaticScenePlaceholder),
                lux_kind: None,
                resource_intent: None,
                generation: 1,
            },
        );
        add_test_dep(&mut g, record, extract);
        let submit = add_test_node(&mut g, RendererWork::submit(1));
        add_test_dep(&mut g, submit, record);
        let present = add_test_node(&mut g, RendererWork::present(1));
        add_test_dep(&mut g, present, submit);
        g
    }

    #[test]
    fn renderer_liveness_accepts_submit_present_shape() {
        let g = minimal_live_renderer_graph();
        validate_renderer_graph_liveness(&g).expect("renderer graph is live");
    }

    #[test]
    fn renderer_liveness_rejects_diagnostics_or_warmup_blocking_present() {
        let mut g = minimal_live_renderer_graph();
        let warmup = add_test_node(
            &mut g,
            RendererWork::for_kind(RendererWorkKind::PipelineWarmup, 1),
        );
        let submit = nodes_with_kind(&g, RendererWorkKind::SubmitQueue)[0];
        add_test_dep(&mut g, submit, warmup);
        let err = validate_renderer_graph_liveness(&g).expect_err("warmup blocks present");
        assert!(matches!(
            err,
            RendererGraphLivenessError::OptionalWorkBlocksPresent { .. }
        ));
    }

    #[test]
    fn renderer_liveness_rejects_present_waiting_on_non_submit() {
        let mut g = minimal_live_renderer_graph();
        let present = nodes_with_kind(&g, RendererWorkKind::PresentSurface)[0];
        let diagnostics = add_test_node(&mut g, RendererWork::diagnostics(1));
        add_test_dep(&mut g, present, diagnostics);
        let err = validate_renderer_graph_liveness(&g).expect_err("present has extra wait");
        assert!(matches!(
            err,
            RendererGraphLivenessError::PresentDependencyCount { .. }
        ));
    }

    #[test]
    fn renderer_liveness_rejects_record_waiting_on_present() {
        let mut g = renderer_work_graph(WorkGraphId::new(701));
        let submit = add_test_node(&mut g, RendererWork::submit(1));
        let present = add_test_node(&mut g, RendererWork::present(1));
        add_test_dep(&mut g, present, submit);
        let record = add_test_node(
            &mut g,
            RendererWork {
                kind: RendererWorkKind::RecordComputePass,
                class: RendererNodeClass::CommandRecord,
                phase: RendererGraphPhase::RenderRecord,
                pass_role: Some(FrameGraphPassRole::LuxDirectLighting),
                lux_kind: None,
                resource_intent: None,
                generation: 1,
            },
        );
        add_test_dep(&mut g, record, present);
        let err = validate_renderer_graph_liveness(&g).expect_err("record waits on present");
        assert!(matches!(
            err,
            RendererGraphLivenessError::SubmitMissingRequiredRecord { .. }
                | RendererGraphLivenessError::RecordWaitsOnPresent { .. }
        ));
    }

    #[test]
    fn renderer_liveness_rejects_resource_cycle() {
        let mut g = minimal_live_renderer_graph();
        let material = add_test_node(
            &mut g,
            RendererWork::for_kind(RendererWorkKind::PrepareMaterialChunk, 1),
        );
        let mesh = add_test_node(
            &mut g,
            RendererWork::for_kind(RendererWorkKind::PrepareMeshChunk, 1),
        );
        g.add_edge(WorkEdge::ResourceReadAfterWrite {
            writer: material,
            reader: mesh,
            resource_id: 9,
        });
        g.add_edge(WorkEdge::ResourceReadAfterWrite {
            writer: mesh,
            reader: material,
            resource_id: 9,
        });
        let err = validate_renderer_graph_liveness(&g).expect_err("resource cycle rejects");
        assert!(matches!(
            err,
            RendererGraphLivenessError::GraphInvariant(GraphInvariantError::Cycle { .. })
                | RendererGraphLivenessError::GraphInvariant(
                    GraphInvariantError::WaitForCycle { .. }
                )
        ));
    }

    #[test]
    fn renderer_liveness_rejects_gpu_readback_on_frame_worker() {
        let mut g = minimal_live_renderer_graph();
        let bad = add_test_node(
            &mut g,
            RendererWork {
                kind: RendererWorkKind::DiagnosticCapture,
                class: RendererNodeClass::Diagnostics,
                phase: RendererGraphPhase::RenderPresent,
                pass_role: Some(FrameGraphPassRole::DiagnosticsReadback),
                lux_kind: None,
                resource_intent: None,
                generation: 1,
            },
        );
        assert_eq!(
            g.node(bad).expect("bad readback").lane.default_lane_kind(),
            fun_scheduler_types::lane::LaneKind::Frame
        );
        let err = validate_renderer_graph_liveness(&g).expect_err("readback on frame worker");
        assert!(matches!(
            err,
            RendererGraphLivenessError::GpuReadbackOnFrameWorker { .. }
        ));
    }

    #[test]
    fn renderer_graph_phase_labels_are_unique() {
        let labels: Vec<&'static str> = RendererGraphPhase::all()
            .iter()
            .map(|p| p.label())
            .collect();
        assert_eq!(labels.len(), 11);
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
    }

    #[test]
    fn renderer_node_class_labels_are_unique_and_round_trip_to_phase() {
        let labels: Vec<&'static str> =
            RendererNodeClass::all().iter().map(|c| c.label()).collect();
        assert_eq!(labels.len(), 11);
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
        // primary_phase is total over every class.
        for class in RendererNodeClass::all() {
            let phase = class.primary_phase();
            let _: &'static str = phase.label();
        }
    }

    #[test]
    fn lux_node_kind_labels_are_unique_and_map_to_pass_role() {
        let labels: Vec<&'static str> = LuxNodeKind::all().iter().map(|k| k.label()).collect();
        // FS-M1.4E extended the taxonomy from 16 → 24 variants
        // (4 granular volumetric kinds + 3 cloud-shadow kinds +
        // 1 cloud-shadow sample-pack kind).
        assert_eq!(labels.len(), 24);
        for i in 0..labels.len() {
            for j in (i + 1)..labels.len() {
                assert_ne!(labels[i], labels[j]);
            }
        }
        for kind in LuxNodeKind::all() {
            let role = kind.pass_role();
            let _: &'static str = role.as_str();
            assert!(!role.as_str().is_empty());
        }
    }

    #[test]
    fn page_warmup_cancellation_round_trips_through_report() {
        // Combined: a generation-invalidated page warmup increments
        // `cancelled_page_work_count` and does not touch
        // `page_upload_bytes`. A coalesced page upload increments
        // `coalesced_page_work_count`. A successful upload
        // increments `page_upload_bytes` directly.
        let mut report = RendererGraphScheduleReport::new();
        report.record_page_upload(1024);
        report.record_coalesced_page();
        report.record_cancelled_page();
        report.record_page_upload(2048);
        assert_eq!(report.page_upload_bytes, 3072);
        assert_eq!(report.coalesced_page_work_count, 1);
        assert_eq!(report.cancelled_page_work_count, 1);
    }
}
