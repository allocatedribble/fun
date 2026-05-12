use crate::{
    EcsAabbI64, EcsPageChannel, EcsSpatialDomainKind, EcsSpatialGridDesc, EcsSpatialPageKey,
    EcsSpatialScalePreset, EcsSpatialValidationError, FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM,
};

pub const VOXEL_CELL_EDGE_UM: u32 = FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM;
pub const VOXEL_CLUSTER_EDGE_CELLS: u8 = 8;
pub const VOXEL_BRICK_EDGE_CELLS: u8 = 32;
pub const VOXEL_REGION_EDGE_BRICKS: u8 = 8;
pub const VOXEL_REGION_EDGE_CELLS: u16 = 256;
pub const VOXEL_MEGAREGION_EDGE_REGIONS: u8 = 8;
pub const VOXEL_MEGAREGION_EDGE_CELLS: u32 = 2_048;
pub const VOXEL_CLUSTERS_PER_BRICK_AXIS: usize =
    (VOXEL_BRICK_EDGE_CELLS / VOXEL_CLUSTER_EDGE_CELLS) as usize;
pub const VOXEL_CLUSTER_SUMMARIES_PER_BRICK: usize =
    VOXEL_CLUSTERS_PER_BRICK_AXIS * VOXEL_CLUSTERS_PER_BRICK_AXIS * VOXEL_CLUSTERS_PER_BRICK_AXIS;
pub const VOXEL_BRICK_FOOT_CELL_COUNT: usize = VOXEL_BRICK_EDGE_CELLS as usize
    * VOXEL_BRICK_EDGE_CELLS as usize
    * VOXEL_BRICK_EDGE_CELLS as usize;
pub const VOXEL_BRICK_OCCUPANCY_WORDS: usize = VOXEL_BRICK_FOOT_CELL_COUNT / 64;
pub const VOXEL_MATERIAL_PALETTE_CAPACITY: usize = 256;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum MaterialPalettePolicy {
    GlobalTerrain = 0,
    #[default]
    PerRegion = 1,
    PerBrickCompact = 2,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VoxelEditPolicy {
    Immutable = 0,
    #[default]
    RuntimeMutable = 1,
    AuthoringOnly = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelGridDesc {
    pub spatial_grid: EcsSpatialGridDesc,
    pub cluster_edge_voxels: u8,
    pub brick_edge_voxels: u8,
    pub region_edge_voxels: u16,
    pub material_palette_policy: MaterialPalettePolicy,
    pub edit_policy: VoxelEditPolicy,
}

impl VoxelGridDesc {
    #[must_use]
    pub fn terrain_default() -> Self {
        Self {
            spatial_grid: EcsSpatialGridDesc::terrain_foot_default(),
            cluster_edge_voxels: VOXEL_CLUSTER_EDGE_CELLS,
            brick_edge_voxels: VOXEL_BRICK_EDGE_CELLS,
            region_edge_voxels: VOXEL_REGION_EDGE_CELLS,
            material_palette_policy: MaterialPalettePolicy::PerRegion,
            edit_policy: VoxelEditPolicy::RuntimeMutable,
        }
    }

    #[must_use]
    pub const fn cell_edge_um(&self) -> u32 {
        self.spatial_grid.unit_edge_um
    }

    #[must_use]
    pub const fn cluster_edge_um(&self) -> u64 {
        self.spatial_grid.unit_edge_um as u64 * self.cluster_edge_voxels as u64
    }

    #[must_use]
    pub const fn brick_edge_um(&self) -> u64 {
        self.spatial_grid.unit_edge_um as u64 * self.brick_edge_voxels as u64
    }

    #[must_use]
    pub const fn region_edge_um(&self) -> u64 {
        self.spatial_grid.unit_edge_um as u64 * self.region_edge_voxels as u64
    }

    #[must_use]
    pub const fn mega_region_edge_voxels(&self) -> u32 {
        self.region_edge_voxels as u32 * VOXEL_MEGAREGION_EDGE_REGIONS as u32
    }

    pub fn validate(&self) -> Result<(), EcsSpatialValidationError> {
        self.spatial_grid.validate()?;
        if self.spatial_grid.domain != EcsSpatialDomainKind::Terrain
            || self.spatial_grid.unit_edge_um != VOXEL_CELL_EDGE_UM
            || self.spatial_grid.cell_edge_units != 1
            || self.spatial_grid.cluster_edge_cells != self.cluster_edge_voxels
            || self.spatial_grid.page_edge_cells != self.brick_edge_voxels as u16
            || self.spatial_grid.region_edge_pages != VOXEL_REGION_EDGE_BRICKS as u16
            || self.cluster_edge_voxels != VOXEL_CLUSTER_EDGE_CELLS
            || self.brick_edge_voxels != VOXEL_BRICK_EDGE_CELLS
            || self.region_edge_voxels != VOXEL_REGION_EDGE_CELLS
            || self.mega_region_edge_voxels() != VOXEL_MEGAREGION_EDGE_CELLS
        {
            return Err(EcsSpatialValidationError::GridInvalidDimension);
        }
        Ok(())
    }
}

impl Default for VoxelGridDesc {
    fn default() -> Self {
        Self::terrain_default()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VoxelPagePayloadKind {
    #[default]
    Empty = 0,
    UniformSolid = 1,
    PaletteRle = 2,
    DenseFootCells = 3,
    ProceduralRecipeRef = 4,
    CoarseProxyOnly = 5,
    SurfaceOnly = 6,
    SdfOnly = 7,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VoxelOccupancyStorageKind {
    #[default]
    Empty = 0,
    UniformSolid = 1,
    Bitset32 = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelOccupancyStorage {
    pub kind: VoxelOccupancyStorageKind,
    pub words: [u64; VOXEL_BRICK_OCCUPANCY_WORDS],
}

impl VoxelOccupancyStorage {
    pub const EMPTY: Self = Self {
        kind: VoxelOccupancyStorageKind::Empty,
        words: [0; VOXEL_BRICK_OCCUPANCY_WORDS],
    };

    pub const UNIFORM_SOLID: Self = Self {
        kind: VoxelOccupancyStorageKind::UniformSolid,
        words: [u64::MAX; VOXEL_BRICK_OCCUPANCY_WORDS],
    };

    #[must_use]
    pub const fn empty() -> Self {
        Self::EMPTY
    }

    #[must_use]
    pub const fn uniform_solid() -> Self {
        Self::UNIFORM_SOLID
    }
}

impl Default for VoxelOccupancyStorage {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VoxelMaterialPalette {
    pub len: u16,
    pub materials: [u16; VOXEL_MATERIAL_PALETTE_CAPACITY],
}

impl VoxelMaterialPalette {
    pub const EMPTY: Self = Self {
        len: 0,
        materials: [0; VOXEL_MATERIAL_PALETTE_CAPACITY],
    };

    #[must_use]
    pub const fn empty() -> Self {
        Self::EMPTY
    }

    #[must_use]
    pub fn single(material: u16) -> Self {
        let mut palette = Self::EMPTY;
        palette.len = 1;
        palette.materials[0] = material;
        palette
    }
}

impl Default for VoxelMaterialPalette {
    fn default() -> Self {
        Self::EMPTY
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VoxelClusterSummary {
    pub occupancy_popcount: u16,
    pub exposed_face_mask: u8,
    pub dominant_material: u16,
    pub min_height_local: i16,
    pub max_height_local: i16,
    pub sdf_min_q: i16,
    pub sdf_max_q: i16,
    pub foliage_density_q: u8,
    pub water_q: u8,
    pub flags: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoxelBrickPayload {
    pub key: EcsSpatialPageKey,
    pub kind: VoxelPagePayloadKind,
    pub occupancy: VoxelOccupancyStorage,
    pub material_palette: VoxelMaterialPalette,
    pub clusters: [VoxelClusterSummary; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
    pub edit_epoch: u32,
}

impl VoxelBrickPayload {
    #[must_use]
    pub const fn empty(key: EcsSpatialPageKey, edit_epoch: u32) -> Self {
        Self {
            key,
            kind: VoxelPagePayloadKind::Empty,
            occupancy: VoxelOccupancyStorage::EMPTY,
            material_palette: VoxelMaterialPalette::EMPTY,
            clusters: [VoxelClusterSummary {
                occupancy_popcount: 0,
                exposed_face_mask: 0,
                dominant_material: 0,
                min_height_local: 0,
                max_height_local: 0,
                sdf_min_q: 0,
                sdf_max_q: 0,
                foliage_density_q: 0,
                water_q: 0,
                flags: 0,
            }; VOXEL_CLUSTER_SUMMARIES_PER_BRICK],
            edit_epoch,
        }
    }

    #[must_use]
    pub fn uniform_solid(key: EcsSpatialPageKey, material: u16, edit_epoch: u32) -> Self {
        let mut payload = Self::empty(key, edit_epoch);
        payload.kind = VoxelPagePayloadKind::UniformSolid;
        payload.occupancy = VoxelOccupancyStorage::UNIFORM_SOLID;
        payload.material_palette = VoxelMaterialPalette::single(material);
        for cluster in &mut payload.clusters {
            cluster.occupancy_popcount = u16::from(VOXEL_CLUSTER_EDGE_CELLS)
                * u16::from(VOXEL_CLUSTER_EDGE_CELLS)
                * u16::from(VOXEL_CLUSTER_EDGE_CELLS);
            cluster.dominant_material = material;
        }
        payload
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsFineOverlayOverride {
    #[default]
    Material = 0,
    MicroSdf = 1,
    Displacement = 2,
    Decals = 3,
    Erosion = 4,
    DestructionScars = 5,
    TireTracks = 6,
    Footprints = 7,
}

impl EcsFineOverlayOverride {
    #[must_use]
    pub const fn bit(self) -> u16 {
        1_u16 << (self as u8)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsFineOverlayOverrideMask {
    bits: u16,
}

impl EcsFineOverlayOverrideMask {
    pub const EMPTY: Self = Self { bits: 0 };
    pub const ALL_TERRAIN_DETAIL: Self = Self {
        bits: EcsFineOverlayOverride::Material.bit()
            | EcsFineOverlayOverride::MicroSdf.bit()
            | EcsFineOverlayOverride::Displacement.bit()
            | EcsFineOverlayOverride::Decals.bit()
            | EcsFineOverlayOverride::Erosion.bit()
            | EcsFineOverlayOverride::DestructionScars.bit()
            | EcsFineOverlayOverride::TireTracks.bit()
            | EcsFineOverlayOverride::Footprints.bit(),
    };

    #[must_use]
    pub const fn from_bits(bits: u16) -> Self {
        Self { bits }
    }

    #[must_use]
    pub const fn contains(self, channel: EcsFineOverlayOverride) -> bool {
        (self.bits & channel.bit()) != 0
    }

    #[must_use]
    pub const fn bits(self) -> u16 {
        self.bits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsFineOverlayLink {
    pub parent_page: EcsSpatialPageKey,
    pub overlay_page: EcsSpatialPageKey,
    pub overlay_scale: EcsSpatialScalePreset,
    pub bounds: EcsAabbI64,
}

impl EcsFineOverlayLink {
    pub const DEFAULT_OVERRIDES: EcsFineOverlayOverrideMask =
        EcsFineOverlayOverrideMask::ALL_TERRAIN_DETAIL;

    #[must_use]
    pub const fn new(
        parent_page: EcsSpatialPageKey,
        overlay_page: EcsSpatialPageKey,
        overlay_scale: EcsSpatialScalePreset,
        bounds: EcsAabbI64,
    ) -> Self {
        Self {
            parent_page,
            overlay_page,
            overlay_scale,
            bounds,
        }
    }

    pub fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if !self.overlay_scale.is_valid() {
            return Err(EcsSpatialValidationError::ZeroScale);
        }
        if !self.overlay_scale.is_fine_overlay_scale() {
            return Err(EcsSpatialValidationError::OverlayScaleNotFine);
        }
        if !self.bounds.is_bounded_non_empty() {
            return Err(EcsSpatialValidationError::FineOverlayMustBeBounded);
        }
        if self.parent_page.domain != EcsSpatialDomainKind::Terrain
            || self.parent_page.channel == EcsPageChannel::FineOverlay
        {
            return Err(EcsSpatialValidationError::FineOverlayParentInvalid);
        }
        if self.overlay_page.domain != EcsSpatialDomainKind::Terrain
            || self.overlay_page.channel != EcsPageChannel::FineOverlay
            || self.overlay_page.grid_id.get() == 0
        {
            return Err(EcsSpatialValidationError::FineOverlayPageInvalid);
        }
        Ok(())
    }
}
