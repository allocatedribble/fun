#![forbid(unsafe_code)]
//! # fun-lux crate doctrine
//!
//! Pass 0 ("Lock the crate ownership and backend contract")
//! locks the architecture so the renderer / lux split is
//! impossible to accidentally route through the wrong backend.
//! The doctrine below is enforced by typed contract +
//! per-crate tests; the durable narrative version lives at
//! [`docs/renderer_ownership.md`](../../docs/renderer_ownership.md)
//! and
//! [`docs/rendering/fun-render-migration.md`](../../docs/rendering/fun-render-migration.md).
//!
//! 1. **`fun-lux` owns lighting policy.** Every typed
//!    lighting decision — direct lighting mode, shadow
//!    policy, GI mode, reflection mode, reconstruction hook,
//!    reservoir reuse policy, virtual shadow demand-page
//!    policy, denoiser policy, quality tier — lives in this
//!    crate. `fun-renderer` reads these typed records; it
//!    never invents them.
//! 2. **`fun-renderer` is the only production executor.**
//!    Lighting plans land on the GPU through `fun-renderer`'s
//!    typed live executors (Pass B / Pass I / Pass J / Pass K /
//!    Pass L / Pass M). No other crate is permitted to drive
//!    a production lighting frame.
//! 3. **`fun-lux` emits backend-neutral lighting frame
//!    plans.** The typed [`LuxFramePlan`] / [`LuxPassRequest`]
//!    / [`LuxResourceIntent`] / [`LuxGpuBufferIntent`] /
//!    [`LuxTextureIntent`] / [`LuxPassDependency`] /
//!    [`LuxQualityTier`] / [`LuxBackendContract`] records
//!    describe *what the renderer should do* without naming
//!    any backend handle. `fun-renderer` translates the
//!    intents into actual `wgpu` resources + dispatches.
//! 4. **Legacy / Bevy / wgpu product lighting paths are
//!    invalid.** Any module that boots
//!    [`NoopLuxCore`](api::NoopLuxCore) under a production
//!    route is a regression; the typed
//!    [`LuxBackendContract::PRODUCT_DEFAULT`] is the audit
//!    handle.
//!
//! ## Dependency rule
//!
//! `fun-lux` MUST NOT import `wgpu`, `wgpu-core`, `wgpu-hal`,
//! `naga`, `raw_window_handle`, `ash`, `metal`, `d3d12`,
//! `windows-rs`, or any other native graphics handle.
//! `fun-lux` MUST NOT take a normal `fun-renderer`
//! dependency. If a cycle ever appears, introduce a thin
//! `fun-renderer-lux-api` crate carrying the shared
//! descriptor types and have both crates depend on it; never
//! resolve the cycle by relaxing fun-lux's backend
//! neutrality.
//!
//! ## Test enforcement
//!
//! The typed [`LuxBackendContract::PRODUCT_DEFAULT`]
//! re-asserts the doctrine at the type-system level. The
//! `fun_lux_remains_backend_neutral_by_dependency_contract`
//! test exercises the predicate. The
//! `noop_lux_core_is_typed_non_production_only` test
//! confirms that `NoopLuxCore` is reserved for tests /
//! diagnostics / early fallback.

pub mod api;
pub mod diagnostics;
pub mod frame_plan;
pub mod gi;
pub mod look;
pub mod many_light;
pub mod pass;
pub mod quality;
pub mod research;
pub mod runtime;
pub mod shadow;
pub mod volumetric;

use bevy_ecs::{
    entity::Entity,
    lifecycle::RemovedComponents,
    prelude::{Added, Changed, Component, Message, Resource},
    schedule::SystemSet,
    system::{Query, ResMut},
};

pub use api::*;
pub use diagnostics::*;
pub use frame_plan::*;
pub use gi::*;
pub use look::*;
pub use many_light::*;
pub use pass::*;
pub use quality::*;
pub use research::*;
pub use runtime::*;
pub use shadow::*;
pub use volumetric::*;

pub const FUN_LUX_SCHEMA_VERSION: u16 = 1;
pub const FUN_LUX_PACKAGE_NAME: &str = "fun-lux";
pub const FUN_LUX_CRATE_NAME: &str = "fun_lux";
pub const FUN_LUX_ECS_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxSubsystem {
    DirectLighting,
    ManyLightSampling,
    VirtualShadowPolicy,
    GlobalIllumination,
    Reflections,
    DenoisingReconstruction,
    RadianceCache,
    SurfaceCache,
    ProbeCache,
}

impl LuxSubsystem {
    pub const ALL: [Self; 9] = [
        Self::DirectLighting,
        Self::ManyLightSampling,
        Self::VirtualShadowPolicy,
        Self::GlobalIllumination,
        Self::Reflections,
        Self::DenoisingReconstruction,
        Self::RadianceCache,
        Self::SurfaceCache,
        Self::ProbeCache,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectLighting => "direct_lighting",
            Self::ManyLightSampling => "many_light_sampling",
            Self::VirtualShadowPolicy => "virtual_shadow_policy",
            Self::GlobalIllumination => "global_illumination",
            Self::Reflections => "reflections",
            Self::DenoisingReconstruction => "denoising_reconstruction",
            Self::RadianceCache => "radiance_cache",
            Self::SurfaceCache => "surface_cache",
            Self::ProbeCache => "probe_cache",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxPolicyKind {
    DirectLighting,
    ManyLightSampling,
    VirtualShadows,
    GlobalIllumination,
    Reflections,
    DenoisingReconstruction,
}

impl LuxPolicyKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectLighting => "direct_lighting",
            Self::ManyLightSampling => "many_light_sampling",
            Self::VirtualShadows => "virtual_shadows",
            Self::GlobalIllumination => "global_illumination",
            Self::Reflections => "reflections",
            Self::DenoisingReconstruction => "denoising_reconstruction",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxCacheKind {
    Radiance,
    Surface,
    Probe,
}

impl LuxCacheKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Radiance => "radiance",
            Self::Surface => "surface",
            Self::Probe => "probe",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxDenoiseReconstructionPath {
    BalancedFast,
    BalancedQuality,
    DlssRayReconstruction,
    ModelAssisted,
    HeuristicFallback,
}

impl LuxDenoiseReconstructionPath {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BalancedFast => "balanced_fast",
            Self::BalancedQuality => "balanced_quality",
            Self::DlssRayReconstruction => "dlss_ray_reconstruction",
            Self::ModelAssisted => "model_assisted",
            Self::HeuristicFallback => "heuristic_fallback",
        }
    }

    #[must_use]
    pub const fn requires_fun_ai_runtime(self) -> bool {
        matches!(self, Self::ModelAssisted)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxPolicyDescriptor {
    pub stable_id: &'static str,
    pub subsystem: LuxSubsystem,
    pub policy_kind: LuxPolicyKind,
    pub default_path: &'static str,
    pub owner_crate: &'static str,
    pub accepts_identifiable_data: bool,
}

pub const FUN_LUX_POLICY_DESCRIPTORS: [LuxPolicyDescriptor; 6] = [
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.direct_lighting",
        subsystem: LuxSubsystem::DirectLighting,
        policy_kind: LuxPolicyKind::DirectLighting,
        default_path: "many_light_reuse",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.many_light_sampling",
        subsystem: LuxSubsystem::ManyLightSampling,
        policy_kind: LuxPolicyKind::ManyLightSampling,
        default_path: "reservoir_sampling",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.virtual_shadows",
        subsystem: LuxSubsystem::VirtualShadowPolicy,
        policy_kind: LuxPolicyKind::VirtualShadows,
        default_path: "virtual_shadow_pages",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.global_illumination",
        subsystem: LuxSubsystem::GlobalIllumination,
        policy_kind: LuxPolicyKind::GlobalIllumination,
        default_path: "surface_cache",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.reflections",
        subsystem: LuxSubsystem::Reflections,
        policy_kind: LuxPolicyKind::Reflections,
        default_path: "surface_cache",
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
    LuxPolicyDescriptor {
        stable_id: "fun_lux.policy.denoising_reconstruction",
        subsystem: LuxSubsystem::DenoisingReconstruction,
        policy_kind: LuxPolicyKind::DenoisingReconstruction,
        default_path: LuxDenoiseReconstructionPath::BalancedFast.as_str(),
        owner_crate: FUN_LUX_CRATE_NAME,
        accepts_identifiable_data: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxCacheDescriptor {
    pub stable_id: &'static str,
    pub kind: LuxCacheKind,
    pub subsystem: LuxSubsystem,
    pub owner_crate: &'static str,
    pub frame_local_default: bool,
    pub accepts_identifiable_data: bool,
}

pub const FUN_LUX_CACHE_DESCRIPTORS: [LuxCacheDescriptor; 3] = [
    LuxCacheDescriptor {
        stable_id: "fun_lux.cache.radiance",
        kind: LuxCacheKind::Radiance,
        subsystem: LuxSubsystem::RadianceCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
    LuxCacheDescriptor {
        stable_id: "fun_lux.cache.surface",
        kind: LuxCacheKind::Surface,
        subsystem: LuxSubsystem::SurfaceCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
    LuxCacheDescriptor {
        stable_id: "fun_lux.cache.probe",
        kind: LuxCacheKind::Probe,
        subsystem: LuxSubsystem::ProbeCache,
        owner_crate: FUN_LUX_CRATE_NAME,
        frame_local_default: true,
        accepts_identifiable_data: false,
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LuxFeatureSet {
    pub direct_lighting: bool,
    pub many_light_sampling: bool,
    pub virtual_shadows: bool,
    pub global_illumination: bool,
    pub reflections: bool,
    pub denoising_reconstruction: bool,
    pub radiance_cache: bool,
    pub surface_cache: bool,
    pub probe_cache: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum LuxExtractSet {
    ExtractLights,
    ExtractEmissives,
    ExtractGiParticipants,
    ExtractShadowParticipants,
}

impl LuxExtractSet {
    pub const ORDER: [Self; 4] = [
        Self::ExtractLights,
        Self::ExtractEmissives,
        Self::ExtractGiParticipants,
        Self::ExtractShadowParticipants,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::ExtractLights => 10,
            Self::ExtractEmissives => 20,
            Self::ExtractGiParticipants => 30,
            Self::ExtractShadowParticipants => 40,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExtractLights => "extract_lights",
            Self::ExtractEmissives => "extract_emissives",
            Self::ExtractGiParticipants => "extract_gi_participants",
            Self::ExtractShadowParticipants => "extract_shadow_participants",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum LuxPrepareSet {
    UpdateLightDatabase,
    UpdateLightClusters,
    UpdateEmissiveCandidates,
    UpdateShadowInvalidation,
    UpdateGiCacheRequests,
}

impl LuxPrepareSet {
    pub const ORDER: [Self; 5] = [
        Self::UpdateLightDatabase,
        Self::UpdateLightClusters,
        Self::UpdateEmissiveCandidates,
        Self::UpdateShadowInvalidation,
        Self::UpdateGiCacheRequests,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::UpdateLightDatabase => 10,
            Self::UpdateLightClusters => 20,
            Self::UpdateEmissiveCandidates => 30,
            Self::UpdateShadowInvalidation => 40,
            Self::UpdateGiCacheRequests => 50,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UpdateLightDatabase => "update_light_database",
            Self::UpdateLightClusters => "update_light_clusters",
            Self::UpdateEmissiveCandidates => "update_emissive_candidates",
            Self::UpdateShadowInvalidation => "update_shadow_invalidation",
            Self::UpdateGiCacheRequests => "update_gi_cache_requests",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, SystemSet)]
pub enum LuxRenderSet {
    DirectLighting,
    ReservoirTemporalReuse,
    ReservoirSpatialReuse,
    VirtualShadowFilter,
    GiTrace,
    GiCacheUpdate,
    ReflectionTrace,
    Denoise,
}

impl LuxRenderSet {
    pub const ORDER: [Self; 8] = [
        Self::DirectLighting,
        Self::ReservoirTemporalReuse,
        Self::ReservoirSpatialReuse,
        Self::VirtualShadowFilter,
        Self::GiTrace,
        Self::GiCacheUpdate,
        Self::ReflectionTrace,
        Self::Denoise,
    ];

    #[must_use]
    pub const fn order_key(self) -> u16 {
        match self {
            Self::DirectLighting => 10,
            Self::ReservoirTemporalReuse => 20,
            Self::ReservoirSpatialReuse => 30,
            Self::VirtualShadowFilter => 40,
            Self::GiTrace => 50,
            Self::GiCacheUpdate => 60,
            Self::ReflectionTrace => 70,
            Self::Denoise => 80,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectLighting => "direct_lighting",
            Self::ReservoirTemporalReuse => "reservoir_temporal_reuse",
            Self::ReservoirSpatialReuse => "reservoir_spatial_reuse",
            Self::VirtualShadowFilter => "virtual_shadow_filter",
            Self::GiTrace => "gi_trace",
            Self::GiCacheUpdate => "gi_cache_update",
            Self::ReflectionTrace => "reflection_trace",
            Self::Denoise => "denoise",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxLightId(pub u64);

impl LuxLightId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != Self::INVALID.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxLightKind {
    Directional,
    Punctual,
    Area,
    EmissiveCandidate,
    Probe,
}

impl LuxLightKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::Punctual => "punctual",
            Self::Area => "area",
            Self::EmissiveCandidate => "emissive_candidate",
            Self::Probe => "probe",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct LuxLight {
    pub light_id: LuxLightId,
    pub kind: LuxLightKind,
    pub intensity_lux: f32,
    pub casts_virtual_shadow: bool,
}

impl LuxLight {
    #[must_use]
    pub const fn new(
        light_id: LuxLightId,
        kind: LuxLightKind,
        intensity_lux: f32,
        casts_virtual_shadow: bool,
    ) -> Self {
        Self {
            light_id,
            kind,
            intensity_lux,
            casts_virtual_shadow,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct LuxEmissive {
    pub light_id: LuxLightId,
    pub candidate_weight: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct LuxGiParticipant {
    pub cache_kind: LuxCacheKind,
    pub dynamic: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct LuxProbeCacheParticipant {
    pub probe_id: u32,
    pub participates_in_relighting: bool,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Component)]
pub struct LightSalience {
    pub selected: bool,
    pub salience: u8,
    pub page_refresh_priority: u8,
}

impl LightSalience {
    pub const SELECTED: Self = Self {
        selected: true,
        salience: u8::MAX,
        page_refresh_priority: u8::MAX,
    };
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct LuxLightDatabase {
    pub revision: u64,
    pub direct_light_count: u32,
    pub emissive_candidate_count: u32,
    pub gi_participant_count: u32,
    pub virtual_shadow_caster_count: u32,
}

impl LuxLightDatabase {
    pub const EMPTY: Self = Self {
        revision: 0,
        direct_light_count: 0,
        emissive_candidate_count: 0,
        gi_participant_count: 0,
        virtual_shadow_caster_count: 0,
    };

    #[must_use]
    pub const fn with_revision(revision: u64) -> Self {
        Self {
            revision,
            ..Self::EMPTY
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Resource)]
pub struct LuxWorld {
    pub lights: LuxLightDatabase,
    pub clusters: LuxClusterGrid,
    pub reservoirs: LuxReservoirStorage,
    pub shadow_requests: LuxShadowRequestQueue,
    pub gi_cache: LuxGiCache,
    pub diagnostics: LuxDiagnostics,
}

impl LuxWorld {
    pub fn record_light(&mut self, light: LuxLight) {
        if light.light_id.is_valid() {
            self.lights.direct_light_count = self.lights.direct_light_count.saturating_add(1);
            self.lights.revision = self.lights.revision.saturating_add(1);
            if light.casts_virtual_shadow {
                self.lights.virtual_shadow_caster_count =
                    self.lights.virtual_shadow_caster_count.saturating_add(1);
                self.shadow_requests.pending_page_requests =
                    self.shadow_requests.pending_page_requests.saturating_add(1);
            }
        }
    }

    pub fn record_light_changed(&mut self, light: LuxLight) {
        self.lights.revision = self.lights.revision.saturating_add(1);
        self.clusters.revision = self.clusters.revision.saturating_add(1);
        self.clusters.dirty_cluster_count = self.clusters.dirty_cluster_count.saturating_add(1);
        self.reservoirs.revision = self.reservoirs.revision.saturating_add(1);
        self.reservoirs.candidate_count = self.reservoirs.candidate_count.saturating_add(1);
        if light.casts_virtual_shadow {
            self.shadow_requests.revision = self.shadow_requests.revision.saturating_add(1);
            self.shadow_requests.invalidated_page_count = self
                .shadow_requests
                .invalidated_page_count
                .saturating_add(1);
        }
    }

    pub fn record_light_removed(&mut self) {
        self.lights.direct_light_count = self.lights.direct_light_count.saturating_sub(1);
        self.lights.revision = self.lights.revision.saturating_add(1);
        self.shadow_requests.revision = self.shadow_requests.revision.saturating_add(1);
        self.shadow_requests.invalidated_page_count = self
            .shadow_requests
            .invalidated_page_count
            .saturating_add(1);
        self.gi_cache.revision = self.gi_cache.revision.saturating_add(1);
        self.gi_cache.invalidated_entry_count =
            self.gi_cache.invalidated_entry_count.saturating_add(1);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LuxClusterGrid {
    pub revision: u64,
    pub occupied_cluster_count: u32,
    pub dirty_cluster_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LuxReservoirStorage {
    pub revision: u64,
    pub candidate_count: u32,
    pub temporal_reuse_passes: u32,
    pub spatial_reuse_passes: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LuxShadowRequestQueue {
    pub revision: u64,
    pub caster_count: u32,
    pub receiver_count: u32,
    pub pending_page_requests: u32,
    pub invalidated_page_count: u32,
    pub refresh_priority: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LuxGiCache {
    pub revision: u64,
    pub participant_count: u32,
    pub request_count: u32,
    pub invalidated_entry_count: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct LuxDiagnostics {
    pub extracted_light_count: u32,
    pub extracted_emissive_count: u32,
    pub extracted_gi_participant_count: u32,
    pub extracted_shadow_participant_count: u32,
    pub selected_light_count: u32,
    pub direct_lighting_passes: u32,
    pub denoise_passes: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxLightEventKind {
    LightChanged,
    EmissiveCandidatePromoted,
    ShadowInvalidated,
    GiCacheInvalidated,
    ProbeCacheInvalidated,
}

impl LuxLightEventKind {
    pub const ALL: [Self; 5] = [
        Self::LightChanged,
        Self::EmissiveCandidatePromoted,
        Self::ShadowInvalidated,
        Self::GiCacheInvalidated,
        Self::ProbeCacheInvalidated,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LightChanged => "light_changed",
            Self::EmissiveCandidatePromoted => "emissive_candidate_promoted",
            Self::ShadowInvalidated => "shadow_invalidated",
            Self::GiCacheInvalidated => "gi_cache_invalidated",
            Self::ProbeCacheInvalidated => "probe_cache_invalidated",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Message)]
pub struct LuxLightEvent {
    pub kind: LuxLightEventKind,
    pub light_id: LuxLightId,
    pub revision: u64,
}

impl LuxLightEvent {
    #[must_use]
    pub const fn new(kind: LuxLightEventKind, light_id: LuxLightId, revision: u64) -> Self {
        Self {
            kind,
            light_id,
            revision,
        }
    }
}

impl LuxFeatureSet {
    pub const DEFAULT_REALTIME: Self = Self {
        direct_lighting: true,
        many_light_sampling: true,
        virtual_shadows: true,
        global_illumination: true,
        reflections: true,
        denoising_reconstruction: true,
        radiance_cache: true,
        surface_cache: true,
        probe_cache: true,
    };

    pub const HEURISTIC_FALLBACK: Self = Self {
        direct_lighting: true,
        many_light_sampling: false,
        virtual_shadows: false,
        global_illumination: false,
        reflections: false,
        denoising_reconstruction: true,
        radiance_cache: false,
        surface_cache: false,
        probe_cache: false,
    };

    #[must_use]
    pub const fn uses_cache(self, kind: LuxCacheKind) -> bool {
        match kind {
            LuxCacheKind::Radiance => self.radiance_cache,
            LuxCacheKind::Surface => self.surface_cache,
            LuxCacheKind::Probe => self.probe_cache,
        }
    }
}

#[must_use]
pub fn compact_light_record(entity: Entity, light: &fun_scene::LuxLight) -> LuxLight {
    LuxLight::new(
        LuxLightId::new(entity.to_bits()),
        map_scene_light_kind(light.kind),
        light.intensity_lux,
        scene_shadow_policy_casts_virtual_shadow(light.shadow_policy),
    )
}

#[must_use]
pub const fn map_scene_light_kind(kind: fun_scene::LuxLightKind) -> LuxLightKind {
    match kind {
        fun_scene::LuxLightKind::Directional => LuxLightKind::Directional,
        fun_scene::LuxLightKind::Punctual => LuxLightKind::Punctual,
        fun_scene::LuxLightKind::Area => LuxLightKind::Area,
        fun_scene::LuxLightKind::EmissiveCandidate => LuxLightKind::EmissiveCandidate,
        fun_scene::LuxLightKind::Probe => LuxLightKind::Probe,
    }
}

#[must_use]
pub const fn scene_shadow_policy_casts_virtual_shadow(policy: fun_scene::LuxShadowPolicy) -> bool {
    matches!(
        policy,
        fun_scene::LuxShadowPolicy::VirtualDemandPaged
            | fun_scene::LuxShadowPolicy::VirtualDirectionalClipmap
            | fun_scene::LuxShadowPolicy::RayTraced
    )
}

#[must_use]
pub const fn importance_page_priority(importance: fun_scene::LuxImportance) -> u8 {
    match importance {
        fun_scene::LuxImportance::Low => 32,
        fun_scene::LuxImportance::Normal => 96,
        fun_scene::LuxImportance::High => 180,
        fun_scene::LuxImportance::Critical => u8::MAX,
    }
}

#[must_use]
pub const fn shadow_receiver_priority(priority: fun_scene::ShadowReceiverPriority) -> u8 {
    match priority {
        fun_scene::ShadowReceiverPriority::Low => 32,
        fun_scene::ShadowReceiverPriority::Normal => 96,
        fun_scene::ShadowReceiverPriority::High => 180,
        fun_scene::ShadowReceiverPriority::Critical => u8::MAX,
    }
}

#[must_use]
pub const fn shadow_caster_priority(
    policy: fun_scene::ShadowCasterPolicy,
    invalidation: fun_scene::ShadowInvalidationPolicy,
) -> u8 {
    match (policy, invalidation) {
        (fun_scene::ShadowCasterPolicy::None, _) => 0,
        (fun_scene::ShadowCasterPolicy::StaticMap, _) => 64,
        (fun_scene::ShadowCasterPolicy::RayTraced, _) => 128,
        (
            fun_scene::ShadowCasterPolicy::VirtualPages,
            fun_scene::ShadowInvalidationPolicy::Static,
        ) => 96,
        (
            fun_scene::ShadowCasterPolicy::VirtualPages,
            fun_scene::ShadowInvalidationPolicy::OnTransformChange,
        ) => 180,
        (
            fun_scene::ShadowCasterPolicy::VirtualPages,
            fun_scene::ShadowInvalidationPolicy::OnTransformOrGeometryChange
            | fun_scene::ShadowInvalidationPolicy::OnTransformGeometryOrLightChange,
        ) => u8::MAX,
    }
}

pub fn extract_lights(query: Query<(), Added<fun_scene::LuxLight>>, mut world: ResMut<LuxWorld>) {
    for _ in query.iter() {
        world.diagnostics.extracted_light_count =
            world.diagnostics.extracted_light_count.saturating_add(1);
    }
}

pub fn extract_emissives(
    query: Query<(), Added<fun_scene::LuxEmissive>>,
    mut world: ResMut<LuxWorld>,
) {
    for _ in query.iter() {
        world.diagnostics.extracted_emissive_count =
            world.diagnostics.extracted_emissive_count.saturating_add(1);
    }
}

pub fn extract_gi_participants(
    query: Query<(), Added<fun_scene::LuxGiParticipant>>,
    mut world: ResMut<LuxWorld>,
) {
    for _ in query.iter() {
        world.diagnostics.extracted_gi_participant_count = world
            .diagnostics
            .extracted_gi_participant_count
            .saturating_add(1);
    }
}

pub fn extract_shadow_participants(
    casters: Query<(), Added<fun_scene::VirtualShadowCaster>>,
    receivers: Query<(), Added<fun_scene::VirtualShadowReceiver>>,
    mut world: ResMut<LuxWorld>,
) {
    for _ in casters.iter() {
        world.diagnostics.extracted_shadow_participant_count = world
            .diagnostics
            .extracted_shadow_participant_count
            .saturating_add(1);
    }
    for _ in receivers.iter() {
        world.diagnostics.extracted_shadow_participant_count = world
            .diagnostics
            .extracted_shadow_participant_count
            .saturating_add(1);
    }
}

pub fn update_light_database(
    query: Query<(Entity, &fun_scene::LuxLight), Added<fun_scene::LuxLight>>,
    mut world: ResMut<LuxWorld>,
) {
    for (entity, light) in query.iter() {
        world.record_light(compact_light_record(entity, light));
        world.shadow_requests.refresh_priority = world
            .shadow_requests
            .refresh_priority
            .max(importance_page_priority(light.importance));
    }
}

pub fn update_light_clusters(
    query: Query<(Entity, &fun_scene::LuxLight), Changed<fun_scene::LuxLight>>,
    mut world: ResMut<LuxWorld>,
) {
    for (entity, light) in query.iter() {
        let record = compact_light_record(entity, light);
        world.record_light_changed(record);
        world.clusters.occupied_cluster_count =
            world.clusters.occupied_cluster_count.saturating_add(1);
    }
}

pub fn update_emissive_candidates(
    query: Query<&fun_scene::LuxEmissive, Changed<fun_scene::LuxEmissive>>,
    mut world: ResMut<LuxWorld>,
) {
    for emissive in query.iter() {
        if emissive.candidate_policy != fun_scene::EmissiveCandidatePolicy::Never {
            world.lights.emissive_candidate_count =
                world.lights.emissive_candidate_count.saturating_add(1);
            world.reservoirs.revision = world.reservoirs.revision.saturating_add(1);
            world.reservoirs.candidate_count = world.reservoirs.candidate_count.saturating_add(1);
        }
    }
}

pub fn update_shadow_invalidation(
    casters: Query<&fun_scene::VirtualShadowCaster, Changed<fun_scene::VirtualShadowCaster>>,
    receivers: Query<&fun_scene::VirtualShadowReceiver, Changed<fun_scene::VirtualShadowReceiver>>,
    selections: Query<&LightSalience, Changed<LightSalience>>,
    mut removed_lights: RemovedComponents<fun_scene::LuxLight>,
    mut world: ResMut<LuxWorld>,
) {
    for caster in casters.iter() {
        if caster.policy == fun_scene::ShadowCasterPolicy::None {
            continue;
        }
        world.shadow_requests.caster_count = world.shadow_requests.caster_count.saturating_add(1);
        world.shadow_requests.pending_page_requests = world
            .shadow_requests
            .pending_page_requests
            .saturating_add(1);
        world.shadow_requests.refresh_priority = world
            .shadow_requests
            .refresh_priority
            .max(shadow_caster_priority(caster.policy, caster.invalidation));
    }
    for receiver in receivers.iter() {
        world.shadow_requests.receiver_count =
            world.shadow_requests.receiver_count.saturating_add(1);
        world.shadow_requests.pending_page_requests = world
            .shadow_requests
            .pending_page_requests
            .saturating_add(1);
        world.shadow_requests.refresh_priority = world
            .shadow_requests
            .refresh_priority
            .max(shadow_receiver_priority(receiver.priority));
    }
    for salience in selections.iter() {
        if salience.selected {
            world.diagnostics.selected_light_count =
                world.diagnostics.selected_light_count.saturating_add(1);
        }
        world.shadow_requests.refresh_priority = world
            .shadow_requests
            .refresh_priority
            .max(salience.page_refresh_priority);
    }
    for _ in removed_lights.read() {
        world.record_light_removed();
    }
}

pub fn update_gi_cache_requests(
    query: Query<&fun_scene::LuxGiParticipant, Changed<fun_scene::LuxGiParticipant>>,
    mut world: ResMut<LuxWorld>,
) {
    for participant in query.iter() {
        if participant.bounce_policy != fun_scene::GiBouncePolicy::Disabled {
            world.lights.gi_participant_count = world.lights.gi_participant_count.saturating_add(1);
            world.gi_cache.participant_count = world.gi_cache.participant_count.saturating_add(1);
            world.gi_cache.request_count = world.gi_cache.request_count.saturating_add(1);
            world.gi_cache.revision = world.gi_cache.revision.saturating_add(1);
        }
    }
}

pub fn direct_lighting(mut world: ResMut<LuxWorld>) {
    world.diagnostics.direct_lighting_passes =
        world.diagnostics.direct_lighting_passes.saturating_add(1);
}

pub fn reservoir_temporal_reuse(mut world: ResMut<LuxWorld>) {
    world.reservoirs.temporal_reuse_passes =
        world.reservoirs.temporal_reuse_passes.saturating_add(1);
}

pub fn reservoir_spatial_reuse(mut world: ResMut<LuxWorld>) {
    world.reservoirs.spatial_reuse_passes = world.reservoirs.spatial_reuse_passes.saturating_add(1);
}

pub fn virtual_shadow_filter(mut world: ResMut<LuxWorld>) {
    world.shadow_requests.revision = world.shadow_requests.revision.saturating_add(1);
}

pub fn gi_trace(mut world: ResMut<LuxWorld>) {
    world.gi_cache.request_count = world.gi_cache.request_count.saturating_add(1);
}

pub fn gi_cache_update(mut world: ResMut<LuxWorld>) {
    world.gi_cache.revision = world.gi_cache.revision.saturating_add(1);
}

pub fn reflection_trace() {}

pub fn denoise(mut world: ResMut<LuxWorld>) {
    world.diagnostics.denoise_passes = world.diagnostics.denoise_passes.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use bevy_ecs::schedule::{IntoScheduleConfigs, Schedule};
    use bevy_ecs::world::World;
    use fun_scene::{
        EmissiveCandidatePolicy, GiBouncePolicy, GiCachePolicy, LuxEmissive as SceneLuxEmissive,
        LuxGiParticipant as SceneLuxGiParticipant, LuxLight as SceneLuxLight, ShadowCasterPolicy,
        ShadowFilterPolicy, ShadowInvalidationPolicy, ShadowReceiverPriority, VirtualShadowCaster,
        VirtualShadowReceiver,
    };

    use super::*;

    #[test]
    fn subsystem_labels_are_stable() {
        assert_eq!(LuxSubsystem::ALL.len(), 9);
        assert_eq!(LuxSubsystem::DirectLighting.as_str(), "direct_lighting");
        assert_eq!(
            LuxSubsystem::DenoisingReconstruction.as_str(),
            "denoising_reconstruction"
        );
        assert_eq!(LuxSubsystem::ProbeCache.as_str(), "probe_cache");
    }

    #[test]
    fn policies_and_caches_are_fun_lux_owned_and_non_identifying() {
        for descriptor in FUN_LUX_POLICY_DESCRIPTORS {
            assert_eq!(descriptor.owner_crate, FUN_LUX_CRATE_NAME);
            assert!(!descriptor.accepts_identifiable_data);
        }

        for descriptor in FUN_LUX_CACHE_DESCRIPTORS {
            assert_eq!(descriptor.owner_crate, FUN_LUX_CRATE_NAME);
            assert!(!descriptor.accepts_identifiable_data);
            assert!(descriptor.frame_local_default);
        }
    }

    #[test]
    fn model_assisted_reconstruction_declares_fun_ai_runtime_dependency() {
        assert!(LuxDenoiseReconstructionPath::ModelAssisted.requires_fun_ai_runtime());
        assert!(!LuxDenoiseReconstructionPath::BalancedFast.requires_fun_ai_runtime());
        assert!(!LuxDenoiseReconstructionPath::DlssRayReconstruction.requires_fun_ai_runtime());
    }

    #[test]
    fn fallback_feature_set_remains_local_and_direct() {
        let features = LuxFeatureSet::HEURISTIC_FALLBACK;

        assert!(features.direct_lighting);
        assert!(features.denoising_reconstruction);
        assert!(!features.many_light_sampling);
        assert!(!features.uses_cache(LuxCacheKind::Radiance));
        assert!(!features.uses_cache(LuxCacheKind::Surface));
        assert!(!features.uses_cache(LuxCacheKind::Probe));
    }

    #[test]
    fn lux_light_components_are_ecs_authored_and_database_backed() {
        let light = LuxLight::new(LuxLightId::new(12), LuxLightKind::Punctual, 1200.0, true);
        assert!(light.light_id.is_valid());
        assert_eq!(light.kind.as_str(), "punctual");
        assert!(light.casts_virtual_shadow);

        let database = LuxLightDatabase::with_revision(9);
        assert_eq!(database.revision, 9);
        assert_eq!(database.direct_light_count, 0);
    }

    #[test]
    fn lux_light_events_cover_shadow_and_gi_invalidation() {
        assert_eq!(LuxLightEventKind::ALL.len(), 5);
        assert!(LuxLightEventKind::ALL.contains(&LuxLightEventKind::ShadowInvalidated));
        assert!(LuxLightEventKind::ALL.contains(&LuxLightEventKind::GiCacheInvalidated));
    }

    #[test]
    fn lux_system_sets_cover_extract_prepare_and_render_lanes() {
        assert_ordered(
            LuxExtractSet::ORDER
                .iter()
                .map(|set| (set.order_key(), set.as_str())),
        );
        assert_ordered(
            LuxPrepareSet::ORDER
                .iter()
                .map(|set| (set.order_key(), set.as_str())),
        );
        assert_ordered(
            LuxRenderSet::ORDER
                .iter()
                .map(|set| (set.order_key(), set.as_str())),
        );

        assert_eq!(LuxExtractSet::ExtractLights.as_str(), "extract_lights");
        assert_eq!(
            LuxPrepareSet::UpdateShadowInvalidation.as_str(),
            "update_shadow_invalidation"
        );
        assert_eq!(LuxRenderSet::Denoise.as_str(), "denoise");
    }

    #[test]
    fn scene_authored_light_entities_appear_in_lux_light_database() {
        let mut world = World::new();
        world.insert_resource(LuxWorld::default());
        world.spawn(SceneLuxLight::directional(80_000.0));

        let mut schedule = Schedule::default();
        schedule.add_systems((extract_lights, update_light_database).chain());
        schedule.run(&mut world);

        let lux_world = world.resource::<LuxWorld>();
        assert_eq!(lux_world.lights.direct_light_count, 1);
        assert_eq!(lux_world.diagnostics.extracted_light_count, 1);
        assert_eq!(lux_world.shadow_requests.pending_page_requests, 1);
    }

    #[test]
    fn changing_light_updates_clusters_and_reservoirs() {
        let mut world = World::new();
        world.insert_resource(LuxWorld::default());
        let entity = world.spawn(SceneLuxLight::directional(20_000.0)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_light_clusters);
        schedule.run(&mut world);
        *world.resource_mut::<LuxWorld>() = LuxWorld::default();

        world
            .entity_mut(entity)
            .get_mut::<SceneLuxLight>()
            .expect("scene light should exist")
            .intensity_lux = 60_000.0;
        schedule.run(&mut world);

        let lux_world = world.resource::<LuxWorld>();
        assert_eq!(lux_world.clusters.dirty_cluster_count, 1);
        assert_eq!(lux_world.clusters.occupied_cluster_count, 1);
        assert_eq!(lux_world.reservoirs.candidate_count, 1);
        assert_eq!(lux_world.lights.revision, 1);
    }

    #[test]
    fn removing_light_invalidates_shadow_and_gi_state() {
        let mut world = World::new();
        world.insert_resource(LuxWorld {
            lights: LuxLightDatabase {
                direct_light_count: 1,
                ..Default::default()
            },
            ..Default::default()
        });
        let entity = world.spawn(SceneLuxLight::directional(20_000.0)).id();

        let mut schedule = Schedule::default();
        schedule.add_systems(update_shadow_invalidation);
        schedule.run(&mut world);
        let _ = world.despawn(entity);
        schedule.run(&mut world);

        let lux_world = world.resource::<LuxWorld>();
        assert_eq!(lux_world.lights.direct_light_count, 0);
        assert_eq!(lux_world.shadow_requests.invalidated_page_count, 1);
        assert_eq!(lux_world.gi_cache.invalidated_entry_count, 1);
    }

    #[test]
    fn editor_light_selection_raises_salience_and_page_refresh_priority() {
        let mut world = World::new();
        world.insert_resource(LuxWorld::default());
        world.spawn((
            SceneLuxLight::directional(20_000.0),
            LightSalience::SELECTED,
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(update_shadow_invalidation);
        schedule.run(&mut world);

        let lux_world = world.resource::<LuxWorld>();
        assert_eq!(lux_world.diagnostics.selected_light_count, 1);
        assert_eq!(lux_world.shadow_requests.refresh_priority, u8::MAX);
    }

    #[test]
    fn emissive_gi_and_shadow_participants_feed_lux_world() {
        let mut world = World::new();
        world.insert_resource(LuxWorld::default());
        world.spawn((
            SceneLuxEmissive {
                luminance: 1200.0,
                candidate_policy: EmissiveCandidatePolicy::AlwaysPromote,
            },
            SceneLuxGiParticipant {
                bounce_policy: GiBouncePolicy::DynamicBudgeted,
                cache_policy: GiCachePolicy::Probe,
            },
            VirtualShadowCaster {
                policy: ShadowCasterPolicy::VirtualPages,
                invalidation: ShadowInvalidationPolicy::OnTransformOrGeometryChange,
            },
            VirtualShadowReceiver {
                priority: ShadowReceiverPriority::High,
                filter_policy: ShadowFilterPolicy::ContactAware,
            },
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                extract_emissives,
                extract_gi_participants,
                extract_shadow_participants,
                update_emissive_candidates,
                update_gi_cache_requests,
                update_shadow_invalidation,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let lux_world = world.resource::<LuxWorld>();
        assert_eq!(lux_world.lights.emissive_candidate_count, 1);
        assert_eq!(lux_world.reservoirs.candidate_count, 1);
        assert_eq!(lux_world.lights.gi_participant_count, 1);
        assert_eq!(lux_world.gi_cache.request_count, 1);
        assert_eq!(lux_world.shadow_requests.caster_count, 1);
        assert_eq!(lux_world.shadow_requests.receiver_count, 1);
        assert_eq!(lux_world.shadow_requests.refresh_priority, u8::MAX);
    }

    #[test]
    fn render_lane_updates_reuse_cache_and_denoise_diagnostics() {
        let mut world = World::new();
        world.insert_resource(LuxWorld::default());

        let mut schedule = Schedule::default();
        schedule.add_systems(
            (
                direct_lighting,
                reservoir_temporal_reuse,
                reservoir_spatial_reuse,
                virtual_shadow_filter,
                gi_trace,
                gi_cache_update,
                reflection_trace,
                denoise,
            )
                .chain(),
        );
        schedule.run(&mut world);

        let lux_world = world.resource::<LuxWorld>();
        assert_eq!(lux_world.diagnostics.direct_lighting_passes, 1);
        assert_eq!(lux_world.reservoirs.temporal_reuse_passes, 1);
        assert_eq!(lux_world.reservoirs.spatial_reuse_passes, 1);
        assert_eq!(lux_world.shadow_requests.revision, 1);
        assert_eq!(lux_world.gi_cache.request_count, 1);
        assert_eq!(lux_world.gi_cache.revision, 1);
        assert_eq!(lux_world.diagnostics.denoise_passes, 1);
    }

    fn assert_ordered<'a>(items: impl Iterator<Item = (u16, &'a str)>) {
        let mut previous = 0;
        for (order_key, label) in items {
            assert!(order_key > previous, "{label}");
            previous = order_key;
        }
    }
}
