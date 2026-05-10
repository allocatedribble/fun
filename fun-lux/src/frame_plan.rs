//! Backend-neutral lighting frame plan that `fun-lux` emits
//! and `fun-renderer` consumes.
//!
//! Every type in this module is intentionally free of `wgpu`,
//! `wgpu-core`, `wgpu-hal`, `naga`, `raw_window_handle`, and
//! every native graphics handle. `fun-renderer` translates the
//! typed intents below into real GPU resources and dispatches
//! through the live executors (Pass B / I / J / K / L / M).
//! `fun-lux` never owns the translation.
//!
//! See the crate-level doctrine in [`crate`] for the full
//! ownership contract. The audit handle is
//! [`LuxBackendContract::PRODUCT_DEFAULT`].

use crate::api::{DirectLightingMode, GiMode, ReconstructionHook, ReflectionMode, ShadowMode};

pub const FUN_LUX_FRAME_PLAN_SCHEMA_VERSION: u16 = 1;
pub const LUX_QUALITY_TIER_COUNT: usize = 4;
pub const LUX_GPU_BUFFER_INTENT_KIND_COUNT: usize = 8;
pub const LUX_TEXTURE_INTENT_KIND_COUNT: usize = 6;
pub const LUX_TEXTURE_FORMAT_HINT_COUNT: usize = 7;
pub const LUX_PASS_KIND_COUNT: usize = 8;

// ============================================================================
// Section 1 — Quality tier
// ============================================================================

/// Typed lighting quality tier. The renderer consumes this as
/// the dominant knob driving cluster bin counts, reservoir
/// pool size, virtual shadow page headroom, GI ray counts, and
/// denoiser strength.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxQualityTier {
    #[default]
    Low,
    Medium,
    High,
    Ultra,
}

impl LuxQualityTier {
    pub const ALL: [Self; LUX_QUALITY_TIER_COUNT] =
        [Self::Low, Self::Medium, Self::High, Self::Ultra];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Ultra => "ultra",
        }
    }

    /// Typed ordering key. `Low = 0`, `Ultra = 3`.
    #[must_use]
    pub const fn order_key(self) -> u8 {
        match self {
            Self::Low => 0,
            Self::Medium => 1,
            Self::High => 2,
            Self::Ultra => 3,
        }
    }

    /// Typed predicate: does this tier permit per-pixel
    /// reservoir reuse? `Medium+` does.
    #[must_use]
    pub const fn permits_reservoir_reuse(self) -> bool {
        self.order_key() >= 1
    }

    /// Typed predicate: does this tier permit virtual shadow
    /// demand-page residency tracking? `High+` does.
    #[must_use]
    pub const fn permits_virtual_shadow_demand_pages(self) -> bool {
        self.order_key() >= 2
    }
}

// ============================================================================
// Section 2 — Typed GPU buffer intent
// ============================================================================

/// Typed kind for a lux-emitted GPU buffer intent. The
/// renderer maps each kind to a concrete `wgpu::Buffer`
/// allocation; the intent itself names no backend.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxGpuBufferIntentKind {
    #[default]
    LightTable,
    LightIndexList,
    ClusterAssignment,
    ClusterAssignmentReadback,
    ShadowMetadata,
    ProbeMetadata,
    GiResource,
    DenoiseIntermediate,
}

impl LuxGpuBufferIntentKind {
    pub const ALL: [Self; LUX_GPU_BUFFER_INTENT_KIND_COUNT] = [
        Self::LightTable,
        Self::LightIndexList,
        Self::ClusterAssignment,
        Self::ClusterAssignmentReadback,
        Self::ShadowMetadata,
        Self::ProbeMetadata,
        Self::GiResource,
        Self::DenoiseIntermediate,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LightTable => "light_table",
            Self::LightIndexList => "light_index_list",
            Self::ClusterAssignment => "cluster_assignment",
            Self::ClusterAssignmentReadback => "cluster_assignment_readback",
            Self::ShadowMetadata => "shadow_metadata",
            Self::ProbeMetadata => "probe_metadata",
            Self::GiResource => "gi_resource",
            Self::DenoiseIntermediate => "denoise_intermediate",
        }
    }

    /// Typed predicate: does this buffer kind ever require
    /// CPU readback? Set by the renderer when it allocates
    /// `BufferUsages::MAP_READ`.
    #[must_use]
    pub const fn allow_cpu_readback_by_default(self) -> bool {
        matches!(self, Self::ClusterAssignmentReadback)
    }
}

/// Typed GPU buffer intent record. fun-renderer reads this
/// and allocates a real `wgpu::Buffer`. The intent names no
/// backend handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxGpuBufferIntent {
    pub schema_version: u16,
    pub kind: LuxGpuBufferIntentKind,
    pub stable_id: &'static str,
    pub byte_size: u64,
    pub allow_cpu_readback: bool,
}

impl LuxGpuBufferIntent {
    #[must_use]
    pub const fn new(
        kind: LuxGpuBufferIntentKind,
        stable_id: &'static str,
        byte_size: u64,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
            kind,
            stable_id,
            byte_size,
            allow_cpu_readback: kind.allow_cpu_readback_by_default(),
        }
    }
}

// ============================================================================
// Section 3 — Typed texture intent
// ============================================================================

/// Typed kind for a lux-emitted texture intent.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxTextureIntentKind {
    #[default]
    ShadowAtlasPage,
    VirtualShadowDemandPage,
    VarianceMap,
    DenoiseHistory,
    ProbeIrradiance,
    GiHistory,
}

impl LuxTextureIntentKind {
    pub const ALL: [Self; LUX_TEXTURE_INTENT_KIND_COUNT] = [
        Self::ShadowAtlasPage,
        Self::VirtualShadowDemandPage,
        Self::VarianceMap,
        Self::DenoiseHistory,
        Self::ProbeIrradiance,
        Self::GiHistory,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ShadowAtlasPage => "shadow_atlas_page",
            Self::VirtualShadowDemandPage => "virtual_shadow_demand_page",
            Self::VarianceMap => "variance_map",
            Self::DenoiseHistory => "denoise_history",
            Self::ProbeIrradiance => "probe_irradiance",
            Self::GiHistory => "gi_history",
        }
    }
}

/// Typed format hint for a texture intent. fun-renderer maps
/// the hint to a concrete `wgpu::TextureFormat`; the hint
/// itself names no backend.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxTextureFormatHint {
    #[default]
    HdrFloat16Rgba,
    HdrFloat16Rg,
    Depth32Float,
    Unorm8R,
    Unorm8Rg,
    Unorm8Rgba,
    PageBookkeepingU32,
}

impl LuxTextureFormatHint {
    pub const ALL: [Self; LUX_TEXTURE_FORMAT_HINT_COUNT] = [
        Self::HdrFloat16Rgba,
        Self::HdrFloat16Rg,
        Self::Depth32Float,
        Self::Unorm8R,
        Self::Unorm8Rg,
        Self::Unorm8Rgba,
        Self::PageBookkeepingU32,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::HdrFloat16Rgba => "hdr_float16_rgba",
            Self::HdrFloat16Rg => "hdr_float16_rg",
            Self::Depth32Float => "depth32_float",
            Self::Unorm8R => "unorm8_r",
            Self::Unorm8Rg => "unorm8_rg",
            Self::Unorm8Rgba => "unorm8_rgba",
            Self::PageBookkeepingU32 => "page_bookkeeping_u32",
        }
    }

    /// Typed predicate: HDR-capable formats.
    #[must_use]
    pub const fn is_hdr_capable(self) -> bool {
        matches!(self, Self::HdrFloat16Rgba | Self::HdrFloat16Rg)
    }
}

/// Typed texture intent record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxTextureIntent {
    pub schema_version: u16,
    pub kind: LuxTextureIntentKind,
    pub stable_id: &'static str,
    pub width: u32,
    pub height: u32,
    pub format_hint: LuxTextureFormatHint,
}

impl LuxTextureIntent {
    #[must_use]
    pub const fn new(
        kind: LuxTextureIntentKind,
        stable_id: &'static str,
        width: u32,
        height: u32,
        format_hint: LuxTextureFormatHint,
    ) -> Self {
        Self {
            schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
            kind,
            stable_id,
            width,
            height,
            format_hint,
        }
    }
}

// ============================================================================
// Section 4 — Unified resource intent
// ============================================================================

/// Typed lux resource intent. Either a GPU buffer or a
/// texture; the renderer dispatches on the variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxResourceIntent {
    GpuBuffer(LuxGpuBufferIntent),
    Texture(LuxTextureIntent),
}

impl LuxResourceIntent {
    #[must_use]
    pub const fn stable_id(&self) -> &'static str {
        match self {
            Self::GpuBuffer(b) => b.stable_id,
            Self::Texture(t) => t.stable_id,
        }
    }

    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::GpuBuffer(_) => "gpu_buffer",
            Self::Texture(_) => "texture",
        }
    }
}

// ============================================================================
// Section 5 — Pass kind + dependency + request
// ============================================================================

/// Typed lighting pass kind. Each kind names a logical lux
/// pass the renderer must dispatch. fun-renderer chooses the
/// concrete pipeline (compute / render) and the WGSL source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxPassKind {
    LightingClusterAssignment,
    DirectLightingShade,
    ManyLightReservoirReuse,
    VirtualShadowPolicyUpdate,
    GiProbeUpdate,
    GiTraceAccumulation,
    DenoiseReconstruction,
    RadianceCacheUpdate,
}

impl LuxPassKind {
    pub const ALL: [Self; LUX_PASS_KIND_COUNT] = [
        Self::LightingClusterAssignment,
        Self::DirectLightingShade,
        Self::ManyLightReservoirReuse,
        Self::VirtualShadowPolicyUpdate,
        Self::GiProbeUpdate,
        Self::GiTraceAccumulation,
        Self::DenoiseReconstruction,
        Self::RadianceCacheUpdate,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LightingClusterAssignment => "lighting_cluster_assignment",
            Self::DirectLightingShade => "direct_lighting_shade",
            Self::ManyLightReservoirReuse => "many_light_reservoir_reuse",
            Self::VirtualShadowPolicyUpdate => "virtual_shadow_policy_update",
            Self::GiProbeUpdate => "gi_probe_update",
            Self::GiTraceAccumulation => "gi_trace_accumulation",
            Self::DenoiseReconstruction => "denoise_reconstruction",
            Self::RadianceCacheUpdate => "radiance_cache_update",
        }
    }
}

/// Typed dependency edge between two lux passes. The
/// renderer uses the typed `upstream_stable_id` to order
/// `wgpu::RenderPass` / `wgpu::ComputePass` recording.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxPassDependency {
    pub schema_version: u16,
    pub upstream_stable_id: &'static str,
}

impl LuxPassDependency {
    #[must_use]
    pub const fn new(upstream_stable_id: &'static str) -> Self {
        Self {
            schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
            upstream_stable_id,
        }
    }
}

/// Typed lighting pass request. The renderer maps the typed
/// fields to a concrete pipeline + bind groups. Reads and
/// writes are typed resource intents — the renderer is
/// responsible for inferring barriers + transitions.
///
/// `LuxPassRequest` intentionally does not derive `Hash`
/// because [`LuxResourceIntent`] aggregates the
/// per-resource intent enums and the lighting policy never
/// uses a frame plan as a hash key; tests compare equality
/// only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuxPassRequest {
    pub schema_version: u16,
    pub kind: LuxPassKind,
    pub stable_id: &'static str,
    pub quality_tier: LuxQualityTier,
    pub depends_on: Vec<LuxPassDependency>,
    pub reads: Vec<LuxResourceIntent>,
    pub writes: Vec<LuxResourceIntent>,
}

impl LuxPassRequest {
    #[must_use]
    pub fn new(kind: LuxPassKind, stable_id: &'static str, quality_tier: LuxQualityTier) -> Self {
        Self {
            schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
            kind,
            stable_id,
            quality_tier,
            depends_on: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
        }
    }

    /// Add a typed upstream dependency.
    pub fn depends_on(mut self, upstream_stable_id: &'static str) -> Self {
        self.depends_on
            .push(LuxPassDependency::new(upstream_stable_id));
        self
    }

    /// Add a typed read resource.
    pub fn reads(mut self, intent: LuxResourceIntent) -> Self {
        self.reads.push(intent);
        self
    }

    /// Add a typed write resource.
    pub fn writes(mut self, intent: LuxResourceIntent) -> Self {
        self.writes.push(intent);
        self
    }
}

// ============================================================================
// Section 6 — Frame plan
// ============================================================================

/// Typed backend-neutral lighting frame plan. fun-lux builds
/// the plan from its current policy + light database;
/// fun-renderer consumes the plan and dispatches against the
/// real GPU.
///
/// `LuxFramePlan` intentionally does not derive `Hash`
/// because [`ReconstructionHook`] (and the embedded
/// resource-intent enums) are pass-by-value records, not
/// hash keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuxFramePlan {
    pub schema_version: u16,
    pub frame_index: u64,
    pub quality_tier: LuxQualityTier,
    pub direct_lighting: DirectLightingMode,
    pub shadows: ShadowMode,
    pub gi: GiMode,
    pub reflections: ReflectionMode,
    pub reconstruction: ReconstructionHook,
    pub passes: Vec<LuxPassRequest>,
}

impl LuxFramePlan {
    /// Stable id for the cold-default zero-work
    /// `ReconstructionHook`.
    pub const COLD_DEFAULT_RECONSTRUCTION_STABLE_ID: &'static str =
        "fun_lux.frame_plan.cold_default.reconstruction";

    /// A typed cold-default plan: zero passes, low quality
    /// tier, every policy at its disabled default. Useful for
    /// the `NoopLuxCore` fallback path + tests.
    #[must_use]
    pub fn cold_default() -> Self {
        use crate::LuxDenoiseReconstructionPath;
        Self {
            schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
            frame_index: 0,
            quality_tier: LuxQualityTier::Low,
            direct_lighting: DirectLightingMode::Disabled,
            shadows: ShadowMode::Disabled,
            gi: GiMode::Disabled,
            reflections: ReflectionMode::Disabled,
            reconstruction: ReconstructionHook::new(
                Self::COLD_DEFAULT_RECONSTRUCTION_STABLE_ID,
                LuxDenoiseReconstructionPath::HeuristicFallback,
            ),
            passes: Vec::new(),
        }
    }

    /// Typed predicate: this frame plan is the zero-work
    /// `NoopLuxCore` fallback path, not a production-frame
    /// plan. Used by [`LuxBackendContract`] to enforce that
    /// no production route boots a `NoopLuxCore`.
    #[must_use]
    pub fn is_noop_baseline(&self) -> bool {
        self.passes.is_empty()
            && matches!(self.direct_lighting, DirectLightingMode::Disabled)
            && matches!(self.shadows, ShadowMode::Disabled)
    }
}

// ============================================================================
// Section 7 — Backend contract
// ============================================================================

/// Typed backend contract record. The audit handle for Pass
/// 0's "Lock the crate ownership and backend contract" goal.
///
/// The `PRODUCT_DEFAULT` constant is the source of truth for
/// the renderer / lux ownership split; every test in the
/// workspace that touches lighting routing should read this
/// record rather than re-implementing the policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxBackendContract {
    pub schema_version: u16,
    /// Lighting policy ownership: `fun-lux` owns it.
    pub fun_lux_owns_lighting_policy: bool,
    /// Production executor: `fun-renderer` is the only one.
    pub fun_renderer_is_only_production_executor: bool,
    /// Plan emission: `fun-lux` emits backend-neutral plans
    /// (no `wgpu`, `wgpu-core`, `wgpu-hal`, `naga`,
    /// `raw_window_handle`, or native backend handles).
    pub fun_lux_emits_backend_neutral_plans: bool,
    /// Legacy lighting: every legacy path is invalid for
    /// production. Diagnostic-only callers stay loud.
    pub legacy_lighting_paths_are_invalid_for_production: bool,
    /// `NoopLuxCore`: reserved for tests, diagnostics, and
    /// early-boot fallback. Never boots under a production
    /// lighting route.
    pub noop_lux_core_is_non_production_only: bool,
    /// Typed list of crate names `fun-lux` MUST NOT import.
    /// Audited by the
    /// `fun_lux_remains_backend_neutral_by_dependency_contract`
    /// test.
    pub forbidden_fun_lux_imports: &'static [&'static str],
    /// `fun-renderer` is permitted to depend on `fun-lux`,
    /// not the other way around. The cycle-break crate
    /// `fun-renderer-lux-api` is reserved for the alternative
    /// split-crate structure documented in
    /// `docs/renderer_ownership.md`.
    pub fun_renderer_depends_on_fun_lux: bool,
    /// `fun-lux` MUST NOT depend on `fun-renderer` directly.
    pub fun_lux_must_not_depend_on_fun_renderer: bool,
}

impl LuxBackendContract {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
        fun_lux_owns_lighting_policy: true,
        fun_renderer_is_only_production_executor: true,
        fun_lux_emits_backend_neutral_plans: true,
        legacy_lighting_paths_are_invalid_for_production: true,
        noop_lux_core_is_non_production_only: true,
        forbidden_fun_lux_imports: &[
            "wgpu",
            "wgpu_core",
            "wgpu-core",
            "wgpu_hal",
            "wgpu-hal",
            "naga",
            "raw_window_handle",
            "raw-window-handle",
            "ash",
            "metal",
            "d3d12",
            "windows",
            "windows-rs",
        ],
        fun_renderer_depends_on_fun_lux: true,
        fun_lux_must_not_depend_on_fun_renderer: true,
    };

    /// Typed predicate: the contract holds end-to-end. All
    /// 7 boolean fields must be `true` and the forbidden
    /// list must be non-empty.
    #[must_use]
    pub const fn contract_holds(&self) -> bool {
        self.fun_lux_owns_lighting_policy
            && self.fun_renderer_is_only_production_executor
            && self.fun_lux_emits_backend_neutral_plans
            && self.legacy_lighting_paths_are_invalid_for_production
            && self.noop_lux_core_is_non_production_only
            && !self.forbidden_fun_lux_imports.is_empty()
            && self.fun_renderer_depends_on_fun_lux
            && self.fun_lux_must_not_depend_on_fun_renderer
    }

    /// Typed predicate: is the named crate forbidden as a
    /// `fun-lux` import?
    #[must_use]
    pub fn is_forbidden_import(&self, crate_name: &str) -> bool {
        self.forbidden_fun_lux_imports
            .iter()
            .any(|forbidden| *forbidden == crate_name)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_versions_are_stable() {
        assert_eq!(FUN_LUX_FRAME_PLAN_SCHEMA_VERSION, 1);
        assert_eq!(LUX_QUALITY_TIER_COUNT, 4);
        assert_eq!(LuxQualityTier::ALL.len(), LUX_QUALITY_TIER_COUNT);
        assert_eq!(LUX_GPU_BUFFER_INTENT_KIND_COUNT, 8);
        assert_eq!(
            LuxGpuBufferIntentKind::ALL.len(),
            LUX_GPU_BUFFER_INTENT_KIND_COUNT
        );
        assert_eq!(LUX_TEXTURE_INTENT_KIND_COUNT, 6);
        assert_eq!(
            LuxTextureIntentKind::ALL.len(),
            LUX_TEXTURE_INTENT_KIND_COUNT
        );
        assert_eq!(LUX_TEXTURE_FORMAT_HINT_COUNT, 7);
        assert_eq!(
            LuxTextureFormatHint::ALL.len(),
            LUX_TEXTURE_FORMAT_HINT_COUNT
        );
        assert_eq!(LUX_PASS_KIND_COUNT, 8);
        assert_eq!(LuxPassKind::ALL.len(), LUX_PASS_KIND_COUNT);
    }

    #[test]
    fn quality_tier_ordering_is_monotonic() {
        assert_eq!(LuxQualityTier::Low.order_key(), 0);
        assert_eq!(LuxQualityTier::Medium.order_key(), 1);
        assert_eq!(LuxQualityTier::High.order_key(), 2);
        assert_eq!(LuxQualityTier::Ultra.order_key(), 3);
        // Reservoir reuse: Medium+.
        assert!(!LuxQualityTier::Low.permits_reservoir_reuse());
        assert!(LuxQualityTier::Medium.permits_reservoir_reuse());
        assert!(LuxQualityTier::High.permits_reservoir_reuse());
        assert!(LuxQualityTier::Ultra.permits_reservoir_reuse());
        // Virtual shadow demand pages: High+.
        assert!(!LuxQualityTier::Low.permits_virtual_shadow_demand_pages());
        assert!(!LuxQualityTier::Medium.permits_virtual_shadow_demand_pages());
        assert!(LuxQualityTier::High.permits_virtual_shadow_demand_pages());
        assert!(LuxQualityTier::Ultra.permits_virtual_shadow_demand_pages());
    }

    #[test]
    fn gpu_buffer_intent_kind_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in LuxGpuBufferIntentKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn cluster_assignment_readback_is_only_default_readback_buffer() {
        for kind in LuxGpuBufferIntentKind::ALL {
            let expect = matches!(kind, LuxGpuBufferIntentKind::ClusterAssignmentReadback);
            assert_eq!(
                kind.allow_cpu_readback_by_default(),
                expect,
                "{}",
                kind.as_str()
            );
        }
    }

    #[test]
    fn texture_intent_kind_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in LuxTextureIntentKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn texture_format_hint_hdr_predicate() {
        for hint in LuxTextureFormatHint::ALL {
            let expect = matches!(
                hint,
                LuxTextureFormatHint::HdrFloat16Rgba | LuxTextureFormatHint::HdrFloat16Rg
            );
            assert_eq!(hint.is_hdr_capable(), expect, "{}", hint.as_str());
        }
    }

    #[test]
    fn pass_kind_strings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for kind in LuxPassKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn pass_request_builder_chains_reads_writes_deps() {
        let req = LuxPassRequest::new(
            LuxPassKind::LightingClusterAssignment,
            "lux.lighting.cluster_assignment.demo",
            LuxQualityTier::Medium,
        )
        .depends_on("upstream.light_table_upload")
        .reads(LuxResourceIntent::GpuBuffer(LuxGpuBufferIntent::new(
            LuxGpuBufferIntentKind::LightTable,
            "lux.light_table",
            1024,
        )))
        .writes(LuxResourceIntent::GpuBuffer(LuxGpuBufferIntent::new(
            LuxGpuBufferIntentKind::ClusterAssignment,
            "lux.cluster_assignment",
            2048,
        )));
        assert_eq!(req.kind, LuxPassKind::LightingClusterAssignment);
        assert_eq!(req.depends_on.len(), 1);
        assert_eq!(
            req.depends_on[0].upstream_stable_id,
            "upstream.light_table_upload"
        );
        assert_eq!(req.reads.len(), 1);
        assert_eq!(req.writes.len(), 1);
        assert_eq!(req.reads[0].as_str(), "gpu_buffer");
        assert_eq!(req.writes[0].stable_id(), "lux.cluster_assignment");
    }

    #[test]
    fn cold_default_frame_plan_is_noop_baseline() {
        let plan = LuxFramePlan::cold_default();
        assert!(plan.is_noop_baseline());
        assert_eq!(plan.passes.len(), 0);
        assert_eq!(plan.frame_index, 0);
        assert_eq!(plan.quality_tier, LuxQualityTier::Low);
    }

    #[test]
    fn populated_frame_plan_is_not_noop_baseline() {
        let mut plan = LuxFramePlan::cold_default();
        plan.passes.push(LuxPassRequest::new(
            LuxPassKind::LightingClusterAssignment,
            "lux.lighting.cluster_assignment",
            LuxQualityTier::High,
        ));
        plan.direct_lighting = DirectLightingMode::TiledClustered;
        assert!(!plan.is_noop_baseline());
    }

    #[test]
    fn backend_contract_product_default_holds() {
        let contract = LuxBackendContract::PRODUCT_DEFAULT;
        assert!(contract.contract_holds());
        assert!(contract.fun_lux_owns_lighting_policy);
        assert!(contract.fun_renderer_is_only_production_executor);
        assert!(contract.fun_lux_emits_backend_neutral_plans);
        assert!(contract.legacy_lighting_paths_are_invalid_for_production);
        assert!(contract.noop_lux_core_is_non_production_only);
        assert!(contract.fun_renderer_depends_on_fun_lux);
        assert!(contract.fun_lux_must_not_depend_on_fun_renderer);
        assert!(!contract.forbidden_fun_lux_imports.is_empty());
    }

    #[test]
    fn backend_contract_forbids_every_native_graphics_crate() {
        let contract = LuxBackendContract::PRODUCT_DEFAULT;
        for forbidden in [
            "wgpu",
            "wgpu_core",
            "wgpu-core",
            "wgpu_hal",
            "wgpu-hal",
            "naga",
            "raw_window_handle",
            "raw-window-handle",
            "ash",
            "metal",
            "d3d12",
            "windows",
            "windows-rs",
        ] {
            assert!(
                contract.is_forbidden_import(forbidden),
                "forbidden list must include {forbidden}",
            );
        }
        // A whitelisted crate is not forbidden.
        assert!(!contract.is_forbidden_import("bevy_ecs"));
        assert!(!contract.is_forbidden_import("fun_scene"));
    }

    /// Pass 0 source-of-truth test. Reads every file under
    /// `fun-lux/src/` and asserts the doctrine: no public
    /// module imports any forbidden crate. The typed
    /// `LuxBackendContract::PRODUCT_DEFAULT` carries the
    /// forbidden list.
    ///
    /// This is the durable per-crate enforcement of the
    /// fun-lux backend-neutrality rule. A regression that
    /// adds `use wgpu::...` anywhere in fun-lux fails this
    /// test immediately.
    #[test]
    fn fun_lux_remains_backend_neutral_by_dependency_contract() {
        use std::fs;
        use std::path::Path;
        let contract = LuxBackendContract::PRODUCT_DEFAULT;
        let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut bad: Vec<(String, String)> = Vec::new();
        walk_rust_files(&src_dir, &mut |path| {
            let body = fs::read_to_string(path).unwrap_or_default();
            for line in body.lines() {
                let trimmed = line.trim_start();
                if !(trimmed.starts_with("use ")
                    || trimmed.starts_with("pub use ")
                    || trimmed.starts_with("extern crate "))
                {
                    continue;
                }
                for forbidden in contract.forbidden_fun_lux_imports {
                    // Only flag exact crate-name prefix matches
                    // like `use wgpu::...` or
                    // `use wgpu;` — not substrings of unrelated
                    // identifiers. Replace dashes with
                    // underscores so `raw-window-handle` matches
                    // `use raw_window_handle::...`.
                    let normalized = forbidden.replace('-', "_");
                    let with_colon = format!("{normalized}::");
                    let with_semi = format!("{normalized};");
                    let standalone_use = format!("use {normalized}");
                    let standalone_pub_use = format!("pub use {normalized}");
                    let standalone_extern = format!("extern crate {normalized}");
                    if trimmed.contains(&with_colon)
                        || trimmed.contains(&with_semi)
                        || trimmed.starts_with(&standalone_use)
                        || trimmed.starts_with(&standalone_pub_use)
                        || trimmed.starts_with(&standalone_extern)
                    {
                        bad.push((path.display().to_string(), trimmed.to_string()));
                    }
                }
            }
        });
        assert!(
            bad.is_empty(),
            "fun-lux must not import any forbidden backend crate; found:\n{}",
            bad.iter()
                .map(|(f, l)| format!("  {f}: {l}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
    }

    fn walk_rust_files(dir: &std::path::Path, callback: &mut dyn FnMut(&std::path::Path)) {
        use std::fs;
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    walk_rust_files(&path, callback);
                } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                    callback(&path);
                }
            }
        }
    }

    /// Pass 0 source-of-truth test. The typed `NoopLuxCore`
    /// type is reserved for tests / diagnostics / early-boot
    /// fallback. The typed `LuxFramePlan::cold_default` is
    /// the canonical zero-work frame plan; it returns
    /// `is_noop_baseline() == true`. A production lighting
    /// route must build a plan with at least one pass, with
    /// `is_noop_baseline() == false`.
    #[test]
    fn noop_lux_core_is_typed_non_production_only() {
        let contract = LuxBackendContract::PRODUCT_DEFAULT;
        assert!(contract.noop_lux_core_is_non_production_only);
        let cold = LuxFramePlan::cold_default();
        assert!(
            cold.is_noop_baseline(),
            "cold_default plan must classify as noop baseline; production routes must populate passes",
        );
    }
}
