use crate::{
    BiomeSourceId, EcsSpatialDomainKind, EcsSpatialGridId, EcsSpatialPageKey,
    EcsSpatialValidationError, FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM, IVec3,
    ProceduralTerrainProfileId, TerrainMaterialId, VOXEL_BRICK_EDGE_CELLS, VOXEL_CELL_EDGE_UM,
};

pub const PROCEDURAL_TERRAIN_DESC_SCHEMA_VERSION: u16 = 1;
pub const PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1: u32 = 1;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;
const NOISE_ONE_Q16: i64 = 65_536;
const NOISE_MASK_Q16: i32 = 0xffff;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralTerrainGeneratorDesc {
    pub schema_version: u16,
    pub generator_version: u32,
    pub world_seed: u64,
    pub grid_id: EcsSpatialGridId,
    pub voxel_edge_um: u32,
    pub page_edge_voxels: u16,
    pub terrain: ProceduralTerrainShapeDesc,
    pub biome: ProceduralBiomeDesc,
    pub materials: ProceduralMaterialDesc,
    pub features: ProceduralFeatureDesc,
    pub budgets: ProceduralGenerationBudget,
}

impl ProceduralTerrainGeneratorDesc {
    pub const BEDROCK_QUARRY: Self = Self {
        schema_version: PROCEDURAL_TERRAIN_DESC_SCHEMA_VERSION,
        generator_version: PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1,
        world_seed: 0x9e37_79b9_7f4a_7c15,
        grid_id: EcsSpatialGridId::new(1),
        voxel_edge_um: FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM,
        page_edge_voxels: VOXEL_BRICK_EDGE_CELLS as u16,
        terrain: ProceduralTerrainShapeDesc::BEDROCK_QUARRY,
        biome: ProceduralBiomeDesc::BEDROCK_QUARRY,
        materials: ProceduralMaterialDesc::BEDROCK_QUARRY,
        features: ProceduralFeatureDesc::BEDROCK_QUARRY,
        budgets: ProceduralGenerationBudget::BEDROCK_QUARRY,
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.schema_version != PROCEDURAL_TERRAIN_DESC_SCHEMA_VERSION
            || self.generator_version != PROCEDURAL_TERRAIN_GENERATOR_VERSION_V1
            || !self.grid_id.is_valid()
            || self.voxel_edge_um != VOXEL_CELL_EDGE_UM
            || self.page_edge_voxels != u16::from(VOXEL_BRICK_EDGE_CELLS)
        {
            return Err(EcsSpatialValidationError::InvalidProceduralManifest);
        }
        self.terrain.validate()?;
        self.biome.validate()?;
        self.materials.validate()?;
        self.features.validate()?;
        self.budgets.validate()
    }

    #[must_use]
    pub fn desc_digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_u16(hash, self.schema_version);
        hash = hash_u32(hash, self.generator_version);
        hash = hash_u64(hash, self.world_seed);
        hash = hash_u64(hash, self.grid_id.get());
        hash = hash_u32(hash, self.voxel_edge_um);
        hash = hash_u16(hash, self.page_edge_voxels);
        hash = hash_u64(hash, self.terrain.digest());
        hash = hash_u64(hash, self.biome.digest());
        hash = hash_u64(hash, self.materials.digest());
        hash = hash_u64(hash, self.features.digest());
        hash = hash_u64(hash, self.budgets.digest());
        hash.max(1)
    }

    #[must_use]
    pub fn page_recipe(self, page: EcsSpatialPageKey) -> ProceduralTerrainPageRecipe {
        ProceduralTerrainPageRecipe {
            desc_digest: self.desc_digest(),
            generator_version: self.generator_version,
            world_seed: self.world_seed,
            page,
            page_origin_world_ft: page_origin_world_ft(page, self.page_edge_voxels),
            page_edge_voxels: self.page_edge_voxels,
            voxel_edge_um: self.voxel_edge_um,
        }
    }
}

impl Default for ProceduralTerrainGeneratorDesc {
    fn default() -> Self {
        Self::BEDROCK_QUARRY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralTerrainShapeDesc {
    pub base_height_ft: i32,
    pub height_amplitude_ft: i32,
    pub continental_frequency_q16: u32,
    pub hill_frequency_q16: u32,
    pub ridge_frequency_q16: u32,
    pub warp_frequency_q16: u32,
    pub octaves: u8,
    pub ridge_strength_q16: u16,
    pub cliff_threshold_q16: u16,
    pub erosion_hint_q16: u16,
}

impl ProceduralTerrainShapeDesc {
    pub const BEDROCK_QUARRY: Self = Self {
        base_height_ft: 38,
        height_amplitude_ft: 18,
        continental_frequency_q16: 341,
        hill_frequency_q16: 1_365,
        ridge_frequency_q16: 512,
        warp_frequency_q16: 256,
        octaves: 3,
        ridge_strength_q16: 28_672,
        cliff_threshold_q16: 48_128,
        erosion_hint_q16: 12_288,
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.height_amplitude_ft <= 0
            || self.continental_frequency_q16 == 0
            || self.hill_frequency_q16 == 0
            || self.ridge_frequency_q16 == 0
            || self.warp_frequency_q16 == 0
            || self.octaves == 0
            || self.octaves > 8
        {
            return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
        }
        Ok(())
    }

    #[must_use]
    pub fn digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_i32(hash, self.base_height_ft);
        hash = hash_i32(hash, self.height_amplitude_ft);
        hash = hash_u32(hash, self.continental_frequency_q16);
        hash = hash_u32(hash, self.hill_frequency_q16);
        hash = hash_u32(hash, self.ridge_frequency_q16);
        hash = hash_u32(hash, self.warp_frequency_q16);
        hash = hash_u8(hash, self.octaves);
        hash = hash_u16(hash, self.ridge_strength_q16);
        hash = hash_u16(hash, self.cliff_threshold_q16);
        hash_u16(hash, self.erosion_hint_q16).max(1)
    }
}

impl Default for ProceduralTerrainShapeDesc {
    fn default() -> Self {
        Self::BEDROCK_QUARRY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralBiomeDesc {
    pub profile: ProceduralTerrainProfileId,
    pub biome: BiomeSourceId,
    pub surface_depth_ft: u8,
    pub bedrock_depth_ft: u16,
    pub water_level_ft: i32,
}

impl ProceduralBiomeDesc {
    pub const BEDROCK_QUARRY: Self = Self {
        profile: ProceduralTerrainProfileId::new(1),
        biome: BiomeSourceId::new(1),
        surface_depth_ft: 3,
        bedrock_depth_ft: 24,
        water_level_ft: -4096,
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if !self.profile.is_valid()
            || !self.biome.is_valid()
            || self.surface_depth_ft == 0
            || self.bedrock_depth_ft <= u16::from(self.surface_depth_ft)
        {
            return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
        }
        Ok(())
    }

    #[must_use]
    pub fn digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_u64(hash, self.profile.get());
        hash = hash_u64(hash, self.biome.get());
        hash = hash_u8(hash, self.surface_depth_ft);
        hash = hash_u16(hash, self.bedrock_depth_ft);
        hash_i32(hash, self.water_level_ft).max(1)
    }
}

impl Default for ProceduralBiomeDesc {
    fn default() -> Self {
        Self::BEDROCK_QUARRY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralMaterialDesc {
    pub grass: TerrainMaterialId,
    pub dirt: TerrainMaterialId,
    pub rock: TerrainMaterialId,
    pub sand: TerrainMaterialId,
    pub snow: TerrainMaterialId,
    pub debug_magenta: TerrainMaterialId,
}

impl ProceduralMaterialDesc {
    pub const BEDROCK_QUARRY: Self = Self {
        grass: TerrainMaterialId::new(1),
        dirt: TerrainMaterialId::new(2),
        rock: TerrainMaterialId::new(3),
        sand: TerrainMaterialId::new(4),
        snow: TerrainMaterialId::new(5),
        debug_magenta: TerrainMaterialId::new(255),
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        validate_material(self.grass)?;
        validate_material(self.dirt)?;
        validate_material(self.rock)?;
        validate_material(self.sand)?;
        validate_material(self.snow)?;
        validate_material(self.debug_magenta)
    }

    #[must_use]
    pub fn digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_u32(hash, self.grass.get());
        hash = hash_u32(hash, self.dirt.get());
        hash = hash_u32(hash, self.rock.get());
        hash = hash_u32(hash, self.sand.get());
        hash = hash_u32(hash, self.snow.get());
        hash_u32(hash, self.debug_magenta.get()).max(1)
    }
}

impl Default for ProceduralMaterialDesc {
    fn default() -> Self {
        Self::BEDROCK_QUARRY
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralFeatureDesc {
    pub caves_enabled: bool,
    pub water_enabled: bool,
    pub strata_enabled: bool,
    pub cliffs_enabled: bool,
    pub debug_checker_enabled: bool,
}

impl ProceduralFeatureDesc {
    pub const BEDROCK_QUARRY: Self = Self {
        caves_enabled: false,
        water_enabled: false,
        strata_enabled: true,
        cliffs_enabled: true,
        debug_checker_enabled: false,
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.caves_enabled || self.water_enabled {
            return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
        }
        Ok(())
    }

    #[must_use]
    pub fn digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_bool(hash, self.caves_enabled);
        hash = hash_bool(hash, self.water_enabled);
        hash = hash_bool(hash, self.strata_enabled);
        hash = hash_bool(hash, self.cliffs_enabled);
        hash_bool(hash, self.debug_checker_enabled).max(1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralGenerationBudget {
    pub max_pages_generated_per_frame: u16,
    pub max_voxels_evaluated_per_frame: u32,
    pub max_surface_pages_built_per_frame: u16,
    pub max_physics_proxies_built_per_frame: u16,
    pub max_generation_ns_per_frame: u64,
}

impl ProceduralGenerationBudget {
    pub const BEDROCK_QUARRY: Self = Self {
        max_pages_generated_per_frame: 64,
        max_voxels_evaluated_per_frame: (VOXEL_BRICK_EDGE_CELLS as u32)
            * (VOXEL_BRICK_EDGE_CELLS as u32)
            * (VOXEL_BRICK_EDGE_CELLS as u32)
            * 64,
        max_surface_pages_built_per_frame: 24,
        max_physics_proxies_built_per_frame: 16,
        max_generation_ns_per_frame: 2_000_000,
    };

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.max_pages_generated_per_frame == 0
            || self.max_voxels_evaluated_per_frame == 0
            || self.max_surface_pages_built_per_frame == 0
            || self.max_physics_proxies_built_per_frame == 0
            || self.max_generation_ns_per_frame == 0
        {
            return Err(EcsSpatialValidationError::InvalidProceduralManifest);
        }
        Ok(())
    }

    #[must_use]
    pub fn digest(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_u16(hash, self.max_pages_generated_per_frame);
        hash = hash_u32(hash, self.max_voxels_evaluated_per_frame);
        hash = hash_u16(hash, self.max_surface_pages_built_per_frame);
        hash = hash_u16(hash, self.max_physics_proxies_built_per_frame);
        hash_u64(hash, self.max_generation_ns_per_frame).max(1)
    }
}

impl Default for ProceduralGenerationBudget {
    fn default() -> Self {
        Self::BEDROCK_QUARRY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ProceduralTerrainPageRecipe {
    pub desc_digest: u64,
    pub generator_version: u32,
    pub world_seed: u64,
    pub page: EcsSpatialPageKey,
    pub page_origin_world_ft: IVec3,
    pub page_edge_voxels: u16,
    pub voxel_edge_um: u32,
}

impl ProceduralTerrainPageRecipe {
    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if self.desc_digest == 0
            || self.generator_version == 0
            || self.page.domain != EcsSpatialDomainKind::Terrain
            || !self.page.grid_id.is_valid()
            || self.page_edge_voxels != u16::from(VOXEL_BRICK_EDGE_CELLS)
            || self.voxel_edge_um != VOXEL_CELL_EDGE_UM
        {
            return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
        }
        Ok(())
    }

    pub fn validate_for_desc(
        self,
        desc: ProceduralTerrainGeneratorDesc,
    ) -> Result<(), EcsSpatialValidationError> {
        self.validate()?;
        desc.validate()?;
        if self.desc_digest != desc.desc_digest()
            || self.generator_version != desc.generator_version
            || self.world_seed != desc.world_seed
            || self.page.grid_id != desc.grid_id
            || self.page_origin_world_ft != page_origin_world_ft(self.page, desc.page_edge_voxels)
            || self.page_edge_voxels != desc.page_edge_voxels
            || self.voxel_edge_um != desc.voxel_edge_um
        {
            return Err(EcsSpatialValidationError::InvalidProceduralRecipe);
        }
        Ok(())
    }

    #[must_use]
    pub fn checksum_value(self) -> u64 {
        let mut hash = FNV_OFFSET;
        hash = hash_u64(hash, self.desc_digest);
        hash = hash_u32(hash, self.generator_version);
        hash = hash_u64(hash, self.world_seed);
        hash = hash_page_key(hash, self.page);
        hash = hash_i32(hash, self.page_origin_world_ft.x);
        hash = hash_i32(hash, self.page_origin_world_ft.y);
        hash = hash_i32(hash, self.page_origin_world_ft.z);
        hash = hash_u16(hash, self.page_edge_voxels);
        hash_u32(hash, self.voxel_edge_um).max(1)
    }
}

pub type EcsProceduralRecipeRef = ProceduralTerrainPageRecipe;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PageHeightRelation {
    EntirelyAboveTerrain = 0,
    EntirelyBelowTerrain = 1,
    IntersectsSurface = 2,
}

impl PageHeightRelation {
    #[must_use]
    pub const fn requires_dense_voxels(self) -> bool {
        matches!(self, Self::IntersectsSurface)
    }
}

#[must_use]
pub fn hash2(seed: u64, x: i32, z: i32) -> u32 {
    let mut hash = FNV_OFFSET;
    hash = hash_u64(hash, seed);
    hash = hash_i32(hash, x);
    hash = hash_i32(hash, z);
    avalanche(hash) as u32
}

#[must_use]
pub fn hash3(seed: u64, x: i32, y: i32, z: i32) -> u32 {
    let mut hash = FNV_OFFSET;
    hash = hash_u64(hash, seed);
    hash = hash_i32(hash, x);
    hash = hash_i32(hash, y);
    hash = hash_i32(hash, z);
    avalanche(hash) as u32
}

#[must_use]
pub fn value_noise2_q16(seed: u64, x_q16: i64, z_q16: i64) -> i32 {
    let x0 = floor_q16_to_i32(x_q16);
    let z0 = floor_q16_to_i32(z_q16);
    let tx = smoothstep_q16(frac_q16(x_q16));
    let tz = smoothstep_q16(frac_q16(z_q16));

    let v00 = (hash2(seed, x0, z0) & NOISE_MASK_Q16 as u32) as i32;
    let v10 = (hash2(seed, x0.saturating_add(1), z0) & NOISE_MASK_Q16 as u32) as i32;
    let v01 = (hash2(seed, x0, z0.saturating_add(1)) & NOISE_MASK_Q16 as u32) as i32;
    let v11 =
        (hash2(seed, x0.saturating_add(1), z0.saturating_add(1)) & NOISE_MASK_Q16 as u32) as i32;

    let x_a = lerp_q16(v00, v10, tx);
    let x_b = lerp_q16(v01, v11, tx);
    lerp_q16(x_a, x_b, tz)
}

#[must_use]
pub fn value_noise3_q16(seed: u64, x_q16: i64, y_q16: i64, z_q16: i64) -> i32 {
    let x0 = floor_q16_to_i32(x_q16);
    let y0 = floor_q16_to_i32(y_q16);
    let z0 = floor_q16_to_i32(z_q16);
    let tx = smoothstep_q16(frac_q16(x_q16));
    let ty = smoothstep_q16(frac_q16(y_q16));
    let tz = smoothstep_q16(frac_q16(z_q16));

    let v000 = (hash3(seed, x0, y0, z0) & NOISE_MASK_Q16 as u32) as i32;
    let v100 = (hash3(seed, x0.saturating_add(1), y0, z0) & NOISE_MASK_Q16 as u32) as i32;
    let v010 = (hash3(seed, x0, y0.saturating_add(1), z0) & NOISE_MASK_Q16 as u32) as i32;
    let v110 = (hash3(seed, x0.saturating_add(1), y0.saturating_add(1), z0) & NOISE_MASK_Q16 as u32)
        as i32;
    let v001 = (hash3(seed, x0, y0, z0.saturating_add(1)) & NOISE_MASK_Q16 as u32) as i32;
    let v101 = (hash3(seed, x0.saturating_add(1), y0, z0.saturating_add(1)) & NOISE_MASK_Q16 as u32)
        as i32;
    let v011 = (hash3(seed, x0, y0.saturating_add(1), z0.saturating_add(1)) & NOISE_MASK_Q16 as u32)
        as i32;
    let v111 = (hash3(
        seed,
        x0.saturating_add(1),
        y0.saturating_add(1),
        z0.saturating_add(1),
    ) & NOISE_MASK_Q16 as u32) as i32;

    let x00 = lerp_q16(v000, v100, tx);
    let x10 = lerp_q16(v010, v110, tx);
    let x01 = lerp_q16(v001, v101, tx);
    let x11 = lerp_q16(v011, v111, tx);
    let y0 = lerp_q16(x00, x10, ty);
    let y1 = lerp_q16(x01, x11, ty);
    lerp_q16(y0, y1, tz)
}

#[must_use]
pub fn fbm2_q16(seed: u64, x_ft: i32, z_ft: i32, frequency_q16: u32, octaves: u8) -> i32 {
    let octave_count = octaves.clamp(1, 8);
    let mut frequency_q16 = frequency_q16.max(1);
    let mut amplitude_q16 = NOISE_ONE_Q16;
    let mut amplitude_sum_q16 = 0_i64;
    let mut sum_q32 = 0_i64;

    for octave in 0..octave_count {
        let octave_seed = seed ^ (u64::from(octave).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        let x_q16 = scale_ft_to_noise_q16(x_ft, frequency_q16);
        let z_q16 = scale_ft_to_noise_q16(z_ft, frequency_q16);
        let centered_q16 = (i64::from(value_noise2_q16(octave_seed, x_q16, z_q16)) - 32_768) * 2;
        sum_q32 = sum_q32.saturating_add(centered_q16.saturating_mul(amplitude_q16));
        amplitude_sum_q16 = amplitude_sum_q16.saturating_add(amplitude_q16);
        amplitude_q16 = (amplitude_q16 / 2).max(1);
        frequency_q16 = frequency_q16.saturating_mul(2).max(1);
    }

    if amplitude_sum_q16 == 0 {
        0
    } else {
        (sum_q32 / amplitude_sum_q16).clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
    }
}

fn validate_material(material: TerrainMaterialId) -> Result<(), EcsSpatialValidationError> {
    if material.get() == 0 || material.get() > u32::from(u16::MAX) {
        return Err(EcsSpatialValidationError::InvalidMaterial);
    }
    Ok(())
}

fn page_origin_world_ft(page: EcsSpatialPageKey, page_edge_voxels: u16) -> IVec3 {
    let page_edge_ft = i64::from(page_edge_voxels).saturating_mul(level_cell_edge_ft(page.level));
    IVec3::new(
        clamp_i64_to_i32(i64::from(page.x).saturating_mul(page_edge_ft)),
        clamp_i64_to_i32(i64::from(page.y).saturating_mul(page_edge_ft)),
        clamp_i64_to_i32(i64::from(page.z).saturating_mul(page_edge_ft)),
    )
}

fn level_cell_edge_ft(level: u8) -> i64 {
    if level >= 20 {
        1_i64 << 20
    } else {
        1_i64 << u32::from(level)
    }
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

fn floor_q16_to_i32(value_q16: i64) -> i32 {
    clamp_i64_to_i32(value_q16.div_euclid(NOISE_ONE_Q16))
}

fn frac_q16(value_q16: i64) -> u32 {
    value_q16.rem_euclid(NOISE_ONE_Q16) as u32
}

fn smoothstep_q16(t_q16: u32) -> u32 {
    let t = u64::from(t_q16);
    let t2 = (t.saturating_mul(t).saturating_add(32_768)) >> 16;
    let three_minus_two_t = 196_608_u64.saturating_sub(t.saturating_mul(2));
    ((t2.saturating_mul(three_minus_two_t).saturating_add(32_768)) >> 16).min(NOISE_ONE_Q16 as u64)
        as u32
}

fn lerp_q16(a: i32, b: i32, t_q16: u32) -> i32 {
    let t = i64::from(t_q16);
    let inv = NOISE_ONE_Q16.saturating_sub(t);
    ((i64::from(a)
        .saturating_mul(inv)
        .saturating_add(i64::from(b).saturating_mul(t))
        .saturating_add(32_768))
        >> 16)
        .clamp(0, NOISE_MASK_Q16 as i64) as i32
}

fn scale_ft_to_noise_q16(ft: i32, frequency_q16: u32) -> i64 {
    i64::from(ft).saturating_mul(i64::from(frequency_q16))
}

fn avalanche(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xff51_afd7_ed55_8ccd);
    value ^= value >> 33;
    value = value.wrapping_mul(0xc4ce_b9fe_1a85_ec53);
    value ^ (value >> 33)
}

const fn hash_bool(hash: u64, value: bool) -> u64 {
    hash_u8(hash, if value { 1 } else { 0 })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{EcsPageChannel, VOXEL_REGION_EDGE_BRICKS};

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
    fn bedrock_quarry_descriptor_validates_foot_scale_budgeted_contract() {
        let desc = ProceduralTerrainGeneratorDesc::BEDROCK_QUARRY;

        desc.validate().expect("descriptor validates");
        assert_eq!(desc.voxel_edge_um, VOXEL_CELL_EDGE_UM);
        assert_eq!(desc.page_edge_voxels, u16::from(VOXEL_BRICK_EDGE_CELLS));
        assert_eq!(desc.biome.water_level_ft, -4096);
        assert!(!desc.features.caves_enabled);
        assert!(!desc.features.water_enabled);
        assert!(desc.features.strata_enabled);
        assert!(desc.features.cliffs_enabled);
        assert_eq!(VOXEL_REGION_EDGE_BRICKS, 8);
    }

    #[test]
    fn page_recipe_bridges_stream_page_to_decode_origin_and_digest() {
        let desc = ProceduralTerrainGeneratorDesc::BEDROCK_QUARRY;
        let page = page(2, -3, 1);
        let recipe = desc.page_recipe(page);

        recipe.validate_for_desc(desc).expect("recipe validates");
        assert_eq!(recipe.desc_digest, desc.desc_digest());
        assert_eq!(recipe.generator_version, desc.generator_version);
        assert_eq!(recipe.world_seed, desc.world_seed);
        assert_eq!(recipe.page_origin_world_ft, IVec3::new(64, -96, 32));
        assert_ne!(recipe.checksum_value(), 0);
    }

    #[test]
    fn unsupported_caves_and_water_reject_in_first_generator_desc() {
        let caves = ProceduralTerrainGeneratorDesc {
            features: ProceduralFeatureDesc {
                caves_enabled: true,
                ..ProceduralFeatureDesc::BEDROCK_QUARRY
            },
            ..ProceduralTerrainGeneratorDesc::BEDROCK_QUARRY
        };
        let water = ProceduralTerrainGeneratorDesc {
            features: ProceduralFeatureDesc {
                water_enabled: true,
                ..ProceduralFeatureDesc::BEDROCK_QUARRY
            },
            ..ProceduralTerrainGeneratorDesc::BEDROCK_QUARRY
        };

        assert_eq!(
            caves.validate(),
            Err(EcsSpatialValidationError::InvalidProceduralRecipe)
        );
        assert_eq!(
            water.validate(),
            Err(EcsSpatialValidationError::InvalidProceduralRecipe)
        );
    }

    #[test]
    fn fixed_point_noise_helpers_are_deterministic_and_bounded() {
        let seed = 0xabc0_ffee_1234_5678;

        assert_eq!(hash2(seed, -7, 11), hash2(seed, -7, 11));
        assert_ne!(hash2(seed, -7, 11), hash2(seed ^ 1, -7, 11));
        assert_eq!(hash3(seed, -7, 3, 11), hash3(seed, -7, 3, 11));

        let n2 = value_noise2_q16(seed, -3_i64 * 65_536 + 12_345, 9_i64 * 65_536);
        let n3 = value_noise3_q16(seed, 65_536, -2_i64 * 65_536, 4_i64 * 65_536);
        assert!((0..=65_535).contains(&n2));
        assert!((0..=65_535).contains(&n3));

        let fbm = fbm2_q16(seed, 128, -64, 512, 4);
        assert!((-65_536..=65_536).contains(&fbm));
        assert_eq!(fbm, fbm2_q16(seed, 128, -64, 512, 4));
    }
}
