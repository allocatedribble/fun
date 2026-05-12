use bevy_ecs::prelude::Resource;
use fun_scheduler_types::{EcsChunkKey, WorkRequiredness};

use crate::{
    DenseSlotMap, ECS_SPATIAL_MAX_DERIVED_ARTIFACTS, ECS_SPATIAL_MAX_HANDOFF_ROWS, EcsAabbF32,
    EcsDecodedPagePayloadKind, EcsDecodedPageRecord, EcsDerivedArtifactId, EcsHandoffQueueId,
    EcsSpatialCommand, EcsSpatialCommandBuffer, EcsSpatialPageKey, EcsSpatialValidationError,
    FixedStepId, TerrainMaterialPaletteId, VoxelCollisionPolicy,
};

pub const ECS_DERIVED_ARTIFACT_BUILD_SYSTEM_COUNT: usize = 24;

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

impl EcsDerivedArtifactBuildSystem {
    #[must_use]
    pub const fn all() -> &'static [Self] {
        &ECS_DERIVED_ARTIFACT_BUILD_SYSTEMS
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
            | Self::BuildFoliageSeeds
            | Self::BuildFoliageClusters
            | Self::BuildCanopyOpacity
            | Self::BuildShadowInvalidationRows
            | Self::BuildRadianceUpdateRows
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
            | Self::BuildTerrainSdf
            | Self::BuildTerrainMaterialPage
            | Self::BuildPhysicsCookRequests
            | Self::BuildCollisionSdfProxy
            | Self::BuildVoxelEditDeltaRows
            | Self::BuildNetworkRelevanceRows => WorkRequiredness::Required,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsDerivedArtifactRecord {
    pub artifact_id: EcsDerivedArtifactId,
    pub source_page: EcsSpatialPageKey,
    pub kind: EcsDerivedArtifactKind,
    pub source_epoch: u32,
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

    #[must_use]
    pub fn is_consistent(&self) -> bool {
        self.artifacts.is_consistent()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct EcsArtifactBuildReport {
    pub decoded_pages: u32,
    pub artifacts_published: u32,
    pub skipped_failed_pages: u32,
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
        for system in systems {
            *next_artifact_id = next_artifact_id.saturating_add(1).max(1);
            let artifact = EcsDerivedArtifactRecord {
                artifact_id: EcsDerivedArtifactId::new(*next_artifact_id),
                source_page: page.key,
                kind: system.artifact_kind(),
                source_epoch: page.source_epoch,
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
