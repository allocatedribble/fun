use bevy_ecs::prelude::Resource;

use crate::{
    BiomeSourceId, EcsDecodeTelemetry, EcsDecodedPagePayloadKind, EcsDecodedPageRecord,
    EcsPageChannel, EcsPageChannelMask, EcsPageFailureCode, EcsProceduralRecipeRef,
    EcsSourceChecksum, EcsSourceChecksumAlgorithm, EcsSourceFailure, EcsSourcePayload,
    EcsSourceRequest, EcsSourceRequestId, EcsSpatialDomainKind, EcsSpatialGridId,
    EcsSpatialPageKey, EcsSpatialRegionKey, EcsSpatialRegionManifest, EcsSpatialSource,
    EcsSpatialSourceId, EcsSpatialSourceKind, EcsSpatialValidationError, EcsStreamPriority, IVec3,
    NetworkPlayerId, PageHeightRelation, ProceduralBiomeDesc, ProceduralFeatureDesc,
    ProceduralGeneratedPageClass, ProceduralGenerationBudget, ProceduralMaterialDesc,
    ProceduralTerrainGeneratorDesc, ProceduralTerrainProfileId, ProceduralTerrainShapeDesc,
    TerrainMaterialId, VOXEL_BRICK_EDGE_CELLS, VOXEL_BRICK_FOOT_CELL_COUNT,
    VOXEL_BRICK_OCCUPANCY_WORDS, VOXEL_CELL_EDGE_UM, VOXEL_CLUSTER_EDGE_CELLS,
    VOXEL_CLUSTER_SUMMARIES_PER_BRICK, VOXEL_CLUSTERS_PER_BRICK_AXIS, VoxelBrickPayload,
    VoxelClusterSummary, VoxelMaterialPalette, VoxelOccupancyStorage, VoxelOccupancyStorageKind,
    VoxelPagePayloadKind, fbm2_q16, value_noise2_q16,
};

pub const ECS_PROCEDURAL_TERRAIN_SCHEMA_VERSION: u16 = 1;
pub const ECS_PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1: EcsTerrainGeneratorVersion =
    EcsTerrainGeneratorVersion(1);
pub const ECS_PROCEDURAL_TERRAIN_DEFAULT_REGION_EDGE_PAGES: u16 = 8;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const PAGE_EDGE_FT: i64 = VOXEL_BRICK_EDGE_CELLS as i64;
const CLUSTER_EDGE_FT: usize = VOXEL_CLUSTER_EDGE_CELLS as usize;
const CLUSTERS_PER_BRICK_AXIS: usize = VOXEL_CLUSTERS_PER_BRICK_AXIS;
const CLUSTER_FULL_POPCOUNT: u16 = VOXEL_CLUSTER_EDGE_CELLS as u16
    * VOXEL_CLUSTER_EDGE_CELLS as u16
    * VOXEL_CLUSTER_EDGE_CELLS as u16;
const FIXED_ONE_Q16: u32 = 65_536;
const COLUMN_FEATURE_RIDGE: u8 = 1 << 0;
const COLUMN_FEATURE_CLIFF: u8 = 1 << 1;
const PROCEDURAL_MATERIAL_INDEX_AIR: u8 = 0;
const PROCEDURAL_MATERIAL_INDEX_GRASS: u8 = 1;
const PROCEDURAL_MATERIAL_INDEX_DIRT: u8 = 2;
const PROCEDURAL_MATERIAL_INDEX_ROCK: u8 = 3;
const PROCEDURAL_MATERIAL_INDEX_SNOW: u8 = 4;
const PROCEDURAL_MATERIAL_INDEX_SAND: u8 = 5;
const PROCEDURAL_MATERIAL_INDEX_COUNT: usize = 6;
const PROCEDURAL_TINY_PALETTE_LIMIT: u16 = 8;
const PROCEDURAL_SURFACE_FACE_POS_Y: u8 = 1 << 3;
const CLUSTER_FLAG_SURFACE: u16 = 1 << 0;
const CLUSTER_FLAG_BEDROCK: u16 = 1 << 1;
const CLUSTER_FLAG_WATER_DEBUG: u16 = 1 << 2;
const CLUSTER_FLAG_RIDGE: u16 = 1 << 3;
const CLUSTER_FLAG_CLIFF: u16 = 1 << 4;
const CLUSTER_FLAG_STRATA_MIXED: u16 = 1 << 5;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EcsTerrainGeneratorVersion(pub u32);

impl EcsTerrainGeneratorVersion {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }

    #[must_use]
    pub const fn is_supported(self) -> bool {
        self.0 == ECS_PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1.0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum WorldOriginPolicy {
    #[default]
    PageKeyOrigin = 0,
}

impl WorldOriginPolicy {
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::PageKeyOrigin)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DeterministicMathMode {
    #[default]
    FixedPointQ16ValueNoise = 1,
}

impl DeterministicMathMode {
    #[must_use]
    pub const fn is_supported(self) -> bool {
        matches!(self, Self::FixedPointQ16ValueNoise)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProceduralFeature {
    FootVoxelCells = 0,
    MaterialPalette = 1,
    ClusterSummaries = 2,
    RidgeCliffStrata = 3,
    DebugWaterPlaneMaterial = 4,
    FoliageHints = 5,
}

impl ProceduralFeature {
    #[must_use]
    pub const fn bit(self) -> u64 {
        1_u64 << (self as u8)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralFeatureMask {
    bits: u64,
}

impl ProceduralFeatureMask {
    pub const EMPTY: Self = Self { bits: 0 };
    pub const BASE_TERRAIN: Self = Self {
        bits: ProceduralFeature::FootVoxelCells.bit()
            | ProceduralFeature::MaterialPalette.bit()
            | ProceduralFeature::ClusterSummaries.bit()
            | ProceduralFeature::RidgeCliffStrata.bit(),
    };

    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self { bits }
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.bits
    }

    #[must_use]
    pub const fn contains(self, feature: ProceduralFeature) -> bool {
        (self.bits & feature.bit()) != 0
    }

    #[must_use]
    pub const fn has_required_generation_bits(self) -> bool {
        self.contains(ProceduralFeature::FootVoxelCells)
            && self.contains(ProceduralFeature::MaterialPalette)
            && self.contains(ProceduralFeature::ClusterSummaries)
    }
}

impl Default for ProceduralFeatureMask {
    fn default() -> Self {
        Self::BASE_TERRAIN
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralWorldAuthorityPolicy {
    pub server_world_manifest: bool,
    pub server_generator_version: bool,
    pub server_world_seed: bool,
    pub server_player_spawn_positions: bool,
    pub server_future_delta_layer: bool,
    pub client_streaming_priority: bool,
    pub client_renderer_artifact_realization: bool,
    pub client_cache_eviction: bool,
    pub client_debug_profiling_display: bool,
}

impl ProceduralWorldAuthorityPolicy {
    pub const SERVER_AUTHORITY_V1: Self = Self {
        server_world_manifest: true,
        server_generator_version: true,
        server_world_seed: true,
        server_player_spawn_positions: true,
        server_future_delta_layer: true,
        client_streaming_priority: true,
        client_renderer_artifact_realization: true,
        client_cache_eviction: true,
        client_debug_profiling_display: true,
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.server_world_manifest
            && self.server_generator_version
            && self.server_world_seed
            && self.server_player_spawn_positions
            && self.server_future_delta_layer
            && self.client_streaming_priority
            && self.client_renderer_artifact_realization
            && self.client_cache_eviction
            && self.client_debug_profiling_display
        {
            Ok(())
        } else {
            Err(EcsSpatialValidationError::InvalidProceduralSyncManifest)
        }
    }
}

impl Default for ProceduralWorldAuthorityPolicy {
    fn default() -> Self {
        Self::SERVER_AUTHORITY_V1
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsBiomeRecipe {
    pub biome: BiomeSourceId,
    pub base_height_ft: i32,
    pub amplitude_ft: u16,
    pub detail_amplitude_ft: u16,
    pub macro_period_ft: u16,
    pub detail_period_ft: u16,
    pub terrace_step_ft: u8,
    pub surface_depth_ft: u8,
    pub bedrock_depth_ft: u16,
    pub water_level_ft: i32,
    pub surface_material: TerrainMaterialId,
    pub subsurface_material: TerrainMaterialId,
    pub bedrock_material: TerrainMaterialId,
    pub water_material: TerrainMaterialId,
    pub foliage_density_q: u8,
}

impl EcsBiomeRecipe {
    pub const BEDROCK_QUARRY: Self = Self {
        biome: BiomeSourceId::new(1),
        base_height_ft: 38,
        amplitude_ft: 18,
        detail_amplitude_ft: 5,
        macro_period_ft: 192,
        detail_period_ft: 48,
        terrace_step_ft: 3,
        surface_depth_ft: 3,
        bedrock_depth_ft: 24,
        water_level_ft: -4096,
        surface_material: TerrainMaterialId::new(1),
        subsurface_material: TerrainMaterialId::new(2),
        bedrock_material: TerrainMaterialId::new(3),
        water_material: TerrainMaterialId::new(4),
        foliage_density_q: 0,
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.biome.get() == 0
            || self.macro_period_ft == 0
            || self.detail_period_ft == 0
            || self.surface_depth_ft == 0
            || self.bedrock_depth_ft <= u16::from(self.surface_depth_ft)
        {
            return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
        }
        material_to_u16(self.surface_material)?;
        material_to_u16(self.subsurface_material)?;
        material_to_u16(self.bedrock_material)?;
        material_to_u16(self.water_material)?;
        Ok(())
    }

    #[must_use]
    pub fn terrain_profile_id(self) -> ProceduralTerrainProfileId {
        ProceduralTerrainProfileId::new(self.biome.get())
    }

    #[must_use]
    pub fn biome_table_digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_u64(hash, self.biome.get());
        hash = hash_i32(hash, self.base_height_ft);
        hash = hash_u16(hash, self.amplitude_ft);
        hash = hash_u16(hash, self.detail_amplitude_ft);
        hash = hash_u16(hash, self.macro_period_ft);
        hash = hash_u16(hash, self.detail_period_ft);
        hash = hash_u8(hash, self.terrace_step_ft);
        hash = hash_u8(hash, self.surface_depth_ft);
        hash = hash_u16(hash, self.bedrock_depth_ft);
        hash = hash_i32(hash, self.water_level_ft);
        hash = hash_u8(hash, self.foliage_density_q);
        hash.max(1)
    }

    #[must_use]
    pub fn material_table_digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_u32(hash, self.surface_material.get());
        hash = hash_u32(hash, self.subsurface_material.get());
        hash = hash_u32(hash, self.bedrock_material.get());
        hash = hash_u32(hash, self.water_material.get());
        hash.max(1)
    }

    #[must_use]
    pub fn signature(self) -> EcsSourceChecksum {
        let mut hash = FNV_OFFSET;
        hash = hash_u64(hash, self.biome.get());
        hash = hash_i32(hash, self.base_height_ft);
        hash = hash_u16(hash, self.amplitude_ft);
        hash = hash_u16(hash, self.detail_amplitude_ft);
        hash = hash_u16(hash, self.macro_period_ft);
        hash = hash_u16(hash, self.detail_period_ft);
        hash = hash_u8(hash, self.terrace_step_ft);
        hash = hash_u8(hash, self.surface_depth_ft);
        hash = hash_u16(hash, self.bedrock_depth_ft);
        hash = hash_i32(hash, self.water_level_ft);
        hash = hash_u32(hash, self.surface_material.get());
        hash = hash_u32(hash, self.subsurface_material.get());
        hash = hash_u32(hash, self.bedrock_material.get());
        hash = hash_u32(hash, self.water_material.get());
        hash = hash_u8(hash, self.foliage_density_q);
        EcsSourceChecksum {
            algorithm: EcsSourceChecksumAlgorithm::Fnv1a64,
            value: hash.max(1),
        }
    }
}

impl Default for EcsBiomeRecipe {
    fn default() -> Self {
        Self::BEDROCK_QUARRY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct EcsProceduralWorldManifest {
    pub schema_version: u16,
    pub world_seed: u64,
    pub generator_version: EcsTerrainGeneratorVersion,
    pub terrain_grid: EcsSpatialGridId,
    pub region_edge_pages: u16,
    pub channel_mask: EcsPageChannelMask,
    pub biome_recipe: EcsBiomeRecipe,
}

impl EcsProceduralWorldManifest {
    pub const BEDROCK_QUARRY: Self = Self {
        schema_version: ECS_PROCEDURAL_TERRAIN_SCHEMA_VERSION,
        world_seed: 0x9e37_79b9_7f4a_7c15,
        generator_version: ECS_PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1,
        terrain_grid: EcsSpatialGridId::new(1),
        region_edge_pages: ECS_PROCEDURAL_TERRAIN_DEFAULT_REGION_EDGE_PAGES,
        channel_mask: EcsPageChannelMask::terrain_primary(),
        biome_recipe: EcsBiomeRecipe::BEDROCK_QUARRY,
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.schema_version != ECS_PROCEDURAL_TERRAIN_SCHEMA_VERSION {
            return Err(EcsSpatialValidationError::InvalidProceduralManifest);
        }
        if !self.generator_version.is_supported() {
            return Err(EcsSpatialValidationError::UnsupportedProceduralVersion);
        }
        if self.terrain_grid.get() == 0 || self.region_edge_pages == 0 {
            return Err(EcsSpatialValidationError::InvalidProceduralManifest);
        }
        if !self.channel_mask.contains(EcsPageChannel::Occupancy)
            || !self.channel_mask.contains(EcsPageChannel::Material)
            || !self.channel_mask.contains(EcsPageChannel::Surface)
        {
            return Err(EcsSpatialValidationError::InvalidProceduralManifest);
        }
        self.biome_recipe.validate()?;
        self.generator_desc().validate()
    }

    #[must_use]
    pub fn signature(self) -> EcsSourceChecksum {
        let mut hash = FNV_OFFSET;
        hash = hash_u16(hash, self.schema_version);
        hash = hash_u64(hash, self.world_seed);
        hash = hash_u32(hash, self.generator_version.get());
        hash = hash_u64(hash, self.terrain_grid.get());
        hash = hash_u16(hash, self.region_edge_pages);
        hash = hash_u32(hash, self.channel_mask.bits());
        hash = hash_u64(hash, self.biome_recipe.signature().value);
        hash = hash_u64(hash, self.generator_desc().desc_digest());
        EcsSourceChecksum {
            algorithm: EcsSourceChecksumAlgorithm::Fnv1a64,
            value: hash.max(1),
        }
    }

    #[must_use]
    pub fn source_epoch(self) -> u32 {
        let low = self.signature().value as u32;
        low.max(self.generator_version.get()).max(1)
    }

    #[must_use]
    pub fn accepts_page(self, key: EcsSpatialPageKey) -> bool {
        key.domain == EcsSpatialDomainKind::Terrain
            && key.grid_id == self.terrain_grid
            && self.channel_mask.contains(key.channel)
    }

    #[must_use]
    pub fn recipe_ref_for_page(self, key: EcsSpatialPageKey) -> EcsProceduralRecipeRef {
        self.generator_desc().page_recipe(key)
    }

    #[must_use]
    pub fn page_checksum(self, key: EcsSpatialPageKey) -> EcsSourceChecksum {
        EcsSourceChecksum {
            algorithm: EcsSourceChecksumAlgorithm::Fnv1a64,
            value: self.recipe_ref_for_page(key).checksum_value(),
        }
    }

    #[must_use]
    pub fn region_checksum(self, region: EcsSpatialRegionKey) -> EcsSourceChecksum {
        let mut hash = self.signature().value;
        hash = hash_u8(hash, region.domain as u8);
        hash = hash_u64(hash, region.grid_id.get());
        hash = hash_u8(hash, region.level);
        hash = hash_i32(hash, region.x);
        hash = hash_i32(hash, region.y);
        hash = hash_i32(hash, region.z);
        EcsSourceChecksum {
            algorithm: EcsSourceChecksumAlgorithm::Fnv1a64,
            value: hash.max(1),
        }
    }

    #[must_use]
    pub fn sync_manifest(self) -> ProceduralWorldSyncManifest {
        ProceduralWorldSyncManifest::from_world_manifest(self)
    }

    #[must_use]
    pub fn generator_desc(self) -> ProceduralTerrainGeneratorDesc {
        ProceduralTerrainGeneratorDesc {
            schema_version: self.schema_version,
            generator_version: self.generator_version.get(),
            world_seed: self.world_seed,
            grid_id: self.terrain_grid,
            voxel_edge_um: VOXEL_CELL_EDGE_UM,
            page_edge_voxels: u16::from(VOXEL_BRICK_EDGE_CELLS),
            terrain: ProceduralTerrainShapeDesc {
                base_height_ft: self.biome_recipe.base_height_ft,
                height_amplitude_ft: i32::from(self.biome_recipe.amplitude_ft),
                continental_frequency_q16: period_to_frequency_q16(
                    self.biome_recipe.macro_period_ft,
                ),
                hill_frequency_q16: period_to_frequency_q16(self.biome_recipe.detail_period_ft),
                ridge_frequency_q16: period_to_frequency_q16(ridge_period_ft(
                    self.biome_recipe.macro_period_ft,
                )),
                warp_frequency_q16: period_to_frequency_q16(warp_period_ft(
                    self.biome_recipe.macro_period_ft,
                )),
                octaves: ProceduralTerrainShapeDesc::BEDROCK_QUARRY.octaves,
                ridge_strength_q16: ProceduralTerrainShapeDesc::BEDROCK_QUARRY.ridge_strength_q16,
                cliff_threshold_q16: ProceduralTerrainShapeDesc::BEDROCK_QUARRY.cliff_threshold_q16,
                erosion_hint_q16: ProceduralTerrainShapeDesc::BEDROCK_QUARRY.erosion_hint_q16,
            },
            biome: ProceduralBiomeDesc {
                profile: self.biome_recipe.terrain_profile_id(),
                biome: self.biome_recipe.biome,
                surface_depth_ft: self.biome_recipe.surface_depth_ft,
                bedrock_depth_ft: self.biome_recipe.bedrock_depth_ft,
                water_level_ft: self.biome_recipe.water_level_ft,
            },
            materials: ProceduralMaterialDesc {
                grass: self.biome_recipe.surface_material,
                dirt: self.biome_recipe.subsurface_material,
                rock: self.biome_recipe.bedrock_material,
                sand: self.biome_recipe.water_material,
                snow: ProceduralMaterialDesc::BEDROCK_QUARRY.snow,
                debug_magenta: ProceduralMaterialDesc::BEDROCK_QUARRY.debug_magenta,
            },
            features: ProceduralFeatureDesc::BEDROCK_QUARRY,
            budgets: ProceduralGenerationBudget::BEDROCK_QUARRY,
        }
    }
}

impl Default for EcsProceduralWorldManifest {
    fn default() -> Self {
        Self::BEDROCK_QUARRY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct ProceduralWorldSyncManifest {
    pub schema_version: u16,
    pub generator_version: u32,
    pub world_seed: u64,
    pub terrain_profile: ProceduralTerrainProfileId,
    pub biome_table_digest: u64,
    pub material_table_digest: u64,
    pub world_origin_policy: WorldOriginPolicy,
    pub voxel_edge_um: u32,
    pub page_edge_voxels: u16,
    pub region_edge_pages: u16,
    pub deterministic_math_mode: DeterministicMathMode,
    pub enabled_features: ProceduralFeatureMask,
}

impl ProceduralWorldSyncManifest {
    #[must_use]
    pub fn from_world_manifest(manifest: EcsProceduralWorldManifest) -> Self {
        Self {
            schema_version: manifest.schema_version,
            generator_version: manifest.generator_version.get(),
            world_seed: manifest.world_seed,
            terrain_profile: manifest.biome_recipe.terrain_profile_id(),
            biome_table_digest: manifest.biome_recipe.biome_table_digest(),
            material_table_digest: manifest.biome_recipe.material_table_digest(),
            world_origin_policy: WorldOriginPolicy::PageKeyOrigin,
            voxel_edge_um: VOXEL_CELL_EDGE_UM,
            page_edge_voxels: u16::from(VOXEL_BRICK_EDGE_CELLS),
            region_edge_pages: manifest.region_edge_pages,
            deterministic_math_mode: DeterministicMathMode::FixedPointQ16ValueNoise,
            enabled_features: ProceduralFeatureMask::BASE_TERRAIN,
        }
    }

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.schema_version != ECS_PROCEDURAL_TERRAIN_SCHEMA_VERSION
            || self.generator_version == 0
            || !EcsTerrainGeneratorVersion::new(self.generator_version).is_supported()
            || !self.terrain_profile.is_valid()
            || self.biome_table_digest == 0
            || self.material_table_digest == 0
            || !self.world_origin_policy.is_supported()
            || self.voxel_edge_um != VOXEL_CELL_EDGE_UM
            || self.page_edge_voxels != u16::from(VOXEL_BRICK_EDGE_CELLS)
            || self.region_edge_pages == 0
            || !self.deterministic_math_mode.is_supported()
            || !self.enabled_features.has_required_generation_bits()
        {
            return Err(EcsSpatialValidationError::InvalidProceduralSyncManifest);
        }
        Ok(())
    }

    #[must_use]
    pub fn manifest_digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_u16(hash, self.schema_version);
        hash = hash_u32(hash, self.generator_version);
        hash = hash_u64(hash, self.world_seed);
        hash = hash_u64(hash, self.terrain_profile.get());
        hash = hash_u64(hash, self.biome_table_digest);
        hash = hash_u64(hash, self.material_table_digest);
        hash = hash_u8(hash, self.world_origin_policy as u8);
        hash = hash_u32(hash, self.voxel_edge_um);
        hash = hash_u16(hash, self.page_edge_voxels);
        hash = hash_u16(hash, self.region_edge_pages);
        hash = hash_u8(hash, self.deterministic_math_mode as u8);
        hash = hash_u64(hash, self.enabled_features.bits());
        hash.max(1)
    }

    pub fn validate_for_client_handshake(
        self,
        expected: ProceduralWorldHandshakeClientExpectation,
    ) -> Result<(), EcsSpatialValidationError> {
        self.validate()?;
        expected.validate()?;
        if self.biome_table_digest != expected.biome_table_digest
            || self.material_table_digest != expected.material_table_digest
            || self.voxel_edge_um != expected.voxel_edge_um
            || self.page_edge_voxels != expected.page_edge_voxels
            || self.deterministic_math_mode != expected.deterministic_math_mode
        {
            return Err(EcsSpatialValidationError::InvalidProceduralSyncManifest);
        }
        Ok(())
    }

    pub fn to_world_manifest(
        self,
        terrain_grid: EcsSpatialGridId,
        channel_mask: EcsPageChannelMask,
        biome_recipe: EcsBiomeRecipe,
    ) -> Result<EcsProceduralWorldManifest, EcsSpatialValidationError> {
        self.validate()?;
        if self.terrain_profile != biome_recipe.terrain_profile_id()
            || self.biome_table_digest != biome_recipe.biome_table_digest()
            || self.material_table_digest != biome_recipe.material_table_digest()
        {
            return Err(EcsSpatialValidationError::InvalidProceduralSyncManifest);
        }
        let manifest = EcsProceduralWorldManifest {
            schema_version: self.schema_version,
            world_seed: self.world_seed,
            generator_version: EcsTerrainGeneratorVersion::new(self.generator_version),
            terrain_grid,
            region_edge_pages: self.region_edge_pages,
            channel_mask,
            biome_recipe,
        };
        manifest.validate()?;
        Ok(manifest)
    }
}

impl From<EcsProceduralWorldManifest> for ProceduralWorldSyncManifest {
    fn from(value: EcsProceduralWorldManifest) -> Self {
        Self::from_world_manifest(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralWorldHandshake {
    pub world_manifest: ProceduralWorldSyncManifest,
    pub initial_region_seed_table_digest: u64,
    pub spawn_position_ft: IVec3,
    pub server_tick: u64,
}

impl ProceduralWorldHandshake {
    #[must_use]
    pub fn from_world_manifest(
        manifest: EcsProceduralWorldManifest,
        spawn_position_ft: IVec3,
        server_tick: u64,
    ) -> Self {
        Self {
            world_manifest: manifest.sync_manifest(),
            initial_region_seed_table_digest: initial_region_seed_table_digest(
                manifest.world_seed,
                manifest.terrain_grid,
                manifest.region_edge_pages,
                spawn_position_ft,
            ),
            spawn_position_ft,
            server_tick,
        }
    }

    pub fn validate_for_client(
        self,
        expected: ProceduralWorldHandshakeClientExpectation,
    ) -> Result<(), EcsSpatialValidationError> {
        self.world_manifest
            .validate_for_client_handshake(expected)?;
        let expected_region_seed_digest = initial_region_seed_table_digest(
            self.world_manifest.world_seed,
            expected.terrain_grid,
            self.world_manifest.region_edge_pages,
            self.spawn_position_ft,
        );
        if self.initial_region_seed_table_digest == 0
            || self.initial_region_seed_table_digest != expected_region_seed_digest
        {
            return Err(EcsSpatialValidationError::InvalidProceduralSyncManifest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralWorldHandshakeClientExpectation {
    pub terrain_grid: EcsSpatialGridId,
    pub biome_table_digest: u64,
    pub material_table_digest: u64,
    pub voxel_edge_um: u32,
    pub page_edge_voxels: u16,
    pub deterministic_math_mode: DeterministicMathMode,
}

impl ProceduralWorldHandshakeClientExpectation {
    #[must_use]
    pub fn from_world_manifest(manifest: EcsProceduralWorldManifest) -> Self {
        Self {
            terrain_grid: manifest.terrain_grid,
            biome_table_digest: manifest.biome_recipe.biome_table_digest(),
            material_table_digest: manifest.biome_recipe.material_table_digest(),
            voxel_edge_um: VOXEL_CELL_EDGE_UM,
            page_edge_voxels: u16::from(VOXEL_BRICK_EDGE_CELLS),
            deterministic_math_mode: DeterministicMathMode::FixedPointQ16ValueNoise,
        }
    }

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.terrain_grid.get() == 0
            || self.biome_table_digest == 0
            || self.material_table_digest == 0
            || self.voxel_edge_um != VOXEL_CELL_EDGE_UM
            || self.page_edge_voxels != u16::from(VOXEL_BRICK_EDGE_CELLS)
            || !self.deterministic_math_mode.is_supported()
        {
            return Err(EcsSpatialValidationError::InvalidProceduralSyncManifest);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralTerrainDeltaLayerHeader {
    pub base_manifest_digest: u64,
    pub delta_epoch: u32,
    pub delta_digest: u64,
}

impl ProceduralTerrainDeltaLayerHeader {
    #[must_use]
    pub fn empty_for_manifest(manifest: ProceduralWorldSyncManifest) -> Self {
        let base_manifest_digest = manifest.manifest_digest();
        Self {
            base_manifest_digest,
            delta_epoch: 0,
            delta_digest: empty_delta_digest(base_manifest_digest),
        }
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.delta_epoch == 0
    }

    pub fn validate_for_manifest(
        self,
        manifest: ProceduralWorldSyncManifest,
    ) -> Result<(), EcsSpatialValidationError> {
        manifest.validate()?;
        let base_manifest_digest = manifest.manifest_digest();
        if self.base_manifest_digest != base_manifest_digest
            || self.delta_epoch != 0
            || self.delta_digest != empty_delta_digest(base_manifest_digest)
        {
            return Err(EcsSpatialValidationError::InvalidProceduralSyncManifest);
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralPageDigest {
    pub generator_version: u32,
    pub page_key_hash: u64,
    pub occupancy_digest: u64,
    pub material_digest: u64,
    pub cluster_summary_digest: u64,
    pub combined_digest: u64,
}

impl ProceduralPageDigest {
    pub fn from_decoded_page(
        manifest: EcsProceduralWorldManifest,
        page: &EcsDecodedPageRecord,
    ) -> Result<Self, EcsSpatialValidationError> {
        let brick = page
            .voxel_brick
            .as_ref()
            .ok_or(EcsSpatialValidationError::DecodeMissingPayload)?;
        if page.key != brick.key {
            return Err(EcsSpatialValidationError::SourceRequestMismatch);
        }
        Self::from_voxel_brick(manifest, brick)
    }

    pub fn from_voxel_brick(
        manifest: EcsProceduralWorldManifest,
        brick: &VoxelBrickPayload,
    ) -> Result<Self, EcsSpatialValidationError> {
        manifest.validate()?;
        if !manifest.accepts_page(brick.key) {
            return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
        }
        let page_key_hash = page_key_digest(brick.key);
        let occupancy_digest = occupancy_storage_digest(&brick.occupancy);
        let material_digest = material_palette_digest(&brick.material_palette);
        let cluster_summary_digest = cluster_summaries_digest(&brick.clusters);
        let mut combined_digest = FNV_OFFSET;
        combined_digest = hash_u32(combined_digest, manifest.generator_version.get());
        combined_digest = hash_u64(combined_digest, manifest.signature().value);
        combined_digest = hash_u64(combined_digest, page_key_hash);
        combined_digest = hash_u64(combined_digest, occupancy_digest);
        combined_digest = hash_u64(combined_digest, material_digest);
        combined_digest = hash_u64(combined_digest, cluster_summary_digest);
        Ok(Self {
            generator_version: manifest.generator_version.get(),
            page_key_hash,
            occupancy_digest,
            material_digest,
            cluster_summary_digest,
            combined_digest: combined_digest.max(1),
        })
    }

    pub fn validate_for_page(
        self,
        manifest: EcsProceduralWorldManifest,
        page: EcsSpatialPageKey,
    ) -> Result<(), EcsSpatialValidationError> {
        manifest.validate()?;
        if self.generator_version != manifest.generator_version.get()
            || self.page_key_hash != page_key_digest(page)
            || self.occupancy_digest == 0
            || self.material_digest == 0
            || self.cluster_summary_digest == 0
            || self.combined_digest == 0
        {
            return Err(EcsSpatialValidationError::ProceduralPageDigestMismatch);
        }
        Ok(())
    }
}

pub fn generate_procedural_page_digest(
    manifest: EcsProceduralWorldManifest,
    page: EcsSpatialPageKey,
) -> Result<ProceduralPageDigest, EcsSpatialValidationError> {
    manifest.validate()?;
    if !manifest.accepts_page(page) {
        return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
    }
    let brick = generate_voxel_brick(manifest, page, manifest.source_epoch())?;
    ProceduralPageDigest::from_voxel_brick(manifest, &brick)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralPageDigestSample {
    pub player_id: NetworkPlayerId,
    pub page: EcsSpatialPageKey,
    pub server_digest: ProceduralPageDigest,
    pub server_tick: u64,
}

impl ProceduralPageDigestSample {
    #[must_use]
    pub const fn new(
        player_id: NetworkPlayerId,
        page: EcsSpatialPageKey,
        server_digest: ProceduralPageDigest,
        server_tick: u64,
    ) -> Self {
        Self {
            player_id,
            page,
            server_digest,
            server_tick,
        }
    }

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if !self.player_id.is_valid()
            || self.server_digest.page_key_hash != page_key_digest(self.page)
            || self.server_digest.combined_digest == 0
        {
            return Err(EcsSpatialValidationError::InvalidProceduralDigestProbe);
        }
        Ok(())
    }

    #[must_use]
    pub fn into_probe(self, client_digest: ProceduralPageDigest) -> ProceduralPageDigestProbe {
        ProceduralPageDigestProbe::new(self.player_id, self.page, self.server_digest, client_digest)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralPageDigestProbe {
    pub player_id: NetworkPlayerId,
    pub page: EcsSpatialPageKey,
    pub server_digest: ProceduralPageDigest,
    pub client_digest: ProceduralPageDigest,
    pub matched: bool,
}

impl ProceduralPageDigestProbe {
    #[must_use]
    pub fn new(
        player_id: NetworkPlayerId,
        page: EcsSpatialPageKey,
        server_digest: ProceduralPageDigest,
        client_digest: ProceduralPageDigest,
    ) -> Self {
        Self {
            player_id,
            page,
            server_digest,
            client_digest,
            matched: server_digest == client_digest,
        }
    }

    #[must_use]
    pub const fn should_report_to_server(self) -> bool {
        !self.matched
    }

    #[must_use]
    pub fn mismatch_report(self, client_tick: u64) -> Option<ProceduralPageDigestMismatchReport> {
        self.should_report_to_server()
            .then_some(ProceduralPageDigestMismatchReport {
                player_id: self.player_id,
                page: self.page,
                server_digest: self.server_digest,
                client_digest: self.client_digest,
                client_tick,
            })
    }

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if !self.player_id.is_valid() {
            return Err(EcsSpatialValidationError::InvalidProceduralDigestProbe);
        }
        let expected_page_hash = page_key_digest(self.page);
        if self.server_digest.page_key_hash != expected_page_hash
            || self.client_digest.page_key_hash != expected_page_hash
        {
            return Err(EcsSpatialValidationError::ProceduralPageDigestMismatch);
        }
        if self.matched != (self.server_digest == self.client_digest) {
            return Err(EcsSpatialValidationError::InvalidProceduralDigestProbe);
        }
        if !self.matched {
            return Err(EcsSpatialValidationError::ProceduralPageDigestMismatch);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralPageDigestMismatchReport {
    pub player_id: NetworkPlayerId,
    pub page: EcsSpatialPageKey,
    pub server_digest: ProceduralPageDigest,
    pub client_digest: ProceduralPageDigest,
    pub client_tick: u64,
}

impl ProceduralPageDigestMismatchReport {
    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if !self.player_id.is_valid()
            || self.server_digest == self.client_digest
            || self.server_digest.page_key_hash != page_key_digest(self.page)
            || self.client_digest.page_key_hash != page_key_digest(self.page)
        {
            return Err(EcsSpatialValidationError::InvalidProceduralDigestProbe);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProceduralGeneratedPage {
    pub page: EcsSpatialPageKey,
    pub class: ProceduralGeneratedPageClass,
    pub payload_kind: VoxelPagePayloadKind,
    pub occupancy: VoxelOccupancyStorage,
    pub material_palette: VoxelMaterialPalette,
    pub cluster_summaries: [VoxelClusterSummary; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
    pub source_epoch: u32,
    pub digest: ProceduralPageDigest,
}

impl ProceduralGeneratedPage {
    fn from_voxel_brick(
        manifest: EcsProceduralWorldManifest,
        brick: &VoxelBrickPayload,
    ) -> Result<Self, EcsSpatialValidationError> {
        Ok(Self {
            page: brick.key,
            class: brick.generated_class,
            payload_kind: brick.kind,
            occupancy: brick.occupancy,
            material_palette: brick.material_palette,
            cluster_summaries: brick.clusters,
            source_epoch: brick.edit_epoch,
            digest: ProceduralPageDigest::from_voxel_brick(manifest, brick)?,
        })
    }

    #[must_use]
    pub fn into_voxel_brick(self) -> VoxelBrickPayload {
        VoxelBrickPayload {
            key: self.page,
            kind: self.payload_kind,
            generated_class: self.class,
            occupancy: self.occupancy,
            material_palette: self.material_palette,
            clusters: self.cluster_summaries,
            edit_epoch: self.source_epoch,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralSurfaceFace {
    pub local_x: u8,
    pub local_y: u8,
    pub local_z: u8,
    pub face_mask: u8,
    pub material_index: u8,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ProceduralGenerationScratch {
    pub occupancy_words: Vec<u64>,
    pub material_indices: Vec<u8>,
    pub cluster_summaries: Vec<VoxelClusterSummary>,
    pub surface_scratch: Vec<ProceduralSurfaceFace>,
}

impl ProceduralGenerationScratch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear_for_fast_path(&mut self) {
        self.occupancy_words.clear();
        self.material_indices.clear();
        self.cluster_summaries.clear();
        self.surface_scratch.clear();
    }

    fn reset_for_mixed_page(&mut self) {
        self.occupancy_words.clear();
        self.occupancy_words.resize(VOXEL_BRICK_OCCUPANCY_WORDS, 0);
        self.material_indices.clear();
        self.cluster_summaries.clear();
        self.cluster_summaries.resize(
            VOXEL_CLUSTER_SUMMARIES_PER_BRICK,
            VoxelClusterSummary::default(),
        );
        self.surface_scratch.clear();
    }

    #[must_use]
    pub fn dense_voxel_evaluations(&self) -> usize {
        self.material_indices.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsProceduralTerrainSource {
    pub source_id: EcsSpatialSourceId,
    pub manifest: EcsProceduralWorldManifest,
}

impl EcsProceduralTerrainSource {
    #[must_use]
    pub const fn new(source_id: EcsSpatialSourceId, manifest: EcsProceduralWorldManifest) -> Self {
        Self {
            source_id,
            manifest,
        }
    }
}

impl EcsSpatialSource for EcsProceduralTerrainSource {
    fn source_id(&self) -> EcsSpatialSourceId {
        self.source_id
    }

    fn manifest_for_region(&self, key: EcsSpatialRegionKey) -> EcsSpatialRegionManifest {
        EcsSpatialRegionManifest {
            region: key,
            source: self.source_id,
            source_kind: EcsSpatialSourceKind::Procedural,
            manifest_epoch: u32::from(self.manifest.schema_version),
            page_count: u32::from(self.manifest.region_edge_pages).pow(3),
            channel_mask: self.manifest.channel_mask,
            checksum: self.manifest.region_checksum(key),
        }
    }

    fn request_page(&self, key: EcsSpatialPageKey) -> EcsSourceRequest {
        let region = EcsSpatialRegionKey::from_page(key, self.manifest.region_edge_pages);
        let payload = if self.manifest.validate().is_ok() && self.manifest.accepts_page(key) {
            EcsSourcePayload::ProceduralTerrainRecipe(self.manifest.recipe_ref_for_page(key))
        } else {
            EcsSourcePayload::Failure(EcsSourceFailure {
                key,
                code: EcsPageFailureCode::DecodeRejected,
                checksum: EcsSourceChecksum::NONE,
                retry_after_frame: 0,
            })
        };
        EcsSourceRequest {
            request_id: EcsSourceRequestId::new(key.chunk_key().get()),
            source: self.source_id,
            source_kind: EcsSpatialSourceKind::Procedural,
            key,
            region,
            manifest_epoch: u32::from(self.manifest.schema_version),
            source_epoch: self.manifest.source_epoch(),
            priority: EcsStreamPriority::default(),
            payload,
        }
    }
}

pub fn terrain_height_ft(
    manifest: EcsProceduralWorldManifest,
    x_ft: i32,
    z_ft: i32,
) -> Result<i32, EcsSpatialValidationError> {
    manifest.validate()?;
    Ok(terrain_shape_sample(manifest.generator_desc(), x_ft, z_ft).height_ft)
}

pub fn estimate_page_height_relation(
    manifest: EcsProceduralWorldManifest,
    key: EcsSpatialPageKey,
) -> Result<PageHeightRelation, EcsSpatialValidationError> {
    manifest.validate()?;
    if !manifest.accepts_page(key) {
        return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
    }
    Ok(estimate_page_height_relation_for_desc(
        manifest.generator_desc(),
        key,
    ))
}

pub fn generate_procedural_terrain_page(
    manifest: EcsProceduralWorldManifest,
    recipe: EcsProceduralRecipeRef,
    source: EcsSpatialSourceId,
    source_epoch: u32,
    decode_epoch: u32,
    overlay_count: u16,
) -> Result<EcsDecodedPageRecord, EcsSpatialValidationError> {
    let mut scratch = ProceduralGenerationScratch::default();
    generate_procedural_terrain_page_with_scratch(
        manifest,
        recipe,
        source,
        source_epoch,
        decode_epoch,
        overlay_count,
        &mut scratch,
    )
}

pub fn generate_procedural_terrain_page_with_scratch(
    manifest: EcsProceduralWorldManifest,
    recipe: EcsProceduralRecipeRef,
    source: EcsSpatialSourceId,
    source_epoch: u32,
    decode_epoch: u32,
    overlay_count: u16,
    scratch: &mut ProceduralGenerationScratch,
) -> Result<EcsDecodedPageRecord, EcsSpatialValidationError> {
    manifest.validate()?;
    validate_recipe_ref(manifest, recipe)?;
    let key = recipe.page;
    let generated =
        generate_procedural_generated_page_with_scratch(manifest, recipe, source_epoch, scratch)?;
    let clusters = generated.cluster_summaries;
    let digest = generated.digest;
    let payload_kind = generated.payload_kind.into_decoded_kind();
    let brick = generated.into_voxel_brick();
    Ok(EcsDecodedPageRecord {
        key,
        source,
        source_epoch,
        payload_kind,
        voxel_brick: Some(brick),
        cluster_summaries: clusters,
        telemetry: EcsDecodeTelemetry {
            key,
            source_epoch,
            decode_epoch,
            source_bytes: 0,
            overlay_count,
            cluster_summary_count: VOXEL_CLUSTER_SUMMARIES_PER_BRICK as u16,
            checksum: EcsSourceChecksum {
                algorithm: EcsSourceChecksumAlgorithm::Fnv1a64,
                value: digest.combined_digest,
            },
            failure: None,
        },
    })
}

pub fn generate_procedural_generated_page(
    manifest: EcsProceduralWorldManifest,
    recipe: EcsProceduralRecipeRef,
    source_epoch: u32,
) -> Result<ProceduralGeneratedPage, EcsSpatialValidationError> {
    let mut scratch = ProceduralGenerationScratch::default();
    generate_procedural_generated_page_with_scratch(manifest, recipe, source_epoch, &mut scratch)
}

pub fn generate_procedural_generated_page_with_scratch(
    manifest: EcsProceduralWorldManifest,
    recipe: EcsProceduralRecipeRef,
    source_epoch: u32,
    scratch: &mut ProceduralGenerationScratch,
) -> Result<ProceduralGeneratedPage, EcsSpatialValidationError> {
    manifest.validate()?;
    validate_recipe_ref(manifest, recipe)?;
    let brick = generate_voxel_brick_with_scratch(manifest, recipe.page, source_epoch, scratch)?;
    ProceduralGeneratedPage::from_voxel_brick(manifest, &brick)
}

fn validate_recipe_ref(
    manifest: EcsProceduralWorldManifest,
    recipe: EcsProceduralRecipeRef,
) -> Result<(), EcsSpatialValidationError> {
    recipe.validate_for_desc(manifest.generator_desc())?;
    if !manifest.accepts_page(recipe.page) {
        return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
    }
    Ok(())
}

fn generate_voxel_brick(
    manifest: EcsProceduralWorldManifest,
    key: EcsSpatialPageKey,
    edit_epoch: u32,
) -> Result<VoxelBrickPayload, EcsSpatialValidationError> {
    let mut scratch = ProceduralGenerationScratch::default();
    generate_voxel_brick_with_scratch(manifest, key, edit_epoch, &mut scratch)
}

fn generate_voxel_brick_with_scratch(
    manifest: EcsProceduralWorldManifest,
    key: EcsSpatialPageKey,
    edit_epoch: u32,
    scratch: &mut ProceduralGenerationScratch,
) -> Result<VoxelBrickPayload, EcsSpatialValidationError> {
    let desc = manifest.generator_desc();
    match estimate_page_height_relation_for_desc(desc, key) {
        PageHeightRelation::EntirelyAboveTerrain => {
            scratch.clear_for_fast_path();
            return Ok(VoxelBrickPayload::empty(key, edit_epoch));
        }
        PageHeightRelation::EntirelyBelowTerrain => {
            scratch.clear_for_fast_path();
            return Ok(VoxelBrickPayload::uniform_solid(
                key,
                material_to_u16(desc.materials.rock)?,
                edit_epoch,
            ));
        }
        PageHeightRelation::IntersectsSurface => {}
    }

    scratch.reset_for_mixed_page();
    let cell_edge_ft = level_cell_edge_ft(key.level);
    let origin = page_origin_ft_i64(key, cell_edge_ft);
    let context = TerrainPageBuildContext {
        manifest,
        desc,
        origin,
        cell_edge_ft,
        materials: ProceduralMaterialTable::from_desc(desc)?,
    };
    let mut stats = [ClusterBuildStats::EMPTY; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    for cluster_z in 0..CLUSTERS_PER_BRICK_AXIS {
        for cluster_y in 0..CLUSTERS_PER_BRICK_AXIS {
            for cluster_x in 0..CLUSTERS_PER_BRICK_AXIS {
                let cluster = ClusterCoords::new(cluster_x, cluster_y, cluster_z);
                let height_bounds = cluster_height_bounds_ft(&context, cluster);
                match classify_height_relation(
                    cluster.min_world_y_ft(&context),
                    cluster.max_world_y_ft(&context),
                    height_bounds,
                ) {
                    PageHeightRelation::EntirelyAboveTerrain => {
                        stats[cluster.index].record_sdf_bounds(
                            height_bounds
                                .max_ft
                                .saturating_sub(cluster.max_world_y_ft(&context)),
                            height_bounds
                                .min_ft
                                .saturating_sub(cluster.min_world_y_ft(&context)),
                        );
                    }
                    PageHeightRelation::EntirelyBelowTerrain => {
                        fill_solid_cluster(&context, cluster, height_bounds, scratch, &mut stats);
                    }
                    PageHeightRelation::IntersectsSurface => {
                        evaluate_mixed_cluster(&context, cluster, scratch, &mut stats);
                    }
                }
            }
        }
    }

    let mut clusters = [VoxelClusterSummary::default(); VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut surface_cluster_count = 0_usize;
    let mut feature_cluster_count = 0_usize;
    let mut solid_cells = 0_usize;
    for index in 0..VOXEL_CLUSTER_SUMMARIES_PER_BRICK {
        let summary = cluster_summary_from_stats(&context, stats[index]);
        let strata_mixed = nonzero_material_lane_count(stats[index].material_counts) > 1;
        if stats[index].surface_count() != 0 {
            surface_cluster_count += 1;
        }
        if stats[index].ridge_count != 0 || stats[index].cliff_count != 0 || strata_mixed {
            feature_cluster_count += 1;
        }
        solid_cells = solid_cells.saturating_add(usize::from(summary.occupancy_popcount));
        scratch.cluster_summaries[index] = summary;
        clusters[index] = summary;
    }

    let palette = material_palette_from_stats(&context.materials, &stats);
    let mixed_materials = palette.len > 1;
    let occupancy = occupancy_from_scratch(scratch, solid_cells);

    let generated_class = classify_generated_page(
        key,
        solid_cells,
        mixed_materials,
        surface_cluster_count,
        feature_cluster_count,
    );
    Ok(VoxelBrickPayload {
        key,
        kind: page_payload_kind(solid_cells, palette.len),
        generated_class,
        occupancy,
        material_palette: palette,
        clusters,
        edit_epoch,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerrainShapeSample {
    height_ft: i32,
    features: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HeightBoundsFt {
    min_ft: i64,
    max_ft: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PageOriginFt {
    x: i64,
    y: i64,
    z: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProceduralMaterialTable {
    by_index: [u16; PROCEDURAL_MATERIAL_INDEX_COUNT],
}

impl ProceduralMaterialTable {
    fn from_desc(desc: ProceduralTerrainGeneratorDesc) -> Result<Self, EcsSpatialValidationError> {
        Ok(Self {
            by_index: [
                0,
                material_to_u16(desc.materials.grass)?,
                material_to_u16(desc.materials.dirt)?,
                material_to_u16(desc.materials.rock)?,
                material_to_u16(desc.materials.snow)?,
                material_to_u16(desc.materials.sand)?,
            ],
        })
    }

    #[must_use]
    fn material(self, index: u8) -> u16 {
        self.by_index
            .get(usize::from(index))
            .copied()
            .unwrap_or_default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TerrainPageBuildContext {
    manifest: EcsProceduralWorldManifest,
    desc: ProceduralTerrainGeneratorDesc,
    origin: PageOriginFt,
    cell_edge_ft: i64,
    materials: ProceduralMaterialTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClusterCoords {
    x: usize,
    y: usize,
    z: usize,
    index: usize,
}

impl ClusterCoords {
    #[must_use]
    fn new(x: usize, y: usize, z: usize) -> Self {
        Self {
            x,
            y,
            z,
            index: cluster_index_from_cluster_coords(x, y, z),
        }
    }

    #[must_use]
    fn local_x0(self) -> usize {
        self.x * CLUSTER_EDGE_FT
    }

    #[must_use]
    fn local_y0(self) -> usize {
        self.y * CLUSTER_EDGE_FT
    }

    #[must_use]
    fn local_z0(self) -> usize {
        self.z * CLUSTER_EDGE_FT
    }

    #[must_use]
    fn local_x1(self) -> usize {
        self.local_x0() + CLUSTER_EDGE_FT - 1
    }

    #[must_use]
    fn local_y1(self) -> usize {
        self.local_y0() + CLUSTER_EDGE_FT - 1
    }

    #[must_use]
    fn local_z1(self) -> usize {
        self.local_z0() + CLUSTER_EDGE_FT - 1
    }

    #[must_use]
    fn min_world_y_ft(self, context: &TerrainPageBuildContext) -> i64 {
        world_axis_ft(context.origin.y, self.local_y0(), context.cell_edge_ft)
    }

    #[must_use]
    fn max_world_y_ft(self, context: &TerrainPageBuildContext) -> i64 {
        world_axis_ft(context.origin.y, self.local_y1(), context.cell_edge_ft)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClusterBuildStats {
    occupancy_popcount: u16,
    material_counts: [u16; PROCEDURAL_MATERIAL_INDEX_COUNT],
    water_count: u16,
    ridge_count: u16,
    cliff_count: u16,
    min_local_y: i16,
    max_local_y: i16,
    sdf_min: i16,
    sdf_max: i16,
}

impl ClusterBuildStats {
    const EMPTY: Self = Self {
        occupancy_popcount: 0,
        material_counts: [0; PROCEDURAL_MATERIAL_INDEX_COUNT],
        water_count: 0,
        ridge_count: 0,
        cliff_count: 0,
        min_local_y: i16::MAX,
        max_local_y: i16::MIN,
        sdf_min: i16::MAX,
        sdf_max: i16::MIN,
    };

    fn record_sdf(&mut self, signed_distance: i16) {
        self.sdf_min = self.sdf_min.min(signed_distance);
        self.sdf_max = self.sdf_max.max(signed_distance);
    }

    fn record_sdf_bounds(&mut self, min_distance: i64, max_distance: i64) {
        self.record_sdf(clamp_i64_to_i16(min_distance));
        self.record_sdf(clamp_i64_to_i16(max_distance));
    }

    fn record_solid_voxel(&mut self, local_y: usize, material_index: u8, feature_bits: u8) {
        self.occupancy_popcount = self.occupancy_popcount.saturating_add(1);
        self.material_counts[usize::from(material_index)] =
            self.material_counts[usize::from(material_index)].saturating_add(1);
        self.min_local_y = self.min_local_y.min(local_y as i16);
        self.max_local_y = self.max_local_y.max(local_y as i16);
        if (feature_bits & COLUMN_FEATURE_RIDGE) != 0 {
            self.ridge_count = self.ridge_count.saturating_add(1);
        }
        if (feature_bits & COLUMN_FEATURE_CLIFF) != 0 {
            self.cliff_count = self.cliff_count.saturating_add(1);
        }
    }

    fn record_solid_cluster(&mut self, cluster: ClusterCoords, material_index: u8, features: u8) {
        self.occupancy_popcount = CLUSTER_FULL_POPCOUNT;
        self.material_counts[usize::from(material_index)] = CLUSTER_FULL_POPCOUNT;
        self.min_local_y = cluster.local_y0() as i16;
        self.max_local_y = cluster.local_y1() as i16;
        if (features & COLUMN_FEATURE_RIDGE) != 0 {
            self.ridge_count = CLUSTER_FULL_POPCOUNT;
        }
        if (features & COLUMN_FEATURE_CLIFF) != 0 {
            self.cliff_count = CLUSTER_FULL_POPCOUNT;
        }
    }

    #[must_use]
    fn surface_count(self) -> u16 {
        self.material_counts[usize::from(PROCEDURAL_MATERIAL_INDEX_GRASS)]
            .saturating_add(self.material_counts[usize::from(PROCEDURAL_MATERIAL_INDEX_SNOW)])
            .saturating_add(self.material_counts[usize::from(PROCEDURAL_MATERIAL_INDEX_SAND)])
    }

    #[must_use]
    fn bedrock_count(self) -> u16 {
        self.material_counts[usize::from(PROCEDURAL_MATERIAL_INDEX_ROCK)]
    }
}

fn estimate_page_height_relation_for_desc(
    desc: ProceduralTerrainGeneratorDesc,
    key: EcsSpatialPageKey,
) -> PageHeightRelation {
    let (min_y_ft, max_y_ft) = page_y_bounds_ft(key);
    classify_height_relation(min_y_ft, max_y_ft, page_height_bounds_ft(desc, key))
}

fn classify_height_relation(
    min_y_ft: i64,
    max_y_ft: i64,
    height_bounds: HeightBoundsFt,
) -> PageHeightRelation {
    if min_y_ft > height_bounds.max_ft {
        PageHeightRelation::EntirelyAboveTerrain
    } else if max_y_ft <= height_bounds.min_ft {
        PageHeightRelation::EntirelyBelowTerrain
    } else {
        PageHeightRelation::IntersectsSurface
    }
}

fn page_height_bounds_ft(
    desc: ProceduralTerrainGeneratorDesc,
    key: EcsSpatialPageKey,
) -> HeightBoundsFt {
    let cell_edge_ft = level_cell_edge_ft(key.level);
    let origin = page_origin_ft_i64(key, cell_edge_ft);
    let span_ft = (PAGE_EDGE_FT - 1).saturating_mul(cell_edge_ft);
    terrain_height_bounds_for_footprint(desc, origin.x, origin.z, span_ft)
}

fn cluster_height_bounds_ft(
    context: &TerrainPageBuildContext,
    cluster: ClusterCoords,
) -> HeightBoundsFt {
    let x = world_axis_ft(context.origin.x, cluster.local_x0(), context.cell_edge_ft);
    let z = world_axis_ft(context.origin.z, cluster.local_z0(), context.cell_edge_ft);
    let span_ft = (CLUSTER_EDGE_FT as i64 - 1).saturating_mul(context.cell_edge_ft);
    terrain_height_bounds_for_footprint(context.desc, x, z, span_ft)
}

fn terrain_height_bounds_for_footprint(
    desc: ProceduralTerrainGeneratorDesc,
    min_x_ft: i64,
    min_z_ft: i64,
    span_ft: i64,
) -> HeightBoundsFt {
    let max_x_ft = min_x_ft.saturating_add(span_ft);
    let max_z_ft = min_z_ft.saturating_add(span_ft);
    let center_x_ft = min_x_ft.saturating_add(span_ft / 2);
    let center_z_ft = min_z_ft.saturating_add(span_ft / 2);
    let samples = [
        (min_x_ft, min_z_ft),
        (max_x_ft, min_z_ft),
        (min_x_ft, max_z_ft),
        (max_x_ft, max_z_ft),
        (center_x_ft, center_z_ft),
    ];
    let mut min_height = i64::MAX;
    let mut max_height = i64::MIN;
    for (x_ft, z_ft) in samples {
        let height = i64::from(
            terrain_shape_sample(desc, clamp_i64_to_i32(x_ft), clamp_i64_to_i32(z_ft)).height_ft,
        );
        min_height = min_height.min(height);
        max_height = max_height.max(height);
    }
    let margin = terrain_sampling_margin_ft(desc.terrain, span_ft);
    HeightBoundsFt {
        min_ft: min_height.saturating_sub(margin),
        max_ft: max_height.saturating_add(margin),
    }
}

fn terrain_sampling_margin_ft(shape: ProceduralTerrainShapeDesc, span_ft: i64) -> i64 {
    let amplitude = i64::from(shape.height_amplitude_ft.abs()).max(1);
    let ridge =
        amplitude.saturating_mul(i64::from(shape.ridge_strength_q16)) / i64::from(FIXED_ONE_Q16);
    let max_frequency = shape
        .continental_frequency_q16
        .max(shape.hill_frequency_q16)
        .max(shape.ridge_frequency_q16)
        .max(shape.warp_frequency_q16);
    let span_q16 = span_ft
        .abs()
        .saturating_mul(i64::from(max_frequency.max(1)));
    let octave_scale = i64::from(shape.octaves.max(1));
    let local_variation = amplitude
        .saturating_mul(span_q16)
        .saturating_mul(octave_scale)
        / i64::from(FIXED_ONE_Q16)
        / 4;
    local_variation.clamp(
        2,
        amplitude
            .saturating_mul(2)
            .saturating_add(ridge)
            .saturating_add(4),
    )
}

fn page_origin_ft_i64(key: EcsSpatialPageKey, cell_edge_ft: i64) -> PageOriginFt {
    PageOriginFt {
        x: i64::from(key.x)
            .saturating_mul(PAGE_EDGE_FT)
            .saturating_mul(cell_edge_ft),
        y: i64::from(key.y)
            .saturating_mul(PAGE_EDGE_FT)
            .saturating_mul(cell_edge_ft),
        z: i64::from(key.z)
            .saturating_mul(PAGE_EDGE_FT)
            .saturating_mul(cell_edge_ft),
    }
}

fn world_axis_ft(origin: i64, local: usize, cell_edge_ft: i64) -> i64 {
    origin.saturating_add((local as i64).saturating_mul(cell_edge_ft))
}

fn fill_solid_cluster(
    context: &TerrainPageBuildContext,
    cluster: ClusterCoords,
    height_bounds: HeightBoundsFt,
    scratch: &mut ProceduralGenerationScratch,
    stats: &mut [ClusterBuildStats; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
) {
    set_cluster_occupancy_solid(&mut scratch.occupancy_words, cluster);
    let center_x = world_axis_ft(
        context.origin.x,
        (cluster.local_x0() + cluster.local_x1()) / 2,
        context.cell_edge_ft,
    );
    let center_y = world_axis_ft(
        context.origin.y,
        (cluster.local_y0() + cluster.local_y1()) / 2,
        context.cell_edge_ft,
    );
    let center_z = world_axis_ft(
        context.origin.z,
        (cluster.local_z0() + cluster.local_z1()) / 2,
        context.cell_edge_ft,
    );
    let sample = terrain_column_sample(context.desc, center_x, center_z);
    let material_index = material_index_for_depth(
        context.desc,
        i64::from(sample.height_ft).saturating_sub(center_y),
        center_y,
        (sample.features & COLUMN_FEATURE_CLIFF) != 0,
    );
    stats[cluster.index].record_solid_cluster(cluster, material_index, sample.features);
    stats[cluster.index].record_sdf_bounds(
        height_bounds
            .min_ft
            .saturating_sub(cluster.max_world_y_ft(context)),
        height_bounds
            .max_ft
            .saturating_sub(cluster.min_world_y_ft(context)),
    );
}

fn evaluate_mixed_cluster(
    context: &TerrainPageBuildContext,
    cluster: ClusterCoords,
    scratch: &mut ProceduralGenerationScratch,
    stats: &mut [ClusterBuildStats; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
) {
    let mut heights = [0_i32; CLUSTER_EDGE_FT * CLUSTER_EDGE_FT];
    let mut features = [0_u8; CLUSTER_EDGE_FT * CLUSTER_EDGE_FT];
    for local_z in cluster.local_z0()..=cluster.local_z1() {
        for local_x in cluster.local_x0()..=cluster.local_x1() {
            let world_x = world_axis_ft(context.origin.x, local_x, context.cell_edge_ft);
            let world_z = world_axis_ft(context.origin.z, local_z, context.cell_edge_ft);
            let sample = terrain_column_sample(context.desc, world_x, world_z);
            let index = cluster_column_index(cluster, local_x, local_z);
            heights[index] = sample.height_ft;
            features[index] = sample.features;
        }
    }
    for local_y in cluster.local_y0()..=cluster.local_y1() {
        let world_y = world_axis_ft(context.origin.y, local_y, context.cell_edge_ft);
        for local_z in cluster.local_z0()..=cluster.local_z1() {
            for local_x in cluster.local_x0()..=cluster.local_x1() {
                let column_index = cluster_column_index(cluster, local_x, local_z);
                let height = i64::from(heights[column_index]);
                let feature_bits = features[column_index];
                let signed_distance = clamp_i64_to_i16(height.saturating_sub(world_y));
                stats[cluster.index].record_sdf(signed_distance);
                if world_y <= height {
                    let material_index = material_index_for_depth(
                        context.desc,
                        height.saturating_sub(world_y),
                        world_y,
                        (feature_bits & COLUMN_FEATURE_CLIFF) != 0,
                    );
                    set_occupancy_word_bit(
                        &mut scratch.occupancy_words,
                        voxel_index(local_x, local_y, local_z),
                    );
                    stats[cluster.index].record_solid_voxel(local_y, material_index, feature_bits);
                    scratch.material_indices.push(material_index);
                    if world_y == height {
                        scratch.surface_scratch.push(ProceduralSurfaceFace {
                            local_x: local_x as u8,
                            local_y: local_y as u8,
                            local_z: local_z as u8,
                            face_mask: PROCEDURAL_SURFACE_FACE_POS_Y,
                            material_index,
                        });
                    }
                } else {
                    scratch.material_indices.push(PROCEDURAL_MATERIAL_INDEX_AIR);
                }
            }
        }
    }
}

fn terrain_column_sample(
    desc: ProceduralTerrainGeneratorDesc,
    world_x_ft: i64,
    world_z_ft: i64,
) -> TerrainShapeSample {
    let mut sample = terrain_shape_sample(
        desc,
        clamp_i64_to_i32(world_x_ft),
        clamp_i64_to_i32(world_z_ft),
    );
    let slope_q16 = terrain_slope_q16(
        desc,
        clamp_i64_to_i32(world_x_ft),
        clamp_i64_to_i32(world_z_ft),
    );
    if desc.features.cliffs_enabled && slope_q16 > u32::from(desc.terrain.cliff_threshold_q16) {
        sample.features |= COLUMN_FEATURE_CLIFF;
    }
    sample
}

fn material_index_for_depth(
    desc: ProceduralTerrainGeneratorDesc,
    depth_ft: i64,
    world_y_ft: i64,
    cliff: bool,
) -> u8 {
    if world_y_ft > i64::from(snow_line_ft(desc.terrain)) {
        PROCEDURAL_MATERIAL_INDEX_SNOW
    } else if cliff {
        PROCEDURAL_MATERIAL_INDEX_ROCK
    } else if depth_ft <= 1 {
        PROCEDURAL_MATERIAL_INDEX_GRASS
    } else if depth_ft <= 6 {
        PROCEDURAL_MATERIAL_INDEX_DIRT
    } else {
        PROCEDURAL_MATERIAL_INDEX_ROCK
    }
}

fn cluster_summary_from_stats(
    context: &TerrainPageBuildContext,
    stats: ClusterBuildStats,
) -> VoxelClusterSummary {
    let strata_mixed = nonzero_material_lane_count(stats.material_counts) > 1;
    VoxelClusterSummary {
        occupancy_popcount: stats.occupancy_popcount,
        exposed_face_mask: exposed_face_mask(stats.occupancy_popcount),
        dominant_material: dominant_material_from_counts(context.materials, stats.material_counts),
        min_height_local: if stats.occupancy_popcount == 0 {
            0
        } else {
            stats.min_local_y
        },
        max_height_local: if stats.occupancy_popcount == 0 {
            0
        } else {
            stats.max_local_y
        },
        sdf_min_q: normalize_sdf_bound(stats.sdf_min),
        sdf_max_q: normalize_sdf_bound(stats.sdf_max),
        foliage_density_q: if stats.surface_count() == 0 {
            0
        } else {
            context.manifest.biome_recipe.foliage_density_q
        },
        water_q: stats.water_count.min(u16::from(u8::MAX)) as u8,
        flags: cluster_flags(
            stats.surface_count(),
            stats.bedrock_count(),
            stats.water_count,
            stats.ridge_count,
            stats.cliff_count,
            strata_mixed,
        ),
    }
}

fn material_palette_from_stats(
    materials: &ProceduralMaterialTable,
    stats: &[ClusterBuildStats; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
) -> VoxelMaterialPalette {
    let mut used = [false; PROCEDURAL_MATERIAL_INDEX_COUNT];
    for cluster in stats {
        for (index, count) in cluster.material_counts.iter().enumerate().skip(1) {
            used[index] |= *count != 0;
        }
    }
    let mut palette = VoxelMaterialPalette::empty();
    for (index, is_used) in used.iter().enumerate().skip(1) {
        if *is_used {
            push_palette_material(&mut palette, materials.by_index[index]);
        }
    }
    palette
}

fn occupancy_from_scratch(
    scratch: &ProceduralGenerationScratch,
    solid_cells: usize,
) -> VoxelOccupancyStorage {
    let mut occupancy = VoxelOccupancyStorage::empty();
    occupancy.kind = occupancy_kind(solid_cells);
    match occupancy.kind {
        VoxelOccupancyStorageKind::Empty => {}
        VoxelOccupancyStorageKind::UniformSolid => {
            occupancy.words = [u64::MAX; VOXEL_BRICK_OCCUPANCY_WORDS];
        }
        VoxelOccupancyStorageKind::Bitset32 => {
            occupancy
                .words
                .copy_from_slice(&scratch.occupancy_words[..VOXEL_BRICK_OCCUPANCY_WORDS]);
        }
    }
    occupancy
}

fn dominant_material_from_counts(
    materials: ProceduralMaterialTable,
    counts: [u16; PROCEDURAL_MATERIAL_INDEX_COUNT],
) -> u16 {
    let mut best_index = PROCEDURAL_MATERIAL_INDEX_AIR;
    let mut best_count = 0_u16;
    for (index, count) in counts.iter().enumerate().skip(1) {
        if *count > best_count {
            best_index = index as u8;
            best_count = *count;
        }
    }
    materials.material(best_index)
}

fn nonzero_material_lane_count(counts: [u16; PROCEDURAL_MATERIAL_INDEX_COUNT]) -> u8 {
    counts
        .iter()
        .skip(1)
        .filter(|count| **count != 0)
        .count()
        .min(u8::MAX as usize) as u8
}

fn cluster_column_index(cluster: ClusterCoords, local_x: usize, local_z: usize) -> usize {
    (local_z - cluster.local_z0()) * CLUSTER_EDGE_FT + (local_x - cluster.local_x0())
}

fn cluster_index_from_cluster_coords(
    cluster_x: usize,
    cluster_y: usize,
    cluster_z: usize,
) -> usize {
    (cluster_z * CLUSTERS_PER_BRICK_AXIS + cluster_y) * CLUSTERS_PER_BRICK_AXIS + cluster_x
}

fn set_cluster_occupancy_solid(words: &mut [u64], cluster: ClusterCoords) {
    for local_y in cluster.local_y0()..=cluster.local_y1() {
        for local_z in cluster.local_z0()..=cluster.local_z1() {
            for local_x in cluster.local_x0()..=cluster.local_x1() {
                set_occupancy_word_bit(words, voxel_index(local_x, local_y, local_z));
            }
        }
    }
}

fn set_occupancy_word_bit(words: &mut [u64], index: usize) {
    debug_assert!(index < VOXEL_BRICK_FOOT_CELL_COUNT);
    let word = index / 64;
    let bit = index % 64;
    words[word] |= 1_u64 << bit;
}

fn terrain_shape_sample(
    desc: ProceduralTerrainGeneratorDesc,
    world_x_ft: i32,
    world_z_ft: i32,
) -> TerrainShapeSample {
    let shape = desc.terrain;
    let (warp_x_ft, warp_z_ft) = domain_warp_ft(desc, world_x_ft, world_z_ft);
    let sample_x_ft = world_x_ft.saturating_add(warp_x_ft);
    let sample_z_ft = world_z_ft.saturating_add(warp_z_ft);
    let continental_q16 = fbm2_q16(
        desc.world_seed ^ 0x3161_9f47_43d2_ba11,
        sample_x_ft,
        sample_z_ft,
        shape.continental_frequency_q16,
        shape.octaves,
    );
    let hill_q16 = fbm2_q16(
        desc.world_seed ^ 0x7e57_1c0a_d42f_0091,
        sample_x_ft,
        sample_z_ft,
        shape.hill_frequency_q16,
        shape.octaves.saturating_add(1).min(8),
    );
    let ridge_noise_q16 = value_noise2_q16(
        desc.world_seed ^ 0x4cf5_ad43_93a1_20d5,
        scale_i32_to_q16(sample_x_ft, shape.ridge_frequency_q16),
        scale_i32_to_q16(sample_z_ft, shape.ridge_frequency_q16),
    );
    let ridge_q16 = ridge_height_q16(ridge_noise_q16);
    let height_ft = i64::from(shape.base_height_ft)
        .saturating_add(q16_height_delta_ft(
            continental_q16,
            shape.height_amplitude_ft,
        ))
        .saturating_add(q16_height_delta_ft(hill_q16, shape.height_amplitude_ft / 2))
        .saturating_add(ridge_height_delta_ft(shape, ridge_q16));

    let mut features = 0_u8;
    if ridge_q16 >= FIXED_ONE_Q16 / 2 {
        features |= COLUMN_FEATURE_RIDGE;
    }
    TerrainShapeSample {
        height_ft: clamp_i64_to_i32(height_ft),
        features,
    }
}

fn domain_warp_ft(
    desc: ProceduralTerrainGeneratorDesc,
    world_x_ft: i32,
    world_z_ft: i32,
) -> (i32, i32) {
    let shape = desc.terrain;
    if shape.warp_frequency_q16 == 0 || shape.erosion_hint_q16 == 0 {
        return (0, 0);
    }
    let max_warp_ft = ((i64::from(shape.height_amplitude_ft.abs())
        * i64::from(shape.erosion_hint_q16))
        / i64::from(FIXED_ONE_Q16))
    .clamp(0, 32) as i32;
    if max_warp_ft == 0 {
        return (0, 0);
    }
    let warp_x = q16_height_delta_ft(
        fbm2_q16(
            desc.world_seed ^ 0x9b4d_421d_c1a7_006d,
            world_x_ft,
            world_z_ft,
            shape.warp_frequency_q16,
            2,
        ),
        max_warp_ft,
    );
    let warp_z = q16_height_delta_ft(
        fbm2_q16(
            desc.world_seed ^ 0xc2b2_ae35_27d4_eb4f,
            world_x_ft,
            world_z_ft,
            shape.warp_frequency_q16,
            2,
        ),
        max_warp_ft,
    );
    (clamp_i64_to_i32(warp_x), clamp_i64_to_i32(warp_z))
}

fn terrain_slope_q16(desc: ProceduralTerrainGeneratorDesc, x_ft: i32, z_ft: i32) -> u32 {
    let x_forward = terrain_shape_sample(desc, x_ft.saturating_add(1), z_ft).height_ft;
    let x_back = terrain_shape_sample(desc, x_ft.saturating_sub(1), z_ft).height_ft;
    let z_forward = terrain_shape_sample(desc, x_ft, z_ft.saturating_add(1)).height_ft;
    let z_back = terrain_shape_sample(desc, x_ft, z_ft.saturating_sub(1)).height_ft;
    let slope_ft = i64::from(x_forward)
        .saturating_sub(i64::from(x_back))
        .abs()
        .saturating_add(i64::from(z_forward).saturating_sub(i64::from(z_back)).abs());
    slope_ft
        .saturating_mul(i64::from(FIXED_ONE_Q16))
        .clamp(0, i64::from(u32::MAX)) as u32
}

fn q16_height_delta_ft(value_q16: i32, amplitude_ft: i32) -> i64 {
    i64::from(value_q16).saturating_mul(i64::from(amplitude_ft)) / i64::from(FIXED_ONE_Q16)
}

fn ridge_height_q16(noise_q16: i32) -> u32 {
    let ridge_q16 = (i64::from(noise_q16).saturating_mul(2) - i64::from(FIXED_ONE_Q16)).abs();
    i64::from(FIXED_ONE_Q16)
        .saturating_sub(ridge_q16)
        .clamp(0, i64::from(FIXED_ONE_Q16)) as u32
}

fn ridge_height_delta_ft(shape: ProceduralTerrainShapeDesc, ridge_q16: u32) -> i64 {
    let ridge_strength_ft = i64::from(shape.height_amplitude_ft)
        .saturating_mul(i64::from(shape.ridge_strength_q16))
        / i64::from(FIXED_ONE_Q16);
    ridge_strength_ft.saturating_mul(i64::from(ridge_q16)) / i64::from(FIXED_ONE_Q16)
}

fn snow_line_ft(shape: ProceduralTerrainShapeDesc) -> i32 {
    shape
        .base_height_ft
        .saturating_add(shape.height_amplitude_ft)
        .saturating_add(shape.height_amplitude_ft / 2)
}

fn page_y_bounds_ft(key: EcsSpatialPageKey) -> (i64, i64) {
    let cell_edge_ft = level_cell_edge_ft(key.level);
    let min_y_ft = i64::from(key.y)
        .saturating_mul(PAGE_EDGE_FT)
        .saturating_mul(cell_edge_ft);
    let max_y_ft = min_y_ft.saturating_add((PAGE_EDGE_FT - 1).saturating_mul(cell_edge_ft));
    (min_y_ft, max_y_ft)
}

fn scale_i32_to_q16(value: i32, frequency_q16: u32) -> i64 {
    i64::from(value).saturating_mul(i64::from(frequency_q16.max(1)))
}

fn material_to_u16(material: TerrainMaterialId) -> Result<u16, EcsSpatialValidationError> {
    if material.get() == 0 || material.get() > u32::from(u16::MAX) {
        return Err(EcsSpatialValidationError::InvalidMaterial);
    }
    Ok(material.get() as u16)
}

fn period_to_frequency_q16(period_ft: u16) -> u32 {
    (FIXED_ONE_Q16 / u32::from(period_ft.max(1))).max(1)
}

fn ridge_period_ft(macro_period_ft: u16) -> u16 {
    let period = (u32::from(macro_period_ft.max(1)) * 2) / 3;
    period.clamp(1, u32::from(u16::MAX)) as u16
}

fn warp_period_ft(macro_period_ft: u16) -> u16 {
    let period = u32::from(macro_period_ft.max(1)) + u32::from(macro_period_ft.max(1)) / 3;
    period.clamp(1, u32::from(u16::MAX)) as u16
}

fn occupancy_kind(solid_cells: usize) -> VoxelOccupancyStorageKind {
    if solid_cells == 0 {
        VoxelOccupancyStorageKind::Empty
    } else if solid_cells == VOXEL_BRICK_FOOT_CELL_COUNT {
        VoxelOccupancyStorageKind::UniformSolid
    } else {
        VoxelOccupancyStorageKind::Bitset32
    }
}

fn page_payload_kind(solid_cells: usize, palette_len: u16) -> VoxelPagePayloadKind {
    if solid_cells == 0 {
        VoxelPagePayloadKind::Empty
    } else if solid_cells == VOXEL_BRICK_FOOT_CELL_COUNT && palette_len <= 1 {
        VoxelPagePayloadKind::UniformSolid
    } else if palette_len <= PROCEDURAL_TINY_PALETTE_LIMIT {
        VoxelPagePayloadKind::PaletteRle
    } else {
        VoxelPagePayloadKind::DenseFootCells
    }
}

fn classify_generated_page(
    key: EcsSpatialPageKey,
    solid_cells: usize,
    mixed_materials: bool,
    surface_cluster_count: usize,
    feature_cluster_count: usize,
) -> ProceduralGeneratedPageClass {
    if key.channel == EcsPageChannel::Debug {
        return ProceduralGeneratedPageClass::DebugOnly;
    }
    if solid_cells == 0 {
        return ProceduralGeneratedPageClass::EmptyAir;
    }
    if solid_cells == VOXEL_BRICK_FOOT_CELL_COUNT && !mixed_materials {
        return ProceduralGeneratedPageClass::UniformSolid;
    }
    if solid_cells >= (VOXEL_BRICK_FOOT_CELL_COUNT * 4) / 5 && surface_cluster_count == 0 {
        return ProceduralGeneratedPageClass::MostlySolid;
    }
    if solid_cells <= VOXEL_BRICK_FOOT_CELL_COUNT / 4 && feature_cluster_count != 0 {
        return ProceduralGeneratedPageClass::MostlyAirWithFeatures;
    }
    ProceduralGeneratedPageClass::SurfaceMixed
}

trait IntoDecodedKind {
    fn into_decoded_kind(self) -> EcsDecodedPagePayloadKind;
}

impl IntoDecodedKind for VoxelPagePayloadKind {
    fn into_decoded_kind(self) -> EcsDecodedPagePayloadKind {
        match self {
            VoxelPagePayloadKind::Empty => EcsDecodedPagePayloadKind::Empty,
            VoxelPagePayloadKind::UniformSolid
            | VoxelPagePayloadKind::PaletteRle
            | VoxelPagePayloadKind::DenseFootCells
            | VoxelPagePayloadKind::CoarseProxyOnly
            | VoxelPagePayloadKind::SurfaceOnly
            | VoxelPagePayloadKind::SdfOnly => EcsDecodedPagePayloadKind::VoxelBrick,
            VoxelPagePayloadKind::ProceduralTerrainRecipe => {
                EcsDecodedPagePayloadKind::ProceduralTerrainRecipe
            }
        }
    }
}

fn exposed_face_mask(popcount: u16) -> u8 {
    if popcount == 0 || popcount == CLUSTER_FULL_POPCOUNT {
        0
    } else {
        0x3f
    }
}

fn normalize_sdf_bound(value: i16) -> i16 {
    if value == i16::MAX || value == i16::MIN {
        0
    } else {
        value
    }
}

fn cluster_flags(
    surface_count: u16,
    bedrock_count: u16,
    water_count: u16,
    ridge_count: u16,
    cliff_count: u16,
    strata_mixed: bool,
) -> u16 {
    (if surface_count != 0 {
        CLUSTER_FLAG_SURFACE
    } else {
        0
    }) | (if bedrock_count != 0 {
        CLUSTER_FLAG_BEDROCK
    } else {
        0
    }) | (if water_count != 0 {
        CLUSTER_FLAG_WATER_DEBUG
    } else {
        0
    }) | (if ridge_count != 0 {
        CLUSTER_FLAG_RIDGE
    } else {
        0
    }) | (if cliff_count != 0 {
        CLUSTER_FLAG_CLIFF
    } else {
        0
    }) | if strata_mixed {
        CLUSTER_FLAG_STRATA_MIXED
    } else {
        0
    }
}

fn push_palette_material(palette: &mut VoxelMaterialPalette, material: u16) {
    if palette.materials[..usize::from(palette.len)].contains(&material) {
        return;
    }
    if usize::from(palette.len) < palette.materials.len() {
        palette.materials[usize::from(palette.len)] = material;
        palette.len = palette.len.saturating_add(1);
    }
}

fn voxel_index(local_x: usize, local_y: usize, local_z: usize) -> usize {
    (local_z * VOXEL_BRICK_EDGE_CELLS as usize + local_y) * VOXEL_BRICK_EDGE_CELLS as usize
        + local_x
}

fn level_cell_edge_ft(level: u8) -> i64 {
    if level >= 20 {
        1_i64 << 20
    } else {
        1_i64 << u32::from(level)
    }
}

fn clamp_i64_to_i16(value: i64) -> i16 {
    value.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
}

fn clamp_i64_to_i32(value: i64) -> i32 {
    value.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn hash_page_key(mut hash: u64, key: EcsSpatialPageKey) -> u64 {
    hash = hash_u8(hash, key.domain as u8);
    hash = hash_u64(hash, key.grid_id.get());
    hash = hash_u8(hash, key.level);
    hash = hash_i32(hash, key.x);
    hash = hash_i32(hash, key.y);
    hash = hash_i32(hash, key.z);
    hash_u8(hash, key.channel as u8)
}

fn hash_region_key(mut hash: u64, key: EcsSpatialRegionKey) -> u64 {
    hash = hash_u8(hash, key.domain as u8);
    hash = hash_u64(hash, key.grid_id.get());
    hash = hash_u8(hash, key.level);
    hash = hash_i32(hash, key.x);
    hash = hash_i32(hash, key.y);
    hash_i32(hash, key.z)
}

fn page_key_digest(key: EcsSpatialPageKey) -> u64 {
    hash_page_key(FNV_OFFSET, key).max(1)
}

#[must_use]
pub fn region_seed(world_seed: u64, region: EcsSpatialRegionKey) -> u64 {
    let mut hash = hash_u64(FNV_OFFSET, world_seed);
    hash = hash_u8(hash, 0x72);
    hash_region_key(hash, region).max(1)
}

#[must_use]
pub fn page_seed(world_seed: u64, page: EcsSpatialPageKey) -> u64 {
    let mut hash = hash_u64(FNV_OFFSET, world_seed);
    hash = hash_u8(hash, 0x70);
    hash_page_key(hash, page).max(1)
}

#[must_use]
pub fn initial_region_seed_table_digest(
    world_seed: u64,
    terrain_grid: EcsSpatialGridId,
    region_edge_pages: u16,
    spawn_position_ft: IVec3,
) -> u64 {
    let center_page = spawn_page_key(terrain_grid, spawn_position_ft);
    let center_region = EcsSpatialRegionKey::from_page(center_page, region_edge_pages.max(1));
    let mut hash = hash_u64(FNV_OFFSET, world_seed);
    hash = hash_u64(hash, terrain_grid.get());
    hash = hash_u16(hash, region_edge_pages.max(1));
    hash = hash_i32(hash, spawn_position_ft.x);
    hash = hash_i32(hash, spawn_position_ft.y);
    hash = hash_i32(hash, spawn_position_ft.z);
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let region = EcsSpatialRegionKey {
                    x: center_region.x.saturating_add(dx),
                    y: center_region.y.saturating_add(dy),
                    z: center_region.z.saturating_add(dz),
                    ..center_region
                };
                hash = hash_u64(hash, region_seed(world_seed, region));
            }
        }
    }
    hash.max(1)
}

fn spawn_page_key(terrain_grid: EcsSpatialGridId, position_ft: IVec3) -> EcsSpatialPageKey {
    let page_edge_ft = i32::from(VOXEL_BRICK_EDGE_CELLS);
    EcsSpatialPageKey::new(
        EcsSpatialDomainKind::Terrain,
        terrain_grid,
        0,
        position_ft.x.div_euclid(page_edge_ft),
        position_ft.y.div_euclid(page_edge_ft),
        position_ft.z.div_euclid(page_edge_ft),
        EcsPageChannel::Surface,
    )
}

fn empty_delta_digest(base_manifest_digest: u64) -> u64 {
    let mut hash = hash_u8(FNV_OFFSET, 0xdd);
    hash = hash_u64(hash, base_manifest_digest);
    hash.max(1)
}

fn occupancy_storage_digest(occupancy: &VoxelOccupancyStorage) -> u64 {
    let mut hash = FNV_OFFSET;
    hash = hash_u8(hash, occupancy.kind as u8);
    for word in occupancy.words {
        hash = hash_u64(hash, word);
    }
    hash.max(1)
}

fn material_palette_digest(palette: &VoxelMaterialPalette) -> u64 {
    let mut hash = FNV_OFFSET;
    let len = usize::from(palette.len).min(palette.materials.len());
    hash = hash_u16(hash, len as u16);
    for material in palette.materials.iter().take(len) {
        hash = hash_u16(hash, *material);
    }
    hash.max(1)
}

fn cluster_summaries_digest(
    clusters: &[VoxelClusterSummary; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
) -> u64 {
    let mut hash = FNV_OFFSET;
    for cluster in clusters {
        hash = hash_u16(hash, cluster.occupancy_popcount);
        hash = hash_u8(hash, cluster.exposed_face_mask);
        hash = hash_u16(hash, cluster.dominant_material);
        hash = hash_i16(hash, cluster.min_height_local);
        hash = hash_i16(hash, cluster.max_height_local);
        hash = hash_i16(hash, cluster.sdf_min_q);
        hash = hash_i16(hash, cluster.sdf_max_q);
        hash = hash_u8(hash, cluster.foliage_density_q);
        hash = hash_u8(hash, cluster.water_q);
        hash = hash_u16(hash, cluster.flags);
    }
    hash.max(1)
}

const fn hash_u8(hash: u64, value: u8) -> u64 {
    (hash ^ value as u64).wrapping_mul(FNV_PRIME)
}

const fn hash_u16(hash: u64, value: u16) -> u64 {
    let hash = hash_u8(hash, (value & 0xff) as u8);
    hash_u8(hash, (value >> 8) as u8)
}

const fn hash_u32(hash: u64, value: u32) -> u64 {
    let hash = hash_u16(hash, (value & 0xffff) as u16);
    hash_u16(hash, (value >> 16) as u16)
}

const fn hash_u64(hash: u64, value: u64) -> u64 {
    let hash = hash_u32(hash, (value & 0xffff_ffff) as u32);
    hash_u32(hash, (value >> 32) as u32)
}

const fn hash_i32(hash: u64, value: i32) -> u64 {
    hash_u32(hash, value as u32)
}

const fn hash_i16(hash: u64, value: i16) -> u64 {
    hash_u16(hash, value as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::VOXEL_BRICK_OCCUPANCY_WORDS;

    fn page(x: i32, y: i32, z: i32) -> EcsSpatialPageKey {
        EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            x,
            y,
            z,
            EcsPageChannel::Occupancy,
        )
    }

    #[test]
    fn default_manifest_validates_one_foot_voxel_contract() {
        let manifest = EcsProceduralWorldManifest::default();
        assert_eq!(
            manifest.schema_version,
            ECS_PROCEDURAL_TERRAIN_SCHEMA_VERSION
        );
        assert_eq!(
            manifest.generator_version,
            ECS_PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1
        );
        assert_eq!(VOXEL_BRICK_EDGE_CELLS, 32);
        assert_eq!(VOXEL_BRICK_OCCUPANCY_WORDS, 512);
        assert_eq!(manifest.biome_recipe.foliage_density_q, 0);
        assert_eq!(manifest.biome_recipe.water_level_ft, -4096);
        assert!(
            manifest
                .sync_manifest()
                .enabled_features
                .contains(ProceduralFeature::RidgeCliffStrata)
        );
        assert!(
            !manifest
                .sync_manifest()
                .enabled_features
                .contains(ProceduralFeature::FoliageHints)
        );
        manifest.validate().expect("default manifest validates");
    }

    #[test]
    fn procedural_source_emits_manifest_bound_recipe_refs() {
        let manifest = EcsProceduralWorldManifest::default();
        let source = EcsProceduralTerrainSource::new(EcsSpatialSourceId::new(7), manifest);
        let key = page(0, 0, 0);
        let request = source.request_page(key);

        assert_eq!(request.source_kind, EcsSpatialSourceKind::Procedural);
        assert_eq!(request.source_epoch, manifest.source_epoch());
        match request.payload {
            EcsSourcePayload::ProceduralTerrainRecipe(recipe) => {
                assert_eq!(recipe.page, key);
                assert_eq!(recipe.world_seed, manifest.world_seed);
                assert_eq!(recipe.generator_version, manifest.generator_version.get());
                assert_eq!(recipe.desc_digest, manifest.generator_desc().desc_digest());
                assert_eq!(recipe.page_origin_world_ft, crate::IVec3::new(0, 0, 0));
                assert_eq!(recipe.page_edge_voxels, u16::from(VOXEL_BRICK_EDGE_CELLS));
                assert_eq!(recipe.voxel_edge_um, VOXEL_CELL_EDGE_UM);
                assert_eq!(recipe.checksum_value(), manifest.page_checksum(key).value);
            }
            EcsSourcePayload::CompressedPage(_) | EcsSourcePayload::Failure(_) => {
                panic!("procedural source emitted non-recipe payload")
            }
        }
    }

    #[test]
    fn terrain_generation_is_deterministic_for_manifest_and_page_key() {
        let manifest = EcsProceduralWorldManifest::default();
        let key = page(0, 1, 0);
        let recipe = manifest.recipe_ref_for_page(key);
        let a = generate_procedural_terrain_page(
            manifest,
            recipe,
            EcsSpatialSourceId::new(1),
            manifest.source_epoch(),
            9,
            0,
        )
        .expect("generate a");
        let b = generate_procedural_terrain_page(
            manifest,
            recipe,
            EcsSpatialSourceId::new(1),
            manifest.source_epoch(),
            9,
            0,
        )
        .expect("generate b");

        assert_eq!(a, b);
        let brick = a.voxel_brick.expect("voxel brick");
        assert_eq!(brick.key, key);
        assert_eq!(brick.occupancy.words.len(), VOXEL_BRICK_OCCUPANCY_WORDS);
        assert!(brick.clusters.iter().any(|cluster| {
            cluster.occupancy_popcount != 0 && cluster.occupancy_popcount != CLUSTER_FULL_POPCOUNT
        }));
    }

    #[test]
    fn generated_page_digest_is_seed_version_and_page_key_contract() {
        let manifest = EcsProceduralWorldManifest::default();
        let key = page(-2, 1, -3);
        let a = generate_procedural_generated_page(manifest, manifest.recipe_ref_for_page(key), 17)
            .expect("generate a");
        let b = generate_procedural_generated_page(manifest, manifest.recipe_ref_for_page(key), 17)
            .expect("generate b");
        let other_manifest = EcsProceduralWorldManifest {
            world_seed: manifest.world_seed ^ 0x5eed,
            ..manifest
        };
        let other = generate_procedural_generated_page(
            other_manifest,
            other_manifest.recipe_ref_for_page(key),
            17,
        )
        .expect("generate other seed");

        assert_eq!(a, b);
        assert_eq!(a.page, key);
        assert_eq!(a.digest.generator_version, manifest.generator_version.get());
        assert_eq!(a.digest.page_key_hash, key.chunk_key().get());
        assert_ne!(a.digest.combined_digest, other.digest.combined_digest);
    }

    #[test]
    fn terrain_height_uses_fixed_point_fbm_and_xz_with_y_vertical() {
        let manifest = EcsProceduralWorldManifest::default();
        let height_a = terrain_height_ft(manifest, 16, -12).expect("height a");
        let height_b = terrain_height_ft(manifest, 16, -12).expect("height b");
        let height_neighbor = terrain_height_ft(manifest, 17, -12).expect("height neighbor");
        let other_seed = terrain_height_ft(
            EcsProceduralWorldManifest {
                world_seed: manifest.world_seed ^ 0x100,
                ..manifest
            },
            16,
            -12,
        )
        .expect("other seed height");

        assert_eq!(height_a, height_b);
        assert_ne!(height_a, other_seed);
        assert!(height_a.abs() < 512);
        assert!(height_neighbor.abs() < 512);
    }

    #[test]
    fn page_height_relation_fast_paths_empty_and_uniform_pages() {
        let manifest = EcsProceduralWorldManifest::default();
        assert_eq!(
            estimate_page_height_relation(manifest, page(0, 4, 0)).expect("above relation"),
            PageHeightRelation::EntirelyAboveTerrain
        );
        assert_eq!(
            estimate_page_height_relation(manifest, page(0, -3, 0)).expect("below relation"),
            PageHeightRelation::EntirelyBelowTerrain
        );
        assert_eq!(
            estimate_page_height_relation(manifest, page(0, 1, 0)).expect("surface relation"),
            PageHeightRelation::IntersectsSurface
        );
    }

    #[test]
    fn bedrock_quarry_profile_generates_ridges_cliff_bands_and_strata() {
        let manifest = EcsProceduralWorldManifest::default();
        let mut saw_surface = false;
        let mut saw_feature = false;
        let mut saw_ridge = false;
        let mut saw_cliff = false;
        let mut saw_strata = false;

        for z in -4..=4 {
            for x in -4..=4 {
                for y in 0..=1 {
                    let record = generate_procedural_terrain_page(
                        manifest,
                        manifest.recipe_ref_for_page(page(x, y, z)),
                        EcsSpatialSourceId::new(1),
                        manifest.source_epoch(),
                        1,
                        0,
                    )
                    .expect("generate terrain sample");
                    let brick = record.voxel_brick.expect("voxel brick");
                    saw_surface |=
                        brick.generated_class == ProceduralGeneratedPageClass::SurfaceMixed;
                    saw_feature |= brick.generated_class
                        == ProceduralGeneratedPageClass::MostlyAirWithFeatures;
                    for cluster in brick.clusters {
                        saw_ridge |= (cluster.flags & CLUSTER_FLAG_RIDGE) != 0;
                        saw_cliff |= (cluster.flags & CLUSTER_FLAG_CLIFF) != 0;
                        saw_strata |= (cluster.flags & CLUSTER_FLAG_STRATA_MIXED) != 0;
                    }
                }
            }
        }

        assert!(saw_surface);
        assert!(saw_feature);
        assert!(saw_ridge);
        assert!(saw_cliff);
        assert!(saw_strata);
    }

    #[test]
    fn generated_pages_classify_empty_uniform_surface_and_feature_costs() {
        let manifest = EcsProceduralWorldManifest::default();
        let empty = generate_procedural_generated_page(
            manifest,
            manifest.recipe_ref_for_page(page(0, 4, 0)),
            manifest.source_epoch(),
        )
        .expect("generate empty page");
        let uniform = generate_procedural_generated_page(
            manifest,
            manifest.recipe_ref_for_page(page(0, -3, 0)),
            manifest.source_epoch(),
        )
        .expect("generate uniform page");
        let surface = generate_procedural_generated_page(
            manifest,
            manifest.recipe_ref_for_page(page(0, 1, 0)),
            manifest.source_epoch(),
        )
        .expect("generate surface page");

        assert_eq!(empty.class, ProceduralGeneratedPageClass::EmptyAir);
        assert_eq!(empty.payload_kind, VoxelPagePayloadKind::Empty);
        assert_eq!(empty.occupancy.kind, VoxelOccupancyStorageKind::Empty);
        assert_eq!(empty.material_palette.len, 0);
        assert!(
            empty
                .cluster_summaries
                .iter()
                .all(|cluster| cluster.occupancy_popcount == 0)
        );

        assert_eq!(uniform.class, ProceduralGeneratedPageClass::UniformSolid);
        assert_eq!(uniform.payload_kind, VoxelPagePayloadKind::UniformSolid);
        assert_eq!(
            uniform.occupancy.kind,
            VoxelOccupancyStorageKind::UniformSolid
        );
        assert_eq!(uniform.material_palette.len, 1);
        assert!(
            uniform
                .cluster_summaries
                .iter()
                .all(|cluster| cluster.occupancy_popcount == CLUSTER_FULL_POPCOUNT)
        );

        assert!(surface.class.builds_surface_artifacts());
        assert_eq!(surface.payload_kind, VoxelPagePayloadKind::PaletteRle);
        assert!(
            surface
                .cluster_summaries
                .iter()
                .any(|cluster| cluster.occupancy_popcount != 0
                    && cluster.occupancy_popcount != CLUSTER_FULL_POPCOUNT)
        );
    }

    #[test]
    fn fast_path_pages_do_not_allocate_dense_generation_scratch() {
        let manifest = EcsProceduralWorldManifest::default();
        let mut scratch = ProceduralGenerationScratch::default();

        let empty = generate_procedural_generated_page_with_scratch(
            manifest,
            manifest.recipe_ref_for_page(page(0, 4, 0)),
            manifest.source_epoch(),
            &mut scratch,
        )
        .expect("generate empty");
        assert_eq!(empty.class, ProceduralGeneratedPageClass::EmptyAir);
        assert_eq!(scratch.occupancy_words.capacity(), 0);
        assert_eq!(scratch.material_indices.capacity(), 0);
        assert_eq!(scratch.cluster_summaries.capacity(), 0);
        assert_eq!(scratch.surface_scratch.capacity(), 0);

        let uniform = generate_procedural_generated_page_with_scratch(
            manifest,
            manifest.recipe_ref_for_page(page(0, -3, 0)),
            manifest.source_epoch(),
            &mut scratch,
        )
        .expect("generate uniform");
        assert_eq!(uniform.class, ProceduralGeneratedPageClass::UniformSolid);
        assert_eq!(scratch.occupancy_words.capacity(), 0);
        assert_eq!(scratch.material_indices.capacity(), 0);
        assert_eq!(scratch.cluster_summaries.capacity(), 0);
        assert_eq!(scratch.surface_scratch.capacity(), 0);
    }

    #[test]
    fn mixed_pages_generate_cluster_order_with_tiny_palette_scratch() {
        let manifest = EcsProceduralWorldManifest::default();
        let mut scratch = ProceduralGenerationScratch::default();
        let surface = generate_procedural_generated_page_with_scratch(
            manifest,
            manifest.recipe_ref_for_page(page(0, 1, 0)),
            manifest.source_epoch(),
            &mut scratch,
        )
        .expect("generate surface");

        assert!(surface.class.builds_surface_artifacts());
        assert_eq!(surface.payload_kind, VoxelPagePayloadKind::PaletteRle);
        assert!(surface.material_palette.len <= PROCEDURAL_TINY_PALETTE_LIMIT);
        assert_eq!(scratch.occupancy_words.len(), VOXEL_BRICK_OCCUPANCY_WORDS);
        assert_eq!(
            scratch.cluster_summaries.len(),
            VOXEL_CLUSTER_SUMMARIES_PER_BRICK
        );
        assert!(scratch.dense_voxel_evaluations() > 0);
        assert!(scratch.dense_voxel_evaluations() < VOXEL_BRICK_FOOT_CELL_COUNT);
        assert!(
            scratch
                .material_indices
                .iter()
                .all(|index| usize::from(*index) < PROCEDURAL_MATERIAL_INDEX_COUNT)
        );
        assert!(!scratch.surface_scratch.is_empty());
    }

    #[test]
    fn sync_manifest_carries_only_deterministic_world_contract() {
        let manifest = EcsProceduralWorldManifest::default();
        let sync = manifest.sync_manifest();

        sync.validate().expect("sync manifest validates");
        ProceduralWorldAuthorityPolicy::SERVER_AUTHORITY_V1
            .validate()
            .expect("authority policy validates");
        assert_eq!(sync.schema_version, ECS_PROCEDURAL_TERRAIN_SCHEMA_VERSION);
        assert_eq!(sync.generator_version, manifest.generator_version.get());
        assert_eq!(sync.world_seed, manifest.world_seed);
        assert_eq!(
            sync.terrain_profile,
            manifest.biome_recipe.terrain_profile_id()
        );
        assert_eq!(
            sync.biome_table_digest,
            manifest.biome_recipe.biome_table_digest()
        );
        assert_eq!(
            sync.material_table_digest,
            manifest.biome_recipe.material_table_digest()
        );
        assert_eq!(sync.voxel_edge_um, VOXEL_CELL_EDGE_UM);
        assert_eq!(sync.page_edge_voxels, u16::from(VOXEL_BRICK_EDGE_CELLS));
        assert_eq!(sync.region_edge_pages, manifest.region_edge_pages);
        assert_eq!(
            sync.deterministic_math_mode,
            DeterministicMathMode::FixedPointQ16ValueNoise
        );
        assert!(
            sync.enabled_features
                .contains(ProceduralFeature::FootVoxelCells)
        );

        let client_manifest = sync
            .to_world_manifest(
                manifest.terrain_grid,
                manifest.channel_mask,
                manifest.biome_recipe,
            )
            .expect("client reconstructs manifest from local recipe table");
        assert_eq!(client_manifest, manifest);
    }

    #[test]
    fn sync_manifest_rejects_wrong_local_biome_or_feature_table() {
        let manifest = EcsProceduralWorldManifest::default();
        let sync = manifest.sync_manifest();
        let wrong_recipe = EcsBiomeRecipe {
            surface_material: TerrainMaterialId::new(99),
            ..manifest.biome_recipe
        };
        let missing_required_features = ProceduralWorldSyncManifest {
            enabled_features: ProceduralFeatureMask::from_bits(0),
            ..sync
        };

        assert_eq!(
            sync.to_world_manifest(manifest.terrain_grid, manifest.channel_mask, wrong_recipe),
            Err(EcsSpatialValidationError::InvalidProceduralSyncManifest)
        );
        assert_eq!(
            missing_required_features.validate(),
            Err(EcsSpatialValidationError::InvalidProceduralSyncManifest)
        );
    }

    #[test]
    fn world_handshake_validates_supported_generator_tables_and_voxel_mode() {
        let manifest = EcsProceduralWorldManifest::default();
        let spawn = IVec3::new(-48, 96, 64);
        let handshake = ProceduralWorldHandshake::from_world_manifest(manifest, spawn, 4096);
        let expected = ProceduralWorldHandshakeClientExpectation::from_world_manifest(manifest);

        handshake
            .validate_for_client(expected)
            .expect("client accepts deterministic world handshake");
        assert_eq!(handshake.world_manifest, manifest.sync_manifest());
        assert_eq!(handshake.spawn_position_ft, spawn);
        assert_eq!(handshake.server_tick, 4096);
        assert_eq!(
            handshake.initial_region_seed_table_digest,
            initial_region_seed_table_digest(
                manifest.world_seed,
                manifest.terrain_grid,
                manifest.region_edge_pages,
                spawn
            )
        );
    }

    #[test]
    fn world_handshake_rejects_table_version_and_voxel_mismatches() {
        let manifest = EcsProceduralWorldManifest::default();
        let spawn = IVec3::new(0, 64, 0);
        let handshake = ProceduralWorldHandshake::from_world_manifest(manifest, spawn, 10);
        let wrong_material = ProceduralWorldHandshakeClientExpectation {
            material_table_digest: manifest.biome_recipe.material_table_digest() ^ 0x10,
            ..ProceduralWorldHandshakeClientExpectation::from_world_manifest(manifest)
        };
        let wrong_voxel = ProceduralWorldHandshakeClientExpectation {
            voxel_edge_um: VOXEL_CELL_EDGE_UM / 2,
            ..ProceduralWorldHandshakeClientExpectation::from_world_manifest(manifest)
        };
        let unsupported_version = ProceduralWorldHandshake {
            world_manifest: ProceduralWorldSyncManifest {
                generator_version: ECS_PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1
                    .get()
                    .saturating_add(1),
                ..handshake.world_manifest
            },
            ..handshake
        };

        assert_eq!(
            handshake.validate_for_client(wrong_material),
            Err(EcsSpatialValidationError::InvalidProceduralSyncManifest)
        );
        assert_eq!(
            handshake.validate_for_client(wrong_voxel),
            Err(EcsSpatialValidationError::InvalidProceduralSyncManifest)
        );
        assert_eq!(
            unsupported_version.validate_for_client(
                ProceduralWorldHandshakeClientExpectation::from_world_manifest(manifest)
            ),
            Err(EcsSpatialValidationError::InvalidProceduralSyncManifest)
        );
    }

    #[test]
    fn region_and_page_seeds_are_derived_from_world_seed_and_spatial_key_only() {
        let manifest = EcsProceduralWorldManifest::default();
        let region_a = EcsSpatialRegionKey::new(
            EcsSpatialDomainKind::Terrain,
            manifest.terrain_grid,
            0,
            -4,
            2,
            9,
        );
        let region_b = EcsSpatialRegionKey::new(
            EcsSpatialDomainKind::Terrain,
            manifest.terrain_grid,
            0,
            -4,
            2,
            10,
        );
        let page_a = page(-33, 1, 17);
        let page_b = page(-33, 1, 18);

        let forward = [
            region_seed(manifest.world_seed, region_a),
            region_seed(manifest.world_seed, region_b),
            page_seed(manifest.world_seed, page_a),
            page_seed(manifest.world_seed, page_b),
        ];
        let reverse = [
            page_seed(manifest.world_seed, page_b),
            page_seed(manifest.world_seed, page_a),
            region_seed(manifest.world_seed, region_b),
            region_seed(manifest.world_seed, region_a),
        ];

        assert_eq!(forward[0], reverse[3]);
        assert_eq!(forward[1], reverse[2]);
        assert_eq!(forward[2], reverse[1]);
        assert_eq!(forward[3], reverse[0]);
        assert_ne!(forward[0], forward[1]);
        assert_ne!(forward[2], forward[3]);
        assert_ne!(
            forward[0],
            region_seed(manifest.world_seed ^ 0x55aa, region_a)
        );
        assert_ne!(forward[2], page_seed(manifest.world_seed ^ 0x55aa, page_a));
    }

    #[test]
    fn empty_delta_layer_header_is_reserved_and_manifest_bound() {
        let manifest = EcsProceduralWorldManifest::default();
        let sync = manifest.sync_manifest();
        let header = ProceduralTerrainDeltaLayerHeader::empty_for_manifest(sync);

        assert!(header.is_empty());
        header
            .validate_for_manifest(sync)
            .expect("empty delta layer is accepted for prototype");
        assert_eq!(header.base_manifest_digest, sync.manifest_digest());

        let edited = ProceduralTerrainDeltaLayerHeader {
            delta_epoch: 1,
            delta_digest: header.delta_digest ^ 0x77,
            ..header
        };
        assert_eq!(
            edited.validate_for_manifest(sync),
            Err(EcsSpatialValidationError::InvalidProceduralSyncManifest)
        );
    }

    #[test]
    fn page_digest_is_multiplayer_checksum_for_manifest_version_and_page() {
        let manifest = EcsProceduralWorldManifest::default();
        let key = page(0, 0, 0);
        let server_digest =
            generate_procedural_page_digest(manifest, key).expect("server digest generated");
        let client_digest =
            generate_procedural_page_digest(manifest, key).expect("client digest generated");

        assert_eq!(server_digest, client_digest);
        assert_eq!(
            server_digest.generator_version,
            manifest.generator_version.get()
        );
        assert_eq!(server_digest.page_key_hash, key.chunk_key().get());
        server_digest
            .validate_for_page(manifest, key)
            .expect("digest validates for page");
    }

    #[test]
    fn digest_samples_create_mismatch_reports_only() {
        let key = page(0, 1, 0);
        let server_manifest = EcsProceduralWorldManifest::default();
        let client_manifest = EcsProceduralWorldManifest {
            world_seed: server_manifest.world_seed ^ 0x55aa,
            ..server_manifest
        };
        let server_digest =
            generate_procedural_page_digest(server_manifest, key).expect("server digest");
        let matching_sample =
            ProceduralPageDigestSample::new(NetworkPlayerId::new(2), key, server_digest, 512);

        matching_sample.validate().expect("sample validates");
        assert!(
            matching_sample
                .into_probe(server_digest)
                .mismatch_report(64)
                .is_none()
        );

        let client_digest =
            generate_procedural_page_digest(client_manifest, key).expect("client digest");
        let report = matching_sample
            .into_probe(client_digest)
            .mismatch_report(65)
            .expect("mismatch report is produced");

        report.validate().expect("mismatch report validates");
        assert_eq!(report.player_id, NetworkPlayerId::new(2));
        assert_eq!(report.page, key);
        assert_eq!(report.client_tick, 65);
        assert_ne!(report.server_digest, report.client_digest);
    }

    #[test]
    fn digest_probe_reports_mismatch_without_page_contents() {
        let key = page(0, 1, 0);
        let server_manifest = EcsProceduralWorldManifest::default();
        let client_manifest = EcsProceduralWorldManifest {
            world_seed: server_manifest.world_seed ^ 0x55aa,
            ..server_manifest
        };
        let server_digest =
            generate_procedural_page_digest(server_manifest, key).expect("server digest");
        let client_digest =
            generate_procedural_page_digest(client_manifest, key).expect("client digest");
        let probe = ProceduralPageDigestProbe::new(
            NetworkPlayerId::new(44),
            key,
            server_digest,
            client_digest,
        );

        assert!(probe.should_report_to_server());
        assert_eq!(
            probe.validate(),
            Err(EcsSpatialValidationError::ProceduralPageDigestMismatch)
        );
    }

    #[test]
    fn digest_probe_rejects_client_supplied_match_flag_drift() {
        let manifest = EcsProceduralWorldManifest::default();
        let key = page(0, 0, 0);
        let digest = generate_procedural_page_digest(manifest, key).expect("digest");
        let probe = ProceduralPageDigestProbe {
            player_id: NetworkPlayerId::new(7),
            page: key,
            server_digest: digest,
            client_digest: digest,
            matched: false,
        };

        assert_eq!(
            probe.validate(),
            Err(EcsSpatialValidationError::InvalidProceduralDigestProbe)
        );
    }

    #[test]
    fn terrain_generation_changes_with_seed_without_changing_page_key() {
        let key = page(0, 1, 0);
        let manifest_a = EcsProceduralWorldManifest::default();
        let manifest_b = EcsProceduralWorldManifest {
            world_seed: manifest_a.world_seed ^ 0x55aa,
            ..manifest_a
        };
        let a = generate_procedural_terrain_page(
            manifest_a,
            manifest_a.recipe_ref_for_page(key),
            EcsSpatialSourceId::new(1),
            manifest_a.source_epoch(),
            1,
            0,
        )
        .expect("generate a");
        let b = generate_procedural_terrain_page(
            manifest_b,
            manifest_b.recipe_ref_for_page(key),
            EcsSpatialSourceId::new(1),
            manifest_b.source_epoch(),
            1,
            0,
        )
        .expect("generate b");

        assert_ne!(a.voxel_brick, b.voxel_brick);
    }

    #[test]
    fn terrain_generation_rejects_mismatched_manifest_recipe() {
        let key = page(0, 0, 0);
        let manifest = EcsProceduralWorldManifest::default();
        let other_manifest = EcsProceduralWorldManifest {
            world_seed: manifest.world_seed ^ 1,
            ..manifest
        };
        let recipe = other_manifest.recipe_ref_for_page(key);

        assert_eq!(
            generate_procedural_terrain_page(
                manifest,
                recipe,
                EcsSpatialSourceId::new(1),
                manifest.source_epoch(),
                1,
                0,
            ),
            Err(EcsSpatialValidationError::InvalidProceduralRecipe)
        );
    }

    #[test]
    fn terrain_source_rejects_non_manifest_page_channel_as_failure_payload() {
        let manifest = EcsProceduralWorldManifest::default();
        let source = EcsProceduralTerrainSource::new(EcsSpatialSourceId::new(7), manifest);
        let key = EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            0,
            0,
            0,
            EcsPageChannel::WeatherExtinction,
        );
        let request = source.request_page(key);

        assert!(matches!(request.payload, EcsSourcePayload::Failure(_)));
    }

    #[test]
    fn invalid_material_recipe_rejects_before_generation() {
        let manifest = EcsProceduralWorldManifest {
            biome_recipe: EcsBiomeRecipe {
                surface_material: TerrainMaterialId::new(u32::from(u16::MAX) + 1),
                ..EcsBiomeRecipe::default()
            },
            ..EcsProceduralWorldManifest::default()
        };

        assert_eq!(
            manifest.validate(),
            Err(EcsSpatialValidationError::InvalidMaterial)
        );
    }

    #[test]
    fn procedural_payload_has_no_compressed_source_bytes() {
        let manifest = EcsProceduralWorldManifest::default();
        let recipe = manifest.recipe_ref_for_page(page(0, 0, 0));
        let payload = EcsSourcePayload::ProceduralTerrainRecipe(recipe);

        assert_eq!(recipe.validate(), Ok(()));
        assert_eq!(payload.byte_len(), 0);
    }
}
