use bevy_ecs::prelude::{Component, Resource};
use fun_scheduler_types::{EcsChunkKey, EcsSpatialDomainKind, EcsVirtualResourceKey};
use smallvec::SmallVec;

use crate::{
    BiomeSourceId, DenseSlotMap, ECS_SPATIAL_MAX_GRIDS, EcsSpatialGridId, EcsSpatialSourceId,
    EcsSpatialVolumeId, FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM, FUN_INCH_VOXEL_EDGE_UM,
    FoliageSpeciesPaletteId, TerrainMaterialId, VoxelTerrainId, WeatherProfileId,
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EcsSpatialScalePreset {
    #[default]
    GameplayFoot,
    HalfFootDebug,
    InchHeroPatch,
    CustomMicrometers(u32),
}

impl EcsSpatialScalePreset {
    #[must_use]
    pub const fn edge_um(self) -> u32 {
        match self {
            Self::GameplayFoot => FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM,
            Self::HalfFootDebug => FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM / 2,
            Self::InchHeroPatch => FUN_INCH_VOXEL_EDGE_UM,
            Self::CustomMicrometers(value) => value,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::GameplayFoot => "gameplay_foot",
            Self::HalfFootDebug => "half_foot_debug",
            Self::InchHeroPatch => "inch_hero_patch",
            Self::CustomMicrometers(_) => "custom_micrometers",
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.edge_um() != 0
    }

    #[must_use]
    pub const fn is_fine_overlay_scale(self) -> bool {
        self.edge_um() < FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM
    }

    #[must_use]
    pub const fn is_global_world_default_allowed(self) -> bool {
        match self {
            Self::GameplayFoot => true,
            Self::HalfFootDebug | Self::InchHeroPatch => false,
            Self::CustomMicrometers(value) => value >= FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EcsSpatialValidationError {
    ZeroScale,
    FineScaleCannotBeGlobal,
    OverlayScaleNotFine,
    FineOverlayMustBeBounded,
    FineOverlayParentInvalid,
    FineOverlayPageInvalid,
    GridInvalidDimension,
    GridHasNoClipLevels,
    GridHasNoChannels,
    GridRegistryFull,
    GridRegistryDuplicate,
    PageTableColumnMismatch,
    PageTableDuplicateKey,
    PageTableFull,
    DirtyRegionQueueFull,
    StreamInterestTableFull,
    StreamRequestQueueFull,
    SourceAcquireQueueFull,
    SourcePayloadTooLarge,
    SourcePayloadLengthMismatch,
    SourceRequestMismatch,
    DecodeQueueFull,
    DecodeOverlayQueueFull,
    DecodeMissingPayload,
    InvalidProceduralManifest,
    InvalidProceduralRecipe,
    UnsupportedProceduralVersion,
    DerivedArtifactRegistryFull,
    HandoffQueueFull,
    LoadAnimationQueueFull,
    InvalidAabb,
    InvalidEditBounds,
    InvalidEditRadius,
    InvalidMaterial,
    InvalidAssetRef,
    InvalidTransform,
    StaleCommandBuffer,
    RevisionLedgerFull,
    InvalidTableKey,
    DuplicateTableKey,
    ResourceTableFull,
}

impl EcsSpatialValidationError {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::ZeroScale => "zero_scale",
            Self::FineScaleCannotBeGlobal => "fine_scale_cannot_be_global",
            Self::OverlayScaleNotFine => "overlay_scale_not_fine",
            Self::FineOverlayMustBeBounded => "fine_overlay_must_be_bounded",
            Self::FineOverlayParentInvalid => "fine_overlay_parent_invalid",
            Self::FineOverlayPageInvalid => "fine_overlay_page_invalid",
            Self::GridInvalidDimension => "grid_invalid_dimension",
            Self::GridHasNoClipLevels => "grid_has_no_clip_levels",
            Self::GridHasNoChannels => "grid_has_no_channels",
            Self::GridRegistryFull => "grid_registry_full",
            Self::GridRegistryDuplicate => "grid_registry_duplicate",
            Self::PageTableColumnMismatch => "page_table_column_mismatch",
            Self::PageTableDuplicateKey => "page_table_duplicate_key",
            Self::PageTableFull => "page_table_full",
            Self::DirtyRegionQueueFull => "dirty_region_queue_full",
            Self::StreamInterestTableFull => "stream_interest_table_full",
            Self::StreamRequestQueueFull => "stream_request_queue_full",
            Self::SourceAcquireQueueFull => "source_acquire_queue_full",
            Self::SourcePayloadTooLarge => "source_payload_too_large",
            Self::SourcePayloadLengthMismatch => "source_payload_length_mismatch",
            Self::SourceRequestMismatch => "source_request_mismatch",
            Self::DecodeQueueFull => "decode_queue_full",
            Self::DecodeOverlayQueueFull => "decode_overlay_queue_full",
            Self::DecodeMissingPayload => "decode_missing_payload",
            Self::InvalidProceduralManifest => "invalid_procedural_manifest",
            Self::InvalidProceduralRecipe => "invalid_procedural_recipe",
            Self::UnsupportedProceduralVersion => "unsupported_procedural_version",
            Self::DerivedArtifactRegistryFull => "derived_artifact_registry_full",
            Self::HandoffQueueFull => "handoff_queue_full",
            Self::LoadAnimationQueueFull => "load_animation_queue_full",
            Self::InvalidAabb => "invalid_aabb",
            Self::InvalidEditBounds => "invalid_edit_bounds",
            Self::InvalidEditRadius => "invalid_edit_radius",
            Self::InvalidMaterial => "invalid_material",
            Self::InvalidAssetRef => "invalid_asset_ref",
            Self::InvalidTransform => "invalid_transform",
            Self::StaleCommandBuffer => "stale_command_buffer",
            Self::RevisionLedgerFull => "revision_ledger_full",
            Self::InvalidTableKey => "invalid_table_key",
            Self::DuplicateTableKey => "duplicate_table_key",
            Self::ResourceTableFull => "resource_table_full",
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct EcsAabbF32 {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl EcsAabbF32 {
    pub const ZERO: Self = Self {
        min: [0.0, 0.0, 0.0],
        max: [0.0, 0.0, 0.0],
    };

    pub const WHOLE_WORLD: Self = Self {
        min: [f32::NEG_INFINITY; 3],
        max: [f32::INFINITY; 3],
    };

    #[must_use]
    pub const fn new(min: [f32; 3], max: [f32; 3]) -> Self {
        Self { min, max }
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.min[0] <= self.max[0]
            && self.min[1] <= self.max[1]
            && self.min[2] <= self.max[2]
            && !self.min[0].is_nan()
            && !self.min[1].is_nan()
            && !self.min[2].is_nan()
            && !self.max[0].is_nan()
            && !self.max[1].is_nan()
            && !self.max[2].is_nan()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsSpatialEntityKind {
    #[default]
    TerrainVolume = 0,
    StreamCamera = 1,
    StreamSource = 2,
    AuthoringTool = 3,
    DebugPin = 4,
    HighLevelWorldObject = 5,
}

impl EcsSpatialEntityKind {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TerrainVolume => "terrain_volume",
            Self::StreamCamera => "stream_camera",
            Self::StreamSource => "stream_source",
            Self::AuthoringTool => "authoring_tool",
            Self::DebugPin => "debug_pin",
            Self::HighLevelWorldObject => "high_level_world_object",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::TerrainVolume,
            Self::StreamCamera,
            Self::StreamSource,
            Self::AuthoringTool,
            Self::DebugPin,
            Self::HighLevelWorldObject,
        ]
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsBoundsUm {
    pub min: [i64; 3],
    pub max: [i64; 3],
}

pub type EcsAabbI64 = EcsBoundsUm;

impl EcsBoundsUm {
    #[must_use]
    pub const fn new(min: [i64; 3], max: [i64; 3]) -> Self {
        Self { min, max }
    }

    #[must_use]
    pub const fn is_bounded_non_empty(self) -> bool {
        self.min[0] < self.max[0] && self.min[1] < self.max[1] && self.min[2] < self.max[2]
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialPageCoord {
    pub level: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl EcsSpatialPageCoord {
    #[must_use]
    pub const fn new(level: u8, x: i32, y: i32, z: i32) -> Self {
        Self { level, x, y, z }
    }

    #[must_use]
    pub fn chunk_key(self, domain: EcsSpatialDomainKind) -> EcsChunkKey {
        EcsSpatialPageKey::new(
            domain,
            EcsSpatialGridId::new(1),
            self.level,
            self.x,
            self.y,
            self.z,
            EcsPageChannel::Debug,
        )
        .chunk_key()
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum EcsPageChannel {
    #[default]
    Occupancy = 0,
    Material = 1,
    Surface = 2,
    NarrowBandSdf = 3,
    CoarseProxy = 4,
    FoliageSeed = 5,
    FoliageGeometry = 6,
    CanopyOpacity = 7,
    Radiance = 8,
    VirtualShadow = 9,
    FineOverlay = 10,
    WeatherExtinction = 11,
    Navigation = 12,
    AudioOcclusion = 13,
    NetworkRelevance = 14,
    Debug = 15,
}

impl EcsPageChannel {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Occupancy => "occupancy",
            Self::Material => "material",
            Self::Surface => "surface",
            Self::NarrowBandSdf => "narrow_band_sdf",
            Self::CoarseProxy => "coarse_proxy",
            Self::FoliageSeed => "foliage_seed",
            Self::FoliageGeometry => "foliage_geometry",
            Self::CanopyOpacity => "canopy_opacity",
            Self::Radiance => "radiance",
            Self::VirtualShadow => "virtual_shadow",
            Self::FineOverlay => "fine_overlay",
            Self::WeatherExtinction => "weather_extinction",
            Self::Navigation => "navigation",
            Self::AudioOcclusion => "audio_occlusion",
            Self::NetworkRelevance => "network_relevance",
            Self::Debug => "debug",
        }
    }

    #[must_use]
    pub const fn all() -> &'static [Self] {
        &[
            Self::Occupancy,
            Self::Material,
            Self::Surface,
            Self::NarrowBandSdf,
            Self::CoarseProxy,
            Self::FoliageSeed,
            Self::FoliageGeometry,
            Self::CanopyOpacity,
            Self::Radiance,
            Self::VirtualShadow,
            Self::FineOverlay,
            Self::WeatherExtinction,
            Self::Navigation,
            Self::AudioOcclusion,
            Self::NetworkRelevance,
            Self::Debug,
        ]
    }

    #[must_use]
    pub const fn bit(self) -> u32 {
        1_u32 << (self as u8)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsPageChannelMask {
    bits: u32,
}

impl EcsPageChannelMask {
    pub const EMPTY: Self = Self { bits: 0 };

    #[must_use]
    pub const fn from_bits(bits: u32) -> Self {
        Self { bits }
    }

    #[must_use]
    pub const fn from_channel(channel: EcsPageChannel) -> Self {
        Self {
            bits: channel.bit(),
        }
    }

    #[must_use]
    pub const fn terrain_primary() -> Self {
        Self {
            bits: EcsPageChannel::Occupancy.bit()
                | EcsPageChannel::Material.bit()
                | EcsPageChannel::Surface.bit()
                | EcsPageChannel::NarrowBandSdf.bit()
                | EcsPageChannel::CoarseProxy.bit(),
        }
    }

    #[must_use]
    pub const fn foliage_pages() -> Self {
        Self {
            bits: EcsPageChannel::FoliageSeed.bit()
                | EcsPageChannel::FoliageGeometry.bit()
                | EcsPageChannel::CanopyOpacity.bit()
                | EcsPageChannel::VirtualShadow.bit(),
        }
    }

    #[must_use]
    pub const fn storm_volume_pages() -> Self {
        Self {
            bits: EcsPageChannel::WeatherExtinction.bit()
                | EcsPageChannel::Radiance.bit()
                | EcsPageChannel::Debug.bit(),
        }
    }

    #[must_use]
    pub const fn with(self, channel: EcsPageChannel) -> Self {
        Self {
            bits: self.bits | channel.bit(),
        }
    }

    #[must_use]
    pub const fn contains(self, channel: EcsPageChannel) -> bool {
        (self.bits & channel.bit()) != 0
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.bits == 0
    }

    #[must_use]
    pub const fn bits(self) -> u32 {
        self.bits
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsClipLevelDesc {
    pub level: u8,
    pub cell_edge_units: u32,
    pub page_edge_cells: u16,
    pub max_resident_pages: u32,
}

impl EcsClipLevelDesc {
    #[must_use]
    pub const fn new(
        level: u8,
        cell_edge_units: u32,
        page_edge_cells: u16,
        max_resident_pages: u32,
    ) -> Self {
        Self {
            level,
            cell_edge_units,
            page_edge_cells,
            max_resident_pages,
        }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.cell_edge_units != 0 && self.page_edge_cells != 0 && self.max_resident_pages != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EcsSpatialGridDesc {
    pub domain: EcsSpatialDomainKind,
    pub unit_edge_um: u32,
    pub cell_edge_units: u32,
    pub cluster_edge_cells: u8,
    pub page_edge_cells: u16,
    pub region_edge_pages: u16,
    pub clip_levels: SmallVec<[EcsClipLevelDesc; 8]>,
    pub channel_mask: EcsPageChannelMask,
}

impl EcsSpatialGridDesc {
    #[must_use]
    pub fn terrain_foot_default() -> Self {
        let mut clip_levels = SmallVec::new();
        clip_levels.push(EcsClipLevelDesc::new(0, 1, 32, 16_384));
        clip_levels.push(EcsClipLevelDesc::new(1, 2, 32, 8_192));
        clip_levels.push(EcsClipLevelDesc::new(2, 4, 32, 4_096));
        Self {
            domain: EcsSpatialDomainKind::Terrain,
            unit_edge_um: FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM,
            cell_edge_units: 1,
            cluster_edge_cells: 8,
            page_edge_cells: 32,
            region_edge_pages: 8,
            clip_levels,
            channel_mask: EcsPageChannelMask::terrain_primary(),
        }
    }

    #[must_use]
    pub fn foliage_default() -> Self {
        let mut clip_levels = SmallVec::new();
        clip_levels.push(EcsClipLevelDesc::new(0, 1, 32, 8_192));
        clip_levels.push(EcsClipLevelDesc::new(1, 2, 32, 4_096));
        clip_levels.push(EcsClipLevelDesc::new(2, 4, 32, 2_048));
        Self {
            domain: EcsSpatialDomainKind::Foliage,
            unit_edge_um: FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM,
            cell_edge_units: 1,
            cluster_edge_cells: 8,
            page_edge_cells: 32,
            region_edge_pages: 8,
            clip_levels,
            channel_mask: EcsPageChannelMask::foliage_pages(),
        }
    }

    #[must_use]
    pub fn storm_volume_default() -> Self {
        let mut clip_levels = SmallVec::new();
        clip_levels.push(EcsClipLevelDesc::new(0, 1, 16, 4_096));
        clip_levels.push(EcsClipLevelDesc::new(1, 2, 16, 2_048));
        clip_levels.push(EcsClipLevelDesc::new(2, 4, 16, 1_024));
        Self {
            domain: EcsSpatialDomainKind::WeatherVolume,
            unit_edge_um: FUN_DEFAULT_TERRAIN_VOXEL_EDGE_UM,
            cell_edge_units: 1,
            cluster_edge_cells: 8,
            page_edge_cells: 16,
            region_edge_pages: 8,
            clip_levels,
            channel_mask: EcsPageChannelMask::storm_volume_pages(),
        }
    }

    pub fn validate(&self) -> Result<(), EcsSpatialValidationError> {
        if self.unit_edge_um == 0
            || self.cell_edge_units == 0
            || self.cluster_edge_cells == 0
            || self.page_edge_cells == 0
            || self.region_edge_pages == 0
        {
            return Err(EcsSpatialValidationError::GridInvalidDimension);
        }
        if self.clip_levels.is_empty() {
            return Err(EcsSpatialValidationError::GridHasNoClipLevels);
        }
        if self.channel_mask.is_empty() {
            return Err(EcsSpatialValidationError::GridHasNoChannels);
        }
        for level in &self.clip_levels {
            if !level.is_valid() {
                return Err(EcsSpatialValidationError::GridInvalidDimension);
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Resource)]
pub struct EcsSpatialGridRegistry {
    pub grids: DenseSlotMap<EcsSpatialGridId, EcsSpatialGridDesc>,
}

impl EcsSpatialGridRegistry {
    #[must_use]
    pub fn len(&self) -> usize {
        self.grids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.grids.is_empty()
    }

    pub fn push(
        &mut self,
        grid_id: EcsSpatialGridId,
        grid: EcsSpatialGridDesc,
    ) -> Result<(), EcsSpatialValidationError> {
        if self.len() >= ECS_SPATIAL_MAX_GRIDS {
            return Err(EcsSpatialValidationError::GridRegistryFull);
        }
        if self.grids.contains_key(grid_id) {
            return Err(EcsSpatialValidationError::GridRegistryDuplicate);
        }
        grid.validate()?;
        self.grids.insert(grid_id, grid);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, grid_id: EcsSpatialGridId) -> Option<&EcsSpatialGridDesc> {
        self.grids.get(grid_id)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EcsSpatialPageKey {
    pub domain: EcsSpatialDomainKind,
    pub grid_id: EcsSpatialGridId,
    pub level: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub channel: EcsPageChannel,
}

impl EcsSpatialPageKey {
    #[must_use]
    pub const fn new(
        domain: EcsSpatialDomainKind,
        grid_id: EcsSpatialGridId,
        level: u8,
        x: i32,
        y: i32,
        z: i32,
        channel: EcsPageChannel,
    ) -> Self {
        Self {
            domain,
            grid_id,
            level,
            x,
            y,
            z,
            channel,
        }
    }

    #[must_use]
    pub fn chunk_key(self) -> EcsChunkKey {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash = fnv1a_u8(hash, self.domain as u8);
        hash = fnv1a_u64(hash, self.grid_id.get());
        hash = fnv1a_u8(hash, self.level);
        hash = fnv1a_u32(hash, self.x as u32);
        hash = fnv1a_u32(hash, self.y as u32);
        hash = fnv1a_u32(hash, self.z as u32);
        hash = fnv1a_u8(hash, self.channel as u8);
        EcsChunkKey::new(hash.max(1))
    }

    #[must_use]
    pub fn virtual_resource_key(self, generation: u32) -> EcsVirtualResourceKey {
        EcsVirtualResourceKey::new(
            self.domain,
            self.grid_id.get() as u16,
            self.level,
            self.channel as u16,
            self.chunk_key(),
            generation,
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EcsSpatialRegionKey {
    pub domain: EcsSpatialDomainKind,
    pub grid_id: EcsSpatialGridId,
    pub level: u8,
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl EcsSpatialRegionKey {
    #[must_use]
    pub const fn new(
        domain: EcsSpatialDomainKind,
        grid_id: EcsSpatialGridId,
        level: u8,
        x: i32,
        y: i32,
        z: i32,
    ) -> Self {
        Self {
            domain,
            grid_id,
            level,
            x,
            y,
            z,
        }
    }

    #[must_use]
    pub fn from_page(page: EcsSpatialPageKey, region_edge_pages: u16) -> Self {
        let divisor = i32::from(region_edge_pages.max(1));
        Self {
            domain: page.domain,
            grid_id: page.grid_id,
            level: page.level,
            x: floor_div_i32(page.x, divisor),
            y: floor_div_i32(page.y, divisor),
            z: floor_div_i32(page.z, divisor),
        }
    }

    #[must_use]
    pub fn chunk_key(self) -> EcsChunkKey {
        let mut hash = 0xcbf2_9ce4_8422_2325_u64;
        hash = fnv1a_u8(hash, self.domain as u8);
        hash = fnv1a_u64(hash, self.grid_id.get());
        hash = fnv1a_u8(hash, self.level);
        hash = fnv1a_u32(hash, self.x as u32);
        hash = fnv1a_u32(hash, self.y as u32);
        hash = fnv1a_u32(hash, self.z as u32);
        EcsChunkKey::new(hash.max(1))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsTerrainVoxelModel {
    pub primary_scale: EcsSpatialScalePreset,
    pub fine_overlay_page_cap: u32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StreamCameraRole {
    #[default]
    Disabled = 0,
    MainView = 1,
    CinematicView = 2,
    Minimap = 3,
    ReflectionProbe = 4,
    EditorPreview = 5,
}

#[derive(Debug, Clone, Copy, PartialEq, Component)]
pub struct SpatialStreamCamera {
    pub role: StreamCameraRole,
    pub enabled: bool,
    pub priority: u8,
    pub required_shells: u16,
    pub desired_shells: u16,
    pub velocity_lookahead_s: f32,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VoxelCollisionPolicy {
    Disabled = 0,
    CoarseProxy = 1,
    #[default]
    NarrowBandSdf = 2,
    FullResolution = 3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VoxelRenderPolicy {
    Hidden = 0,
    CoarseProxy = 1,
    #[default]
    SurfacePackets = 2,
    FineOverlay = 3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum VoxelFoliagePolicy {
    #[default]
    Disabled = 0,
    SeedOnly = 1,
    Clusters = 2,
    ClustersAndCanopy = 3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FoliagePlacementPolicy {
    #[default]
    BiomeDensity = 0,
    TerrainMaterialDriven = 1,
    ArtistPainted = 2,
    ProceduralSuccession = 3,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum FoliageRenderPolicy {
    Hidden = 0,
    SeedOnly = 1,
    #[default]
    NearGeometryMidCardsFarCanopy = 2,
    CanopyOnly = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct VoxelTerrainVolume {
    pub grid: EcsSpatialGridId,
    pub terrain_id: VoxelTerrainId,
    pub default_material: TerrainMaterialId,
    pub source: EcsSpatialSourceId,
    pub collision_policy: VoxelCollisionPolicy,
    pub render_policy: VoxelRenderPolicy,
    pub foliage_policy: VoxelFoliagePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FineDetailOverlayVolume {
    pub parent_grid: EcsSpatialGridId,
    pub scale: EcsSpatialScalePreset,
    pub bounds: EcsAabbI64,
    pub channel_mask: EcsPageChannelMask,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct FoliageField {
    pub source_grid: EcsSpatialGridId,
    pub biome_source: BiomeSourceId,
    pub species_palette: FoliageSpeciesPaletteId,
    pub placement_policy: FoliagePlacementPolicy,
    pub render_policy: FoliageRenderPolicy,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum StormClipmapPolicy {
    Disabled = 0,
    #[default]
    CameraRelative = 1,
    WorldAnchored = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct StormVolumeField {
    pub profile: WeatherProfileId,
    pub clipmap_levels: u8,
    pub near_resolution: [u16; 3],
    pub max_distance_ft: u32,
    pub temporal_reprojection: bool,
}

impl StormVolumeField {
    #[must_use]
    pub const fn is_streaming_independent_from_terrain(self) -> bool {
        self.clipmap_levels != 0
            && self.near_resolution[0] != 0
            && self.near_resolution[1] != 0
            && self.near_resolution[2] != 0
            && self.max_distance_ft != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StormVolumeStreamingRules {
    pub can_hide_streaming_transitions: bool,
    pub required_for_terrain_correctness: bool,
    pub evicted_with_terrain_pages: bool,
}

impl StormVolumeStreamingRules {
    pub const PRODUCT_DEFAULT: Self = Self {
        can_hide_streaming_transitions: true,
        required_for_terrain_correctness: false,
        evicted_with_terrain_pages: false,
    };

    #[must_use]
    pub const fn contract_holds(self) -> bool {
        self.can_hide_streaming_transitions
            && !self.required_for_terrain_correctness
            && !self.evicted_with_terrain_pages
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DebugPinReason {
    #[default]
    Authoring = 0,
    ResidencyDebug = 1,
    ArtifactDebug = 2,
    StreamingDebug = 3,
    Test = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct SpatialDebugPin {
    pub key: EcsSpatialPageKey,
    pub reason: DebugPinReason,
}

impl EcsTerrainVoxelModel {
    pub const PRODUCT_DEFAULT: Self = Self {
        primary_scale: EcsSpatialScalePreset::GameplayFoot,
        fine_overlay_page_cap: 0,
    };

    #[must_use]
    pub const fn primary_cell_edge_um(self) -> u32 {
        self.primary_scale.edge_um()
    }

    pub const fn validate_global_default(self) -> Result<(), EcsSpatialValidationError> {
        if !self.primary_scale.is_valid() {
            return Err(EcsSpatialValidationError::ZeroScale);
        }
        if !self.primary_scale.is_global_world_default_allowed() {
            return Err(EcsSpatialValidationError::FineScaleCannotBeGlobal);
        }
        Ok(())
    }
}

impl Default for EcsTerrainVoxelModel {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcsFineOverlayDecl {
    pub scale: EcsSpatialScalePreset,
    pub bounds: EcsBoundsUm,
    pub max_pages: u32,
}

impl EcsFineOverlayDecl {
    #[must_use]
    pub const fn inch_hero_patch(bounds: EcsBoundsUm, max_pages: u32) -> Self {
        Self {
            scale: EcsSpatialScalePreset::InchHeroPatch,
            bounds,
            max_pages,
        }
    }

    pub const fn validate(self) -> Result<(), EcsSpatialValidationError> {
        if !self.scale.is_valid() {
            return Err(EcsSpatialValidationError::ZeroScale);
        }
        if !self.scale.is_fine_overlay_scale() {
            return Err(EcsSpatialValidationError::OverlayScaleNotFine);
        }
        if !self.bounds.is_bounded_non_empty() || self.max_pages == 0 {
            return Err(EcsSpatialValidationError::FineOverlayMustBeBounded);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Component)]
pub struct EcsSpatialVolume {
    pub id: EcsSpatialVolumeId,
    pub domain: EcsSpatialDomainKind,
    pub grid_id: EcsSpatialGridId,
    pub bounds: EcsBoundsUm,
    pub scale: EcsSpatialScalePreset,
    pub chunk_key: EcsChunkKey,
}

impl EcsSpatialVolume {
    #[must_use]
    pub fn terrain(id: EcsSpatialVolumeId, bounds: EcsBoundsUm) -> Self {
        let page_key = EcsSpatialPageKey::new(
            EcsSpatialDomainKind::Terrain,
            EcsSpatialGridId::new(1),
            0,
            0,
            0,
            0,
            EcsPageChannel::Occupancy,
        );
        Self {
            id,
            domain: EcsSpatialDomainKind::Terrain,
            grid_id: EcsSpatialGridId::new(1),
            bounds,
            scale: EcsSpatialScalePreset::GameplayFoot,
            chunk_key: page_key.chunk_key(),
        }
    }
}

fn fnv1a_u8(mut hash: u64, value: u8) -> u64 {
    hash ^= u64::from(value);
    hash.wrapping_mul(0x0000_0100_0000_01b3)
}

fn fnv1a_u32(mut hash: u64, value: u32) -> u64 {
    for byte in value.to_le_bytes() {
        hash = fnv1a_u8(hash, byte);
    }
    hash
}

fn fnv1a_u64(mut hash: u64, value: u64) -> u64 {
    for byte in value.to_le_bytes() {
        hash = fnv1a_u8(hash, byte);
    }
    hash
}

fn floor_div_i32(value: i32, divisor: i32) -> i32 {
    let quotient = value / divisor;
    let remainder = value % divisor;
    if remainder != 0 && ((remainder < 0) != (divisor < 0)) {
        quotient - 1
    } else {
        quotient
    }
}
