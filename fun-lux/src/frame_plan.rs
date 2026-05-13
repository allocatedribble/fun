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

use crate::api::{DirectLightingMode, GiMode, ReflectionMode, ShadowMode};
use crate::diagnostics::LuxFramePlanDiagnostics;
use crate::look::FunLuxLookProfile;
use crate::pass::LuxPassRequest;
use crate::quality::{LuxQualitySettings, LuxQualityTier};
use crate::volumetric::FunLuxVolumetricSettings;

pub const FUN_LUX_FRAME_PLAN_SCHEMA_VERSION: u16 = 1;
pub const LUX_RESOURCE_INTENT_KIND_COUNT: usize = 22;

// ============================================================================
// Section 1 — Scene plan (per-scene record)
// ============================================================================

/// Typed scene identifier. The renderer maps the typed id to
/// its own scene registry; the typed id itself is opaque to
/// the renderer.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxSceneId(pub u64);

impl LuxSceneId {
    pub const PROOF_SCENE: Self = Self(0);

    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

/// Typed scene priority — drives the renderer's dispatch
/// order when multiple scenes share the same frame.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxScenePriority {
    Background,
    #[default]
    World,
    Hud,
    DebugOverlay,
}

impl LuxScenePriority {
    pub const ALL: [Self; 4] = [Self::Background, Self::World, Self::Hud, Self::DebugOverlay];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::World => "world",
            Self::Hud => "hud",
            Self::DebugOverlay => "debug_overlay",
        }
    }

    #[must_use]
    pub const fn order_key(self) -> u8 {
        match self {
            Self::Background => 0,
            Self::World => 1,
            Self::Hud => 2,
            Self::DebugOverlay => 3,
        }
    }
}

// Pass 2 replaced the Pass 1 pixel-rect `LuxDirtyRegion`
// (framebuffer-coord rectangle) with the typed world-space
// [`crate::dirty::LuxDirtyRegion`] (LuxAabb + LuxDirtyFlags +
// priority + cost estimate + deadline). The renderer
// projects world-space dirty volumes into pixel-space scissor
// rects when it needs them; the typed lighting plan carries
// the richer world-space record.
//
// `LuxSceneFramePlan::dirty_regions: Vec<LuxDirtyRegion>` uses
// the Pass 2 type via the re-export below.

pub use crate::dirty::LuxDirtyRegion;

/// Typed per-scene frame plan. `LuxFramePlan` aggregates one
/// `LuxSceneFramePlan` per scene.
#[derive(Debug, Clone, PartialEq)]
pub struct LuxSceneFramePlan {
    pub schema_version: u16,
    pub scene_id: LuxSceneId,
    pub scene_revision: u64,
    pub visible: bool,
    pub priority: LuxScenePriority,
    pub passes: Vec<LuxPassRequest>,
    pub resources: Vec<LuxResourceIntent>,
    pub dirty_regions: Vec<LuxDirtyRegion>,
}

impl LuxSceneFramePlan {
    /// Typed cold-default scene plan: invisible, no work.
    #[must_use]
    pub fn cold_default(scene_id: LuxSceneId) -> Self {
        Self {
            schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
            scene_id,
            scene_revision: 0,
            visible: false,
            priority: LuxScenePriority::default(),
            passes: Vec::new(),
            resources: Vec::new(),
            dirty_regions: Vec::new(),
        }
    }

    /// Typed predicate: minimal scene plan (no passes, no
    /// resources, no dirty regions).
    #[must_use]
    pub fn is_minimal(&self) -> bool {
        self.passes.is_empty() && self.resources.is_empty() && self.dirty_regions.is_empty()
    }

    /// Typed predicate: scene plan emitted at least one
    /// pass dispatch.
    #[must_use]
    pub fn emitted_work(&self) -> bool {
        !self.passes.is_empty()
    }
}

// ============================================================================
// Section 2 — 22-variant LuxResourceIntent
// ============================================================================

/// Typed lux resource intent. The renderer reads the variant
/// and allocates / binds the appropriate concrete `wgpu`
/// resource; the intent itself names no backend handle.
///
/// Every variant carries a stable id so the renderer-side
/// resource cache can match repeat allocations across
/// frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxResourceIntent {
    LightBuffer {
        stable_id: &'static str,
        max_light_count: u32,
        bytes_per_light: u32,
    },
    LightIndexBuffer {
        stable_id: &'static str,
        max_cluster_count: u32,
        max_lights_per_cluster: u32,
    },
    ClusterGrid {
        stable_id: &'static str,
        clusters_x: u32,
        clusters_y: u32,
        clusters_z: u32,
    },
    ReservoirBuffer {
        stable_id: &'static str,
        sample_count: u32,
    },
    ShadowRequestBuffer {
        stable_id: &'static str,
        max_request_count: u32,
    },
    ShadowAtlas {
        stable_id: &'static str,
        atlas_extent: u32,
        slot_count: u32,
    },
    VirtualShadowPageTable {
        stable_id: &'static str,
        page_count: u32,
    },
    VoxelShadowPageTable {
        stable_id: &'static str,
        page_count: u32,
    },
    VoxelTerrainSdfPool {
        stable_id: &'static str,
        page_count: u32,
    },
    SurfaceCache {
        stable_id: &'static str,
        surface_count: u32,
    },
    RadianceCache {
        stable_id: &'static str,
        voxel_count: u64,
    },
    VoxelTerrainRadianceClipmap {
        stable_id: &'static str,
        voxel_count: u64,
    },
    VoxelCanopyOpacityClipmap {
        stable_id: &'static str,
        voxel_count: u64,
    },
    StormExtinctionClipmap {
        stable_id: &'static str,
        voxel_count: u64,
    },
    ProbeCache {
        stable_id: &'static str,
        probe_count: u32,
    },
    ReflectionTraceBuffer {
        stable_id: &'static str,
        ray_count: u32,
    },
    DenoiseHistory {
        stable_id: &'static str,
        width: u32,
        height: u32,
    },
    VolumetricFroxelDensity {
        stable_id: &'static str,
        width: u32,
        height: u32,
        depth: u32,
    },
    VolumetricFroxelScattering {
        stable_id: &'static str,
        width: u32,
        height: u32,
        depth: u32,
    },
    VolumetricHistory {
        stable_id: &'static str,
        width: u32,
        height: u32,
        depth: u32,
    },
    IntegratedFog {
        stable_id: &'static str,
        width: u32,
        height: u32,
    },
    LuxDebugBuffer {
        stable_id: &'static str,
        byte_size: u64,
    },
}

/// Typed kind tag for [`LuxResourceIntent`]. Used by
/// observability / audit harnesses to histogram resources
/// without unpacking variant payloads.
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
    pub const fn as_str(self) -> &'static str {
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

impl LuxResourceIntent {
    /// Typed convenience accessor: the resource's stable id.
    #[must_use]
    pub const fn stable_id(&self) -> &'static str {
        match self {
            Self::LightBuffer { stable_id, .. }
            | Self::LightIndexBuffer { stable_id, .. }
            | Self::ClusterGrid { stable_id, .. }
            | Self::ReservoirBuffer { stable_id, .. }
            | Self::ShadowRequestBuffer { stable_id, .. }
            | Self::ShadowAtlas { stable_id, .. }
            | Self::VirtualShadowPageTable { stable_id, .. }
            | Self::VoxelShadowPageTable { stable_id, .. }
            | Self::VoxelTerrainSdfPool { stable_id, .. }
            | Self::SurfaceCache { stable_id, .. }
            | Self::RadianceCache { stable_id, .. }
            | Self::VoxelTerrainRadianceClipmap { stable_id, .. }
            | Self::VoxelCanopyOpacityClipmap { stable_id, .. }
            | Self::StormExtinctionClipmap { stable_id, .. }
            | Self::ProbeCache { stable_id, .. }
            | Self::ReflectionTraceBuffer { stable_id, .. }
            | Self::DenoiseHistory { stable_id, .. }
            | Self::VolumetricFroxelDensity { stable_id, .. }
            | Self::VolumetricFroxelScattering { stable_id, .. }
            | Self::VolumetricHistory { stable_id, .. }
            | Self::IntegratedFog { stable_id, .. }
            | Self::LuxDebugBuffer { stable_id, .. } => stable_id,
        }
    }

    /// Typed kind tag.
    #[must_use]
    pub const fn kind(&self) -> LuxResourceIntentKind {
        match self {
            Self::LightBuffer { .. } => LuxResourceIntentKind::LightBuffer,
            Self::LightIndexBuffer { .. } => LuxResourceIntentKind::LightIndexBuffer,
            Self::ClusterGrid { .. } => LuxResourceIntentKind::ClusterGrid,
            Self::ReservoirBuffer { .. } => LuxResourceIntentKind::ReservoirBuffer,
            Self::ShadowRequestBuffer { .. } => LuxResourceIntentKind::ShadowRequestBuffer,
            Self::ShadowAtlas { .. } => LuxResourceIntentKind::ShadowAtlas,
            Self::VirtualShadowPageTable { .. } => LuxResourceIntentKind::VirtualShadowPageTable,
            Self::VoxelShadowPageTable { .. } => LuxResourceIntentKind::VoxelShadowPageTable,
            Self::VoxelTerrainSdfPool { .. } => LuxResourceIntentKind::VoxelTerrainSdfPool,
            Self::SurfaceCache { .. } => LuxResourceIntentKind::SurfaceCache,
            Self::RadianceCache { .. } => LuxResourceIntentKind::RadianceCache,
            Self::VoxelTerrainRadianceClipmap { .. } => {
                LuxResourceIntentKind::VoxelTerrainRadianceClipmap
            }
            Self::VoxelCanopyOpacityClipmap { .. } => {
                LuxResourceIntentKind::VoxelCanopyOpacityClipmap
            }
            Self::StormExtinctionClipmap { .. } => LuxResourceIntentKind::StormExtinctionClipmap,
            Self::ProbeCache { .. } => LuxResourceIntentKind::ProbeCache,
            Self::ReflectionTraceBuffer { .. } => LuxResourceIntentKind::ReflectionTraceBuffer,
            Self::DenoiseHistory { .. } => LuxResourceIntentKind::DenoiseHistory,
            Self::VolumetricFroxelDensity { .. } => LuxResourceIntentKind::VolumetricFroxelDensity,
            Self::VolumetricFroxelScattering { .. } => {
                LuxResourceIntentKind::VolumetricFroxelScattering
            }
            Self::VolumetricHistory { .. } => LuxResourceIntentKind::VolumetricHistory,
            Self::IntegratedFog { .. } => LuxResourceIntentKind::IntegratedFog,
            Self::LuxDebugBuffer { .. } => LuxResourceIntentKind::LuxDebugBuffer,
        }
    }
}

// ============================================================================
// Section 3 — Frame plan
// ============================================================================

/// Typed backend-neutral lighting frame plan. The audit
/// handle for Pass 1's "Replace `NoopLuxCore` with a real
/// `LuxFramePlan`" goal. `fun-renderer` consumes the plan
/// and dispatches against the real GPU.
///
/// `LuxFramePlan` does not derive `Eq` because Pass 2's
/// world-space [`LuxDirtyRegion`] carries `LuxAabb` (f32) —
/// equality uses bitwise comparison via `LuxAabb::bitwise_eq`
/// or the typed `PartialEq` impl below.
#[derive(Debug, Clone, PartialEq)]
pub struct LuxFramePlan {
    pub schema_version: u16,
    pub frame_index: u64,
    pub scene_plans: Vec<LuxSceneFramePlan>,
    pub global_quality: LuxQualitySettings,
    pub look_profile: FunLuxLookProfile,
    pub volumetric: FunLuxVolumetricSettings,
    pub diagnostics: LuxFramePlanDiagnostics,
    pub direct_lighting: DirectLightingMode,
    pub shadows: ShadowMode,
    pub gi: GiMode,
    pub reflections: ReflectionMode,
}

impl LuxFramePlan {
    /// Typed cold-default plan: no scenes, no passes, no
    /// resources. Equivalent to the `NoopLuxCore` baseline
    /// frame and reserved for tests / diagnostics / early
    /// fallback.
    #[must_use]
    pub fn cold_default() -> Self {
        Self {
            schema_version: FUN_LUX_FRAME_PLAN_SCHEMA_VERSION,
            frame_index: 0,
            scene_plans: Vec::new(),
            global_quality: LuxQualitySettings::PRODUCT_DEFAULT,
            look_profile: FunLuxLookProfile::COLD_DEFAULT,
            volumetric: FunLuxVolumetricSettings::COLD_DEFAULT,
            diagnostics: LuxFramePlanDiagnostics::COLD_DEFAULT,
            direct_lighting: DirectLightingMode::Disabled,
            shadows: ShadowMode::Disabled,
            gi: GiMode::Disabled,
            reflections: ReflectionMode::Disabled,
        }
    }

    /// Typed predicate: minimal plan (no scene work).
    #[must_use]
    pub fn is_minimal(&self) -> bool {
        self.scene_plans.iter().all(|s| s.is_minimal())
    }

    /// Typed predicate: zero scenes, zero passes, zero
    /// resources, zero dirty regions — the strictest minimal
    /// plan, equivalent to the `NoopLuxCore` baseline.
    #[must_use]
    pub fn is_noop_baseline(&self) -> bool {
        self.scene_plans.is_empty()
            && matches!(self.direct_lighting, DirectLightingMode::Disabled)
            && matches!(self.shadows, ShadowMode::Disabled)
    }

    /// Typed convenience: aggregate the typed quality tier
    /// from `global_quality.default_tier`.
    #[must_use]
    pub const fn aggregate_quality_tier(&self) -> LuxQualityTier {
        self.global_quality.default_tier
    }

    /// Typed convenience: aggregate pass count across every
    /// scene plan (used by diagnostics).
    #[must_use]
    pub fn aggregate_pass_count(&self) -> u32 {
        self.scene_plans.iter().map(|s| s.passes.len() as u32).sum()
    }

    /// Typed convenience: aggregate resource count across
    /// every scene plan.
    #[must_use]
    pub fn aggregate_resource_count(&self) -> u32 {
        self.scene_plans
            .iter()
            .map(|s| s.resources.len() as u32)
            .sum()
    }
}

// ============================================================================
// Section 4 — Backend contract (Pass 0 audit handle, preserved)
// ============================================================================

/// Typed backend contract record (Pass 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxBackendContract {
    pub schema_version: u16,
    pub fun_lux_owns_lighting_policy: bool,
    pub fun_renderer_is_only_production_executor: bool,
    pub fun_lux_emits_backend_neutral_plans: bool,
    pub legacy_lighting_paths_are_invalid_for_production: bool,
    pub noop_lux_core_is_non_production_only: bool,
    pub forbidden_fun_lux_imports: &'static [&'static str],
    pub fun_renderer_depends_on_fun_lux: bool,
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

    #[must_use]
    pub fn is_forbidden_import(&self, crate_name: &str) -> bool {
        self.forbidden_fun_lux_imports.contains(&crate_name)
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
        assert_eq!(LUX_RESOURCE_INTENT_KIND_COUNT, 22);
        assert_eq!(
            LuxResourceIntentKind::ALL.len(),
            LUX_RESOURCE_INTENT_KIND_COUNT,
        );
    }

    #[test]
    fn scene_priority_taxonomy() {
        let mut seen = hashbrown::HashSet::new();
        for p in LuxScenePriority::ALL {
            assert!(seen.insert(p.as_str()), "duplicate: {}", p.as_str());
        }
        assert!(LuxScenePriority::Background.order_key() < LuxScenePriority::World.order_key());
        assert!(LuxScenePriority::World.order_key() < LuxScenePriority::Hud.order_key());
        assert!(LuxScenePriority::Hud.order_key() < LuxScenePriority::DebugOverlay.order_key());
    }

    #[test]
    fn dirty_region_predicates() {
        // Pass 2: LuxDirtyRegion is now the world-space type
        // from `crate::dirty`. The pixel-rect Pass 1 surface
        // is gone; the renderer projects bounds to pixel-space
        // when it needs scissor rects.
        use crate::aabb::LuxAabb;
        use crate::dirty::LuxDirtyFlags;
        let r = LuxDirtyRegion::new(
            LuxSceneId::PROOF_SCENE,
            LuxAabb::new([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]),
            LuxDirtyFlags::TRANSFORM,
            128,
            42,
        );
        assert_eq!(r.scene_id, LuxSceneId::PROOF_SCENE);
        assert!(r.flags.contains(LuxDirtyFlags::TRANSFORM));
        assert!(r.invalidates_shadow_maps());
        let whole = LuxDirtyRegion::whole_scene(LuxSceneId::PROOF_SCENE, LuxDirtyFlags::COLOR);
        assert!(whole.bounds.is_whole_world());
        // COLOR-only does NOT invalidate shadow maps
        // (Pass 2 acceptance).
        assert!(!whole.invalidates_shadow_maps());
    }

    #[test]
    fn scene_plan_minimal_and_emitted_work_predicates() {
        let scene = LuxSceneFramePlan::cold_default(LuxSceneId::PROOF_SCENE);
        assert!(scene.is_minimal());
        assert!(!scene.emitted_work());
        assert!(!scene.visible);
    }

    #[test]
    fn resource_intent_kind_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for kind in LuxResourceIntentKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn resource_intent_kind_tag_matches_variant() {
        let probes: Vec<LuxResourceIntent> = vec![
            LuxResourceIntent::LightBuffer {
                stable_id: "lights",
                max_light_count: 64,
                bytes_per_light: 32,
            },
            LuxResourceIntent::LightIndexBuffer {
                stable_id: "light_index",
                max_cluster_count: 16,
                max_lights_per_cluster: 4,
            },
            LuxResourceIntent::ClusterGrid {
                stable_id: "cluster_grid",
                clusters_x: 16,
                clusters_y: 8,
                clusters_z: 24,
            },
            LuxResourceIntent::ReservoirBuffer {
                stable_id: "reservoir",
                sample_count: 4,
            },
            LuxResourceIntent::ShadowRequestBuffer {
                stable_id: "shadow_requests",
                max_request_count: 256,
            },
            LuxResourceIntent::ShadowAtlas {
                stable_id: "shadow_atlas",
                atlas_extent: 256,
                slot_count: 4,
            },
            LuxResourceIntent::VirtualShadowPageTable {
                stable_id: "vshadow_page_table",
                page_count: 1024,
            },
            LuxResourceIntent::VoxelShadowPageTable {
                stable_id: "voxel_shadow_page_table",
                page_count: 1024,
            },
            LuxResourceIntent::VoxelTerrainSdfPool {
                stable_id: "voxel_terrain_sdf_pool",
                page_count: 512,
            },
            LuxResourceIntent::SurfaceCache {
                stable_id: "surface_cache",
                surface_count: 256,
            },
            LuxResourceIntent::RadianceCache {
                stable_id: "radiance_cache",
                voxel_count: 1_000_000,
            },
            LuxResourceIntent::VoxelTerrainRadianceClipmap {
                stable_id: "voxel_terrain_radiance_clipmap",
                voxel_count: 1_000_000,
            },
            LuxResourceIntent::VoxelCanopyOpacityClipmap {
                stable_id: "voxel_canopy_opacity_clipmap",
                voxel_count: 250_000,
            },
            LuxResourceIntent::StormExtinctionClipmap {
                stable_id: "storm_extinction_clipmap",
                voxel_count: 250_000,
            },
            LuxResourceIntent::ProbeCache {
                stable_id: "probe_cache",
                probe_count: 4096,
            },
            LuxResourceIntent::ReflectionTraceBuffer {
                stable_id: "reflection_trace",
                ray_count: 1024,
            },
            LuxResourceIntent::DenoiseHistory {
                stable_id: "denoise_history",
                width: 1920,
                height: 1080,
            },
            LuxResourceIntent::VolumetricFroxelDensity {
                stable_id: "froxel_density",
                width: 160,
                height: 90,
                depth: 64,
            },
            LuxResourceIntent::VolumetricFroxelScattering {
                stable_id: "froxel_scattering",
                width: 160,
                height: 90,
                depth: 64,
            },
            LuxResourceIntent::VolumetricHistory {
                stable_id: "froxel_history",
                width: 160,
                height: 90,
                depth: 64,
            },
            LuxResourceIntent::IntegratedFog {
                stable_id: "integrated_fog",
                width: 1920,
                height: 1080,
            },
            LuxResourceIntent::LuxDebugBuffer {
                stable_id: "lux_debug",
                byte_size: 4096,
            },
        ];
        assert_eq!(probes.len(), LUX_RESOURCE_INTENT_KIND_COUNT);
        let kinds: Vec<LuxResourceIntentKind> = probes.iter().map(|r| r.kind()).collect();
        assert_eq!(kinds, LuxResourceIntentKind::ALL.to_vec());
        // Stable id accessor smoke test.
        for p in &probes {
            let id = p.stable_id();
            assert!(!id.is_empty());
        }
    }

    #[test]
    fn cold_default_frame_plan_is_noop_baseline() {
        let plan = LuxFramePlan::cold_default();
        assert!(plan.is_noop_baseline());
        assert!(plan.is_minimal());
        assert_eq!(plan.aggregate_pass_count(), 0);
        assert_eq!(plan.aggregate_resource_count(), 0);
        assert!(plan.diagnostics.minimal_plan_no_scene_work);
    }

    #[test]
    fn frame_plan_aggregate_counts_walk_every_scene_plan() {
        use crate::pass::{LuxPassCommon, LuxPassRequest, UploadLightBuffersPass};
        let mut plan = LuxFramePlan::cold_default();
        let mut scene = LuxSceneFramePlan::cold_default(LuxSceneId::PROOF_SCENE);
        scene
            .passes
            .push(LuxPassRequest::UploadLightBuffers(UploadLightBuffersPass {
                common: LuxPassCommon::new("fun_lux.upload", LuxQualityTier::Medium),
                light_count: 4,
                bytes_per_light: 32,
            }));
        scene.resources.push(LuxResourceIntent::LightBuffer {
            stable_id: "fun_lux.lights",
            max_light_count: 32,
            bytes_per_light: 32,
        });
        plan.scene_plans.push(scene);
        assert_eq!(plan.aggregate_pass_count(), 1);
        assert_eq!(plan.aggregate_resource_count(), 1);
        assert!(!plan.is_minimal());
    }

    #[test]
    fn backend_contract_product_default_holds() {
        let contract = LuxBackendContract::PRODUCT_DEFAULT;
        assert!(contract.contract_holds());
        assert!(contract.fun_renderer_is_only_production_executor);
        assert!(contract.fun_lux_owns_lighting_policy);
        assert!(contract.fun_lux_emits_backend_neutral_plans);
        assert!(contract.legacy_lighting_paths_are_invalid_for_production);
        assert!(contract.noop_lux_core_is_non_production_only);
        assert!(contract.fun_renderer_depends_on_fun_lux);
        assert!(contract.fun_lux_must_not_depend_on_fun_renderer);
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
        assert!(!contract.is_forbidden_import("fun_ecs"));
        assert!(!contract.is_forbidden_import("fun_scene"));
    }

    /// Pass 0 source-of-truth test, carried forward to Pass 1.
    /// Reads every file under `fun-lux/src/` and asserts that
    /// no public module imports any forbidden crate.
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
                        bad.push((path.display().to_string(), trimmed.to_owned()));
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

    /// Pass 1: the cold-default plan is the `NoopLuxCore`
    /// baseline. Production lighting routes must build a plan
    /// with at least one scene plan + passes.
    #[test]
    fn cold_default_plan_classifies_as_noop_baseline() {
        let cold = LuxFramePlan::cold_default();
        assert!(
            cold.is_noop_baseline(),
            "cold_default plan must classify as noop baseline; production routes must populate scene_plans",
        );
    }
}
