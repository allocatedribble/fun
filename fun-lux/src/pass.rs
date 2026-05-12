//! Typed lighting pass requests.
//!
//! `LuxPassRequest` is the typed 24-variant enum a `fun-lux`
//! frame planner emits per scene. The renderer reads each
//! variant and dispatches the matching live pipeline. Every
//! variant carries:
//!
//! - a stable id (typed `&'static str`),
//! - a typed quality tier (`LuxQualityTier`),
//! - typed dependencies (other pass stable ids the renderer
//!   must order this pass after),
//! - typed reads + writes (`LuxResourceIntent` records).
//!
//! No variant names a `wgpu` resource directly — the renderer
//! translates the typed intent into the concrete dispatch.

use crate::frame_plan::LuxResourceIntent;
use crate::quality::LuxQualityTier;

pub const FUN_LUX_PASS_SCHEMA_VERSION: u16 = 1;
pub const LUX_PASS_KIND_COUNT: usize = 24;

/// Typed upstream dependency edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LuxPassDependency {
    pub schema_version: u16,
    pub upstream_stable_id: &'static str,
}

impl LuxPassDependency {
    #[must_use]
    pub const fn new(upstream_stable_id: &'static str) -> Self {
        Self {
            schema_version: FUN_LUX_PASS_SCHEMA_VERSION,
            upstream_stable_id,
        }
    }
}

/// Shared metadata every pass variant carries. The renderer
/// reads the common fields via [`LuxPassRequest::common`] to
/// order dispatches + apply quality scaling without unpacking
/// the variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuxPassCommon {
    pub schema_version: u16,
    pub stable_id: &'static str,
    pub quality_tier: LuxQualityTier,
    pub depends_on: Vec<LuxPassDependency>,
    pub reads: Vec<LuxResourceIntent>,
    pub writes: Vec<LuxResourceIntent>,
}

impl LuxPassCommon {
    #[must_use]
    pub fn new(stable_id: &'static str, quality_tier: LuxQualityTier) -> Self {
        Self {
            schema_version: FUN_LUX_PASS_SCHEMA_VERSION,
            stable_id,
            quality_tier,
            depends_on: Vec::new(),
            reads: Vec::new(),
            writes: Vec::new(),
        }
    }

    #[must_use]
    pub fn depends_on(mut self, upstream_stable_id: &'static str) -> Self {
        self.depends_on
            .push(LuxPassDependency::new(upstream_stable_id));
        self
    }

    #[must_use]
    pub fn reads(mut self, intent: LuxResourceIntent) -> Self {
        self.reads.push(intent);
        self
    }

    #[must_use]
    pub fn writes(mut self, intent: LuxResourceIntent) -> Self {
        self.writes.push(intent);
        self
    }
}

/// Typed pass kind for taxonomy access (one tag per variant
/// of [`LuxPassRequest`]). The renderer / observability layer
/// uses this tag to histogram passes per frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxPassKind {
    UploadLightBuffers,
    ClusterLights,
    SelectReservoirs,
    BuildShadowRequests,
    RenderVirtualShadowPages,
    FilterVirtualShadows,
    VoxelShadowDemandMark,
    VoxelShadowPageBuild,
    VoxelSdfDistantShadowResolve,
    VoxelRadianceClipmapUpdate,
    VoxelCanopyTransmittanceInject,
    VoxelTerrainAoResolve,
    StormExtinctionInject,
    DirectLighting,
    GiTrace,
    GiCacheUpdate,
    ReflectionTrace,
    Denoise,
    VolumetricFogInject,
    VolumetricLightInject,
    VolumetricTemporalReproject,
    VolumetricIntegrate,
    VolumetricComposite,
    LuxDebugOverlay,
}

impl LuxPassKind {
    pub const ALL: [Self; LUX_PASS_KIND_COUNT] = [
        Self::UploadLightBuffers,
        Self::ClusterLights,
        Self::SelectReservoirs,
        Self::BuildShadowRequests,
        Self::RenderVirtualShadowPages,
        Self::FilterVirtualShadows,
        Self::VoxelShadowDemandMark,
        Self::VoxelShadowPageBuild,
        Self::VoxelSdfDistantShadowResolve,
        Self::VoxelRadianceClipmapUpdate,
        Self::VoxelCanopyTransmittanceInject,
        Self::VoxelTerrainAoResolve,
        Self::StormExtinctionInject,
        Self::DirectLighting,
        Self::GiTrace,
        Self::GiCacheUpdate,
        Self::ReflectionTrace,
        Self::Denoise,
        Self::VolumetricFogInject,
        Self::VolumetricLightInject,
        Self::VolumetricTemporalReproject,
        Self::VolumetricIntegrate,
        Self::VolumetricComposite,
        Self::LuxDebugOverlay,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UploadLightBuffers => "upload_light_buffers",
            Self::ClusterLights => "cluster_lights",
            Self::SelectReservoirs => "select_reservoirs",
            Self::BuildShadowRequests => "build_shadow_requests",
            Self::RenderVirtualShadowPages => "render_virtual_shadow_pages",
            Self::FilterVirtualShadows => "filter_virtual_shadows",
            Self::VoxelShadowDemandMark => "voxel_shadow_demand_mark",
            Self::VoxelShadowPageBuild => "voxel_shadow_page_build",
            Self::VoxelSdfDistantShadowResolve => "voxel_sdf_distant_shadow_resolve",
            Self::VoxelRadianceClipmapUpdate => "voxel_radiance_clipmap_update",
            Self::VoxelCanopyTransmittanceInject => "voxel_canopy_transmittance_inject",
            Self::VoxelTerrainAoResolve => "voxel_terrain_ao_resolve",
            Self::StormExtinctionInject => "storm_extinction_inject",
            Self::DirectLighting => "direct_lighting",
            Self::GiTrace => "gi_trace",
            Self::GiCacheUpdate => "gi_cache_update",
            Self::ReflectionTrace => "reflection_trace",
            Self::Denoise => "denoise",
            Self::VolumetricFogInject => "volumetric_fog_inject",
            Self::VolumetricLightInject => "volumetric_light_inject",
            Self::VolumetricTemporalReproject => "volumetric_temporal_reproject",
            Self::VolumetricIntegrate => "volumetric_integrate",
            Self::VolumetricComposite => "volumetric_composite",
            Self::LuxDebugOverlay => "lux_debug_overlay",
        }
    }
}

// ============================================================================
// Section — Per-variant payloads
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadLightBuffersPass {
    pub common: LuxPassCommon,
    pub light_count: u32,
    pub bytes_per_light: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClusterLightsPass {
    pub common: LuxPassCommon,
    pub clusters_x: u32,
    pub clusters_y: u32,
    pub clusters_z: u32,
    pub max_lights_per_cluster: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectReservoirsPass {
    pub common: LuxPassCommon,
    pub reservoir_samples_per_pixel: u32,
    pub temporal_reuse_enabled: bool,
    pub spatial_reuse_enabled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuildShadowRequestsPass {
    pub common: LuxPassCommon,
    pub max_shadow_request_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderVirtualShadowPagesPass {
    pub common: LuxPassCommon,
    pub max_pages_per_frame: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterVirtualShadowsPass {
    pub common: LuxPassCommon,
    pub spatial_filter_kernel_radius: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelShadowDemandMarkPass {
    pub common: LuxPassCommon,
    pub dirty_row_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelShadowPageBuildPass {
    pub common: LuxPassCommon,
    pub max_pages_per_frame: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelSdfDistantShadowResolvePass {
    pub common: LuxPassCommon,
    pub sdf_page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelRadianceClipmapUpdatePass {
    pub common: LuxPassCommon,
    pub dirty_row_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelCanopyTransmittanceInjectPass {
    pub common: LuxPassCommon,
    pub opacity_page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelTerrainAoResolvePass {
    pub common: LuxPassCommon,
    pub sdf_page_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StormExtinctionInjectPass {
    pub common: LuxPassCommon,
    pub dirty_row_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirectLightingPass {
    pub common: LuxPassCommon,
    pub uses_reservoirs: bool,
    pub samples_virtual_shadow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiTracePass {
    pub common: LuxPassCommon,
    pub max_bounces: u8,
    pub uses_radiance_cache: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GiCacheUpdatePass {
    pub common: LuxPassCommon,
    pub voxel_resolution: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReflectionTracePass {
    pub common: LuxPassCommon,
    pub ray_budget_per_pixel: u8,
    pub uses_surface_cache: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DenoisePass {
    pub common: LuxPassCommon,
    pub temporal_history_frames: u8,
    pub spatial_kernel_radius: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumetricFogInjectPass {
    pub common: LuxPassCommon,
    pub froxel_width: u32,
    pub froxel_height: u32,
    pub froxel_depth: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumetricLightInjectPass {
    pub common: LuxPassCommon,
    pub light_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumetricTemporalReprojectPass {
    pub common: LuxPassCommon,
    pub reproject_alpha_q8: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumetricIntegratePass {
    pub common: LuxPassCommon,
    pub multi_tap: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumetricCompositePass {
    pub common: LuxPassCommon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LuxDebugOverlayPass {
    pub common: LuxPassCommon,
    pub overlay_kind: LuxDebugOverlayKind,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LuxDebugOverlayKind {
    #[default]
    LightsPerCluster,
    ShadowAtlasResidency,
    VirtualShadowPageActivity,
    GiProbeOccupancy,
    ReflectionTraceCost,
    VolumetricDensity,
    DenoiseHistoryConfidence,
}

impl LuxDebugOverlayKind {
    pub const ALL: [Self; 7] = [
        Self::LightsPerCluster,
        Self::ShadowAtlasResidency,
        Self::VirtualShadowPageActivity,
        Self::GiProbeOccupancy,
        Self::ReflectionTraceCost,
        Self::VolumetricDensity,
        Self::DenoiseHistoryConfidence,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LightsPerCluster => "lights_per_cluster",
            Self::ShadowAtlasResidency => "shadow_atlas_residency",
            Self::VirtualShadowPageActivity => "virtual_shadow_page_activity",
            Self::GiProbeOccupancy => "gi_probe_occupancy",
            Self::ReflectionTraceCost => "reflection_trace_cost",
            Self::VolumetricDensity => "volumetric_density",
            Self::DenoiseHistoryConfidence => "denoise_history_confidence",
        }
    }
}

// ============================================================================
// Section — Top-level enum
// ============================================================================

/// Typed lighting pass request — one variant per logical
/// renderer dispatch the lighting policy wants the renderer
/// to execute this frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LuxPassRequest {
    UploadLightBuffers(UploadLightBuffersPass),
    ClusterLights(ClusterLightsPass),
    SelectReservoirs(SelectReservoirsPass),
    BuildShadowRequests(BuildShadowRequestsPass),
    RenderVirtualShadowPages(RenderVirtualShadowPagesPass),
    FilterVirtualShadows(FilterVirtualShadowsPass),
    VoxelShadowDemandMark(VoxelShadowDemandMarkPass),
    VoxelShadowPageBuild(VoxelShadowPageBuildPass),
    VoxelSdfDistantShadowResolve(VoxelSdfDistantShadowResolvePass),
    VoxelRadianceClipmapUpdate(VoxelRadianceClipmapUpdatePass),
    VoxelCanopyTransmittanceInject(VoxelCanopyTransmittanceInjectPass),
    VoxelTerrainAoResolve(VoxelTerrainAoResolvePass),
    StormExtinctionInject(StormExtinctionInjectPass),
    DirectLighting(DirectLightingPass),
    GiTrace(GiTracePass),
    GiCacheUpdate(GiCacheUpdatePass),
    ReflectionTrace(ReflectionTracePass),
    Denoise(DenoisePass),
    VolumetricFogInject(VolumetricFogInjectPass),
    VolumetricLightInject(VolumetricLightInjectPass),
    VolumetricTemporalReproject(VolumetricTemporalReprojectPass),
    VolumetricIntegrate(VolumetricIntegratePass),
    VolumetricComposite(VolumetricCompositePass),
    LuxDebugOverlay(LuxDebugOverlayPass),
}

impl LuxPassRequest {
    /// Typed accessor returning the shared metadata.
    #[must_use]
    pub fn common(&self) -> &LuxPassCommon {
        match self {
            Self::UploadLightBuffers(p) => &p.common,
            Self::ClusterLights(p) => &p.common,
            Self::SelectReservoirs(p) => &p.common,
            Self::BuildShadowRequests(p) => &p.common,
            Self::RenderVirtualShadowPages(p) => &p.common,
            Self::FilterVirtualShadows(p) => &p.common,
            Self::VoxelShadowDemandMark(p) => &p.common,
            Self::VoxelShadowPageBuild(p) => &p.common,
            Self::VoxelSdfDistantShadowResolve(p) => &p.common,
            Self::VoxelRadianceClipmapUpdate(p) => &p.common,
            Self::VoxelCanopyTransmittanceInject(p) => &p.common,
            Self::VoxelTerrainAoResolve(p) => &p.common,
            Self::StormExtinctionInject(p) => &p.common,
            Self::DirectLighting(p) => &p.common,
            Self::GiTrace(p) => &p.common,
            Self::GiCacheUpdate(p) => &p.common,
            Self::ReflectionTrace(p) => &p.common,
            Self::Denoise(p) => &p.common,
            Self::VolumetricFogInject(p) => &p.common,
            Self::VolumetricLightInject(p) => &p.common,
            Self::VolumetricTemporalReproject(p) => &p.common,
            Self::VolumetricIntegrate(p) => &p.common,
            Self::VolumetricComposite(p) => &p.common,
            Self::LuxDebugOverlay(p) => &p.common,
        }
    }

    /// Typed pass-kind tag (for histograms / observability).
    #[must_use]
    pub const fn kind(&self) -> LuxPassKind {
        match self {
            Self::UploadLightBuffers(_) => LuxPassKind::UploadLightBuffers,
            Self::ClusterLights(_) => LuxPassKind::ClusterLights,
            Self::SelectReservoirs(_) => LuxPassKind::SelectReservoirs,
            Self::BuildShadowRequests(_) => LuxPassKind::BuildShadowRequests,
            Self::RenderVirtualShadowPages(_) => LuxPassKind::RenderVirtualShadowPages,
            Self::FilterVirtualShadows(_) => LuxPassKind::FilterVirtualShadows,
            Self::VoxelShadowDemandMark(_) => LuxPassKind::VoxelShadowDemandMark,
            Self::VoxelShadowPageBuild(_) => LuxPassKind::VoxelShadowPageBuild,
            Self::VoxelSdfDistantShadowResolve(_) => LuxPassKind::VoxelSdfDistantShadowResolve,
            Self::VoxelRadianceClipmapUpdate(_) => LuxPassKind::VoxelRadianceClipmapUpdate,
            Self::VoxelCanopyTransmittanceInject(_) => LuxPassKind::VoxelCanopyTransmittanceInject,
            Self::VoxelTerrainAoResolve(_) => LuxPassKind::VoxelTerrainAoResolve,
            Self::StormExtinctionInject(_) => LuxPassKind::StormExtinctionInject,
            Self::DirectLighting(_) => LuxPassKind::DirectLighting,
            Self::GiTrace(_) => LuxPassKind::GiTrace,
            Self::GiCacheUpdate(_) => LuxPassKind::GiCacheUpdate,
            Self::ReflectionTrace(_) => LuxPassKind::ReflectionTrace,
            Self::Denoise(_) => LuxPassKind::Denoise,
            Self::VolumetricFogInject(_) => LuxPassKind::VolumetricFogInject,
            Self::VolumetricLightInject(_) => LuxPassKind::VolumetricLightInject,
            Self::VolumetricTemporalReproject(_) => LuxPassKind::VolumetricTemporalReproject,
            Self::VolumetricIntegrate(_) => LuxPassKind::VolumetricIntegrate,
            Self::VolumetricComposite(_) => LuxPassKind::VolumetricComposite,
            Self::LuxDebugOverlay(_) => LuxPassKind::LuxDebugOverlay,
        }
    }

    /// Typed convenience accessor.
    #[must_use]
    pub fn stable_id(&self) -> &'static str {
        self.common().stable_id
    }

    /// Typed convenience accessor.
    #[must_use]
    pub fn quality_tier(&self) -> LuxQualityTier {
        self.common().quality_tier
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pass_kind_count_matches_taxonomy() {
        assert_eq!(LUX_PASS_KIND_COUNT, 24);
        assert_eq!(LuxPassKind::ALL.len(), LUX_PASS_KIND_COUNT);
    }

    #[test]
    fn pass_kind_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for kind in LuxPassKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn debug_overlay_kind_taxonomy_strings_are_unique() {
        let mut seen = hashbrown::HashSet::new();
        for kind in LuxDebugOverlayKind::ALL {
            assert!(seen.insert(kind.as_str()), "duplicate: {}", kind.as_str());
        }
    }

    #[test]
    fn pass_common_builder_chains_deps_reads_writes() {
        use crate::frame_plan::LuxResourceIntent;
        let common = LuxPassCommon::new("fun_lux.demo.cluster_lights", LuxQualityTier::Medium)
            .depends_on("fun_lux.demo.upload_light_buffers")
            .reads(LuxResourceIntent::LightBuffer {
                stable_id: "fun_lux.lights.demo",
                max_light_count: 1024,
                bytes_per_light: 32,
            })
            .writes(LuxResourceIntent::ClusterGrid {
                stable_id: "fun_lux.cluster_grid.demo",
                clusters_x: 16,
                clusters_y: 8,
                clusters_z: 24,
            });
        assert_eq!(common.depends_on.len(), 1);
        assert_eq!(common.reads.len(), 1);
        assert_eq!(common.writes.len(), 1);
        assert_eq!(common.quality_tier, LuxQualityTier::Medium);
    }

    #[test]
    fn every_pass_request_variant_reports_its_kind_and_common() {
        let common = LuxPassCommon::new("fun_lux.demo.pass", LuxQualityTier::High);
        let requests = [
            LuxPassRequest::UploadLightBuffers(UploadLightBuffersPass {
                common: common.clone(),
                light_count: 16,
                bytes_per_light: 32,
            }),
            LuxPassRequest::ClusterLights(ClusterLightsPass {
                common: common.clone(),
                clusters_x: 16,
                clusters_y: 8,
                clusters_z: 24,
                max_lights_per_cluster: 32,
            }),
            LuxPassRequest::SelectReservoirs(SelectReservoirsPass {
                common: common.clone(),
                reservoir_samples_per_pixel: 4,
                temporal_reuse_enabled: true,
                spatial_reuse_enabled: true,
            }),
            LuxPassRequest::BuildShadowRequests(BuildShadowRequestsPass {
                common: common.clone(),
                max_shadow_request_count: 256,
            }),
            LuxPassRequest::RenderVirtualShadowPages(RenderVirtualShadowPagesPass {
                common: common.clone(),
                max_pages_per_frame: 64,
            }),
            LuxPassRequest::FilterVirtualShadows(FilterVirtualShadowsPass {
                common: common.clone(),
                spatial_filter_kernel_radius: 2,
            }),
            LuxPassRequest::VoxelShadowDemandMark(VoxelShadowDemandMarkPass {
                common: common.clone(),
                dirty_row_count: 4,
            }),
            LuxPassRequest::VoxelShadowPageBuild(VoxelShadowPageBuildPass {
                common: common.clone(),
                max_pages_per_frame: 16,
            }),
            LuxPassRequest::VoxelSdfDistantShadowResolve(VoxelSdfDistantShadowResolvePass {
                common: common.clone(),
                sdf_page_count: 8,
            }),
            LuxPassRequest::VoxelRadianceClipmapUpdate(VoxelRadianceClipmapUpdatePass {
                common: common.clone(),
                dirty_row_count: 6,
            }),
            LuxPassRequest::VoxelCanopyTransmittanceInject(VoxelCanopyTransmittanceInjectPass {
                common: common.clone(),
                opacity_page_count: 5,
            }),
            LuxPassRequest::VoxelTerrainAoResolve(VoxelTerrainAoResolvePass {
                common: common.clone(),
                sdf_page_count: 8,
            }),
            LuxPassRequest::StormExtinctionInject(StormExtinctionInjectPass {
                common: common.clone(),
                dirty_row_count: 3,
            }),
            LuxPassRequest::DirectLighting(DirectLightingPass {
                common: common.clone(),
                uses_reservoirs: true,
                samples_virtual_shadow: true,
            }),
            LuxPassRequest::GiTrace(GiTracePass {
                common: common.clone(),
                max_bounces: 2,
                uses_radiance_cache: true,
            }),
            LuxPassRequest::GiCacheUpdate(GiCacheUpdatePass {
                common: common.clone(),
                voxel_resolution: 128,
            }),
            LuxPassRequest::ReflectionTrace(ReflectionTracePass {
                common: common.clone(),
                ray_budget_per_pixel: 1,
                uses_surface_cache: true,
            }),
            LuxPassRequest::Denoise(DenoisePass {
                common: common.clone(),
                temporal_history_frames: 8,
                spatial_kernel_radius: 2,
            }),
            LuxPassRequest::VolumetricFogInject(VolumetricFogInjectPass {
                common: common.clone(),
                froxel_width: 160,
                froxel_height: 90,
                froxel_depth: 64,
            }),
            LuxPassRequest::VolumetricLightInject(VolumetricLightInjectPass {
                common: common.clone(),
                light_count: 16,
            }),
            LuxPassRequest::VolumetricTemporalReproject(VolumetricTemporalReprojectPass {
                common: common.clone(),
                reproject_alpha_q8: 200,
            }),
            LuxPassRequest::VolumetricIntegrate(VolumetricIntegratePass {
                common: common.clone(),
                multi_tap: false,
            }),
            LuxPassRequest::VolumetricComposite(VolumetricCompositePass {
                common: common.clone(),
            }),
            LuxPassRequest::LuxDebugOverlay(LuxDebugOverlayPass {
                common,
                overlay_kind: LuxDebugOverlayKind::LightsPerCluster,
            }),
        ];
        assert_eq!(requests.len(), LUX_PASS_KIND_COUNT);
        // Every variant must report a distinct kind tag, in
        // the same order as the typed `LuxPassKind::ALL`.
        let kinds: Vec<LuxPassKind> = requests.iter().map(|r| r.kind()).collect();
        assert_eq!(kinds, LuxPassKind::ALL.to_vec());
        // Every variant exposes the shared common record.
        for r in &requests {
            assert_eq!(r.stable_id(), "fun_lux.demo.pass");
            assert_eq!(r.quality_tier(), LuxQualityTier::High);
        }
    }
}
