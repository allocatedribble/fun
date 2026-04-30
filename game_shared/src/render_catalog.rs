#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RenderAssetId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ColliderAssetId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialPresetId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LodPolicyId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OcclusionCellId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CatalogGeometry {
    Plane { size: [f32; 3] },
    Cuboid { size: [f32; 3] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CatalogCollider {
    Cuboid { size: [f32; 3] },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderCostClass {
    Tiny,
    Simple,
    DenseStatic,
    DenseDynamic,
    Transparent,
    Viewmodel,
    RayProxyOnly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualImportance {
    Foreground,
    GameplayCover,
    Navigation,
    Background,
    SetDressing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightingParticipation(pub u32);

impl LightingParticipation {
    pub const DIRECT_SHADOW: Self = Self(1 << 0);
    pub const GI: Self = Self(1 << 1);
    pub const SPECULAR: Self = Self(1 << 2);
    pub const RAY_PROXY: Self = Self(1 << 3);
    pub const EMISSIVE: Self = Self(1 << 4);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MaterialPreset {
    pub id: MaterialPresetId,
    pub name: &'static str,
    pub srgb: [u8; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderCatalogEntry {
    pub asset_id: RenderAssetId,
    pub name: &'static str,
    pub visual_mesh: &'static str,
    pub meshlet_mesh: &'static str,
    pub fallback_raster_mesh: &'static str,
    pub ray_proxy: &'static str,
    pub collider: Option<(ColliderAssetId, CatalogCollider)>,
    pub geometry: Option<CatalogGeometry>,
    pub material: MaterialPresetId,
    pub lod_policy: LodPolicyId,
    pub occlusion_cell: OcclusionCellId,
    pub gameplay_tag: &'static str,
    pub cost_class: RenderCostClass,
    pub visual_importance: VisualImportance,
    pub lighting: LightingParticipation,
}

pub const MATERIAL_FLOOR: MaterialPresetId = MaterialPresetId(1);
pub const MATERIAL_WALL: MaterialPresetId = MaterialPresetId(2);
pub const MATERIAL_RAMP: MaterialPresetId = MaterialPresetId(3);
pub const MATERIAL_COVER: MaterialPresetId = MaterialPresetId(4);

pub const ASSET_FLOOR: RenderAssetId = RenderAssetId(1);
pub const ASSET_FLOOR_COLLIDER: RenderAssetId = RenderAssetId(2);
pub const ASSET_WALL: RenderAssetId = RenderAssetId(3);
pub const ASSET_RAMP: RenderAssetId = RenderAssetId(4);
pub const ASSET_COVER_CUBE: RenderAssetId = RenderAssetId(5);

pub const COLLIDER_NONE: ColliderAssetId = ColliderAssetId(0);
pub const COLLIDER_FLOOR: ColliderAssetId = ColliderAssetId(1);
pub const COLLIDER_WALL: ColliderAssetId = ColliderAssetId(2);
pub const COLLIDER_RAMP: ColliderAssetId = ColliderAssetId(3);
pub const COLLIDER_COVER_CUBE: ColliderAssetId = ColliderAssetId(4);

pub const MATERIAL_PRESETS: &[MaterialPreset] = &[
    MaterialPreset {
        id: MATERIAL_FLOOR,
        name: "demo_floor_matte",
        srgb: [41, 48, 41, 255],
    },
    MaterialPreset {
        id: MATERIAL_WALL,
        name: "demo_wall_blue_gray",
        srgb: [82, 89, 107, 255],
    },
    MaterialPreset {
        id: MATERIAL_RAMP,
        name: "demo_ramp_warm_brown",
        srgb: [97, 71, 56, 255],
    },
    MaterialPreset {
        id: MATERIAL_COVER,
        name: "demo_cover_orange",
        srgb: [204, 102, 77, 255],
    },
];

pub const DEMO_RENDER_CATALOG: &[RenderCatalogEntry] = &[
    RenderCatalogEntry {
        asset_id: ASSET_FLOOR,
        name: "floor_visual",
        visual_mesh: "demo/floor.visual",
        meshlet_mesh: "demo/floor.meshlet",
        fallback_raster_mesh: "demo/floor.raster",
        ray_proxy: "demo/floor.ray",
        collider: None,
        geometry: Some(CatalogGeometry::Plane {
            size: [60.0, 0.0, 60.0],
        }),
        material: MATERIAL_FLOOR,
        lod_policy: LodPolicyId(1),
        occlusion_cell: OcclusionCellId(1),
        gameplay_tag: "floor",
        cost_class: RenderCostClass::Simple,
        visual_importance: VisualImportance::Navigation,
        lighting: LightingParticipation::DIRECT_SHADOW
            .union(LightingParticipation::GI)
            .union(LightingParticipation::RAY_PROXY),
    },
    RenderCatalogEntry {
        asset_id: ASSET_FLOOR_COLLIDER,
        name: "floor_collider",
        visual_mesh: "",
        meshlet_mesh: "",
        fallback_raster_mesh: "",
        ray_proxy: "",
        collider: Some((
            COLLIDER_FLOOR,
            CatalogCollider::Cuboid {
                size: [60.0, 0.5, 60.0],
            },
        )),
        geometry: None,
        material: MATERIAL_FLOOR,
        lod_policy: LodPolicyId(0),
        occlusion_cell: OcclusionCellId(1),
        gameplay_tag: "floor_collider",
        cost_class: RenderCostClass::RayProxyOnly,
        visual_importance: VisualImportance::Navigation,
        lighting: LightingParticipation(0),
    },
    RenderCatalogEntry {
        asset_id: ASSET_WALL,
        name: "wall_block",
        visual_mesh: "demo/wall.visual",
        meshlet_mesh: "demo/wall.meshlet",
        fallback_raster_mesh: "demo/wall.raster",
        ray_proxy: "demo/wall.ray",
        collider: Some((
            COLLIDER_WALL,
            CatalogCollider::Cuboid {
                size: [5.0, 3.0, 1.0],
            },
        )),
        geometry: Some(CatalogGeometry::Cuboid {
            size: [5.0, 3.0, 1.0],
        }),
        material: MATERIAL_WALL,
        lod_policy: LodPolicyId(1),
        occlusion_cell: OcclusionCellId(2),
        gameplay_tag: "cover",
        cost_class: RenderCostClass::Simple,
        visual_importance: VisualImportance::GameplayCover,
        lighting: LightingParticipation::DIRECT_SHADOW
            .union(LightingParticipation::GI)
            .union(LightingParticipation::SPECULAR)
            .union(LightingParticipation::RAY_PROXY),
    },
    RenderCatalogEntry {
        asset_id: ASSET_RAMP,
        name: "ramp_block",
        visual_mesh: "demo/ramp.visual",
        meshlet_mesh: "demo/ramp.meshlet",
        fallback_raster_mesh: "demo/ramp.raster",
        ray_proxy: "demo/ramp.ray",
        collider: Some((
            COLLIDER_RAMP,
            CatalogCollider::Cuboid {
                size: [3.0, 0.5, 6.0],
            },
        )),
        geometry: Some(CatalogGeometry::Cuboid {
            size: [3.0, 0.5, 6.0],
        }),
        material: MATERIAL_RAMP,
        lod_policy: LodPolicyId(1),
        occlusion_cell: OcclusionCellId(2),
        gameplay_tag: "ramp",
        cost_class: RenderCostClass::Simple,
        visual_importance: VisualImportance::Navigation,
        lighting: LightingParticipation::DIRECT_SHADOW
            .union(LightingParticipation::GI)
            .union(LightingParticipation::RAY_PROXY),
    },
    RenderCatalogEntry {
        asset_id: ASSET_COVER_CUBE,
        name: "cover_cube",
        visual_mesh: "demo/cover_cube.visual",
        meshlet_mesh: "demo/cover_cube.meshlet",
        fallback_raster_mesh: "demo/cover_cube.raster",
        ray_proxy: "demo/cover_cube.ray",
        collider: Some((
            COLLIDER_COVER_CUBE,
            CatalogCollider::Cuboid {
                size: [1.0, 1.0, 1.0],
            },
        )),
        geometry: Some(CatalogGeometry::Cuboid {
            size: [1.0, 1.0, 1.0],
        }),
        material: MATERIAL_COVER,
        lod_policy: LodPolicyId(1),
        occlusion_cell: OcclusionCellId(3),
        gameplay_tag: "cover",
        cost_class: RenderCostClass::Tiny,
        visual_importance: VisualImportance::GameplayCover,
        lighting: LightingParticipation::DIRECT_SHADOW
            .union(LightingParticipation::GI)
            .union(LightingParticipation::RAY_PROXY),
    },
];

pub fn demo_catalog_entry(asset_id: RenderAssetId) -> Option<&'static RenderCatalogEntry> {
    DEMO_RENDER_CATALOG
        .iter()
        .find(|entry| entry.asset_id == asset_id)
}

pub fn material_preset(id: MaterialPresetId) -> Option<&'static MaterialPreset> {
    MATERIAL_PRESETS.iter().find(|preset| preset.id == id)
}
