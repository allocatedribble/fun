use bevy_ecs::prelude::Resource;

use crate::{
    BiomeSourceId, EcsDecodeTelemetry, EcsDecodedPagePayloadKind, EcsDecodedPageRecord,
    EcsPageChannel, EcsPageChannelMask, EcsPageFailureCode, EcsProceduralRecipeRef,
    EcsSourceChecksum, EcsSourceChecksumAlgorithm, EcsSourceFailure, EcsSourcePayload,
    EcsSourceRequest, EcsSourceRequestId, EcsSpatialDomainKind, EcsSpatialGridId,
    EcsSpatialPageKey, EcsSpatialRegionKey, EcsSpatialRegionManifest, EcsSpatialSource,
    EcsSpatialSourceId, EcsSpatialSourceKind, EcsSpatialValidationError, EcsStreamPriority,
    NetworkPlayerId, ProceduralTerrainProfileId, TerrainMaterialId, VOXEL_BRICK_EDGE_CELLS,
    VOXEL_BRICK_FOOT_CELL_COUNT, VOXEL_CELL_EDGE_UM, VOXEL_CLUSTER_EDGE_CELLS,
    VOXEL_CLUSTER_SUMMARIES_PER_BRICK, VoxelBrickPayload, VoxelClusterSummary,
    VoxelMaterialPalette, VoxelOccupancyStorage, VoxelOccupancyStorageKind, VoxelPagePayloadKind,
};

pub const ECS_PROCEDURAL_TERRAIN_SCHEMA_VERSION: u16 = 1;
pub const ECS_PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1: EcsTerrainGeneratorVersion =
    EcsTerrainGeneratorVersion(1);
pub const ECS_PROCEDURAL_TERRAIN_DEFAULT_REGION_EDGE_PAGES: u16 = 8;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const PAGE_EDGE_FT: i64 = VOXEL_BRICK_EDGE_CELLS as i64;
const CLUSTER_EDGE_FT: usize = VOXEL_CLUSTER_EDGE_CELLS as usize;
const CLUSTER_FULL_POPCOUNT: u16 = VOXEL_CLUSTER_EDGE_CELLS as u16
    * VOXEL_CLUSTER_EDGE_CELLS as u16
    * VOXEL_CLUSTER_EDGE_CELLS as u16;
const NOISE_Q8_CENTER: i64 = 128;
const FIXED_ONE_Q16: u32 = 65_536;

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
    WaterHints = 3,
    FoliageHints = 4,
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
            | ProceduralFeature::WaterHints.bit()
            | ProceduralFeature::FoliageHints.bit(),
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
        base_height_ft: 22,
        amplitude_ft: 10,
        detail_amplitude_ft: 3,
        macro_period_ft: 128,
        detail_period_ft: 32,
        terrace_step_ft: 2,
        surface_depth_ft: 3,
        bedrock_depth_ft: 18,
        water_level_ft: 4,
        surface_material: TerrainMaterialId::new(1),
        subsurface_material: TerrainMaterialId::new(2),
        bedrock_material: TerrainMaterialId::new(3),
        water_material: TerrainMaterialId::new(4),
        foliage_density_q: 24,
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
        self.biome_recipe.validate()
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
        EcsProceduralRecipeRef {
            key,
            recipe_id: self.biome_recipe.biome.get(),
            seed: self.world_seed,
            generator_version: self.generator_version.get(),
            manifest_signature: self.signature().value,
            checksum: self.page_checksum(key),
        }
    }

    #[must_use]
    pub fn page_checksum(self, key: EcsSpatialPageKey) -> EcsSourceChecksum {
        let mut hash = self.signature().value;
        hash = hash_page_key(hash, key);
        hash = hash_u64(hash, self.biome_recipe.signature().value);
        EcsSourceChecksum {
            algorithm: EcsSourceChecksumAlgorithm::Fnv1a64,
            value: hash.max(1),
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
            EcsSourcePayload::ProceduralRecipe(self.manifest.recipe_ref_for_page(key))
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

pub fn generate_procedural_terrain_page(
    manifest: EcsProceduralWorldManifest,
    recipe: EcsProceduralRecipeRef,
    source: EcsSpatialSourceId,
    source_epoch: u32,
    decode_epoch: u32,
    overlay_count: u16,
) -> Result<EcsDecodedPageRecord, EcsSpatialValidationError> {
    manifest.validate()?;
    validate_recipe_ref(manifest, recipe)?;
    let key = recipe.key;
    let brick = generate_voxel_brick(manifest, key, source_epoch)?;
    let clusters = brick.clusters;
    Ok(EcsDecodedPageRecord {
        key,
        source,
        source_epoch,
        payload_kind: brick.kind.into_decoded_kind(),
        voxel_brick: Some(brick),
        cluster_summaries: clusters,
        telemetry: EcsDecodeTelemetry {
            key,
            source_epoch,
            decode_epoch,
            source_bytes: 0,
            overlay_count,
            cluster_summary_count: VOXEL_CLUSTER_SUMMARIES_PER_BRICK as u16,
            checksum: recipe.checksum,
            failure: None,
        },
    })
}

fn validate_recipe_ref(
    manifest: EcsProceduralWorldManifest,
    recipe: EcsProceduralRecipeRef,
) -> Result<(), EcsSpatialValidationError> {
    recipe.validate()?;
    if !manifest.accepts_page(recipe.key)
        || recipe.seed != manifest.world_seed
        || recipe.recipe_id != manifest.biome_recipe.biome.get()
        || recipe.generator_version != manifest.generator_version.get()
        || recipe.manifest_signature != manifest.signature().value
        || recipe.checksum != manifest.page_checksum(recipe.key)
    {
        return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
    }
    Ok(())
}

fn generate_voxel_brick(
    manifest: EcsProceduralWorldManifest,
    key: EcsSpatialPageKey,
    edit_epoch: u32,
) -> Result<VoxelBrickPayload, EcsSpatialValidationError> {
    let recipe = manifest.biome_recipe;
    let mut heights = [0_i32; VOXEL_BRICK_EDGE_CELLS as usize * VOXEL_BRICK_EDGE_CELLS as usize];
    let cell_edge_ft = level_cell_edge_ft(key.level);
    let origin_x = i64::from(key.x)
        .saturating_mul(PAGE_EDGE_FT)
        .saturating_mul(cell_edge_ft);
    let origin_y = i64::from(key.y)
        .saturating_mul(PAGE_EDGE_FT)
        .saturating_mul(cell_edge_ft);
    let origin_z = i64::from(key.z)
        .saturating_mul(PAGE_EDGE_FT)
        .saturating_mul(cell_edge_ft);

    for local_y in 0..VOXEL_BRICK_EDGE_CELLS as usize {
        for local_x in 0..VOXEL_BRICK_EDGE_CELLS as usize {
            let world_x = origin_x.saturating_add((local_x as i64).saturating_mul(cell_edge_ft));
            let world_y = origin_y.saturating_add((local_y as i64).saturating_mul(cell_edge_ft));
            heights[height_index(local_x, local_y)] = terrain_height_ft(manifest, world_x, world_y);
        }
    }

    let mut occupancy = VoxelOccupancyStorage::empty();
    let mut clusters = [VoxelClusterSummary::default(); VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut surface_counts = [0_u16; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut subsurface_counts = [0_u16; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut bedrock_counts = [0_u16; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut water_counts = [0_u16; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut min_local_z = [i16::MAX; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut max_local_z = [i16::MIN; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut sdf_min = [i16::MAX; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut sdf_max = [i16::MIN; VOXEL_CLUSTER_SUMMARIES_PER_BRICK];
    let mut solid_cells = 0_usize;
    let mut first_material = 0_u16;
    let mut mixed_materials = false;

    let surface_material = material_to_u16(recipe.surface_material)?;
    let subsurface_material = material_to_u16(recipe.subsurface_material)?;
    let bedrock_material = material_to_u16(recipe.bedrock_material)?;
    let water_material = material_to_u16(recipe.water_material)?;

    for local_z in 0..VOXEL_BRICK_EDGE_CELLS as usize {
        let world_z = origin_z.saturating_add((local_z as i64).saturating_mul(cell_edge_ft));
        for local_y in 0..VOXEL_BRICK_EDGE_CELLS as usize {
            for local_x in 0..VOXEL_BRICK_EDGE_CELLS as usize {
                let height = i64::from(heights[height_index(local_x, local_y)]);
                let signed_distance = clamp_i64_to_i16(height.saturating_sub(world_z));
                let cluster = cluster_index(local_x, local_y, local_z);
                sdf_min[cluster] = sdf_min[cluster].min(signed_distance);
                sdf_max[cluster] = sdf_max[cluster].max(signed_distance);
                if world_z <= height {
                    let material = material_for_depth(
                        recipe,
                        height.saturating_sub(world_z),
                        surface_material,
                        subsurface_material,
                        bedrock_material,
                    );
                    set_occupancy_bit(&mut occupancy, voxel_index(local_x, local_y, local_z));
                    clusters[cluster].occupancy_popcount =
                        clusters[cluster].occupancy_popcount.saturating_add(1);
                    min_local_z[cluster] = min_local_z[cluster].min(local_z as i16);
                    max_local_z[cluster] = max_local_z[cluster].max(local_z as i16);
                    match material {
                        value if value == surface_material => {
                            surface_counts[cluster] = surface_counts[cluster].saturating_add(1);
                        }
                        value if value == subsurface_material => {
                            subsurface_counts[cluster] =
                                subsurface_counts[cluster].saturating_add(1);
                        }
                        _ => {
                            bedrock_counts[cluster] = bedrock_counts[cluster].saturating_add(1);
                        }
                    }
                    if first_material == 0 {
                        first_material = material;
                    } else if first_material != material {
                        mixed_materials = true;
                    }
                    solid_cells += 1;
                } else if world_z <= i64::from(recipe.water_level_ft) {
                    water_counts[cluster] = water_counts[cluster].saturating_add(1);
                }
            }
        }
    }

    for index in 0..VOXEL_CLUSTER_SUMMARIES_PER_BRICK {
        let popcount = clusters[index].occupancy_popcount;
        clusters[index].dominant_material = dominant_material([
            (surface_material, surface_counts[index]),
            (subsurface_material, subsurface_counts[index]),
            (bedrock_material, bedrock_counts[index]),
            (water_material, water_counts[index]),
        ]);
        clusters[index].exposed_face_mask = exposed_face_mask(popcount);
        clusters[index].min_height_local = if popcount == 0 { 0 } else { min_local_z[index] };
        clusters[index].max_height_local = if popcount == 0 { 0 } else { max_local_z[index] };
        clusters[index].sdf_min_q = normalize_sdf_bound(sdf_min[index]);
        clusters[index].sdf_max_q = normalize_sdf_bound(sdf_max[index]);
        clusters[index].foliage_density_q = if surface_counts[index] == 0 {
            0
        } else {
            recipe.foliage_density_q
        };
        clusters[index].water_q = water_counts[index].min(u16::from(u8::MAX)) as u8;
        clusters[index].flags = cluster_flags(
            surface_counts[index],
            bedrock_counts[index],
            water_counts[index],
        );
    }

    let mut palette = VoxelMaterialPalette::empty();
    push_palette_material(&mut palette, surface_material);
    push_palette_material(&mut palette, subsurface_material);
    push_palette_material(&mut palette, bedrock_material);
    if water_counts.iter().any(|count| *count != 0) {
        push_palette_material(&mut palette, water_material);
    }

    occupancy.kind = occupancy_kind(solid_cells, mixed_materials);
    Ok(VoxelBrickPayload {
        key,
        kind: page_payload_kind(solid_cells, mixed_materials),
        occupancy,
        material_palette: palette,
        clusters,
        edit_epoch,
    })
}

fn terrain_height_ft(
    manifest: EcsProceduralWorldManifest,
    world_x_ft: i64,
    world_y_ft: i64,
) -> i32 {
    let recipe = manifest.biome_recipe;
    let macro_noise = value_noise_q8(
        manifest.world_seed,
        manifest.generator_version,
        recipe.biome.get(),
        recipe.macro_period_ft,
        world_x_ft,
        world_y_ft,
        0x51,
    );
    let detail_noise = value_noise_q8(
        manifest.world_seed,
        manifest.generator_version,
        recipe.biome.get(),
        recipe.detail_period_ft,
        world_x_ft,
        world_y_ft,
        0xa7,
    );
    let macro_delta =
        (i64::from(macro_noise) - NOISE_Q8_CENTER) * i64::from(recipe.amplitude_ft) / 128;
    let detail_delta =
        (i64::from(detail_noise) - NOISE_Q8_CENTER) * i64::from(recipe.detail_amplitude_ft) / 128;
    let mut height = i64::from(recipe.base_height_ft)
        .saturating_add(macro_delta)
        .saturating_add(detail_delta);
    let terrace = i64::from(recipe.terrace_step_ft);
    if terrace > 1 {
        height = height.div_euclid(terrace).saturating_mul(terrace);
    }
    clamp_i64_to_i32(height)
}

fn value_noise_q8(
    seed: u64,
    version: EcsTerrainGeneratorVersion,
    recipe_id: u64,
    period_ft: u16,
    x_ft: i64,
    y_ft: i64,
    salt: u8,
) -> i32 {
    let period = i64::from(period_ft.max(1));
    let gx = x_ft.div_euclid(period);
    let gy = y_ft.div_euclid(period);
    let tx = fixed_fraction_q16(x_ft.rem_euclid(period), period);
    let ty = fixed_fraction_q16(y_ft.rem_euclid(period), period);
    let sx = smoothstep_q16(tx);
    let sy = smoothstep_q16(ty);

    let v00 = lattice_hash_q8(seed, version, recipe_id, gx, gy, salt);
    let v10 = lattice_hash_q8(seed, version, recipe_id, gx + 1, gy, salt);
    let v01 = lattice_hash_q8(seed, version, recipe_id, gx, gy + 1, salt);
    let v11 = lattice_hash_q8(seed, version, recipe_id, gx + 1, gy + 1, salt);
    let x0 = lerp_q8(v00, v10, sx);
    let x1 = lerp_q8(v01, v11, sx);
    lerp_q8(x0, x1, sy)
}

fn lattice_hash_q8(
    seed: u64,
    version: EcsTerrainGeneratorVersion,
    recipe_id: u64,
    gx: i64,
    gy: i64,
    salt: u8,
) -> i32 {
    let mut hash = FNV_OFFSET;
    hash = hash_u64(hash, seed);
    hash = hash_u32(hash, version.get());
    hash = hash_u64(hash, recipe_id);
    hash = hash_i64(hash, gx);
    hash = hash_i64(hash, gy);
    hash = hash_u8(hash, salt);
    ((avalanche(hash) >> 56) & 0xff) as i32
}

fn fixed_fraction_q16(remainder: i64, period: i64) -> u32 {
    let value = (remainder.saturating_mul(i64::from(FIXED_ONE_Q16)) / period).max(0);
    value.min(i64::from(FIXED_ONE_Q16)) as u32
}

fn smoothstep_q16(t_q16: u32) -> u32 {
    let t = u64::from(t_q16);
    let t2 = (t.saturating_mul(t).saturating_add(32_768)) >> 16;
    let three_minus_two_t = 196_608_u64.saturating_sub(t.saturating_mul(2));
    ((t2.saturating_mul(three_minus_two_t).saturating_add(32_768)) >> 16)
        .min(u64::from(FIXED_ONE_Q16)) as u32
}

fn lerp_q8(a: i32, b: i32, t_q16: u32) -> i32 {
    let t = i64::from(t_q16);
    let inv = i64::from(FIXED_ONE_Q16).saturating_sub(t);
    ((i64::from(a)
        .saturating_mul(inv)
        .saturating_add(i64::from(b).saturating_mul(t))
        .saturating_add(32_768))
        >> 16) as i32
}

fn material_for_depth(
    recipe: EcsBiomeRecipe,
    depth_ft: i64,
    surface: u16,
    subsurface: u16,
    bedrock: u16,
) -> u16 {
    if depth_ft >= i64::from(recipe.bedrock_depth_ft) {
        bedrock
    } else if depth_ft > i64::from(recipe.surface_depth_ft) {
        subsurface
    } else {
        surface
    }
}

fn material_to_u16(material: TerrainMaterialId) -> Result<u16, EcsSpatialValidationError> {
    if material.get() == 0 || material.get() > u32::from(u16::MAX) {
        return Err(EcsSpatialValidationError::InvalidMaterial);
    }
    Ok(material.get() as u16)
}

fn occupancy_kind(solid_cells: usize, mixed_materials: bool) -> VoxelOccupancyStorageKind {
    if solid_cells == 0 {
        VoxelOccupancyStorageKind::Empty
    } else if solid_cells == VOXEL_BRICK_FOOT_CELL_COUNT && !mixed_materials {
        VoxelOccupancyStorageKind::UniformSolid
    } else {
        VoxelOccupancyStorageKind::Bitset32
    }
}

fn page_payload_kind(solid_cells: usize, mixed_materials: bool) -> VoxelPagePayloadKind {
    if solid_cells == 0 {
        VoxelPagePayloadKind::Empty
    } else if solid_cells == VOXEL_BRICK_FOOT_CELL_COUNT && !mixed_materials {
        VoxelPagePayloadKind::UniformSolid
    } else {
        VoxelPagePayloadKind::DenseFootCells
    }
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
            VoxelPagePayloadKind::ProceduralRecipeRef => {
                EcsDecodedPagePayloadKind::ProceduralRecipeRef
            }
        }
    }
}

fn dominant_material(candidates: [(u16, u16); 4]) -> u16 {
    let mut material = candidates[0].0;
    let mut count = candidates[0].1;
    for (candidate_material, candidate_count) in candidates.iter().copied().skip(1) {
        if candidate_count > count {
            material = candidate_material;
            count = candidate_count;
        }
    }
    material
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

fn cluster_flags(surface_count: u16, bedrock_count: u16, water_count: u16) -> u16 {
    u16::from(surface_count != 0)
        | (u16::from(bedrock_count != 0) << 1)
        | (u16::from(water_count != 0) << 2)
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

fn set_occupancy_bit(occupancy: &mut VoxelOccupancyStorage, index: usize) {
    debug_assert!(index < VOXEL_BRICK_FOOT_CELL_COUNT);
    let word = index / 64;
    let bit = index % 64;
    occupancy.words[word] |= 1_u64 << bit;
}

fn height_index(local_x: usize, local_y: usize) -> usize {
    local_y * VOXEL_BRICK_EDGE_CELLS as usize + local_x
}

fn voxel_index(local_x: usize, local_y: usize, local_z: usize) -> usize {
    (local_z * VOXEL_BRICK_EDGE_CELLS as usize + local_y) * VOXEL_BRICK_EDGE_CELLS as usize
        + local_x
}

fn cluster_index(local_x: usize, local_y: usize, local_z: usize) -> usize {
    let cx = local_x / CLUSTER_EDGE_FT;
    let cy = local_y / CLUSTER_EDGE_FT;
    let cz = local_z / CLUSTER_EDGE_FT;
    (cz * 4 + cy) * 4 + cx
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

fn page_key_digest(key: EcsSpatialPageKey) -> u64 {
    hash_page_key(FNV_OFFSET, key).max(1)
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

const fn hash_i64(hash: u64, value: i64) -> u64 {
    hash_u64(hash, value as u64)
}

fn avalanche(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
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
            EcsSourcePayload::ProceduralRecipe(recipe) => {
                assert_eq!(recipe.key, key);
                assert_eq!(recipe.seed, manifest.world_seed);
                assert_eq!(recipe.generator_version, manifest.generator_version.get());
                assert_eq!(recipe.manifest_signature, manifest.signature().value);
                assert_eq!(recipe.checksum, manifest.page_checksum(key));
            }
            EcsSourcePayload::CompressedPage(_) | EcsSourcePayload::Failure(_) => {
                panic!("procedural source emitted non-recipe payload")
            }
        }
    }

    #[test]
    fn terrain_generation_is_deterministic_for_manifest_and_page_key() {
        let manifest = EcsProceduralWorldManifest::default();
        let key = page(0, 0, 0);
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
    fn digest_probe_reports_mismatch_without_page_contents() {
        let key = page(0, 0, 0);
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
        let key = page(0, 0, 0);
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
        let payload = EcsSourcePayload::ProceduralRecipe(recipe);

        assert_eq!(recipe.validate(), Ok(()));
        assert_eq!(payload.byte_len(), 0);
    }
}
