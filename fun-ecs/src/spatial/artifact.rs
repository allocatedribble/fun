use crate::Resource;
use fun_scheduler_types::{
    EcsChunkKey, EcsSpatialDomainKind, EcsVirtualResourceKey, ScheduleDeadline, ScheduleDomain,
    ScheduleLane, WorkRequiredness, WorkSplitHint, WorkWaitToken,
};

use crate::{
    DenseSlotMap, ECS_SPATIAL_MAX_DERIVED_ARTIFACTS, ECS_SPATIAL_MAX_HANDOFF_ROWS, EcsAabbF32,
    EcsDecodedPagePayloadKind, EcsDecodedPageRecord, EcsDerivedArtifactId, EcsHandoffQueueId,
    EcsSpatialCommand, EcsSpatialCommandBuffer, EcsSpatialPageKey, EcsSpatialValidationError,
    FixedStepId, ProceduralGeneratedPageClass, TerrainMaterialId, TerrainMaterialPaletteId,
    VOXEL_BRICK_EDGE_CELLS, VOXEL_CLUSTER_SUMMARIES_PER_BRICK, VoxelBrickPayload,
    VoxelCollisionPolicy, VoxelMaterialPalette, VoxelOccupancyStorageKind,
    frame_graph::EcsCrossDomainWaitTokenKind,
};

pub const ECS_DERIVED_ARTIFACT_BUILD_SYSTEM_COUNT: usize = 24;
pub const ECS_PROCEDURAL_TERRAIN_REQUIRED_ARTIFACT_COUNT: usize = 4;
pub const ECS_PROCEDURAL_TERRAIN_OPTIONAL_ARTIFACT_COUNT: usize = 3;

pub const ECS_PROCEDURAL_TERRAIN_REQUIRED_ARTIFACT_KINDS: [EcsDerivedArtifactKind;
    ECS_PROCEDURAL_TERRAIN_REQUIRED_ARTIFACT_COUNT] = [
    EcsDerivedArtifactKind::TerrainCoarseProxy,
    EcsDerivedArtifactKind::TerrainSurfacePackets,
    EcsDerivedArtifactKind::TerrainMaterialPage,
    EcsDerivedArtifactKind::LoadAnimationRecord,
];

pub const ECS_PROCEDURAL_TERRAIN_OPTIONAL_ARTIFACT_KINDS: [EcsDerivedArtifactKind;
    ECS_PROCEDURAL_TERRAIN_OPTIONAL_ARTIFACT_COUNT] = [
    EcsDerivedArtifactKind::TerrainSdf,
    EcsDerivedArtifactKind::PhysicsCookRequests,
    EcsDerivedArtifactKind::VirtualShadowInvalidation,
];

pub const ECS_PROCEDURAL_TERRAIN_REQUIRED_BUILD_SYSTEMS: [EcsDerivedArtifactBuildSystem;
    ECS_PROCEDURAL_TERRAIN_REQUIRED_ARTIFACT_COUNT] = [
    EcsDerivedArtifactBuildSystem::BuildTerrainCoarseProxy,
    EcsDerivedArtifactBuildSystem::BuildTerrainSurfacePackets,
    EcsDerivedArtifactBuildSystem::BuildTerrainMaterialPage,
    EcsDerivedArtifactBuildSystem::BuildLoadAnimationRecord,
];

pub const ECS_DERIVED_ARTIFACT_BUILD_SYSTEMS: [EcsDerivedArtifactBuildSystem;
    ECS_DERIVED_ARTIFACT_BUILD_SYSTEM_COUNT] = [
    EcsDerivedArtifactBuildSystem::BuildTerrainSurfacePackets,
    EcsDerivedArtifactBuildSystem::BuildTerrainCoarseProxy,
    EcsDerivedArtifactBuildSystem::BuildTerrainSdf,
    EcsDerivedArtifactBuildSystem::BuildTerrainMaterialPage,
    EcsDerivedArtifactBuildSystem::BuildLoadAnimationRecord,
    EcsDerivedArtifactBuildSystem::BuildFoliageSeeds,
    EcsDerivedArtifactBuildSystem::BuildFoliageClusters,
    EcsDerivedArtifactBuildSystem::BuildCanopyOpacity,
    EcsDerivedArtifactBuildSystem::BuildShadowInvalidationRows,
    EcsDerivedArtifactBuildSystem::BuildRadianceUpdateRows,
    EcsDerivedArtifactBuildSystem::BuildPhysicsCookRequests,
    EcsDerivedArtifactBuildSystem::BuildCollisionSdfProxy,
    EcsDerivedArtifactBuildSystem::BuildVoxelEditDeltaRows,
    EcsDerivedArtifactBuildSystem::BuildNetworkRelevanceRows,
    EcsDerivedArtifactBuildSystem::BuildFoliageTrunkBranchVirtualGeometry,
    EcsDerivedArtifactBuildSystem::BuildFoliageGrassBrushInstanceClusters,
    EcsDerivedArtifactBuildSystem::BuildFoliageCollisionLargeObjectProxy,
    EcsDerivedArtifactBuildSystem::BuildFoliageCardsImpostors,
    EcsDerivedArtifactBuildSystem::BuildFoliageClusteredAnimation,
    EcsDerivedArtifactBuildSystem::BuildFoliageCanopyTransmittance,
    EcsDerivedArtifactBuildSystem::BuildFoliageBiomeTint,
    EcsDerivedArtifactBuildSystem::BuildFoliageHorizonImpostorField,
    EcsDerivedArtifactBuildSystem::BuildFoliageSdfOpacityShadows,
    EcsDerivedArtifactBuildSystem::BuildStormExtinctionDirtyRows,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsDerivedArtifactKind {
    TerrainSurfacePackets = 0,
    TerrainCoarseProxy = 1,
    TerrainSdf = 2,
    TerrainMaterialPage = 3,
    FoliageClusters = 4,
    CanopyOpacity = 5,
    VirtualShadowInvalidation = 6,
    RadianceCacheUpdate = 7,
    PhysicsCollisionProxy = 8,
    NavTile = 9,
    AudioOcclusionTile = 10,
    NetworkDeltaRows = 11,
    LoadAnimationRecord = 12,
    FoliageSeeds = 13,
    ShadowInvalidationRows = 14,
    RadianceUpdateRows = 15,
    PhysicsCookRequests = 16,
    CollisionSdfProxy = 17,
    VoxelEditDeltaRows = 18,
    NetworkRelevanceRows = 19,
    FoliageTrunkBranchVirtualGeometry = 20,
    FoliageGrassBrushInstanceClusters = 21,
    FoliageCollisionLargeObjectProxy = 22,
    FoliageCardsImpostors = 23,
    FoliageClusteredAnimation = 24,
    FoliageCanopyTransmittance = 25,
    FoliageBiomeTint = 26,
    FoliageHorizonImpostorField = 27,
    FoliageSdfOpacityShadows = 28,
    StormExtinctionDirtyRows = 29,
}

impl EcsDerivedArtifactKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TerrainSurfacePackets => "terrain_surface_packets",
            Self::TerrainCoarseProxy => "terrain_coarse_proxy",
            Self::TerrainSdf => "terrain_sdf",
            Self::TerrainMaterialPage => "terrain_material_page",
            Self::FoliageClusters => "foliage_clusters",
            Self::CanopyOpacity => "canopy_opacity",
            Self::VirtualShadowInvalidation => "virtual_shadow_invalidation",
            Self::RadianceCacheUpdate => "radiance_cache_update",
            Self::PhysicsCollisionProxy => "physics_collision_proxy",
            Self::NavTile => "nav_tile",
            Self::AudioOcclusionTile => "audio_occlusion_tile",
            Self::NetworkDeltaRows => "network_delta_rows",
            Self::LoadAnimationRecord => "load_animation_record",
            Self::FoliageSeeds => "foliage_seeds",
            Self::ShadowInvalidationRows => "shadow_invalidation_rows",
            Self::RadianceUpdateRows => "radiance_update_rows",
            Self::PhysicsCookRequests => "physics_cook_requests",
            Self::CollisionSdfProxy => "collision_sdf_proxy",
            Self::VoxelEditDeltaRows => "voxel_edit_delta_rows",
            Self::NetworkRelevanceRows => "network_relevance_rows",
            Self::FoliageTrunkBranchVirtualGeometry => "foliage_trunk_branch_virtual_geometry",
            Self::FoliageGrassBrushInstanceClusters => "foliage_grass_brush_instance_clusters",
            Self::FoliageCollisionLargeObjectProxy => "foliage_collision_large_object_proxy",
            Self::FoliageCardsImpostors => "foliage_cards_impostors",
            Self::FoliageClusteredAnimation => "foliage_clustered_animation",
            Self::FoliageCanopyTransmittance => "foliage_canopy_transmittance",
            Self::FoliageBiomeTint => "foliage_biome_tint",
            Self::FoliageHorizonImpostorField => "foliage_horizon_impostor_field",
            Self::FoliageSdfOpacityShadows => "foliage_sdf_opacity_shadows",
            Self::StormExtinctionDirtyRows => "storm_extinction_dirty_rows",
        }
    }

    #[must_use]
    pub const fn foliage_lod_band(self) -> Option<FoliageArtifactLodBand> {
        match self {
            Self::FoliageTrunkBranchVirtualGeometry
            | Self::FoliageGrassBrushInstanceClusters
            | Self::FoliageCollisionLargeObjectProxy => Some(FoliageArtifactLodBand::Near),
            Self::FoliageCardsImpostors | Self::FoliageClusteredAnimation | Self::CanopyOpacity => {
                Some(FoliageArtifactLodBand::Mid)
            }
            Self::FoliageCanopyTransmittance
            | Self::FoliageBiomeTint
            | Self::FoliageHorizonImpostorField
            | Self::FoliageSdfOpacityShadows => Some(FoliageArtifactLodBand::Far),
            Self::TerrainSurfacePackets
            | Self::TerrainCoarseProxy
            | Self::TerrainSdf
            | Self::TerrainMaterialPage
            | Self::FoliageClusters
            | Self::VirtualShadowInvalidation
            | Self::RadianceCacheUpdate
            | Self::PhysicsCollisionProxy
            | Self::NavTile
            | Self::AudioOcclusionTile
            | Self::NetworkDeltaRows
            | Self::LoadAnimationRecord
            | Self::FoliageSeeds
            | Self::ShadowInvalidationRows
            | Self::RadianceUpdateRows
            | Self::PhysicsCookRequests
            | Self::CollisionSdfProxy
            | Self::VoxelEditDeltaRows
            | Self::NetworkRelevanceRows
            | Self::StormExtinctionDirtyRows => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FoliageArtifactLodBand {
    #[default]
    Near = 0,
    Mid = 1,
    Far = 2,
}

impl FoliageArtifactLodBand {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Near => "near",
            Self::Mid => "mid",
            Self::Far => "far",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RendererArtifactHandoffKind {
    #[default]
    TerrainSurfacePackets = 0,
    TerrainCoarseProxy = 1,
    TerrainMaterialPage = 2,
    TerrainSdfPage = 3,
    FoliageClusters = 4,
    CanopyOpacity = 5,
    LoadAnimationRecords = 6,
    FoliageVirtualGeometry = 7,
    FoliageInstanceClusters = 8,
    FoliageCardsImpostors = 9,
    FoliageClusteredAnimation = 10,
    FoliageBiomeTint = 11,
    FoliageHorizonImpostorField = 12,
}

impl RendererArtifactHandoffKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TerrainSurfacePackets => "terrain_surface_packets",
            Self::TerrainCoarseProxy => "terrain_coarse_proxy",
            Self::TerrainMaterialPage => "terrain_material_page",
            Self::TerrainSdfPage => "terrain_sdf_page",
            Self::FoliageClusters => "foliage_clusters",
            Self::CanopyOpacity => "canopy_opacity",
            Self::LoadAnimationRecords => "load_animation_records",
            Self::FoliageVirtualGeometry => "foliage_virtual_geometry",
            Self::FoliageInstanceClusters => "foliage_instance_clusters",
            Self::FoliageCardsImpostors => "foliage_cards_impostors",
            Self::FoliageClusteredAnimation => "foliage_clustered_animation",
            Self::FoliageBiomeTint => "foliage_biome_tint",
            Self::FoliageHorizonImpostorField => "foliage_horizon_impostor_field",
        }
    }

    #[must_use]
    pub const fn from_derived(kind: EcsDerivedArtifactKind) -> Option<Self> {
        match kind {
            EcsDerivedArtifactKind::TerrainSurfacePackets => Some(Self::TerrainSurfacePackets),
            EcsDerivedArtifactKind::TerrainCoarseProxy => Some(Self::TerrainCoarseProxy),
            EcsDerivedArtifactKind::TerrainMaterialPage => Some(Self::TerrainMaterialPage),
            EcsDerivedArtifactKind::TerrainSdf => Some(Self::TerrainSdfPage),
            EcsDerivedArtifactKind::FoliageClusters => Some(Self::FoliageClusters),
            EcsDerivedArtifactKind::FoliageTrunkBranchVirtualGeometry => {
                Some(Self::FoliageVirtualGeometry)
            }
            EcsDerivedArtifactKind::FoliageGrassBrushInstanceClusters => {
                Some(Self::FoliageInstanceClusters)
            }
            EcsDerivedArtifactKind::FoliageCardsImpostors => Some(Self::FoliageCardsImpostors),
            EcsDerivedArtifactKind::FoliageClusteredAnimation => {
                Some(Self::FoliageClusteredAnimation)
            }
            EcsDerivedArtifactKind::FoliageBiomeTint => Some(Self::FoliageBiomeTint),
            EcsDerivedArtifactKind::FoliageHorizonImpostorField => {
                Some(Self::FoliageHorizonImpostorField)
            }
            EcsDerivedArtifactKind::CanopyOpacity => Some(Self::CanopyOpacity),
            EcsDerivedArtifactKind::LoadAnimationRecord => Some(Self::LoadAnimationRecords),
            EcsDerivedArtifactKind::VirtualShadowInvalidation
            | EcsDerivedArtifactKind::RadianceCacheUpdate
            | EcsDerivedArtifactKind::PhysicsCollisionProxy
            | EcsDerivedArtifactKind::FoliageCollisionLargeObjectProxy
            | EcsDerivedArtifactKind::NavTile
            | EcsDerivedArtifactKind::AudioOcclusionTile
            | EcsDerivedArtifactKind::NetworkDeltaRows
            | EcsDerivedArtifactKind::FoliageSeeds
            | EcsDerivedArtifactKind::ShadowInvalidationRows
            | EcsDerivedArtifactKind::RadianceUpdateRows
            | EcsDerivedArtifactKind::PhysicsCookRequests
            | EcsDerivedArtifactKind::CollisionSdfProxy
            | EcsDerivedArtifactKind::VoxelEditDeltaRows
            | EcsDerivedArtifactKind::NetworkRelevanceRows
            | EcsDerivedArtifactKind::FoliageCanopyTransmittance
            | EcsDerivedArtifactKind::FoliageSdfOpacityShadows
            | EcsDerivedArtifactKind::StormExtinctionDirtyRows => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RendererVisibilityHint {
    #[default]
    Unknown = 0,
    VisibleNear = 1,
    VisibleFar = 2,
    CoarseFallback = 3,
    Hidden = 4,
    DebugPinned = 5,
}

impl RendererVisibilityHint {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::VisibleNear => "visible_near",
            Self::VisibleFar => "visible_far",
            Self::CoarseFallback => "coarse_fallback",
            Self::Hidden => "hidden",
            Self::DebugPinned => "debug_pinned",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedRange {
    pub first: u32,
    pub count: u32,
}

impl PackedRange {
    pub const EMPTY: Self = Self { first: 0, count: 0 };

    #[must_use]
    pub const fn new(first: u32, count: u32) -> Self {
        Self { first, count }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PackedAabb {
    pub min_q: [i16; 3],
    pub max_q: [i16; 3],
}

impl PackedAabb {
    #[must_use]
    pub const fn new(min_q: [i16; 3], max_q: [i16; 3]) -> Self {
        Self { min_q, max_q }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoxelSurfacePacketArtifact {
    pub source_page: EcsSpatialPageKey,
    pub packet_count: u32,
    pub packet_range: PackedRange,
    pub local_bounds: PackedAabb,
    pub material_palette_id: TerrainMaterialPaletteId,
    pub load_animation_id: Option<EcsDerivedArtifactId>,
    pub source_epoch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralTerrainCoarseProxy {
    pub source_page: EcsSpatialPageKey,
    pub min_height_ft: i32,
    pub max_height_ft: i32,
    pub dominant_material: TerrainMaterialId,
    pub occupied_cluster_mask: u64,
    pub source_epoch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralTerrainSurfacePacket {
    pub source_page: EcsSpatialPageKey,
    pub local_bounds: PackedAabb,
    pub exposed_face_count: u32,
    pub packet_range: PackedRange,
    pub material_palette_id: TerrainMaterialPaletteId,
    pub source_epoch: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProceduralTerrainMaterialPage {
    pub source_page: EcsSpatialPageKey,
    pub palette: VoxelMaterialPalette,
    pub dominant_material_per_cluster: [TerrainMaterialId; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
    pub source_epoch: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProceduralTerrainLoadStage {
    #[default]
    PageRequested = 0,
    RecipeAcquired = 1,
    PageDecoded = 2,
    ArtifactReady = 3,
    RendererPublished = 4,
}

impl ProceduralTerrainLoadStage {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::PageRequested => "page_requested",
            Self::RecipeAcquired => "recipe_acquired",
            Self::PageDecoded => "page_decoded",
            Self::ArtifactReady => "artifact_ready",
            Self::RendererPublished => "renderer_published",
        }
    }

    #[must_use]
    pub const fn progress(self) -> f32 {
        match self {
            Self::PageRequested => 0.10,
            Self::RecipeAcquired => 0.25,
            Self::PageDecoded => 0.45,
            Self::ArtifactReady => 0.75,
            Self::RendererPublished => 1.00,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProceduralTerrainLoadAnimationRecord {
    pub source_page: EcsSpatialPageKey,
    pub stage: ProceduralTerrainLoadStage,
    pub progress: f32,
    pub source_epoch: u32,
}

impl ProceduralTerrainLoadAnimationRecord {
    #[must_use]
    pub fn new(
        source_page: EcsSpatialPageKey,
        stage: ProceduralTerrainLoadStage,
        source_epoch: u32,
    ) -> Self {
        Self {
            source_page,
            stage,
            progress: stage.progress(),
            source_epoch,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsDerivedArtifactBuildSystem {
    #[default]
    BuildTerrainSurfacePackets = 0,
    BuildTerrainCoarseProxy = 1,
    BuildTerrainSdf = 2,
    BuildTerrainMaterialPage = 3,
    BuildLoadAnimationRecord = 4,
    BuildFoliageSeeds = 5,
    BuildFoliageClusters = 6,
    BuildCanopyOpacity = 7,
    BuildShadowInvalidationRows = 8,
    BuildRadianceUpdateRows = 9,
    BuildPhysicsCookRequests = 10,
    BuildCollisionSdfProxy = 11,
    BuildVoxelEditDeltaRows = 12,
    BuildNetworkRelevanceRows = 13,
    BuildFoliageTrunkBranchVirtualGeometry = 14,
    BuildFoliageGrassBrushInstanceClusters = 15,
    BuildFoliageCollisionLargeObjectProxy = 16,
    BuildFoliageCardsImpostors = 17,
    BuildFoliageClusteredAnimation = 18,
    BuildFoliageCanopyTransmittance = 19,
    BuildFoliageBiomeTint = 20,
    BuildFoliageHorizonImpostorField = 21,
    BuildFoliageSdfOpacityShadows = 22,
    BuildStormExtinctionDirtyRows = 23,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralTerrainArtifactFlags {
    pub terrain_sdf: bool,
    pub physics_cook_requests: bool,
    pub virtual_shadow_invalidation: bool,
}

impl ProceduralTerrainArtifactFlags {
    pub const FIRST_VISUAL: Self = Self {
        terrain_sdf: false,
        physics_cook_requests: false,
        virtual_shadow_invalidation: false,
    };

    pub const ALL_OPTIONAL: Self = Self {
        terrain_sdf: true,
        physics_cook_requests: true,
        virtual_shadow_invalidation: true,
    };

    #[must_use]
    pub const fn allows_optional_kind(self, kind: EcsDerivedArtifactKind) -> bool {
        match kind {
            EcsDerivedArtifactKind::TerrainSdf => self.terrain_sdf,
            EcsDerivedArtifactKind::PhysicsCookRequests => self.physics_cook_requests,
            EcsDerivedArtifactKind::VirtualShadowInvalidation => self.virtual_shadow_invalidation,
            EcsDerivedArtifactKind::TerrainCoarseProxy
            | EcsDerivedArtifactKind::TerrainSurfacePackets
            | EcsDerivedArtifactKind::TerrainMaterialPage
            | EcsDerivedArtifactKind::LoadAnimationRecord => true,
            _ => false,
        }
    }
}

impl EcsDerivedArtifactBuildSystem {
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &ECS_DERIVED_ARTIFACT_BUILD_SYSTEMS
    }

    #[must_use]
    pub const fn prototype_required()
    -> &'static [Self; ECS_PROCEDURAL_TERRAIN_REQUIRED_ARTIFACT_COUNT] {
        &ECS_PROCEDURAL_TERRAIN_REQUIRED_BUILD_SYSTEMS
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::BuildTerrainSurfacePackets => "build_terrain_surface_packets",
            Self::BuildTerrainCoarseProxy => "build_terrain_coarse_proxy",
            Self::BuildTerrainSdf => "build_terrain_sdf",
            Self::BuildTerrainMaterialPage => "build_terrain_material_page",
            Self::BuildLoadAnimationRecord => "build_load_animation_record",
            Self::BuildFoliageSeeds => "build_foliage_seeds",
            Self::BuildFoliageClusters => "build_foliage_clusters",
            Self::BuildCanopyOpacity => "build_canopy_opacity",
            Self::BuildShadowInvalidationRows => "build_shadow_invalidation_rows",
            Self::BuildRadianceUpdateRows => "build_radiance_update_rows",
            Self::BuildPhysicsCookRequests => "build_physics_cook_requests",
            Self::BuildCollisionSdfProxy => "build_collision_sdf_proxy",
            Self::BuildVoxelEditDeltaRows => "build_voxel_edit_delta_rows",
            Self::BuildNetworkRelevanceRows => "build_network_relevance_rows",
            Self::BuildFoliageTrunkBranchVirtualGeometry => {
                "build_foliage_trunk_branch_virtual_geometry"
            }
            Self::BuildFoliageGrassBrushInstanceClusters => {
                "build_foliage_grass_brush_instance_clusters"
            }
            Self::BuildFoliageCollisionLargeObjectProxy => {
                "build_foliage_collision_large_object_proxy"
            }
            Self::BuildFoliageCardsImpostors => "build_foliage_cards_impostors",
            Self::BuildFoliageClusteredAnimation => "build_foliage_clustered_animation",
            Self::BuildFoliageCanopyTransmittance => "build_foliage_canopy_transmittance",
            Self::BuildFoliageBiomeTint => "build_foliage_biome_tint",
            Self::BuildFoliageHorizonImpostorField => "build_foliage_horizon_impostor_field",
            Self::BuildFoliageSdfOpacityShadows => "build_foliage_sdf_opacity_shadows",
            Self::BuildStormExtinctionDirtyRows => "build_storm_extinction_dirty_rows",
        }
    }

    #[must_use]
    pub const fn artifact_kind(self) -> EcsDerivedArtifactKind {
        match self {
            Self::BuildTerrainSurfacePackets => EcsDerivedArtifactKind::TerrainSurfacePackets,
            Self::BuildTerrainCoarseProxy => EcsDerivedArtifactKind::TerrainCoarseProxy,
            Self::BuildTerrainSdf => EcsDerivedArtifactKind::TerrainSdf,
            Self::BuildTerrainMaterialPage => EcsDerivedArtifactKind::TerrainMaterialPage,
            Self::BuildLoadAnimationRecord => EcsDerivedArtifactKind::LoadAnimationRecord,
            Self::BuildFoliageSeeds => EcsDerivedArtifactKind::FoliageSeeds,
            Self::BuildFoliageClusters => EcsDerivedArtifactKind::FoliageClusters,
            Self::BuildCanopyOpacity => EcsDerivedArtifactKind::CanopyOpacity,
            Self::BuildShadowInvalidationRows => EcsDerivedArtifactKind::ShadowInvalidationRows,
            Self::BuildRadianceUpdateRows => EcsDerivedArtifactKind::RadianceUpdateRows,
            Self::BuildPhysicsCookRequests => EcsDerivedArtifactKind::PhysicsCookRequests,
            Self::BuildCollisionSdfProxy => EcsDerivedArtifactKind::CollisionSdfProxy,
            Self::BuildVoxelEditDeltaRows => EcsDerivedArtifactKind::VoxelEditDeltaRows,
            Self::BuildNetworkRelevanceRows => EcsDerivedArtifactKind::NetworkRelevanceRows,
            Self::BuildFoliageTrunkBranchVirtualGeometry => {
                EcsDerivedArtifactKind::FoliageTrunkBranchVirtualGeometry
            }
            Self::BuildFoliageGrassBrushInstanceClusters => {
                EcsDerivedArtifactKind::FoliageGrassBrushInstanceClusters
            }
            Self::BuildFoliageCollisionLargeObjectProxy => {
                EcsDerivedArtifactKind::FoliageCollisionLargeObjectProxy
            }
            Self::BuildFoliageCardsImpostors => EcsDerivedArtifactKind::FoliageCardsImpostors,
            Self::BuildFoliageClusteredAnimation => {
                EcsDerivedArtifactKind::FoliageClusteredAnimation
            }
            Self::BuildFoliageCanopyTransmittance => {
                EcsDerivedArtifactKind::FoliageCanopyTransmittance
            }
            Self::BuildFoliageBiomeTint => EcsDerivedArtifactKind::FoliageBiomeTint,
            Self::BuildFoliageHorizonImpostorField => {
                EcsDerivedArtifactKind::FoliageHorizonImpostorField
            }
            Self::BuildFoliageSdfOpacityShadows => EcsDerivedArtifactKind::FoliageSdfOpacityShadows,
            Self::BuildStormExtinctionDirtyRows => EcsDerivedArtifactKind::StormExtinctionDirtyRows,
        }
    }

    #[must_use]
    pub const fn prototype_artifact_kind(
        self,
        flags: ProceduralTerrainArtifactFlags,
    ) -> Option<EcsDerivedArtifactKind> {
        match self {
            Self::BuildTerrainCoarseProxy => Some(EcsDerivedArtifactKind::TerrainCoarseProxy),
            Self::BuildTerrainSurfacePackets => Some(EcsDerivedArtifactKind::TerrainSurfacePackets),
            Self::BuildTerrainMaterialPage => Some(EcsDerivedArtifactKind::TerrainMaterialPage),
            Self::BuildLoadAnimationRecord => Some(EcsDerivedArtifactKind::LoadAnimationRecord),
            Self::BuildTerrainSdf if flags.terrain_sdf => Some(EcsDerivedArtifactKind::TerrainSdf),
            Self::BuildPhysicsCookRequests if flags.physics_cook_requests => {
                Some(EcsDerivedArtifactKind::PhysicsCookRequests)
            }
            Self::BuildShadowInvalidationRows if flags.virtual_shadow_invalidation => {
                Some(EcsDerivedArtifactKind::VirtualShadowInvalidation)
            }
            _ => None,
        }
    }

    #[must_use]
    pub const fn consumer(self) -> EcsArtifactConsumer {
        match self {
            Self::BuildTerrainSurfacePackets
            | Self::BuildTerrainCoarseProxy
            | Self::BuildTerrainMaterialPage
            | Self::BuildLoadAnimationRecord
            | Self::BuildFoliageSeeds
            | Self::BuildFoliageClusters
            | Self::BuildFoliageTrunkBranchVirtualGeometry
            | Self::BuildFoliageGrassBrushInstanceClusters
            | Self::BuildFoliageCardsImpostors
            | Self::BuildFoliageClusteredAnimation
            | Self::BuildFoliageBiomeTint
            | Self::BuildFoliageHorizonImpostorField => EcsArtifactConsumer::Renderer,
            Self::BuildTerrainSdf
            | Self::BuildCanopyOpacity
            | Self::BuildShadowInvalidationRows
            | Self::BuildRadianceUpdateRows
            | Self::BuildFoliageCanopyTransmittance
            | Self::BuildFoliageSdfOpacityShadows
            | Self::BuildStormExtinctionDirtyRows => EcsArtifactConsumer::Lux,
            Self::BuildPhysicsCookRequests
            | Self::BuildCollisionSdfProxy
            | Self::BuildFoliageCollisionLargeObjectProxy => EcsArtifactConsumer::AvisPhysics,
            Self::BuildVoxelEditDeltaRows | Self::BuildNetworkRelevanceRows => {
                EcsArtifactConsumer::ThunderNetwork
            }
        }
    }

    #[must_use]
    pub const fn requiredness(self) -> WorkRequiredness {
        match self {
            Self::BuildLoadAnimationRecord
            | Self::BuildTerrainSdf
            | Self::BuildFoliageSeeds
            | Self::BuildFoliageClusters
            | Self::BuildCanopyOpacity
            | Self::BuildShadowInvalidationRows
            | Self::BuildRadianceUpdateRows
            | Self::BuildPhysicsCookRequests
            | Self::BuildFoliageTrunkBranchVirtualGeometry
            | Self::BuildFoliageGrassBrushInstanceClusters
            | Self::BuildFoliageCollisionLargeObjectProxy
            | Self::BuildFoliageCardsImpostors
            | Self::BuildFoliageClusteredAnimation
            | Self::BuildFoliageCanopyTransmittance
            | Self::BuildFoliageBiomeTint
            | Self::BuildFoliageHorizonImpostorField
            | Self::BuildFoliageSdfOpacityShadows
            | Self::BuildStormExtinctionDirtyRows => WorkRequiredness::Optional,
            Self::BuildTerrainSurfacePackets
            | Self::BuildTerrainCoarseProxy
            | Self::BuildTerrainMaterialPage
            | Self::BuildCollisionSdfProxy
            | Self::BuildVoxelEditDeltaRows
            | Self::BuildNetworkRelevanceRows => WorkRequiredness::Required,
        }
    }

    #[must_use]
    pub const fn builds_for_generated_page_class(
        self,
        class: ProceduralGeneratedPageClass,
    ) -> bool {
        match self {
            Self::BuildTerrainSurfacePackets
            | Self::BuildTerrainMaterialPage
            | Self::BuildLoadAnimationRecord => class.builds_surface_artifacts(),
            Self::BuildTerrainCoarseProxy => class.builds_coarse_proxy(),
            Self::BuildTerrainSdf
            | Self::BuildShadowInvalidationRows
            | Self::BuildRadianceUpdateRows => class.builds_lux_invalidation(),
            Self::BuildPhysicsCookRequests | Self::BuildCollisionSdfProxy => {
                class.builds_physics_proxy()
            }
            Self::BuildVoxelEditDeltaRows
            | Self::BuildNetworkRelevanceRows
            | Self::BuildFoliageSeeds
            | Self::BuildFoliageClusters
            | Self::BuildCanopyOpacity
            | Self::BuildFoliageTrunkBranchVirtualGeometry
            | Self::BuildFoliageGrassBrushInstanceClusters
            | Self::BuildFoliageCollisionLargeObjectProxy
            | Self::BuildFoliageCardsImpostors
            | Self::BuildFoliageClusteredAnimation
            | Self::BuildFoliageCanopyTransmittance
            | Self::BuildFoliageBiomeTint
            | Self::BuildFoliageHorizonImpostorField
            | Self::BuildFoliageSdfOpacityShadows
            | Self::BuildStormExtinctionDirtyRows => false,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsArtifactState {
    #[default]
    Requested = 0,
    Building = 1,
    Ready = 2,
    HandoffQueued = 3,
    ExternalPublishing = 4,
    Published = 5,
    Retiring = 6,
    Failed = 7,
}

impl EcsArtifactState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Building => "building",
            Self::Ready => "ready",
            Self::HandoffQueued => "handoff_queued",
            Self::ExternalPublishing => "external_publishing",
            Self::Published => "published",
            Self::Retiring => "retiring",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsArtifactConsumer {
    #[default]
    Renderer = 0,
    Lux = 1,
    AvisPhysics = 2,
    ThunderNetwork = 3,
    Navigation = 4,
    Audio = 5,
    Telemetry = 6,
    Editor = 7,
}

impl EcsArtifactConsumer {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Renderer => "renderer",
            Self::Lux => "lux",
            Self::AvisPhysics => "avis_physics",
            Self::ThunderNetwork => "thunder_network",
            Self::Navigation => "navigation",
            Self::Audio => "audio",
            Self::Telemetry => "telemetry",
            Self::Editor => "editor",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactNodeId(pub u32);

impl ArtifactNodeId {
    pub const INVALID: Self = Self(0);

    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.0 != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactProducer {
    #[default]
    SourceDecode,
    VoxelEditPropagation,
    DerivedArtifactBuildSystem(EcsDerivedArtifactBuildSystem),
    RendererUploadPublish,
    LuxPlanPolicy,
    AvisPhysicsCook,
    ThunderNetworkRowBuilder,
    RvelteUiPacketBuilder,
}

impl ArtifactProducer {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::SourceDecode => "source_decode",
            Self::VoxelEditPropagation => "voxel_edit_propagation",
            Self::DerivedArtifactBuildSystem(system) => system.label(),
            Self::RendererUploadPublish => "renderer_upload_publish",
            Self::LuxPlanPolicy => "lux_plan_policy",
            Self::AvisPhysicsCook => "avis_physics_cook",
            Self::ThunderNetworkRowBuilder => "thunder_network_row_builder",
            Self::RvelteUiPacketBuilder => "rvelte_ui_packet_builder",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ArtifactConsumer {
    #[default]
    Renderer = 0,
    Lux = 1,
    Avis = 2,
    Thunder = 3,
    Navigation = 4,
    Audio = 5,
    Telemetry = 6,
    Editor = 7,
    LoadAnimation = 8,
    Rvelte = 9,
}

impl ArtifactConsumer {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Renderer => "renderer",
            Self::Lux => "lux",
            Self::Avis => "avis",
            Self::Thunder => "thunder",
            Self::Navigation => "navigation",
            Self::Audio => "audio",
            Self::Telemetry => "telemetry",
            Self::Editor => "editor",
            Self::LoadAnimation => "load_animation",
            Self::Rvelte => "rvelte",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Renderer,
            Self::Lux,
            Self::Avis,
            Self::Thunder,
            Self::Navigation,
            Self::Audio,
            Self::Telemetry,
            Self::Editor,
            Self::LoadAnimation,
            Self::Rvelte,
        ]
    }

    #[must_use]
    pub const fn from_ecs(consumer: EcsArtifactConsumer) -> Self {
        match consumer {
            EcsArtifactConsumer::Renderer => Self::Renderer,
            EcsArtifactConsumer::Lux => Self::Lux,
            EcsArtifactConsumer::AvisPhysics => Self::Avis,
            EcsArtifactConsumer::ThunderNetwork => Self::Thunder,
            EcsArtifactConsumer::Navigation => Self::Navigation,
            EcsArtifactConsumer::Audio => Self::Audio,
            EcsArtifactConsumer::Telemetry => Self::Telemetry,
            EcsArtifactConsumer::Editor => Self::Editor,
        }
    }

    #[must_use]
    pub const fn domain(self) -> ScheduleDomain {
        match self {
            Self::Renderer | Self::LoadAnimation => ScheduleDomain::Renderer,
            Self::Lux => ScheduleDomain::RendererLux,
            Self::Avis => ScheduleDomain::AvisPhysics,
            Self::Thunder => ScheduleDomain::ThunderNetwork,
            Self::Telemetry => ScheduleDomain::Telemetry,
            Self::Rvelte => ScheduleDomain::RvelteUi,
            Self::Navigation | Self::Audio | Self::Editor => ScheduleDomain::FunEcs,
        }
    }

    #[must_use]
    pub const fn lane(self) -> ScheduleLane {
        match self {
            Self::Renderer | Self::LoadAnimation => ScheduleLane::RenderPrepare,
            Self::Lux => ScheduleLane::RenderGraphCompile,
            Self::Avis => ScheduleLane::PhysicsSolve,
            Self::Thunder => ScheduleLane::NetworkRealtime,
            Self::Telemetry => ScheduleLane::TelemetryIngest,
            Self::Rvelte => ScheduleLane::UiAnimationFrame,
            Self::Navigation | Self::Audio | Self::Editor => ScheduleLane::EcsSystem,
        }
    }
}

impl EcsDerivedArtifactBuildSystem {
    #[must_use]
    pub const fn dag_consumer(self) -> ArtifactConsumer {
        match self {
            Self::BuildLoadAnimationRecord => ArtifactConsumer::LoadAnimation,
            _ => ArtifactConsumer::from_ecs(self.consumer()),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ArtifactFallbackPolicy {
    #[default]
    None = 0,
    DropOptional = 1,
    UsePreviousGeneration = 2,
    CoarseRendererArtifact = 3,
    ConservativePhysicsProxy = 4,
    BoundedRequiredUiPacket = 5,
}

impl ArtifactFallbackPolicy {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DropOptional => "drop_optional",
            Self::UsePreviousGeneration => "use_previous_generation",
            Self::CoarseRendererArtifact => "coarse_renderer_artifact",
            Self::ConservativePhysicsProxy => "conservative_physics_proxy",
            Self::BoundedRequiredUiPacket => "bounded_required_ui_packet",
        }
    }

    #[must_use]
    pub const fn is_available(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtifactInput {
    pub artifact_kind: Option<EcsDerivedArtifactKind>,
    pub virtual_resource: EcsVirtualResourceKey,
    pub wait_token_kind: EcsCrossDomainWaitTokenKind,
    pub generation: u32,
    pub requiredness: WorkRequiredness,
    pub fallback_policy: ArtifactFallbackPolicy,
    pub gates_present: bool,
}

impl ArtifactInput {
    #[must_use]
    pub fn decoded_page(page: EcsSpatialPageKey, generation: u32) -> Self {
        Self {
            artifact_kind: None,
            virtual_resource: page.virtual_resource_key(generation),
            wait_token_kind: EcsCrossDomainWaitTokenKind::VoxelPageDecoded,
            generation,
            requiredness: WorkRequiredness::Required,
            fallback_policy: ArtifactFallbackPolicy::None,
            gates_present: false,
        }
    }

    #[must_use]
    pub fn artifact(
        kind: EcsDerivedArtifactKind,
        page: EcsSpatialPageKey,
        generation: u32,
        requiredness: WorkRequiredness,
    ) -> Self {
        Self {
            artifact_kind: Some(kind),
            virtual_resource: artifact_virtual_resource_key(kind, page, generation),
            wait_token_kind: kind.readiness_token_kind(),
            generation,
            requiredness,
            fallback_policy: ArtifactFallbackPolicy::None,
            gates_present: false,
        }
    }

    #[must_use]
    pub const fn with_fallback(mut self, fallback_policy: ArtifactFallbackPolicy) -> Self {
        self.fallback_policy = fallback_policy;
        self
    }

    #[must_use]
    pub const fn gate_present(mut self) -> Self {
        self.gates_present = true;
        self
    }

    #[must_use]
    pub fn scheduler_token(self) -> WorkWaitToken {
        self.wait_token_kind
            .scheduler_kind()
            .to_wait_token(self.virtual_resource)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtifactOutput {
    pub artifact_id: Option<EcsDerivedArtifactId>,
    pub artifact_kind: Option<EcsDerivedArtifactKind>,
    pub virtual_resource: EcsVirtualResourceKey,
    pub wait_token_kind: EcsCrossDomainWaitTokenKind,
    pub generation: u32,
    pub requiredness: WorkRequiredness,
    pub consumer: ArtifactConsumer,
}

impl ArtifactOutput {
    #[must_use]
    pub fn decoded_page(page: EcsSpatialPageKey, generation: u32) -> Self {
        Self {
            artifact_id: None,
            artifact_kind: None,
            virtual_resource: page.virtual_resource_key(generation),
            wait_token_kind: EcsCrossDomainWaitTokenKind::VoxelPageDecoded,
            generation,
            requiredness: WorkRequiredness::Required,
            consumer: ArtifactConsumer::Editor,
        }
    }

    #[must_use]
    pub fn artifact(
        kind: EcsDerivedArtifactKind,
        artifact_id: EcsDerivedArtifactId,
        page: EcsSpatialPageKey,
        generation: u32,
        requiredness: WorkRequiredness,
        consumer: ArtifactConsumer,
    ) -> Self {
        Self {
            artifact_id: Some(artifact_id),
            artifact_kind: Some(kind),
            virtual_resource: artifact_virtual_resource_key(kind, page, generation),
            wait_token_kind: kind.readiness_token_kind(),
            generation,
            requiredness,
            consumer,
        }
    }

    #[must_use]
    pub fn from_record(record: EcsDerivedArtifactRecord) -> Self {
        Self::artifact(
            record.kind,
            record.artifact_id,
            record.source_page,
            record.artifact_epoch,
            record.requiredness,
            ArtifactConsumer::from_ecs(record.consumer),
        )
    }

    #[must_use]
    pub fn scheduler_token(self) -> WorkWaitToken {
        self.wait_token_kind
            .scheduler_kind()
            .to_wait_token(self.virtual_resource)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactNode {
    pub id: ArtifactNodeId,
    pub producer: ArtifactProducer,
    pub inputs: Vec<ArtifactInput>,
    pub outputs: Vec<ArtifactOutput>,
    pub requiredness: WorkRequiredness,
    pub fallback_policy: ArtifactFallbackPolicy,
    pub deadline: ScheduleDeadline,
    pub domain: ScheduleDomain,
    pub lane: ScheduleLane,
    pub chunk_split_policy: WorkSplitHint,
    pub producer_wait_token: Option<WorkWaitToken>,
    pub consumer_wait_token: Option<WorkWaitToken>,
}

impl ArtifactNode {
    #[must_use]
    pub fn new(id: ArtifactNodeId, producer: ArtifactProducer) -> Self {
        Self {
            id,
            producer,
            inputs: Vec::new(),
            outputs: Vec::new(),
            requiredness: WorkRequiredness::Required,
            fallback_policy: ArtifactFallbackPolicy::None,
            deadline: ScheduleDeadline::Stream,
            domain: ScheduleDomain::FunEcs,
            lane: ScheduleLane::EcsSystem,
            chunk_split_policy: WorkSplitHint::Splittable,
            producer_wait_token: None,
            consumer_wait_token: None,
        }
    }

    #[must_use]
    pub fn source_decode(id: ArtifactNodeId, page: EcsSpatialPageKey, generation: u32) -> Self {
        Self::new(id, ArtifactProducer::SourceDecode)
            .with_output(ArtifactOutput::decoded_page(page, generation))
    }

    #[must_use]
    pub fn from_build_system(
        id: ArtifactNodeId,
        page: EcsSpatialPageKey,
        artifact_id: EcsDerivedArtifactId,
        generation: u32,
        system: EcsDerivedArtifactBuildSystem,
    ) -> Self {
        let consumer = system.dag_consumer();
        Self::new(id, ArtifactProducer::DerivedArtifactBuildSystem(system))
            .with_input(ArtifactInput::decoded_page(page, generation))
            .with_output(ArtifactOutput::artifact(
                system.artifact_kind(),
                artifact_id,
                page,
                generation,
                system.requiredness(),
                consumer,
            ))
            .with_execution(
                consumer.domain(),
                consumer.lane(),
                ScheduleDeadline::Stream,
                WorkSplitHint::Splittable,
            )
            .with_requiredness(system.requiredness())
    }

    #[must_use]
    pub fn renderer_publish(
        id: ArtifactNodeId,
        page: EcsSpatialPageKey,
        artifact_id: EcsDerivedArtifactId,
        generation: u32,
    ) -> Self {
        Self::new(id, ArtifactProducer::RendererUploadPublish)
            .with_input(
                ArtifactInput::artifact(
                    EcsDerivedArtifactKind::TerrainSurfacePackets,
                    page,
                    generation,
                    WorkRequiredness::Required,
                )
                .gate_present()
                .with_fallback(ArtifactFallbackPolicy::CoarseRendererArtifact),
            )
            .with_output(ArtifactOutput {
                artifact_id: Some(artifact_id),
                artifact_kind: Some(EcsDerivedArtifactKind::TerrainSurfacePackets),
                virtual_resource: artifact_virtual_resource_key(
                    EcsDerivedArtifactKind::TerrainSurfacePackets,
                    page,
                    generation,
                ),
                wait_token_kind: EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished,
                generation,
                requiredness: WorkRequiredness::Required,
                consumer: ArtifactConsumer::Renderer,
            })
            .with_execution(
                ScheduleDomain::Renderer,
                ScheduleLane::RenderPrepare,
                ScheduleDeadline::Present,
                WorkSplitHint::Splittable,
            )
    }

    #[must_use]
    pub fn rvelte_paint_packet(
        id: ArtifactNodeId,
        packet_generation: u32,
        frame_generation: u32,
    ) -> Self {
        let input_key = rvelte_artifact_virtual_resource_key(
            EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady,
            packet_generation,
            frame_generation,
        );
        let output_key = rvelte_artifact_virtual_resource_key(
            EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished,
            packet_generation,
            frame_generation,
        );
        Self::new(id, ArtifactProducer::RvelteUiPacketBuilder)
            .with_input(ArtifactInput {
                artifact_kind: None,
                virtual_resource: input_key,
                wait_token_kind: EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady,
                generation: frame_generation,
                requiredness: WorkRequiredness::Required,
                fallback_policy: ArtifactFallbackPolicy::BoundedRequiredUiPacket,
                gates_present: false,
            })
            .with_output(ArtifactOutput {
                artifact_id: None,
                artifact_kind: None,
                virtual_resource: output_key,
                wait_token_kind: EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished,
                generation: frame_generation,
                requiredness: WorkRequiredness::Required,
                consumer: ArtifactConsumer::Renderer,
            })
            .with_execution(
                ScheduleDomain::RvelteUi,
                ScheduleLane::UiAnimationFrame,
                ScheduleDeadline::Frame,
                WorkSplitHint::Unsplittable,
            )
    }

    #[must_use]
    pub fn with_input(mut self, input: ArtifactInput) -> Self {
        if self.producer_wait_token.is_none() {
            self.producer_wait_token = Some(input.scheduler_token());
        }
        self.inputs.push(input);
        self
    }

    #[must_use]
    pub fn with_output(mut self, output: ArtifactOutput) -> Self {
        if self.consumer_wait_token.is_none() {
            self.consumer_wait_token = Some(output.scheduler_token());
        }
        self.outputs.push(output);
        self
    }

    #[must_use]
    pub const fn with_requiredness(mut self, requiredness: WorkRequiredness) -> Self {
        self.requiredness = requiredness;
        self
    }

    #[must_use]
    pub const fn with_fallback(mut self, fallback_policy: ArtifactFallbackPolicy) -> Self {
        self.fallback_policy = fallback_policy;
        self
    }

    #[must_use]
    pub const fn with_execution(
        mut self,
        domain: ScheduleDomain,
        lane: ScheduleLane,
        deadline: ScheduleDeadline,
        chunk_split_policy: WorkSplitHint,
    ) -> Self {
        self.domain = domain;
        self.lane = lane;
        self.deadline = deadline;
        self.chunk_split_policy = chunk_split_policy;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtifactDependency {
    pub producer: ArtifactNodeId,
    pub consumer: ArtifactNodeId,
    pub output_index: u16,
    pub input_index: u16,
    pub artifact_kind: Option<EcsDerivedArtifactKind>,
    pub virtual_resource: EcsVirtualResourceKey,
    pub requiredness: WorkRequiredness,
    pub fallback_policy: ArtifactFallbackPolicy,
    pub producer_wait_token: WorkWaitToken,
    pub consumer_wait_token: WorkWaitToken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtifactReadinessToken {
    pub producer: ArtifactNodeId,
    pub artifact_id: Option<EcsDerivedArtifactId>,
    pub artifact_kind: Option<EcsDerivedArtifactKind>,
    pub consumer: ArtifactConsumer,
    pub token_kind: EcsCrossDomainWaitTokenKind,
    pub scheduler_token: WorkWaitToken,
    pub virtual_resource: EcsVirtualResourceKey,
    pub generation: u32,
    pub requiredness: WorkRequiredness,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ArtifactRetirePlan {
    pub artifacts: Vec<EcsDerivedArtifactId>,
}

impl ArtifactRetirePlan {
    #[must_use]
    pub fn from_registry(registry: &EcsDerivedArtifactRegistry) -> Self {
        let mut artifacts: Vec<EcsDerivedArtifactId> = registry
            .artifacts
            .iter()
            .filter_map(|(artifact_id, record)| {
                matches!(record.state, EcsArtifactState::Retiring).then_some(artifact_id)
            })
            .collect();
        artifacts.sort_unstable();
        Self { artifacts }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ArtifactBuildPlan {
    pub nodes: Vec<ArtifactNode>,
    pub dependencies: Vec<ArtifactDependency>,
    pub readiness_tokens: Vec<ArtifactReadinessToken>,
    pub retire_plan: ArtifactRetirePlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArtifactDagError {
    MissingRequiredProducer {
        consumer: ArtifactNodeId,
        artifact_kind: Option<EcsDerivedArtifactKind>,
        virtual_resource: EcsVirtualResourceKey,
    },
    OptionalPresentGateWithoutFallback {
        consumer: ArtifactNodeId,
        artifact_kind: Option<EcsDerivedArtifactKind>,
    },
    ArtifactGenerationMismatch {
        artifact_id: EcsDerivedArtifactId,
        expected: u32,
        actual: u32,
    },
    RetiredArtifactConsumedWithoutFallback {
        artifact_id: EcsDerivedArtifactId,
    },
    MissingArtifactRecord {
        artifact_id: EcsDerivedArtifactId,
    },
}

impl ArtifactDagError {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::MissingRequiredProducer { .. } => "missing_required_producer",
            Self::OptionalPresentGateWithoutFallback { .. } => {
                "optional_present_gate_without_fallback"
            }
            Self::ArtifactGenerationMismatch { .. } => "artifact_generation_mismatch",
            Self::RetiredArtifactConsumedWithoutFallback { .. } => {
                "retired_artifact_consumed_without_fallback"
            }
            Self::MissingArtifactRecord { .. } => "missing_artifact_record",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ArtifactConsumerRead {
    pub artifact_id: EcsDerivedArtifactId,
    pub expected_generation: u32,
    pub consumer: ArtifactConsumer,
    pub fallback_policy: ArtifactFallbackPolicy,
}

impl ArtifactConsumerRead {
    #[must_use]
    pub const fn new(
        artifact_id: EcsDerivedArtifactId,
        expected_generation: u32,
        consumer: ArtifactConsumer,
    ) -> Self {
        Self {
            artifact_id,
            expected_generation,
            consumer,
            fallback_policy: ArtifactFallbackPolicy::None,
        }
    }

    #[must_use]
    pub const fn with_fallback(mut self, fallback_policy: ArtifactFallbackPolicy) -> Self {
        self.fallback_policy = fallback_policy;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArtifactConsumerReadDecision {
    ConsumeArtifact(EcsDerivedArtifactRecord),
    UseFallback(ArtifactFallbackPolicy),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ArtifactDag {
    pub nodes: Vec<ArtifactNode>,
    pub retired_artifacts: Vec<EcsDerivedArtifactId>,
}

impl ArtifactDag {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn from_decoded_page(
        page: EcsSpatialPageKey,
        generation: u32,
        first_artifact_id: u64,
        systems: &[EcsDerivedArtifactBuildSystem],
    ) -> Self {
        let mut dag = Self::new().with_node(ArtifactNode::source_decode(
            ArtifactNodeId::new(1),
            page,
            generation,
        ));
        for (index, system) in systems.iter().copied().enumerate() {
            dag = dag.with_node(ArtifactNode::from_build_system(
                ArtifactNodeId::new(index as u32 + 2),
                page,
                EcsDerivedArtifactId::new(first_artifact_id + index as u64),
                generation,
                system,
            ));
        }
        dag
    }

    #[must_use]
    pub fn with_node(mut self, node: ArtifactNode) -> Self {
        self.nodes.push(node);
        self.nodes.sort_by_key(|node| node.id);
        self
    }

    #[must_use]
    pub fn with_retired_artifact(mut self, artifact_id: EcsDerivedArtifactId) -> Self {
        if !self.retired_artifacts.contains(&artifact_id) {
            self.retired_artifacts.push(artifact_id);
            self.retired_artifacts.sort_unstable();
        }
        self
    }

    pub fn build_plan(&self) -> Result<ArtifactBuildPlan, ArtifactDagError> {
        let mut dependencies = Vec::new();
        for consumer in &self.nodes {
            for (input_index, input) in consumer.inputs.iter().copied().enumerate() {
                if input.gates_present && !input.fallback_policy.is_available() {
                    return Err(ArtifactDagError::OptionalPresentGateWithoutFallback {
                        consumer: consumer.id,
                        artifact_kind: input.artifact_kind,
                    });
                }
                match self.find_producer(input) {
                    Some((producer, output_index, output)) => {
                        dependencies.push(ArtifactDependency {
                            producer: producer.id,
                            consumer: consumer.id,
                            output_index: output_index as u16,
                            input_index: input_index as u16,
                            artifact_kind: input.artifact_kind,
                            virtual_resource: input.virtual_resource,
                            requiredness: input.requiredness,
                            fallback_policy: input.fallback_policy,
                            producer_wait_token: output.scheduler_token(),
                            consumer_wait_token: input.scheduler_token(),
                        })
                    }
                    None if matches!(input.requiredness, WorkRequiredness::Required) => {
                        return Err(ArtifactDagError::MissingRequiredProducer {
                            consumer: consumer.id,
                            artifact_kind: input.artifact_kind,
                            virtual_resource: input.virtual_resource,
                        });
                    }
                    None => {}
                }
            }
        }
        let readiness_tokens = self.readiness_tokens();
        Ok(ArtifactBuildPlan {
            nodes: self.nodes.clone(),
            dependencies,
            readiness_tokens,
            retire_plan: ArtifactRetirePlan {
                artifacts: self.retired_artifacts.clone(),
            },
        })
    }

    #[must_use = "artifact consumer reads must handle stale generation and retired fallback decisions"]
    pub fn validate_consumer_read(
        registry: &EcsDerivedArtifactRegistry,
        read: ArtifactConsumerRead,
    ) -> Result<ArtifactConsumerReadDecision, ArtifactDagError> {
        let Some(record) = registry.get(read.artifact_id).copied() else {
            return Err(ArtifactDagError::MissingArtifactRecord {
                artifact_id: read.artifact_id,
            });
        };
        if record.artifact_epoch != read.expected_generation {
            return Err(ArtifactDagError::ArtifactGenerationMismatch {
                artifact_id: read.artifact_id,
                expected: read.expected_generation,
                actual: record.artifact_epoch,
            });
        }
        if matches!(record.state, EcsArtifactState::Retiring) {
            if read.fallback_policy.is_available() {
                return Ok(ArtifactConsumerReadDecision::UseFallback(
                    read.fallback_policy,
                ));
            }
            return Err(ArtifactDagError::RetiredArtifactConsumedWithoutFallback {
                artifact_id: read.artifact_id,
            });
        }
        Ok(ArtifactConsumerReadDecision::ConsumeArtifact(record))
    }

    fn find_producer(
        &self,
        input: ArtifactInput,
    ) -> Option<(&ArtifactNode, usize, ArtifactOutput)> {
        self.nodes.iter().find_map(|node| {
            node.outputs
                .iter()
                .copied()
                .enumerate()
                .find(|(_, output)| {
                    output.artifact_kind == input.artifact_kind
                        && output.virtual_resource == input.virtual_resource
                        && output.generation == input.generation
                })
                .map(|(index, output)| (node, index, output))
        })
    }

    fn readiness_tokens(&self) -> Vec<ArtifactReadinessToken> {
        let mut tokens = Vec::new();
        for node in &self.nodes {
            for output in &node.outputs {
                tokens.push(ArtifactReadinessToken {
                    producer: node.id,
                    artifact_id: output.artifact_id,
                    artifact_kind: output.artifact_kind,
                    consumer: output.consumer,
                    token_kind: output.wait_token_kind,
                    scheduler_token: output.scheduler_token(),
                    virtual_resource: output.virtual_resource,
                    generation: output.generation,
                    requiredness: output.requiredness,
                });
            }
        }
        tokens.sort_by_key(|token| {
            (
                token.producer,
                token
                    .artifact_kind
                    .map(|kind| kind as u8)
                    .unwrap_or(u8::MAX),
                token
                    .artifact_id
                    .map(EcsDerivedArtifactId::get)
                    .unwrap_or(0),
                token.virtual_resource.key(),
            )
        });
        tokens
    }
}

impl EcsDerivedArtifactKind {
    #[must_use]
    pub const fn readiness_token_kind(self) -> EcsCrossDomainWaitTokenKind {
        match self {
            Self::TerrainSurfacePackets
            | Self::TerrainCoarseProxy
            | Self::TerrainMaterialPage
            | Self::FoliageSeeds
            | Self::FoliageClusters
            | Self::FoliageTrunkBranchVirtualGeometry
            | Self::FoliageGrassBrushInstanceClusters
            | Self::FoliageCardsImpostors
            | Self::FoliageClusteredAnimation
            | Self::FoliageBiomeTint
            | Self::FoliageHorizonImpostorField => {
                EcsCrossDomainWaitTokenKind::VoxelSurfaceArtifactReady
            }
            Self::TerrainSdf
            | Self::CanopyOpacity
            | Self::VirtualShadowInvalidation
            | Self::RadianceCacheUpdate
            | Self::ShadowInvalidationRows
            | Self::RadianceUpdateRows
            | Self::FoliageCanopyTransmittance
            | Self::FoliageSdfOpacityShadows
            | Self::StormExtinctionDirtyRows => {
                EcsCrossDomainWaitTokenKind::VoxelLuxInvalidationPublished
            }
            Self::PhysicsCollisionProxy
            | Self::PhysicsCookRequests
            | Self::CollisionSdfProxy
            | Self::FoliageCollisionLargeObjectProxy => {
                EcsCrossDomainWaitTokenKind::VoxelPhysicsProxyReady
            }
            Self::NavTile | Self::AudioOcclusionTile => {
                EcsCrossDomainWaitTokenKind::VoxelSurfaceArtifactReady
            }
            Self::NetworkDeltaRows | Self::VoxelEditDeltaRows | Self::NetworkRelevanceRows => {
                EcsCrossDomainWaitTokenKind::VoxelNetworkDeltaPublished
            }
            Self::LoadAnimationRecord => EcsCrossDomainWaitTokenKind::VoxelLoadAnimationPublished,
        }
    }
}

#[must_use]
pub fn artifact_virtual_resource_key(
    kind: EcsDerivedArtifactKind,
    page: EcsSpatialPageKey,
    generation: u32,
) -> EcsVirtualResourceKey {
    EcsVirtualResourceKey::new(
        page.domain,
        page.grid_id.get() as u16,
        page.level,
        0x100_u16 + kind as u16,
        page.chunk_key(),
        generation,
    )
}

#[must_use]
pub const fn rvelte_artifact_virtual_resource_key(
    token_kind: EcsCrossDomainWaitTokenKind,
    packet_generation: u32,
    frame_generation: u32,
) -> EcsVirtualResourceKey {
    EcsVirtualResourceKey::new(
        EcsSpatialDomainKind::Debug,
        0,
        0,
        0x200_u16 + token_kind as u16,
        EcsChunkKey::new(packet_generation as u64 + 1),
        frame_generation,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsDerivedArtifactRecord {
    pub artifact_id: EcsDerivedArtifactId,
    pub source_page: EcsSpatialPageKey,
    pub kind: EcsDerivedArtifactKind,
    pub source_epoch: u32,
    pub source_digest: u64,
    pub artifact_epoch: u32,
    pub state: EcsArtifactState,
    pub requiredness: WorkRequiredness,
    pub consumer: EcsArtifactConsumer,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsDerivedArtifactRegistry {
    pub artifacts: DenseSlotMap<EcsDerivedArtifactId, EcsDerivedArtifactRecord>,
}

impl EcsDerivedArtifactRegistry {
    pub fn push(
        &mut self,
        record: EcsDerivedArtifactRecord,
    ) -> Result<(), EcsSpatialValidationError> {
        if !self.artifacts.contains_key(record.artifact_id)
            && self.artifacts.len() >= ECS_SPATIAL_MAX_DERIVED_ARTIFACTS
        {
            return Err(EcsSpatialValidationError::DerivedArtifactRegistryFull);
        }
        self.artifacts.insert(record.artifact_id, record);
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.artifacts.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.artifacts.is_empty()
    }

    #[must_use]
    pub fn get(&self, artifact_id: EcsDerivedArtifactId) -> Option<&EcsDerivedArtifactRecord> {
        self.artifacts.get(artifact_id)
    }

    pub fn retire(&mut self, artifact_id: EcsDerivedArtifactId) -> bool {
        if let Some(record) = self.artifacts.get_mut(artifact_id) {
            record.state = EcsArtifactState::Retiring;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.artifacts.is_consistent()
    }
}

#[must_use]
pub fn derived_artifact_source_digest(
    source_page: EcsSpatialPageKey,
    source_epoch: u32,
    content_epoch: u32,
) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    hash = fnv1a_u64(hash, source_page.chunk_key().get());
    hash = fnv1a_u32(hash, source_epoch);
    hash = fnv1a_u32(hash, content_epoch);
    hash.max(1)
}

const fn fnv1a_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(0x0000_0100_0000_01b3)
}

const fn fnv1a_u16(hash: u64, value: u16) -> u64 {
    let hash = fnv1a_u8(hash, (value & 0xff) as u8);
    fnv1a_u8(hash, (value >> 8) as u8)
}

const fn fnv1a_u32(hash: u64, value: u32) -> u64 {
    let hash = fnv1a_u16(hash, (value & 0xffff) as u16);
    fnv1a_u16(hash, (value >> 16) as u16)
}

const fn fnv1a_u64(hash: u64, value: u64) -> u64 {
    let hash = fnv1a_u32(hash, (value & 0xffff_ffff) as u32);
    fnv1a_u32(hash, (value >> 32) as u32)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EcsArtifactBuildReport {
    pub decoded_pages: u32,
    pub artifacts_published: u32,
    pub skipped_failed_pages: u32,
    pub skipped_by_page_class: u32,
    pub skipped_by_profile: u32,
}

pub fn build_derived_artifacts(
    decoded_pages: &[EcsDecodedPageRecord],
    systems: &[EcsDerivedArtifactBuildSystem],
    next_artifact_id: &mut u64,
    commands: &mut EcsSpatialCommandBuffer,
) -> Result<EcsArtifactBuildReport, EcsSpatialValidationError> {
    let mut report = EcsArtifactBuildReport::default();
    for page in decoded_pages {
        report.decoded_pages += 1;
        if page.payload_kind == EcsDecodedPagePayloadKind::SourceFailed {
            report.skipped_failed_pages += 1;
            continue;
        }
        let class = generated_page_class(page);
        for system in systems {
            if !system.builds_for_generated_page_class(class) {
                report.skipped_by_page_class += 1;
                continue;
            }
            *next_artifact_id = next_artifact_id.saturating_add(1).max(1);
            let artifact = EcsDerivedArtifactRecord {
                artifact_id: EcsDerivedArtifactId::new(*next_artifact_id),
                source_page: page.key,
                kind: system.artifact_kind(),
                source_epoch: page.source_epoch,
                source_digest: page.telemetry.checksum.value,
                artifact_epoch: page.telemetry.decode_epoch,
                state: EcsArtifactState::Ready,
                requiredness: system.requiredness(),
                consumer: system.consumer(),
            };
            commands.push(EcsSpatialCommand::PublishArtifact(artifact))?;
            report.artifacts_published += 1;
        }
    }
    Ok(report)
}

pub fn build_prototype_terrain_artifacts(
    decoded_pages: &[EcsDecodedPageRecord],
    consumer_filter: Option<EcsArtifactConsumer>,
    flags: ProceduralTerrainArtifactFlags,
    next_artifact_id: &mut u64,
    commands: &mut EcsSpatialCommandBuffer,
) -> Result<EcsArtifactBuildReport, EcsSpatialValidationError> {
    let mut report = EcsArtifactBuildReport::default();
    for page in decoded_pages {
        report.decoded_pages += 1;
        if page.payload_kind == EcsDecodedPagePayloadKind::SourceFailed {
            report.skipped_failed_pages += 1;
            continue;
        }
        let class = generated_page_class(page);
        for system in EcsDerivedArtifactBuildSystem::all() {
            if consumer_filter.is_some_and(|consumer| system.consumer() != consumer) {
                continue;
            }
            let Some(kind) = system.prototype_artifact_kind(flags) else {
                report.skipped_by_profile += 1;
                continue;
            };
            if !system.builds_for_generated_page_class(class) {
                report.skipped_by_page_class += 1;
                continue;
            }
            *next_artifact_id = next_artifact_id.saturating_add(1).max(1);
            let artifact = EcsDerivedArtifactRecord {
                artifact_id: EcsDerivedArtifactId::new(*next_artifact_id),
                source_page: page.key,
                kind,
                source_epoch: page.source_epoch,
                source_digest: page.telemetry.checksum.value,
                artifact_epoch: page.telemetry.decode_epoch,
                state: EcsArtifactState::Ready,
                requiredness: system.requiredness(),
                consumer: system.consumer(),
            };
            commands.push(EcsSpatialCommand::PublishArtifact(artifact))?;
            report.artifacts_published += 1;
        }
    }
    Ok(report)
}

fn generated_page_class(page: &EcsDecodedPageRecord) -> ProceduralGeneratedPageClass {
    if let Some(brick) = &page.voxel_brick {
        brick.generated_class
    } else if page.payload_kind == EcsDecodedPagePayloadKind::Empty {
        ProceduralGeneratedPageClass::EmptyAir
    } else {
        ProceduralGeneratedPageClass::DebugOnly
    }
}

#[must_use]
pub fn procedural_terrain_coarse_proxy_from_decoded_page(
    page: &EcsDecodedPageRecord,
) -> Option<ProceduralTerrainCoarseProxy> {
    let brick = page.voxel_brick.as_ref()?;
    let mut occupied_cluster_mask = 0_u64;
    let mut min_local_y = i16::MAX;
    let mut max_local_y = i16::MIN;
    let mut dominant_material = TerrainMaterialId::INVALID;
    let mut dominant_material_count = 0_u16;

    for (index, cluster) in brick.clusters.iter().enumerate() {
        if cluster.occupancy_popcount == 0 {
            continue;
        }
        occupied_cluster_mask |= 1_u64 << index;
        min_local_y = min_local_y.min(cluster.min_height_local);
        max_local_y = max_local_y.max(cluster.max_height_local);
        if cluster.occupancy_popcount >= dominant_material_count && cluster.dominant_material != 0 {
            dominant_material_count = cluster.occupancy_popcount;
            dominant_material = TerrainMaterialId::new(u32::from(cluster.dominant_material));
        }
    }

    if occupied_cluster_mask == 0 {
        return None;
    }
    if !dominant_material.is_valid() {
        dominant_material = first_palette_material(brick).unwrap_or(TerrainMaterialId::new(1));
    }
    let origin_y = page_origin_y_ft(page.key);
    let cell_edge_ft = level_cell_edge_ft(page.key.level);
    Some(ProceduralTerrainCoarseProxy {
        source_page: page.key,
        min_height_ft: clamp_i64_to_i32(
            origin_y.saturating_add(i64::from(min_local_y).saturating_mul(cell_edge_ft)),
        ),
        max_height_ft: clamp_i64_to_i32(
            origin_y.saturating_add(i64::from(max_local_y).saturating_mul(cell_edge_ft)),
        ),
        dominant_material,
        occupied_cluster_mask,
        source_epoch: page.source_epoch,
    })
}

#[must_use]
pub fn procedural_terrain_surface_packet_from_decoded_page(
    page: &EcsDecodedPageRecord,
) -> Option<ProceduralTerrainSurfacePacket> {
    let brick = page.voxel_brick.as_ref()?;
    let (local_bounds, exposed_face_count) = blocky_surface_bounds_and_faces(brick)?;
    Some(ProceduralTerrainSurfacePacket {
        source_page: page.key,
        local_bounds,
        exposed_face_count,
        packet_range: PackedRange::new(0, exposed_face_count),
        material_palette_id: material_palette_id_for_page(page.key),
        source_epoch: page.source_epoch,
    })
}

#[must_use]
pub fn procedural_terrain_material_page_from_decoded_page(
    page: &EcsDecodedPageRecord,
) -> Option<ProceduralTerrainMaterialPage> {
    let brick = page.voxel_brick.as_ref()?;
    let mut dominant_material_per_cluster =
        [TerrainMaterialId::INVALID; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    for (index, cluster) in brick.clusters.iter().enumerate() {
        dominant_material_per_cluster[index] = if cluster.dominant_material == 0 {
            TerrainMaterialId::INVALID
        } else {
            TerrainMaterialId::new(u32::from(cluster.dominant_material))
        };
    }
    Some(ProceduralTerrainMaterialPage {
        source_page: page.key,
        palette: brick.material_palette,
        dominant_material_per_cluster,
        source_epoch: page.source_epoch,
    })
}

fn blocky_surface_bounds_and_faces(brick: &VoxelBrickPayload) -> Option<(PackedAabb, u32)> {
    let mut min = [i16::MAX; 3];
    let mut max = [i16::MIN; 3];
    let mut exposed_face_count = 0_u32;
    for z in 0..VOXEL_BRICK_EDGE_CELLS {
        for y in 0..VOXEL_BRICK_EDGE_CELLS {
            for x in 0..VOXEL_BRICK_EDGE_CELLS {
                if !voxel_is_solid(brick, x, y, z) {
                    continue;
                }
                let local = [i16::from(x), i16::from(y), i16::from(z)];
                for axis in 0..3 {
                    min[axis] = min[axis].min(local[axis]);
                    max[axis] = max[axis].max(local[axis]);
                }
                exposed_face_count =
                    exposed_face_count.saturating_add(exposed_faces_for_cell(brick, x, y, z));
            }
        }
    }
    (exposed_face_count != 0).then_some((PackedAabb::new(min, max), exposed_face_count))
}

fn exposed_faces_for_cell(brick: &VoxelBrickPayload, x: u8, y: u8, z: u8) -> u32 {
    let x = i16::from(x);
    let y = i16::from(y);
    let z = i16::from(z);
    let neighbors = [
        (x.saturating_sub(1), y, z),
        (x.saturating_add(1), y, z),
        (x, y.saturating_sub(1), z),
        (x, y.saturating_add(1), z),
        (x, y, z.saturating_sub(1)),
        (x, y, z.saturating_add(1)),
    ];
    neighbors
        .iter()
        .filter(|(nx, ny, nz)| !voxel_is_solid_i16(brick, *nx, *ny, *nz))
        .count()
        .min(u32::MAX as usize) as u32
}

fn voxel_is_solid_i16(brick: &VoxelBrickPayload, x: i16, y: i16, z: i16) -> bool {
    if x < 0
        || y < 0
        || z < 0
        || x >= i16::from(VOXEL_BRICK_EDGE_CELLS)
        || y >= i16::from(VOXEL_BRICK_EDGE_CELLS)
        || z >= i16::from(VOXEL_BRICK_EDGE_CELLS)
    {
        return false;
    }
    voxel_is_solid(brick, x as u8, y as u8, z as u8)
}

fn voxel_is_solid(brick: &VoxelBrickPayload, x: u8, y: u8, z: u8) -> bool {
    match brick.occupancy.kind {
        VoxelOccupancyStorageKind::Empty => false,
        VoxelOccupancyStorageKind::UniformSolid => true,
        VoxelOccupancyStorageKind::Bitset32 => {
            let index = voxel_index(x, y, z);
            let word = index / 64;
            let bit = index % 64;
            (brick.occupancy.words[word] & (1_u64 << bit)) != 0
        }
    }
}

fn voxel_index(x: u8, y: u8, z: u8) -> usize {
    ((usize::from(z) * usize::from(VOXEL_BRICK_EDGE_CELLS) + usize::from(y))
        * usize::from(VOXEL_BRICK_EDGE_CELLS))
        + usize::from(x)
}

fn first_palette_material(brick: &VoxelBrickPayload) -> Option<TerrainMaterialId> {
    (brick.material_palette.len != 0)
        .then(|| TerrainMaterialId::new(u32::from(brick.material_palette.materials[0])))
}

fn material_palette_id_for_page(page: EcsSpatialPageKey) -> TerrainMaterialPaletteId {
    TerrainMaterialPaletteId::new((page.chunk_key().get() as u32).max(1))
}

fn page_origin_y_ft(page: EcsSpatialPageKey) -> i64 {
    i64::from(page.y)
        .saturating_mul(i64::from(VOXEL_BRICK_EDGE_CELLS))
        .saturating_mul(level_cell_edge_ft(page.level))
}

fn level_cell_edge_ft(level: u8) -> i64 {
    if level >= 30 {
        1_i64 << 30
    } else {
        1_i64 << u32::from(level)
    }
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EcsRendererHandoffPublishReport {
    pub inspected: u32,
    pub published: u32,
    pub skipped_non_renderer: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EcsLuxHandoffPublishReport {
    pub inspected: u32,
    pub published: u32,
    pub skipped_non_lux: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EcsPhysicsCookPublishReport {
    pub inspected: u32,
    pub published: u32,
    pub skipped_non_physics: u32,
}

#[must_use]
pub fn renderer_handoff_from_artifact(
    artifact: EcsDerivedArtifactRecord,
    visibility_hint: RendererVisibilityHint,
) -> Option<RendererArtifactHandoff> {
    let kind = RendererArtifactHandoffKind::from_derived(artifact.kind)?;
    Some(RendererArtifactHandoff {
        artifact_id: artifact.artifact_id,
        source_page: artifact.source_page,
        kind,
        source_epoch: artifact.source_epoch,
        source_digest: artifact.source_digest,
        artifact_epoch: artifact.artifact_epoch,
        requiredness: artifact.requiredness,
        visibility_hint,
    })
}

pub fn publish_renderer_handoffs(
    artifacts: &[EcsDerivedArtifactRecord],
    visibility_hint: RendererVisibilityHint,
    commands: &mut EcsSpatialCommandBuffer,
) -> Result<EcsRendererHandoffPublishReport, EcsSpatialValidationError> {
    let mut report = EcsRendererHandoffPublishReport::default();
    for artifact in artifacts {
        report.inspected += 1;
        if let Some(handoff) = renderer_handoff_from_artifact(*artifact, visibility_hint) {
            commands.push(EcsSpatialCommand::PublishRendererHandoff(handoff))?;
            report.published += 1;
        } else {
            report.skipped_non_renderer += 1;
        }
    }
    Ok(report)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LuxArtifactHandoffKind {
    #[default]
    VoxelShadowInvalidationRows = 0,
    TerrainSdfForDistantShadow = 1,
    CanopyOpacityForTransmittance = 2,
    RadianceClipmapDirtyRows = 3,
    StormExtinctionDirtyRows = 4,
}

impl LuxArtifactHandoffKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::VoxelShadowInvalidationRows => "voxel_shadow_invalidation_rows",
            Self::TerrainSdfForDistantShadow => "terrain_sdf_for_distant_shadow",
            Self::CanopyOpacityForTransmittance => "canopy_opacity_for_transmittance",
            Self::RadianceClipmapDirtyRows => "radiance_clipmap_dirty_rows",
            Self::StormExtinctionDirtyRows => "storm_extinction_dirty_rows",
        }
    }

    #[must_use]
    pub const fn from_derived(kind: EcsDerivedArtifactKind) -> Option<Self> {
        match kind {
            EcsDerivedArtifactKind::VirtualShadowInvalidation
            | EcsDerivedArtifactKind::ShadowInvalidationRows => {
                Some(Self::VoxelShadowInvalidationRows)
            }
            EcsDerivedArtifactKind::TerrainSdf => Some(Self::TerrainSdfForDistantShadow),
            EcsDerivedArtifactKind::CanopyOpacity
            | EcsDerivedArtifactKind::FoliageCanopyTransmittance => {
                Some(Self::CanopyOpacityForTransmittance)
            }
            EcsDerivedArtifactKind::RadianceCacheUpdate
            | EcsDerivedArtifactKind::RadianceUpdateRows => Some(Self::RadianceClipmapDirtyRows),
            EcsDerivedArtifactKind::FoliageSdfOpacityShadows => {
                Some(Self::VoxelShadowInvalidationRows)
            }
            EcsDerivedArtifactKind::StormExtinctionDirtyRows => {
                Some(Self::StormExtinctionDirtyRows)
            }
            EcsDerivedArtifactKind::TerrainSurfacePackets
            | EcsDerivedArtifactKind::TerrainCoarseProxy
            | EcsDerivedArtifactKind::TerrainMaterialPage
            | EcsDerivedArtifactKind::PhysicsCollisionProxy
            | EcsDerivedArtifactKind::FoliageCollisionLargeObjectProxy
            | EcsDerivedArtifactKind::NavTile
            | EcsDerivedArtifactKind::AudioOcclusionTile
            | EcsDerivedArtifactKind::NetworkDeltaRows
            | EcsDerivedArtifactKind::LoadAnimationRecord
            | EcsDerivedArtifactKind::FoliageSeeds
            | EcsDerivedArtifactKind::FoliageClusters
            | EcsDerivedArtifactKind::FoliageTrunkBranchVirtualGeometry
            | EcsDerivedArtifactKind::FoliageGrassBrushInstanceClusters
            | EcsDerivedArtifactKind::FoliageCardsImpostors
            | EcsDerivedArtifactKind::FoliageClusteredAnimation
            | EcsDerivedArtifactKind::FoliageBiomeTint
            | EcsDerivedArtifactKind::FoliageHorizonImpostorField
            | EcsDerivedArtifactKind::PhysicsCookRequests
            | EcsDerivedArtifactKind::CollisionSdfProxy
            | EcsDerivedArtifactKind::VoxelEditDeltaRows
            | EcsDerivedArtifactKind::NetworkRelevanceRows => None,
        }
    }
}

#[must_use]
pub fn lux_handoff_from_artifact(
    artifact: EcsDerivedArtifactRecord,
    bounds_world: EcsAabbF32,
) -> Option<LuxArtifactHandoff> {
    let kind = LuxArtifactHandoffKind::from_derived(artifact.kind)?;
    Some(LuxArtifactHandoff {
        source_page: artifact.source_page,
        kind,
        dirty_epoch: artifact.artifact_epoch,
        bounds_world,
        requiredness: artifact.requiredness,
    })
}

pub fn publish_lux_handoffs(
    artifacts: &[EcsDerivedArtifactRecord],
    bounds_world: EcsAabbF32,
    commands: &mut EcsSpatialCommandBuffer,
) -> Result<EcsLuxHandoffPublishReport, EcsSpatialValidationError> {
    if !bounds_world.is_valid() {
        return Err(EcsSpatialValidationError::InvalidAabb);
    }
    let mut report = EcsLuxHandoffPublishReport::default();
    for artifact in artifacts {
        report.inspected += 1;
        if let Some(handoff) = lux_handoff_from_artifact(*artifact, bounds_world) {
            commands.push(EcsSpatialCommand::PublishLuxHandoff(handoff))?;
            report.published += 1;
        } else {
            report.skipped_non_lux += 1;
        }
    }
    Ok(report)
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CollisionCookMode {
    #[default]
    NarrowBandSdf = 0,
    SurfaceMeshProxy = 1,
    HeightProxy = 2,
    ConvexClusterProxy = 3,
    AggregateOnly = 4,
}

impl CollisionCookMode {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::NarrowBandSdf => "narrow_band_sdf",
            Self::SurfaceMeshProxy => "surface_mesh_proxy",
            Self::HeightProxy => "height_proxy",
            Self::ConvexClusterProxy => "convex_cluster_proxy",
            Self::AggregateOnly => "aggregate_only",
        }
    }

    #[must_use]
    pub const fn from_collision_policy(policy: VoxelCollisionPolicy) -> Option<Self> {
        match policy {
            VoxelCollisionPolicy::Disabled => None,
            VoxelCollisionPolicy::CoarseProxy => Some(Self::HeightProxy),
            VoxelCollisionPolicy::NarrowBandSdf => Some(Self::NarrowBandSdf),
            VoxelCollisionPolicy::FullResolution => Some(Self::SurfaceMeshProxy),
        }
    }

    #[must_use]
    pub const fn conservative_proxy_before_full_cook(self) -> Self {
        match self {
            Self::NarrowBandSdf | Self::SurfaceMeshProxy => Self::ConvexClusterProxy,
            Self::HeightProxy | Self::ConvexClusterProxy | Self::AggregateOnly => self,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsPhysicsSourceTruth {
    #[default]
    EcsVoxelSourceAndEditState = 0,
    RendererGpuBuffers = 1,
}

impl EcsPhysicsSourceTruth {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::EcsVoxelSourceAndEditState => "ecs_voxel_source_and_edit_state",
            Self::RendererGpuBuffers => "renderer_gpu_buffers",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsTerrainPhysicsRules {
    pub physics_source: EcsPhysicsSourceTruth,
    pub render_source: EcsPhysicsSourceTruth,
    pub collision_critical_requiredness: WorkRequiredness,
    pub far_collision_requiredness: WorkRequiredness,
    pub gameplay_pins_far_collision: bool,
    pub conservative_proxy_allowed: bool,
}

impl EcsTerrainPhysicsRules {
    pub const PRODUCT_DEFAULT: Self = Self {
        physics_source: EcsPhysicsSourceTruth::EcsVoxelSourceAndEditState,
        render_source: EcsPhysicsSourceTruth::EcsVoxelSourceAndEditState,
        collision_critical_requiredness: WorkRequiredness::Required,
        far_collision_requiredness: WorkRequiredness::Optional,
        gameplay_pins_far_collision: false,
        conservative_proxy_allowed: true,
    };

    #[must_use]
    pub const fn contract_holds(self) -> bool {
        matches!(
            self.physics_source,
            EcsPhysicsSourceTruth::EcsVoxelSourceAndEditState
        ) && matches!(
            self.render_source,
            EcsPhysicsSourceTruth::EcsVoxelSourceAndEditState
        ) && matches!(
            self.collision_critical_requiredness,
            WorkRequiredness::Required
        ) && matches!(self.far_collision_requiredness, WorkRequiredness::Optional)
            && self.conservative_proxy_allowed
    }

    #[must_use]
    pub const fn far_collision_requiredness(self) -> WorkRequiredness {
        if self.gameplay_pins_far_collision {
            WorkRequiredness::Required
        } else {
            self.far_collision_requiredness
        }
    }
}

#[must_use]
pub fn physics_cook_from_artifact(
    artifact: EcsDerivedArtifactRecord,
    mode: CollisionCookMode,
    fixed_step_deadline: Option<FixedStepId>,
) -> Option<VoxelPhysicsCookRequest> {
    match artifact.kind {
        EcsDerivedArtifactKind::PhysicsCollisionProxy
        | EcsDerivedArtifactKind::PhysicsCookRequests
        | EcsDerivedArtifactKind::CollisionSdfProxy
        | EcsDerivedArtifactKind::FoliageCollisionLargeObjectProxy => {
            Some(VoxelPhysicsCookRequest {
                source_page: artifact.source_page,
                dirty_epoch: artifact.artifact_epoch,
                mode,
                requiredness: artifact.requiredness,
                fixed_step_deadline,
            })
        }
        EcsDerivedArtifactKind::TerrainSurfacePackets
        | EcsDerivedArtifactKind::TerrainCoarseProxy
        | EcsDerivedArtifactKind::TerrainSdf
        | EcsDerivedArtifactKind::TerrainMaterialPage
        | EcsDerivedArtifactKind::FoliageClusters
        | EcsDerivedArtifactKind::CanopyOpacity
        | EcsDerivedArtifactKind::FoliageTrunkBranchVirtualGeometry
        | EcsDerivedArtifactKind::FoliageGrassBrushInstanceClusters
        | EcsDerivedArtifactKind::FoliageCardsImpostors
        | EcsDerivedArtifactKind::FoliageClusteredAnimation
        | EcsDerivedArtifactKind::FoliageCanopyTransmittance
        | EcsDerivedArtifactKind::FoliageBiomeTint
        | EcsDerivedArtifactKind::FoliageHorizonImpostorField
        | EcsDerivedArtifactKind::FoliageSdfOpacityShadows
        | EcsDerivedArtifactKind::StormExtinctionDirtyRows
        | EcsDerivedArtifactKind::VirtualShadowInvalidation
        | EcsDerivedArtifactKind::RadianceCacheUpdate
        | EcsDerivedArtifactKind::NavTile
        | EcsDerivedArtifactKind::AudioOcclusionTile
        | EcsDerivedArtifactKind::NetworkDeltaRows
        | EcsDerivedArtifactKind::LoadAnimationRecord
        | EcsDerivedArtifactKind::FoliageSeeds
        | EcsDerivedArtifactKind::ShadowInvalidationRows
        | EcsDerivedArtifactKind::RadianceUpdateRows
        | EcsDerivedArtifactKind::VoxelEditDeltaRows
        | EcsDerivedArtifactKind::NetworkRelevanceRows => None,
    }
}

pub fn publish_physics_cooks(
    artifacts: &[EcsDerivedArtifactRecord],
    mode: CollisionCookMode,
    fixed_step_deadline: Option<FixedStepId>,
    commands: &mut EcsSpatialCommandBuffer,
) -> Result<EcsPhysicsCookPublishReport, EcsSpatialValidationError> {
    let mut report = EcsPhysicsCookPublishReport::default();
    for artifact in artifacts {
        report.inspected += 1;
        if let Some(request) = physics_cook_from_artifact(*artifact, mode, fixed_step_deadline) {
            commands.push(EcsSpatialCommand::PublishPhysicsCook(request))?;
            report.published += 1;
        } else {
            report.skipped_non_physics += 1;
        }
    }
    Ok(report)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsCrossDomainHandoffKind {
    RenderArtifact = 0,
    LuxLightingIntent = 1,
    PhysicsCookRequest = 2,
    NetworkRelevanceRow = 3,
    NavigationTile = 4,
    AudioOcclusionTile = 5,
    TelemetryRow = 6,
    EditorRecord = 7,
    LoadingAnimationCue = 8,
}

impl EcsCrossDomainHandoffKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RenderArtifact => "render_artifact",
            Self::LuxLightingIntent => "lux_lighting_intent",
            Self::PhysicsCookRequest => "physics_cook_request",
            Self::NetworkRelevanceRow => "network_relevance_row",
            Self::NavigationTile => "navigation_tile",
            Self::AudioOcclusionTile => "audio_occlusion_tile",
            Self::TelemetryRow => "telemetry_row",
            Self::EditorRecord => "editor_record",
            Self::LoadingAnimationCue => "loading_animation_cue",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsCrossDomainHandoff {
    pub queue: EcsHandoffQueueId,
    pub kind: EcsCrossDomainHandoffKind,
    pub consumer: EcsArtifactConsumer,
    pub artifact: EcsDerivedArtifactId,
    pub source_page: EcsSpatialPageKey,
    pub chunk_key: EcsChunkKey,
    pub artifact_epoch: u32,
}

impl EcsCrossDomainHandoff {
    #[must_use]
    pub fn new(
        queue: EcsHandoffQueueId,
        kind: EcsCrossDomainHandoffKind,
        consumer: EcsArtifactConsumer,
        artifact: EcsDerivedArtifactId,
        source_page: EcsSpatialPageKey,
        artifact_epoch: u32,
    ) -> Self {
        Self {
            queue,
            kind,
            consumer,
            artifact,
            source_page,
            chunk_key: source_page.chunk_key(),
            artifact_epoch,
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsCrossDomainHandoffQueues {
    pub rows: Vec<EcsCrossDomainHandoff>,
}

impl EcsCrossDomainHandoffQueues {
    pub fn push(&mut self, row: EcsCrossDomainHandoff) -> Result<(), EcsSpatialValidationError> {
        if self.rows.len() >= ECS_SPATIAL_MAX_HANDOFF_ROWS {
            return Err(EcsSpatialValidationError::HandoffQueueFull);
        }
        self.rows.push(row);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RendererArtifactHandoff {
    pub artifact_id: EcsDerivedArtifactId,
    pub source_page: EcsSpatialPageKey,
    pub kind: RendererArtifactHandoffKind,
    pub source_epoch: u32,
    pub source_digest: u64,
    pub artifact_epoch: u32,
    pub requiredness: WorkRequiredness,
    pub visibility_hint: RendererVisibilityHint,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsRendererHandoffQueue {
    pub items: Vec<RendererArtifactHandoff>,
}

impl EcsRendererHandoffQueue {
    pub fn push(&mut self, item: RendererArtifactHandoff) -> Result<(), EcsSpatialValidationError> {
        if self.items.len() >= ECS_SPATIAL_MAX_HANDOFF_ROWS {
            return Err(EcsSpatialValidationError::HandoffQueueFull);
        }
        self.items.push(item);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuxArtifactHandoff {
    pub source_page: EcsSpatialPageKey,
    pub kind: LuxArtifactHandoffKind,
    pub dirty_epoch: u32,
    pub bounds_world: EcsAabbF32,
    pub requiredness: WorkRequiredness,
}

#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct EcsLuxHandoffQueue {
    pub items: Vec<LuxArtifactHandoff>,
}

impl EcsLuxHandoffQueue {
    pub fn push(&mut self, item: LuxArtifactHandoff) -> Result<(), EcsSpatialValidationError> {
        if self.items.len() >= ECS_SPATIAL_MAX_HANDOFF_ROWS {
            return Err(EcsSpatialValidationError::HandoffQueueFull);
        }
        self.items.push(item);
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelPhysicsCookRequest {
    pub source_page: EcsSpatialPageKey,
    pub dirty_epoch: u32,
    pub mode: CollisionCookMode,
    pub requiredness: WorkRequiredness,
    pub fixed_step_deadline: Option<FixedStepId>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsPhysicsCookQueue {
    pub items: Vec<VoxelPhysicsCookRequest>,
}

impl EcsPhysicsCookQueue {
    pub fn push(&mut self, item: VoxelPhysicsCookRequest) -> Result<(), EcsSpatialValidationError> {
        if self.items.len() >= ECS_SPATIAL_MAX_HANDOFF_ROWS {
            return Err(EcsSpatialValidationError::HandoffQueueFull);
        }
        self.items.push(item);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EcsPageChannel, EcsSpatialGridId};

    fn terrain_page() -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            4,
            0,
            0,
            EcsPageChannel::Surface,
        )
    }

    fn artifact_record(
        artifact_id: EcsDerivedArtifactId,
        generation: u32,
        state: EcsArtifactState,
    ) -> EcsDerivedArtifactRecord {
        EcsDerivedArtifactRecord {
            artifact_id,
            source_page: terrain_page(),
            kind: EcsDerivedArtifactKind::TerrainSurfacePackets,
            source_epoch: 2,
            source_digest: derived_artifact_source_digest(terrain_page(), 2, generation),
            artifact_epoch: generation,
            state,
            requiredness: WorkRequiredness::Required,
            consumer: EcsArtifactConsumer::Renderer,
        }
    }

    #[test]
    fn artifact_dag_declares_scheduler_readiness_edges_for_current_producers() {
        let page = terrain_page();
        let systems = [
            EcsDerivedArtifactBuildSystem::BuildTerrainSurfacePackets,
            EcsDerivedArtifactBuildSystem::BuildTerrainSdf,
            EcsDerivedArtifactBuildSystem::BuildCollisionSdfProxy,
            EcsDerivedArtifactBuildSystem::BuildNetworkRelevanceRows,
            EcsDerivedArtifactBuildSystem::BuildLoadAnimationRecord,
        ];
        let dag = ArtifactDag::from_decoded_page(page, 7, 100, &systems).with_node(
            ArtifactNode::renderer_publish(
                ArtifactNodeId::new(20),
                page,
                EcsDerivedArtifactId::new(100),
                7,
            ),
        );

        let plan = dag.build_plan().expect("artifact dag compiles");

        assert_eq!(plan.nodes.len(), systems.len() + 2);
        assert!(
            plan.dependencies
                .iter()
                .any(|dependency| dependency.producer == ArtifactNodeId::new(1)
                    && dependency.consumer == ArtifactNodeId::new(2))
        );
        assert!(plan.readiness_tokens.iter().any(|token| {
            token.artifact_kind == Some(EcsDerivedArtifactKind::TerrainSurfacePackets)
                && token.consumer == ArtifactConsumer::Renderer
                && token.token_kind == EcsCrossDomainWaitTokenKind::VoxelSurfaceArtifactReady
        }));
        assert!(
            plan.readiness_tokens.iter().any(|token| token.consumer
                == ArtifactConsumer::LoadAnimation
                && token.token_kind == EcsCrossDomainWaitTokenKind::VoxelLoadAnimationPublished)
        );
        assert!(
            plan.readiness_tokens.iter().any(|token| token.token_kind
                == EcsCrossDomainWaitTokenKind::VoxelRendererArtifactPublished)
        );
        assert!(ArtifactConsumer::all().contains(&ArtifactConsumer::Rvelte));
    }

    #[test]
    fn rvelte_ui_packet_path_is_modeled_as_artifact_dag_nodes() {
        let layout_output = ArtifactOutput {
            artifact_id: None,
            artifact_kind: None,
            virtual_resource: rvelte_artifact_virtual_resource_key(
                EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady,
                3,
                9,
            ),
            wait_token_kind: EcsCrossDomainWaitTokenKind::RvelteLayoutSnapshotReady,
            generation: 9,
            requiredness: WorkRequiredness::Required,
            consumer: ArtifactConsumer::Rvelte,
        };
        let layout_node = ArtifactNode::new(
            ArtifactNodeId::new(30),
            ArtifactProducer::RvelteUiPacketBuilder,
        )
        .with_output(layout_output)
        .with_execution(
            ScheduleDomain::RvelteUi,
            ScheduleLane::UiDeferredLayout,
            ScheduleDeadline::Frame,
            WorkSplitHint::Unsplittable,
        );
        let dag =
            ArtifactDag::new()
                .with_node(layout_node)
                .with_node(ArtifactNode::rvelte_paint_packet(
                    ArtifactNodeId::new(31),
                    3,
                    9,
                ));

        let plan = dag.build_plan().expect("rvelte dag compiles");

        assert!(
            plan.dependencies
                .iter()
                .any(|dependency| dependency.producer == ArtifactNodeId::new(30)
                    && dependency.consumer == ArtifactNodeId::new(31))
        );
        assert!(plan.readiness_tokens.iter().any(|token| {
            token.token_kind == EcsCrossDomainWaitTokenKind::RvelteRendererUiPacketPublished
                && token.consumer == ArtifactConsumer::Renderer
        }));
    }

    #[test]
    fn missing_required_artifact_producer_rejects_graph() {
        let page = terrain_page();
        let dag = ArtifactDag::new().with_node(ArtifactNode::from_build_system(
            ArtifactNodeId::new(2),
            page,
            EcsDerivedArtifactId::new(11),
            1,
            EcsDerivedArtifactBuildSystem::BuildTerrainSurfacePackets,
        ));

        let err = dag
            .build_plan()
            .expect_err("required decoded page is absent");

        assert!(matches!(
            err,
            ArtifactDagError::MissingRequiredProducer {
                consumer: ArtifactNodeId(2),
                ..
            }
        ));
        assert_eq!(err.label(), "missing_required_producer");
    }

    #[test]
    fn optional_artifact_without_fallback_cannot_gate_present() {
        let page = terrain_page();
        let node = ArtifactNode::new(
            ArtifactNodeId::new(8),
            ArtifactProducer::RendererUploadPublish,
        )
        .with_input(
            ArtifactInput::artifact(
                EcsDerivedArtifactKind::RadianceUpdateRows,
                page,
                3,
                WorkRequiredness::Optional,
            )
            .gate_present(),
        );
        let dag = ArtifactDag::new().with_node(node);

        let err = dag
            .build_plan()
            .expect_err("optional lux refinement cannot gate present");

        assert!(matches!(
            err,
            ArtifactDagError::OptionalPresentGateWithoutFallback {
                consumer: ArtifactNodeId(8),
                artifact_kind: Some(EcsDerivedArtifactKind::RadianceUpdateRows)
            }
        ));
    }

    #[test]
    fn artifact_generation_mismatch_rejects_consumer_read() {
        let artifact_id = EcsDerivedArtifactId::new(41);
        let mut registry = EcsDerivedArtifactRegistry::default();
        registry
            .push(artifact_record(artifact_id, 5, EcsArtifactState::Ready))
            .expect("record insert");

        let err = ArtifactDag::validate_consumer_read(
            &registry,
            ArtifactConsumerRead::new(artifact_id, 6, ArtifactConsumer::Renderer),
        )
        .expect_err("generation mismatch rejects read");

        assert!(matches!(
            err,
            ArtifactDagError::ArtifactGenerationMismatch {
                artifact_id: EcsDerivedArtifactId(41),
                expected: 6,
                actual: 5
            }
        ));
    }

    #[test]
    fn retired_artifact_requires_fallback_for_consumer_read() {
        let artifact_id = EcsDerivedArtifactId::new(42);
        let mut registry = EcsDerivedArtifactRegistry::default();
        registry
            .push(artifact_record(artifact_id, 5, EcsArtifactState::Retiring))
            .expect("record insert");

        let err = ArtifactDag::validate_consumer_read(
            &registry,
            ArtifactConsumerRead::new(artifact_id, 5, ArtifactConsumer::Renderer),
        )
        .expect_err("retired artifact cannot be consumed directly");
        assert!(matches!(
            err,
            ArtifactDagError::RetiredArtifactConsumedWithoutFallback {
                artifact_id: EcsDerivedArtifactId(42)
            }
        ));

        let decision = ArtifactDag::validate_consumer_read(
            &registry,
            ArtifactConsumerRead::new(artifact_id, 5, ArtifactConsumer::Renderer)
                .with_fallback(ArtifactFallbackPolicy::UsePreviousGeneration),
        )
        .expect("fallback is allowed");

        assert_eq!(
            decision,
            ArtifactConsumerReadDecision::UseFallback(
                ArtifactFallbackPolicy::UsePreviousGeneration
            )
        );
        assert_eq!(
            ArtifactRetirePlan::from_registry(&registry).artifacts,
            vec![artifact_id]
        );
    }
}
