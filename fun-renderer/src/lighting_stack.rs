//! Pass 24 — Lighting, Shadows, and PBR Expansion.
//!
//! Reach baseline commercial scene quality. The module installs:
//!
//! 1. A typed `PbrMaterialDescriptor` with the standard PBR
//!    channels (base color, metallic, roughness, normal, occlusion,
//!    emissive) plus alpha clip / blend modes and clearcoat /
//!    transmission scaffold slots. Unlike the asset-prep record
//!    from Pass 21, this descriptor carries the renderer-side
//!    typed parameters that drive material binding and shading.
//! 2. A typed `LightingLightTable` RetiredEngine Resource fed from the
//!    existing ECS `DirectionalLight` / `PointLight` / `SpotLight`
//!    components plus `LightLayer` / `LightBounds` / `ShadowCaster`
//!    so every ECS light becomes a typed GPU light record.
//! 3. A typed `ShadowAtlasPlan` with cascade scaffold (4-cascade
//!    directional CSM by default), per-light tile allocation,
//!    a depth-only PSO family, and a `ShadowDebugView` selector.
//! 4. A typed `ClusteredLightingPlan` (Forward+) with cluster
//!    grid descriptor, per-cluster light index list, tile
//!    assignment compute pass plan, and a debug heatmap selector.
//! 5. A typed `IblPlaceholder` carrying diffuse SH coefficients
//!    and a specular envmap reference so the lighting pass has a
//!    typed indirect-light contract.
//! 6. `LightingStackDiagnostics` rolling up light count by kind,
//!    shadow atlas tile occupancy, cluster utilization, and
//!    typed lighting cost timings.

use fun_ecs::Resource;

use crate::component_api::{
    DirectionalLight, LightBounds, LightLayer, MaterialFeatureMask, PointLight, RenderAabb,
    RenderColor, RenderLayerMask, RenderLightId, RenderStableId, RenderTextureId, RenderVec3,
    ShadowCaster, ShadowMode, SpotLight,
};

pub const LIGHTING_STACK_SCHEMA_VERSION: u16 = 1;

pub const PBR_TEXTURE_SLOT_COUNT: usize = 7;
pub const LIGHTING_LIGHT_KIND_COUNT: usize = 3;
pub const SHADOW_DEBUG_VIEW_COUNT: usize = 6;
pub const CLUSTERED_LIGHTING_DEBUG_VIEW_COUNT: usize = 5;
pub const DEFAULT_CSM_CASCADE_COUNT: usize = 4;
pub const DEFAULT_CLUSTER_GRID_X: u8 = 16;
pub const DEFAULT_CLUSTER_GRID_Y: u8 = 9;
pub const DEFAULT_CLUSTER_GRID_Z: u8 = 24;
pub const DEFAULT_SHADOW_ATLAS_TILE_PX: u16 = 512;
pub const DEFAULT_SHADOW_ATLAS_EXTENT_PX: u16 = 4096;
pub const DEFAULT_MAX_LIGHTS_PER_CLUSTER: u8 = 32;

// ============================================================================
// Section 1 — PBR material expansion
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PbrTextureSlot {
    BaseColor,
    MetallicRoughness,
    Normal,
    Occlusion,
    Emissive,
    Clearcoat,
    Transmission,
}

impl PbrTextureSlot {
    pub const ALL: [Self; PBR_TEXTURE_SLOT_COUNT] = [
        Self::BaseColor,
        Self::MetallicRoughness,
        Self::Normal,
        Self::Occlusion,
        Self::Emissive,
        Self::Clearcoat,
        Self::Transmission,
    ];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::BaseColor => 0,
            Self::MetallicRoughness => 1,
            Self::Normal => 2,
            Self::Occlusion => 3,
            Self::Emissive => 4,
            Self::Clearcoat => 5,
            Self::Transmission => 6,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaseColor => "base_color",
            Self::MetallicRoughness => "metallic_roughness",
            Self::Normal => "normal",
            Self::Occlusion => "occlusion",
            Self::Emissive => "emissive",
            Self::Clearcoat => "clearcoat",
            Self::Transmission => "transmission",
        }
    }

    #[must_use]
    pub const fn feature_mask_bit(self) -> u32 {
        match self {
            Self::BaseColor => 1 << 0,
            Self::Normal => 1 << 1,
            Self::MetallicRoughness => 1 << 2,
            Self::Emissive => 1 << 3,
            // Pass 24 extends MaterialFeatureMask via dedicated bits
            // for occlusion / clearcoat / transmission. Bits 4 & 5
            // are reserved by the existing AlphaBlend / DoubleSided
            // flags, so the new bits start at 7.
            Self::Occlusion => 1 << 7,
            Self::Clearcoat => 1 << 8,
            Self::Transmission => 1 << 9,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PbrAlphaMode {
    #[default]
    Opaque,
    Mask,
    Blend,
}

impl PbrAlphaMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Opaque => "opaque",
            Self::Mask => "mask",
            Self::Blend => "blend",
        }
    }

    #[must_use]
    pub const fn is_translucent(self) -> bool {
        matches!(self, Self::Blend)
    }

    #[must_use]
    pub const fn writes_to_transparent_queue(self) -> bool {
        matches!(self, Self::Blend)
    }

    #[must_use]
    pub const fn writes_to_alpha_test_queue(self) -> bool {
        matches!(self, Self::Mask)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PbrClearcoatScaffold {
    pub enabled: bool,
    pub clearcoat_factor: f32,
    pub clearcoat_roughness: f32,
    pub clearcoat_normal_intensity: f32,
}

impl PbrClearcoatScaffold {
    pub const DISABLED: Self = Self {
        enabled: false,
        clearcoat_factor: 0.0,
        clearcoat_roughness: 0.0,
        clearcoat_normal_intensity: 0.0,
    };

    #[must_use]
    pub const fn opaque_layer(factor: f32, roughness: f32) -> Self {
        Self {
            enabled: true,
            clearcoat_factor: factor,
            clearcoat_roughness: roughness,
            clearcoat_normal_intensity: 1.0,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct PbrTransmissionScaffold {
    pub enabled: bool,
    pub transmission_factor: f32,
    pub ior: f32,
    pub thickness: f32,
    pub attenuation_distance: f32,
    pub attenuation_color: RenderColor,
}

impl PbrTransmissionScaffold {
    pub const DISABLED: Self = Self {
        enabled: false,
        transmission_factor: 0.0,
        ior: 1.5,
        thickness: 0.0,
        attenuation_distance: 0.0,
        attenuation_color: RenderColor::WHITE,
    };
}

/// Typed PBR material descriptor — the renderer-side parameter
/// table the lighting pass consumes. The descriptor is filled by
/// the asset prep stage from `MaterialAssetPrepRecord` and the
/// underlying material asset; the renderer never holds a wgpu
/// material binding directly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PbrMaterialDescriptor {
    pub schema_version: u16,
    pub base_color_factor: RenderColor,
    pub metallic_factor: f32,
    pub roughness_factor: f32,
    pub normal_intensity: f32,
    pub occlusion_strength: f32,
    pub emissive_factor: RenderColor,
    pub alpha_mode: PbrAlphaMode,
    pub alpha_cutoff: f32,
    pub double_sided: bool,
    pub feature_mask: PbrFeatureMask,
    pub texture_refs: [RenderTextureId; PBR_TEXTURE_SLOT_COUNT],
    pub clearcoat: PbrClearcoatScaffold,
    pub transmission: PbrTransmissionScaffold,
    pub debug_label: &'static str,
}

impl PbrMaterialDescriptor {
    #[must_use]
    pub const fn opaque_default() -> Self {
        Self {
            schema_version: LIGHTING_STACK_SCHEMA_VERSION,
            base_color_factor: RenderColor::WHITE,
            metallic_factor: 0.0,
            roughness_factor: 0.5,
            normal_intensity: 1.0,
            occlusion_strength: 1.0,
            emissive_factor: RenderColor::BLACK,
            alpha_mode: PbrAlphaMode::Opaque,
            alpha_cutoff: 0.5,
            double_sided: false,
            feature_mask: PbrFeatureMask::NONE,
            texture_refs: [RenderTextureId::INVALID; PBR_TEXTURE_SLOT_COUNT],
            clearcoat: PbrClearcoatScaffold::DISABLED,
            transmission: PbrTransmissionScaffold::DISABLED,
            debug_label: "pbr.opaque_default",
        }
    }

    #[must_use]
    pub const fn alpha_blended_default() -> Self {
        Self {
            alpha_mode: PbrAlphaMode::Blend,
            debug_label: "pbr.alpha_blended_default",
            ..Self::opaque_default()
        }
    }

    #[must_use]
    pub const fn alpha_masked_default(alpha_cutoff: f32) -> Self {
        Self {
            alpha_mode: PbrAlphaMode::Mask,
            alpha_cutoff,
            debug_label: "pbr.alpha_masked_default",
            ..Self::opaque_default()
        }
    }

    pub fn set_texture(&mut self, slot: PbrTextureSlot, texture: RenderTextureId) {
        self.texture_refs[slot.index()] = texture;
        if texture.is_valid() {
            self.feature_mask = self
                .feature_mask
                .union(PbrFeatureMask(slot.feature_mask_bit()));
        } else {
            self.feature_mask = self
                .feature_mask
                .without(PbrFeatureMask(slot.feature_mask_bit()));
        }
    }

    #[must_use]
    pub fn texture_for(&self, slot: PbrTextureSlot) -> RenderTextureId {
        self.texture_refs[slot.index()]
    }

    #[must_use]
    pub fn has_texture(&self, slot: PbrTextureSlot) -> bool {
        self.texture_refs[slot.index()].is_valid()
    }

    pub fn enable_clearcoat(&mut self, scaffold: PbrClearcoatScaffold) {
        self.clearcoat = scaffold;
        if scaffold.enabled {
            self.feature_mask = self
                .feature_mask
                .union(PbrFeatureMask(PbrTextureSlot::Clearcoat.feature_mask_bit()));
        }
    }

    pub fn enable_transmission(&mut self, scaffold: PbrTransmissionScaffold) {
        self.transmission = scaffold;
        if scaffold.enabled {
            self.feature_mask = self.feature_mask.union(PbrFeatureMask(
                PbrTextureSlot::Transmission.feature_mask_bit(),
            ));
        }
    }

    /// Convert the renderer-side feature mask into the existing
    /// `MaterialFeatureMask` carried by the GPU scene's
    /// material record. Bits the existing mask does not declare
    /// (occlusion / clearcoat / transmission) are dropped on the
    /// way out so the GPU material table stays compatible with
    /// pre-Pass-24 consumers.
    #[must_use]
    pub fn to_material_feature_mask(&self) -> MaterialFeatureMask {
        let mut mask = MaterialFeatureMask::NONE;
        if self
            .feature_mask
            .contains_bit(PbrTextureSlot::BaseColor.feature_mask_bit())
        {
            mask = mask.union(MaterialFeatureMask::BASE_COLOR_TEXTURE);
        }
        if self
            .feature_mask
            .contains_bit(PbrTextureSlot::Normal.feature_mask_bit())
        {
            mask = mask.union(MaterialFeatureMask::NORMAL_TEXTURE);
        }
        if self
            .feature_mask
            .contains_bit(PbrTextureSlot::MetallicRoughness.feature_mask_bit())
        {
            mask = mask.union(MaterialFeatureMask::METALLIC_ROUGHNESS_TEXTURE);
        }
        if self
            .feature_mask
            .contains_bit(PbrTextureSlot::Emissive.feature_mask_bit())
        {
            mask = mask.union(MaterialFeatureMask::EMISSIVE);
        }
        if matches!(self.alpha_mode, PbrAlphaMode::Blend) {
            mask = mask.union(MaterialFeatureMask::ALPHA_BLEND);
        }
        if self.double_sided {
            mask = mask.union(MaterialFeatureMask::DOUBLE_SIDED);
        }
        mask
    }
}

/// Renderer-side feature mask. Mirrors the existing
/// `MaterialFeatureMask` but keeps Pass 24's three new bits
/// (occlusion / clearcoat / transmission) renderer-internal so the
/// existing GPU scene table layout stays stable.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PbrFeatureMask(pub u32);

impl PbrFeatureMask {
    pub const NONE: Self = Self(0);

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn contains_bit(self, bit: u32) -> bool {
        self.0 & bit == bit
    }

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

// ============================================================================
// Section 2 — Lighting (typed light table fed from ECS)
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LightingLightKind {
    #[default]
    Directional,
    Point,
    Spot,
}

impl LightingLightKind {
    pub const ALL: [Self; LIGHTING_LIGHT_KIND_COUNT] = [Self::Directional, Self::Point, Self::Spot];

    #[must_use]
    pub const fn index(self) -> usize {
        match self {
            Self::Directional => 0,
            Self::Point => 1,
            Self::Spot => 2,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Directional => "directional",
            Self::Point => "point",
            Self::Spot => "spot",
        }
    }
}

/// Typed GPU light table row. The lighting pipeline reads from a
/// dense table of these — never from individual ECS components.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct LightingLightRecord {
    pub light_id: RenderLightId,
    pub kind: LightingLightKind,
    pub layers: RenderLayerMask,
    pub bounds: RenderAabb,
    pub world_position: RenderVec3,
    pub world_direction: RenderVec3,
    pub color: RenderColor,
    pub intensity_or_illuminance: f32,
    pub range: f32,
    pub inner_cone_radians: f32,
    pub outer_cone_radians: f32,
    pub shadow_caster: bool,
    pub shadow_mode: PbrShadowMode,
    pub shadow_atlas_tile: ShadowAtlasTileSlot,
}

impl LightingLightRecord {
    #[must_use]
    pub fn from_directional(
        light_id: RenderLightId,
        light: DirectionalLight,
        direction: RenderVec3,
        layer: LightLayer,
        bounds: LightBounds,
        shadow: ShadowCaster,
    ) -> Self {
        Self {
            light_id,
            kind: LightingLightKind::Directional,
            layers: layer.mask,
            bounds: bounds.bounds,
            world_position: RenderVec3::ZERO,
            world_direction: direction,
            color: light.color,
            intensity_or_illuminance: light.illuminance_lux,
            range: f32::INFINITY,
            inner_cone_radians: 0.0,
            outer_cone_radians: 0.0,
            shadow_caster: !matches!(shadow.mode, ShadowMode::None),
            shadow_mode: PbrShadowMode::from_shadow_caster(shadow),
            shadow_atlas_tile: ShadowAtlasTileSlot::INVALID,
        }
    }

    #[must_use]
    pub fn from_point(
        light_id: RenderLightId,
        light: PointLight,
        position: RenderVec3,
        layer: LightLayer,
        bounds: LightBounds,
        shadow: ShadowCaster,
    ) -> Self {
        Self {
            light_id,
            kind: LightingLightKind::Point,
            layers: layer.mask,
            bounds: bounds.bounds,
            world_position: position,
            world_direction: RenderVec3::ZERO,
            color: light.color,
            intensity_or_illuminance: light.intensity_lumens,
            range: light.range,
            inner_cone_radians: 0.0,
            outer_cone_radians: 0.0,
            shadow_caster: !matches!(shadow.mode, ShadowMode::None),
            shadow_mode: PbrShadowMode::from_shadow_caster(shadow),
            shadow_atlas_tile: ShadowAtlasTileSlot::INVALID,
        }
    }

    #[must_use]
    pub fn from_spot(
        light_id: RenderLightId,
        light: SpotLight,
        position: RenderVec3,
        direction: RenderVec3,
        layer: LightLayer,
        bounds: LightBounds,
        shadow: ShadowCaster,
    ) -> Self {
        Self {
            light_id,
            kind: LightingLightKind::Spot,
            layers: layer.mask,
            bounds: bounds.bounds,
            world_position: position,
            world_direction: direction,
            color: light.color,
            intensity_or_illuminance: light.intensity_lumens,
            range: light.range,
            inner_cone_radians: light.inner_angle_radians,
            outer_cone_radians: light.outer_angle_radians,
            shadow_caster: !matches!(shadow.mode, ShadowMode::None),
            shadow_mode: PbrShadowMode::from_shadow_caster(shadow),
            shadow_atlas_tile: ShadowAtlasTileSlot::INVALID,
        }
    }
}

/// Renderer-internal shadow mode mirror that does not depend on
/// the unstable virtual-page paths from `virtual_shadow.rs`.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PbrShadowMode {
    #[default]
    None,
    Atlas,
    VirtualPages,
    RayTraced,
}

impl PbrShadowMode {
    #[must_use]
    pub const fn from_shadow_caster(caster: ShadowCaster) -> Self {
        match caster.mode {
            ShadowMode::None => Self::None,
            ShadowMode::VirtualPages => Self::VirtualPages,
            ShadowMode::RayTraced => Self::RayTraced,
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Atlas => "atlas",
            Self::VirtualPages => "virtual_pages",
            Self::RayTraced => "ray_traced",
        }
    }

    #[must_use]
    pub const fn requires_shadow_atlas(self) -> bool {
        matches!(self, Self::Atlas)
    }
}

/// RetiredEngine Resource holding the dense light table the renderer
/// recording side iterates. The table is rebuilt each frame from
/// the ECS lights via `record_*` helpers and consumed by the
/// clustered lighting pipeline.
#[derive(Debug, Default, Clone, PartialEq, Resource)]
pub struct LightingLightTable {
    records: Vec<LightingLightRecord>,
    counts_by_kind: [u32; LIGHTING_LIGHT_KIND_COUNT],
    shadow_caster_count: u32,
}

impl LightingLightTable {
    pub fn clear(&mut self) {
        self.records.clear();
        self.counts_by_kind = [0; LIGHTING_LIGHT_KIND_COUNT];
        self.shadow_caster_count = 0;
    }

    pub fn record(&mut self, light: LightingLightRecord) {
        self.counts_by_kind[light.kind.index()] =
            self.counts_by_kind[light.kind.index()].saturating_add(1);
        if light.shadow_caster {
            self.shadow_caster_count = self.shadow_caster_count.saturating_add(1);
        }
        self.records.push(light);
    }

    #[must_use]
    pub fn records(&self) -> &[LightingLightRecord] {
        &self.records
    }

    #[must_use]
    pub fn count_for(&self, kind: LightingLightKind) -> u32 {
        self.counts_by_kind[kind.index()]
    }

    #[must_use]
    pub const fn shadow_caster_count(&self) -> u32 {
        self.shadow_caster_count
    }

    #[must_use]
    pub fn total_count(&self) -> u32 {
        self.records.len() as u32
    }

    /// Test whether a record's bounds intersect a given AABB. Used
    /// by the cluster assignment compute pass to drop lights that
    /// cannot affect any geometry inside the cluster.
    #[must_use]
    pub fn light_affects_aabb(record: &LightingLightRecord, target: RenderAabb) -> bool {
        match record.kind {
            LightingLightKind::Directional => true,
            LightingLightKind::Point | LightingLightKind::Spot => {
                aabb_intersects(record.bounds, target)
            }
        }
    }

    pub fn assign_shadow_atlas_tile(&mut self, index: u32, slot: ShadowAtlasTileSlot) -> bool {
        if let Some(record) = self.records.get_mut(index as usize) {
            record.shadow_atlas_tile = slot;
            true
        } else {
            false
        }
    }
}

#[must_use]
fn aabb_intersects(left: RenderAabb, right: RenderAabb) -> bool {
    let dx = (left.center.x - right.center.x).abs();
    let dy = (left.center.y - right.center.y).abs();
    let dz = (left.center.z - right.center.z).abs();
    let sx = left.half_extents.x.abs() + right.half_extents.x.abs();
    let sy = left.half_extents.y.abs() + right.half_extents.y.abs();
    let sz = left.half_extents.z.abs() + right.half_extents.z.abs();
    dx <= sx && dy <= sy && dz <= sz
}

/// IBL placeholder — typed indirect-light contract carrying
/// diffuse SH coefficients (9 bands * RGB) and a specular envmap
/// reference. The bridge consumes the placeholder by sampling the
/// envmap through the asset registry; the SH coefficients are
/// uploaded as a small constant buffer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IblPlaceholder {
    pub schema_version: u16,
    pub diffuse_sh: [RenderVec3; 9],
    pub specular_envmap: RenderTextureId,
    pub specular_lod_count: u8,
    pub intensity: f32,
    pub debug_label: &'static str,
}

impl IblPlaceholder {
    pub const NEUTRAL_GREY: Self = Self {
        schema_version: LIGHTING_STACK_SCHEMA_VERSION,
        diffuse_sh: [RenderVec3 {
            x: 0.5,
            y: 0.5,
            z: 0.5,
        }; 9],
        specular_envmap: RenderTextureId::INVALID,
        specular_lod_count: 0,
        intensity: 1.0,
        debug_label: "ibl.neutral_grey_placeholder",
    };

    #[must_use]
    pub const fn is_envmap_bound(&self) -> bool {
        self.specular_envmap.is_valid() && self.specular_lod_count > 0
    }
}

impl Default for IblPlaceholder {
    fn default() -> Self {
        Self::NEUTRAL_GREY
    }
}

// ============================================================================
// Section 3 — Shadow atlas + cascade scaffold + depth-only PSO family
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShadowAtlasTileSlot {
    pub slot: u16,
    pub generation: u16,
}

impl ShadowAtlasTileSlot {
    pub const INVALID: Self = Self {
        slot: u16::MAX,
        generation: 0,
    };

    #[must_use]
    pub const fn new(slot: u16, generation: u16) -> Self {
        Self { slot, generation }
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.slot != u16::MAX && self.generation != 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShadowAtlasDescriptor {
    pub schema_version: u16,
    pub extent_px: u16,
    pub tile_size_px: u16,
    pub tile_grid_dim: u16,
    pub max_tiles: u16,
}

impl ShadowAtlasDescriptor {
    #[must_use]
    pub const fn new(extent_px: u16, tile_size_px: u16) -> Self {
        let dim = match extent_px.checked_div(tile_size_px) {
            Some(value) => value,
            None => 0,
        };
        let max_tiles = dim.saturating_mul(dim);
        Self {
            schema_version: LIGHTING_STACK_SCHEMA_VERSION,
            extent_px,
            tile_size_px,
            tile_grid_dim: dim,
            max_tiles,
        }
    }
}

impl Default for ShadowAtlasDescriptor {
    fn default() -> Self {
        Self::new(DEFAULT_SHADOW_ATLAS_EXTENT_PX, DEFAULT_SHADOW_ATLAS_TILE_PX)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShadowAtlasAllocation {
    pub light_id: RenderLightId,
    pub slot: ShadowAtlasTileSlot,
    pub tile_x: u16,
    pub tile_y: u16,
    pub last_used_frame: u64,
    pub cascade_index: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Resource)]
pub struct ShadowAtlasPlan {
    descriptor: ShadowAtlasDescriptor,
    allocations: Vec<ShadowAtlasAllocation>,
    next_tile: u16,
    generation: u16,
    rejected_overflow_count: u32,
}

impl ShadowAtlasPlan {
    #[must_use]
    pub fn new(descriptor: ShadowAtlasDescriptor) -> Self {
        Self {
            descriptor,
            allocations: Vec::new(),
            next_tile: 0,
            generation: 1,
            rejected_overflow_count: 0,
        }
    }

    #[must_use]
    pub const fn descriptor(&self) -> ShadowAtlasDescriptor {
        self.descriptor
    }

    pub fn begin_frame(&mut self) {
        self.allocations.clear();
        self.next_tile = 0;
        self.generation = self.generation.saturating_add(1).max(1);
        self.rejected_overflow_count = 0;
    }

    pub fn allocate(
        &mut self,
        light_id: RenderLightId,
        cascade_index: u8,
        frame_index: u64,
    ) -> Option<ShadowAtlasTileSlot> {
        if self.next_tile >= self.descriptor.max_tiles {
            self.rejected_overflow_count = self.rejected_overflow_count.saturating_add(1);
            return None;
        }
        let slot = ShadowAtlasTileSlot::new(self.next_tile, self.generation);
        let tile_x = self.next_tile % self.descriptor.tile_grid_dim.max(1);
        let tile_y = self.next_tile / self.descriptor.tile_grid_dim.max(1);
        self.next_tile = self.next_tile.saturating_add(1);
        self.allocations.push(ShadowAtlasAllocation {
            light_id,
            slot,
            tile_x,
            tile_y,
            last_used_frame: frame_index,
            cascade_index,
        });
        Some(slot)
    }

    #[must_use]
    pub fn allocations(&self) -> &[ShadowAtlasAllocation] {
        &self.allocations
    }

    #[must_use]
    pub const fn rejected_overflow_count(&self) -> u32 {
        self.rejected_overflow_count
    }

    #[must_use]
    pub const fn next_tile(&self) -> u16 {
        self.next_tile
    }
}

impl Default for ShadowAtlasPlan {
    fn default() -> Self {
        Self::new(ShadowAtlasDescriptor::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowCascadeSlice {
    pub cascade_index: u8,
    pub near_distance: f32,
    pub far_distance: f32,
    pub atlas_slot: ShadowAtlasTileSlot,
}

#[derive(Debug, Clone, PartialEq, Resource)]
pub struct ShadowCascadePlan {
    pub schema_version: u16,
    pub light_id: RenderLightId,
    pub cascade_count: u8,
    pub slices: Vec<ShadowCascadeSlice>,
}

impl ShadowCascadePlan {
    #[must_use]
    pub fn build_default(light_id: RenderLightId, near: f32, far: f32) -> Self {
        let count = DEFAULT_CSM_CASCADE_COUNT as u8;
        let mut slices = Vec::with_capacity(count as usize);
        let log_near = near.max(0.05).ln();
        let log_far = far.max(near + 1.0).ln();
        for cascade_index in 0..count {
            let t0 = cascade_index as f32 / count as f32;
            let t1 = (cascade_index + 1) as f32 / count as f32;
            let cascade_near = (log_near + (log_far - log_near) * t0).exp();
            let cascade_far = (log_near + (log_far - log_near) * t1).exp();
            slices.push(ShadowCascadeSlice {
                cascade_index,
                near_distance: cascade_near,
                far_distance: cascade_far,
                atlas_slot: ShadowAtlasTileSlot::INVALID,
            });
        }
        Self {
            schema_version: LIGHTING_STACK_SCHEMA_VERSION,
            light_id,
            cascade_count: count,
            slices,
        }
    }

    pub fn assign_slot(&mut self, cascade_index: u8, slot: ShadowAtlasTileSlot) -> bool {
        if let Some(slice) = self
            .slices
            .iter_mut()
            .find(|slice| slice.cascade_index == cascade_index)
        {
            slice.atlas_slot = slot;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepthOnlyPso {
    DirectionalCascade,
    PointParaboloid,
    SpotPerspective,
    StaticGeometry,
    DynamicGeometry,
}

impl DepthOnlyPso {
    pub const ALL: [Self; 5] = [
        Self::DirectionalCascade,
        Self::PointParaboloid,
        Self::SpotPerspective,
        Self::StaticGeometry,
        Self::DynamicGeometry,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DirectionalCascade => "directional_cascade",
            Self::PointParaboloid => "point_paraboloid",
            Self::SpotPerspective => "spot_perspective",
            Self::StaticGeometry => "static_geometry",
            Self::DynamicGeometry => "dynamic_geometry",
        }
    }

    #[must_use]
    pub const fn matches_light_kind(self, kind: LightingLightKind) -> bool {
        matches!(
            (self, kind),
            (Self::DirectionalCascade, LightingLightKind::Directional)
                | (Self::PointParaboloid, LightingLightKind::Point)
                | (Self::SpotPerspective, LightingLightKind::Spot)
        )
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShadowDebugView {
    #[default]
    None,
    AtlasOccupancy,
    CascadeSplits,
    ShadowMapVisualization,
    ShadowReceiverHeatmap,
    ShadowCasterCount,
}

impl ShadowDebugView {
    pub const ALL: [Self; SHADOW_DEBUG_VIEW_COUNT] = [
        Self::None,
        Self::AtlasOccupancy,
        Self::CascadeSplits,
        Self::ShadowMapVisualization,
        Self::ShadowReceiverHeatmap,
        Self::ShadowCasterCount,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::AtlasOccupancy => "atlas_occupancy",
            Self::CascadeSplits => "cascade_splits",
            Self::ShadowMapVisualization => "shadow_map_visualization",
            Self::ShadowReceiverHeatmap => "shadow_receiver_heatmap",
            Self::ShadowCasterCount => "shadow_caster_count",
        }
    }
}

// ============================================================================
// Section 4 — Forward+ / clustered lighting scaffold
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClusterGridDescriptor {
    pub schema_version: u16,
    pub clusters_x: u8,
    pub clusters_y: u8,
    pub clusters_z: u8,
    pub max_lights_per_cluster: u8,
    pub depth_split_strategy: ClusterDepthSplitStrategy,
}

impl ClusterGridDescriptor {
    pub const PRODUCT_DEFAULT: Self = Self {
        schema_version: LIGHTING_STACK_SCHEMA_VERSION,
        clusters_x: DEFAULT_CLUSTER_GRID_X,
        clusters_y: DEFAULT_CLUSTER_GRID_Y,
        clusters_z: DEFAULT_CLUSTER_GRID_Z,
        max_lights_per_cluster: DEFAULT_MAX_LIGHTS_PER_CLUSTER,
        depth_split_strategy: ClusterDepthSplitStrategy::ExponentialNearFar,
    };

    #[must_use]
    pub const fn cluster_count(self) -> u32 {
        (self.clusters_x as u32)
            .saturating_mul(self.clusters_y as u32)
            .saturating_mul(self.clusters_z as u32)
    }

    #[must_use]
    pub const fn light_index_capacity(self) -> u32 {
        self.cluster_count()
            .saturating_mul(self.max_lights_per_cluster as u32)
    }

    #[must_use]
    pub const fn cluster_index(self, x: u8, y: u8, z: u8) -> u32 {
        (z as u32) * (self.clusters_x as u32) * (self.clusters_y as u32)
            + (y as u32) * (self.clusters_x as u32)
            + x as u32
    }
}

impl Default for ClusterGridDescriptor {
    fn default() -> Self {
        Self::PRODUCT_DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClusterDepthSplitStrategy {
    #[default]
    ExponentialNearFar,
    UniformNearFar,
    LogarithmicNearFar,
}

impl ClusterDepthSplitStrategy {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ExponentialNearFar => "exponential_near_far",
            Self::UniformNearFar => "uniform_near_far",
            Self::LogarithmicNearFar => "logarithmic_near_far",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClusterTileAssignmentPassPlan {
    pub schema_version: u16,
    pub workgroup_size_x: u8,
    pub workgroup_size_y: u8,
    pub workgroup_size_z: u8,
    pub dispatch_x: u32,
    pub dispatch_y: u32,
    pub dispatch_z: u32,
}

impl ClusterTileAssignmentPassPlan {
    #[must_use]
    pub fn for_grid(grid: ClusterGridDescriptor) -> Self {
        let wgsize_x = 4u32;
        let wgsize_y = 4u32;
        let wgsize_z = 4u32;
        let dispatch_x = (grid.clusters_x as u32).div_ceil(wgsize_x);
        let dispatch_y = (grid.clusters_y as u32).div_ceil(wgsize_y);
        let dispatch_z = (grid.clusters_z as u32).div_ceil(wgsize_z);
        Self {
            schema_version: LIGHTING_STACK_SCHEMA_VERSION,
            workgroup_size_x: wgsize_x as u8,
            workgroup_size_y: wgsize_y as u8,
            workgroup_size_z: wgsize_z as u8,
            dispatch_x,
            dispatch_y,
            dispatch_z,
        }
    }

    #[must_use]
    pub const fn total_dispatches(&self) -> u32 {
        self.dispatch_x
            .saturating_mul(self.dispatch_y)
            .saturating_mul(self.dispatch_z)
    }
}

#[derive(Debug, Clone, PartialEq, Resource)]
pub struct ClusteredLightListBuffer {
    pub schema_version: u16,
    pub grid: ClusterGridDescriptor,
    pub light_count_per_cluster: Vec<u8>,
    pub light_indices: Vec<u32>,
    pub overflow_cluster_count: u32,
}

impl ClusteredLightListBuffer {
    #[must_use]
    pub fn for_grid(grid: ClusterGridDescriptor) -> Self {
        let cluster_count = grid.cluster_count() as usize;
        let capacity = grid.light_index_capacity() as usize;
        Self {
            schema_version: LIGHTING_STACK_SCHEMA_VERSION,
            grid,
            light_count_per_cluster: vec![0u8; cluster_count],
            light_indices: vec![u32::MAX; capacity],
            overflow_cluster_count: 0,
        }
    }

    pub fn clear(&mut self) {
        self.light_count_per_cluster.fill(0);
        self.light_indices.fill(u32::MAX);
        self.overflow_cluster_count = 0;
    }

    pub fn record_light(&mut self, cluster_index: u32, light_table_index: u32) -> bool {
        let cluster_idx = cluster_index as usize;
        if cluster_idx >= self.light_count_per_cluster.len() {
            return false;
        }
        let count_slot = &mut self.light_count_per_cluster[cluster_idx];
        if *count_slot >= self.grid.max_lights_per_cluster {
            self.overflow_cluster_count = self.overflow_cluster_count.saturating_add(1);
            return false;
        }
        let index_in_cluster = *count_slot as u32;
        let buffer_offset = (cluster_index.saturating_mul(self.grid.max_lights_per_cluster as u32))
            + index_in_cluster;
        if let Some(slot) = self.light_indices.get_mut(buffer_offset as usize) {
            *slot = light_table_index;
            *count_slot = count_slot.saturating_add(1);
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn count_for(&self, cluster_index: u32) -> u8 {
        self.light_count_per_cluster
            .get(cluster_index as usize)
            .copied()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn lights_for(&self, cluster_index: u32) -> &[u32] {
        let count = self.count_for(cluster_index) as usize;
        let offset = cluster_index as usize * self.grid.max_lights_per_cluster as usize;
        &self.light_indices[offset..offset + count]
    }

    #[must_use]
    pub fn total_assignments(&self) -> u32 {
        self.light_count_per_cluster
            .iter()
            .copied()
            .fold(0u32, |acc, v| acc.saturating_add(v as u32))
    }
}

impl Default for ClusteredLightListBuffer {
    fn default() -> Self {
        Self::for_grid(ClusterGridDescriptor::default())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClusteredLightingDebugHeatmap {
    #[default]
    None,
    LightsPerCluster,
    OverflowClusters,
    LightLayerCoverage,
    DepthSliceUtilization,
}

impl ClusteredLightingDebugHeatmap {
    pub const ALL: [Self; CLUSTERED_LIGHTING_DEBUG_VIEW_COUNT] = [
        Self::None,
        Self::LightsPerCluster,
        Self::OverflowClusters,
        Self::LightLayerCoverage,
        Self::DepthSliceUtilization,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::LightsPerCluster => "lights_per_cluster",
            Self::OverflowClusters => "overflow_clusters",
            Self::LightLayerCoverage => "light_layer_coverage",
            Self::DepthSliceUtilization => "depth_slice_utilization",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Resource)]
pub struct ClusteredLightingPlan {
    pub schema_version: u16,
    pub view_id: RenderStableId,
    pub grid: ClusterGridDescriptor,
    pub assignment_pass: ClusterTileAssignmentPassPlan,
    pub debug_heatmap: ClusteredLightingDebugHeatmap,
}

impl ClusteredLightingPlan {
    #[must_use]
    pub fn build(view_id: RenderStableId, grid: ClusterGridDescriptor) -> Self {
        Self {
            schema_version: LIGHTING_STACK_SCHEMA_VERSION,
            view_id,
            grid,
            assignment_pass: ClusterTileAssignmentPassPlan::for_grid(grid),
            debug_heatmap: ClusteredLightingDebugHeatmap::None,
        }
    }
}

// ============================================================================
// Section 5 — Diagnostics
// ============================================================================

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Resource)]
pub struct LightingStackDiagnostics {
    pub schema_version: u16,
    pub directional_light_count: u32,
    pub point_light_count: u32,
    pub spot_light_count: u32,
    pub shadow_caster_count: u32,
    pub shadow_atlas_tiles_used: u16,
    pub shadow_atlas_tile_capacity: u16,
    pub shadow_atlas_overflow_count: u32,
    pub cluster_count: u32,
    pub cluster_assignments: u32,
    pub cluster_overflow_count: u32,
    pub lighting_setup_cpu_ns: u64,
    pub shadow_render_gpu_ns: u64,
    pub clustered_assignment_gpu_ns: u64,
    pub forward_lighting_gpu_ns: u64,
    pub ibl_envmap_bound: bool,
    pub debug_view: ShadowDebugView,
    pub debug_heatmap: ClusteredLightingDebugHeatmap,
}

impl LightingStackDiagnostics {
    #[must_use]
    pub fn from_state(
        light_table: &LightingLightTable,
        atlas: &ShadowAtlasPlan,
        cluster_buffer: &ClusteredLightListBuffer,
        ibl: &IblPlaceholder,
    ) -> Self {
        Self {
            schema_version: LIGHTING_STACK_SCHEMA_VERSION,
            directional_light_count: light_table.count_for(LightingLightKind::Directional),
            point_light_count: light_table.count_for(LightingLightKind::Point),
            spot_light_count: light_table.count_for(LightingLightKind::Spot),
            shadow_caster_count: light_table.shadow_caster_count(),
            shadow_atlas_tiles_used: atlas.next_tile(),
            shadow_atlas_tile_capacity: atlas.descriptor().max_tiles,
            shadow_atlas_overflow_count: atlas.rejected_overflow_count(),
            cluster_count: cluster_buffer.grid.cluster_count(),
            cluster_assignments: cluster_buffer.total_assignments(),
            cluster_overflow_count: cluster_buffer.overflow_cluster_count,
            lighting_setup_cpu_ns: 0,
            shadow_render_gpu_ns: 0,
            clustered_assignment_gpu_ns: 0,
            forward_lighting_gpu_ns: 0,
            ibl_envmap_bound: ibl.is_envmap_bound(),
            debug_view: ShadowDebugView::None,
            debug_heatmap: ClusteredLightingDebugHeatmap::None,
        }
    }

    pub fn record_lighting_cpu_setup_ns(&mut self, ns: u64) {
        self.lighting_setup_cpu_ns = self.lighting_setup_cpu_ns.saturating_add(ns);
    }

    pub fn record_shadow_render_gpu_ns(&mut self, ns: u64) {
        self.shadow_render_gpu_ns = self.shadow_render_gpu_ns.saturating_add(ns);
    }

    pub fn record_clustered_assignment_gpu_ns(&mut self, ns: u64) {
        self.clustered_assignment_gpu_ns = self.clustered_assignment_gpu_ns.saturating_add(ns);
    }

    pub fn record_forward_lighting_gpu_ns(&mut self, ns: u64) {
        self.forward_lighting_gpu_ns = self.forward_lighting_gpu_ns.saturating_add(ns);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component_api::{RenderColor, RenderLayerMask, RenderLightId};

    fn directional_light_record() -> LightingLightRecord {
        LightingLightRecord::from_directional(
            RenderLightId::new(1, 1),
            DirectionalLight::default(),
            RenderVec3::new(0.0, -1.0, 0.0),
            LightLayer::default(),
            LightBounds::default(),
            ShadowCaster {
                mode: ShadowMode::None,
            },
        )
    }

    fn point_light_record_at(position: RenderVec3, range: f32) -> LightingLightRecord {
        let mut bounds = LightBounds::default();
        bounds.bounds.center = position;
        bounds.bounds.half_extents = RenderVec3::new(range, range, range);
        LightingLightRecord::from_point(
            RenderLightId::new(2, 1),
            PointLight {
                color: RenderColor::WHITE,
                intensity_lumens: 800.0,
                range,
            },
            position,
            LightLayer::default(),
            bounds,
            ShadowCaster::default(),
        )
    }

    #[test]
    fn schema_version_is_stable() {
        assert_eq!(LIGHTING_STACK_SCHEMA_VERSION, 1);
        assert_eq!(PBR_TEXTURE_SLOT_COUNT, 7);
        assert_eq!(LIGHTING_LIGHT_KIND_COUNT, 3);
        assert_eq!(SHADOW_DEBUG_VIEW_COUNT, 6);
        assert_eq!(CLUSTERED_LIGHTING_DEBUG_VIEW_COUNT, 5);
    }

    #[test]
    fn pbr_descriptor_default_is_opaque_with_neutral_factors() {
        let descriptor = PbrMaterialDescriptor::opaque_default();
        assert_eq!(descriptor.alpha_mode, PbrAlphaMode::Opaque);
        assert_eq!(descriptor.metallic_factor, 0.0);
        assert_eq!(descriptor.roughness_factor, 0.5);
        assert!(!descriptor.double_sided);
        assert!(!descriptor.has_texture(PbrTextureSlot::BaseColor));
    }

    #[test]
    fn pbr_set_texture_updates_feature_mask_bit() {
        let mut descriptor = PbrMaterialDescriptor::opaque_default();
        descriptor.set_texture(PbrTextureSlot::BaseColor, RenderTextureId::new(5, 1));
        assert!(descriptor.has_texture(PbrTextureSlot::BaseColor));
        assert!(
            descriptor
                .feature_mask
                .contains_bit(PbrTextureSlot::BaseColor.feature_mask_bit())
        );
    }

    #[test]
    fn pbr_set_texture_invalid_clears_feature_mask_bit() {
        let mut descriptor = PbrMaterialDescriptor::opaque_default();
        descriptor.set_texture(PbrTextureSlot::Normal, RenderTextureId::new(7, 1));
        assert!(
            descriptor
                .feature_mask
                .contains_bit(PbrTextureSlot::Normal.feature_mask_bit())
        );
        descriptor.set_texture(PbrTextureSlot::Normal, RenderTextureId::INVALID);
        assert!(
            !descriptor
                .feature_mask
                .contains_bit(PbrTextureSlot::Normal.feature_mask_bit())
        );
    }

    #[test]
    fn pbr_alpha_mode_routes_through_correct_queue() {
        assert!(PbrAlphaMode::Blend.writes_to_transparent_queue());
        assert!(!PbrAlphaMode::Blend.writes_to_alpha_test_queue());
        assert!(PbrAlphaMode::Mask.writes_to_alpha_test_queue());
        assert!(!PbrAlphaMode::Opaque.writes_to_alpha_test_queue());
        assert!(!PbrAlphaMode::Opaque.writes_to_transparent_queue());
    }

    #[test]
    fn pbr_clearcoat_scaffold_disabled_by_default() {
        let descriptor = PbrMaterialDescriptor::opaque_default();
        assert!(!descriptor.clearcoat.enabled);
    }

    #[test]
    fn pbr_enable_clearcoat_lights_feature_bit() {
        let mut descriptor = PbrMaterialDescriptor::opaque_default();
        descriptor.enable_clearcoat(PbrClearcoatScaffold::opaque_layer(1.0, 0.1));
        assert!(descriptor.clearcoat.enabled);
        assert!(
            descriptor
                .feature_mask
                .contains_bit(PbrTextureSlot::Clearcoat.feature_mask_bit())
        );
    }

    #[test]
    fn pbr_enable_transmission_lights_feature_bit() {
        let mut descriptor = PbrMaterialDescriptor::opaque_default();
        descriptor.enable_transmission(PbrTransmissionScaffold {
            enabled: true,
            transmission_factor: 0.8,
            ior: 1.5,
            thickness: 0.01,
            attenuation_distance: 1.0,
            attenuation_color: RenderColor::WHITE,
        });
        assert!(descriptor.transmission.enabled);
        assert!(
            descriptor
                .feature_mask
                .contains_bit(PbrTextureSlot::Transmission.feature_mask_bit())
        );
    }

    #[test]
    fn pbr_to_material_feature_mask_drops_pass_24_only_bits() {
        let mut descriptor = PbrMaterialDescriptor::alpha_blended_default();
        descriptor.set_texture(PbrTextureSlot::BaseColor, RenderTextureId::new(1, 1));
        descriptor.set_texture(PbrTextureSlot::Occlusion, RenderTextureId::new(3, 1));
        let mask = descriptor.to_material_feature_mask();
        assert!(mask.contains(MaterialFeatureMask::BASE_COLOR_TEXTURE));
        assert!(mask.contains(MaterialFeatureMask::ALPHA_BLEND));
        // Occlusion is a Pass-24-internal bit; the legacy mask
        // does not declare it so it must not appear in the
        // converted MaterialFeatureMask.
        assert!(!mask.contains(MaterialFeatureMask(1 << 7)));
    }

    #[test]
    fn light_table_records_directional_light() {
        let mut table = LightingLightTable::default();
        table.record(directional_light_record());
        assert_eq!(table.count_for(LightingLightKind::Directional), 1);
        assert_eq!(table.total_count(), 1);
    }

    #[test]
    fn light_table_attributes_shadow_casters() {
        let mut table = LightingLightTable::default();
        let mut record = directional_light_record();
        record.shadow_caster = true;
        table.record(record);
        assert_eq!(table.shadow_caster_count(), 1);
    }

    #[test]
    fn directional_light_record_carries_inf_range() {
        let record = directional_light_record();
        assert_eq!(record.kind, LightingLightKind::Directional);
        assert!(record.range.is_infinite());
    }

    #[test]
    fn point_light_aabb_overlap_drives_cluster_assignment() {
        let record = point_light_record_at(RenderVec3::new(0.0, 0.0, 0.0), 10.0);
        let cluster_aabb = RenderAabb::new(
            RenderVec3::new(2.0, 0.0, 0.0),
            RenderVec3::new(1.0, 1.0, 1.0),
        );
        assert!(LightingLightTable::light_affects_aabb(
            &record,
            cluster_aabb
        ));
    }

    #[test]
    fn point_light_outside_cluster_aabb_is_dropped() {
        let record = point_light_record_at(RenderVec3::new(0.0, 0.0, 0.0), 1.0);
        let cluster_aabb = RenderAabb::new(
            RenderVec3::new(100.0, 0.0, 0.0),
            RenderVec3::new(1.0, 1.0, 1.0),
        );
        assert!(!LightingLightTable::light_affects_aabb(
            &record,
            cluster_aabb
        ));
    }

    #[test]
    fn directional_light_always_affects_every_cluster() {
        let record = directional_light_record();
        let cluster_aabb = RenderAabb::new(
            RenderVec3::new(1_000.0, 0.0, 0.0),
            RenderVec3::new(1.0, 1.0, 1.0),
        );
        assert!(LightingLightTable::light_affects_aabb(
            &record,
            cluster_aabb
        ));
    }

    #[test]
    fn ibl_placeholder_neutral_grey_is_unbound_envmap() {
        let placeholder = IblPlaceholder::default();
        assert!(!placeholder.is_envmap_bound());
        assert_eq!(placeholder.intensity, 1.0);
    }

    #[test]
    fn shadow_atlas_descriptor_default_matches_pass_24_contract() {
        let descriptor = ShadowAtlasDescriptor::default();
        assert_eq!(descriptor.extent_px, DEFAULT_SHADOW_ATLAS_EXTENT_PX);
        assert_eq!(descriptor.tile_size_px, DEFAULT_SHADOW_ATLAS_TILE_PX);
        assert_eq!(descriptor.tile_grid_dim, 8);
        assert_eq!(descriptor.max_tiles, 64);
    }

    #[test]
    fn shadow_atlas_allocates_until_capacity_then_overflows() {
        let descriptor = ShadowAtlasDescriptor::new(1024, 256);
        let mut atlas = ShadowAtlasPlan::new(descriptor);
        atlas.begin_frame();
        for index in 0..descriptor.max_tiles {
            let _ = atlas
                .allocate(RenderLightId::new(index as u32 + 1, 1), 0, 1)
                .expect("allocation");
        }
        let overflow = atlas.allocate(RenderLightId::new(99, 1), 0, 1);
        assert!(overflow.is_none());
        assert_eq!(atlas.rejected_overflow_count(), 1);
        assert_eq!(atlas.allocations().len(), descriptor.max_tiles as usize);
    }

    #[test]
    fn shadow_cascade_default_builds_four_slices_with_increasing_distances() {
        let plan = ShadowCascadePlan::build_default(RenderLightId::new(1, 1), 0.1, 100.0);
        assert_eq!(plan.cascade_count, DEFAULT_CSM_CASCADE_COUNT as u8);
        assert_eq!(plan.slices.len(), DEFAULT_CSM_CASCADE_COUNT);
        for window in plan.slices.windows(2) {
            assert!(window[0].far_distance <= window[1].far_distance);
        }
        assert_eq!(plan.slices[0].atlas_slot, ShadowAtlasTileSlot::INVALID);
    }

    #[test]
    fn shadow_cascade_assign_slot_updates_named_cascade() {
        let mut plan = ShadowCascadePlan::build_default(RenderLightId::new(1, 1), 0.1, 100.0);
        let slot = ShadowAtlasTileSlot::new(3, 1);
        assert!(plan.assign_slot(2, slot));
        assert_eq!(plan.slices[2].atlas_slot, slot);
    }

    #[test]
    fn depth_only_pso_matches_light_kind_correctly() {
        assert!(
            DepthOnlyPso::DirectionalCascade.matches_light_kind(LightingLightKind::Directional)
        );
        assert!(DepthOnlyPso::PointParaboloid.matches_light_kind(LightingLightKind::Point));
        assert!(DepthOnlyPso::SpotPerspective.matches_light_kind(LightingLightKind::Spot));
        assert!(!DepthOnlyPso::DirectionalCascade.matches_light_kind(LightingLightKind::Point));
    }

    #[test]
    fn cluster_grid_default_has_3456_clusters() {
        let grid = ClusterGridDescriptor::default();
        assert_eq!(grid.cluster_count(), 16 * 9 * 24);
    }

    #[test]
    fn cluster_grid_index_round_trips_for_corner_cells() {
        let grid = ClusterGridDescriptor::default();
        let zero = grid.cluster_index(0, 0, 0);
        let upper = grid.cluster_index(
            grid.clusters_x - 1,
            grid.clusters_y - 1,
            grid.clusters_z - 1,
        );
        assert_eq!(zero, 0);
        assert_eq!(upper, grid.cluster_count() - 1);
    }

    #[test]
    fn clustered_light_list_records_lights_until_per_cluster_cap() {
        let grid = ClusterGridDescriptor {
            max_lights_per_cluster: 2,
            ..ClusterGridDescriptor::default()
        };
        let mut buffer = ClusteredLightListBuffer::for_grid(grid);
        assert!(buffer.record_light(0, 100));
        assert!(buffer.record_light(0, 200));
        assert!(!buffer.record_light(0, 300));
        assert_eq!(buffer.count_for(0), 2);
        assert_eq!(buffer.lights_for(0), &[100, 200]);
        assert_eq!(buffer.overflow_cluster_count, 1);
    }

    #[test]
    fn cluster_assignment_pass_dispatches_cover_full_grid() {
        let grid = ClusterGridDescriptor::default();
        let pass = ClusterTileAssignmentPassPlan::for_grid(grid);
        assert_eq!(pass.dispatch_x, 4);
        assert_eq!(pass.dispatch_y, 3);
        assert_eq!(pass.dispatch_z, 6);
        assert!(pass.total_dispatches() > 0);
    }

    #[test]
    fn diagnostics_roll_up_state_from_resources() {
        let mut table = LightingLightTable::default();
        table.record(directional_light_record());
        let mut directional_with_shadows = directional_light_record();
        directional_with_shadows.shadow_caster = true;
        table.record(directional_with_shadows);
        let atlas = ShadowAtlasPlan::default();
        let buffer = ClusteredLightListBuffer::default();
        let ibl = IblPlaceholder::default();
        let diagnostics = LightingStackDiagnostics::from_state(&table, &atlas, &buffer, &ibl);
        assert_eq!(diagnostics.directional_light_count, 2);
        assert_eq!(diagnostics.shadow_caster_count, 1);
        assert_eq!(
            diagnostics.shadow_atlas_tile_capacity,
            atlas.descriptor().max_tiles,
        );
        assert_eq!(diagnostics.cluster_count, buffer.grid.cluster_count());
        assert!(!diagnostics.ibl_envmap_bound);
    }

    #[test]
    fn diagnostics_record_timings_accumulate() {
        let mut diagnostics = LightingStackDiagnostics::default();
        diagnostics.record_lighting_cpu_setup_ns(100);
        diagnostics.record_lighting_cpu_setup_ns(50);
        diagnostics.record_shadow_render_gpu_ns(200);
        diagnostics.record_clustered_assignment_gpu_ns(75);
        diagnostics.record_forward_lighting_gpu_ns(125);
        assert_eq!(diagnostics.lighting_setup_cpu_ns, 150);
        assert_eq!(diagnostics.shadow_render_gpu_ns, 200);
        assert_eq!(diagnostics.clustered_assignment_gpu_ns, 75);
        assert_eq!(diagnostics.forward_lighting_gpu_ns, 125);
    }

    #[test]
    fn pbr_shadow_mode_from_shadow_caster_routes_correctly() {
        assert_eq!(
            PbrShadowMode::from_shadow_caster(ShadowCaster {
                mode: ShadowMode::None,
            }),
            PbrShadowMode::None,
        );
        assert_eq!(
            PbrShadowMode::from_shadow_caster(ShadowCaster {
                mode: ShadowMode::VirtualPages,
            }),
            PbrShadowMode::VirtualPages,
        );
        assert_eq!(
            PbrShadowMode::from_shadow_caster(ShadowCaster {
                mode: ShadowMode::RayTraced,
            }),
            PbrShadowMode::RayTraced,
        );
    }

    #[test]
    fn light_table_layer_mask_round_trips() {
        let mut record = directional_light_record();
        record.layers = RenderLayerMask(0b1010);
        let mut table = LightingLightTable::default();
        table.record(record);
        assert_eq!(table.records()[0].layers.0, 0b1010);
    }
}
