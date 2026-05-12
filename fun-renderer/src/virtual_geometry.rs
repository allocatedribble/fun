use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    LogicalPageId, PageOwner, PageOwnerFrameRequests, PagePriorityInputs, PagePriorityScore,
    PagePriorityWeights, PageRequest, PageRequestSubmissionReport, PageScheduler,
};

pub const STATIC_VIRTUAL_GEOMETRY_SCHEMA_VERSION: u16 = 1;
pub const STATIC_VIRTUAL_GEOMETRY_TOOL_VERSION: u16 = 1;
pub const STATIC_VIRTUAL_GEOMETRY_PROTOTYPE_NAME: &str = "FunStaticVG";
pub const STATIC_VIRTUAL_GEOMETRY_BENCHMARK_ARTIFACT_ENV: &str =
    "FUN_RENDERER_STATIC_VIRTUAL_GEOMETRY_BENCHMARK_ARTIFACT";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VirtualGeometryAssetId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VirtualGeometryPageId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VirtualGeometryClusterId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VirtualGeometryHierarchyNodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VirtualGeometryMaterialId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VirtualGeometryMaterialSignature(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct VirtualGeometryHash(pub u64);

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualGeometryAssetKind {
    #[default]
    StaticOpaqueDense,
    StaticDestroyedAlternateClusterSet,
    Foliage,
    AlphaHeavyAggregate,
    Transparent,
    NativeUi,
    ArbitraryRuntimeFracture,
}

impl VirtualGeometryAssetKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticOpaqueDense => "static_opaque_dense",
            Self::StaticDestroyedAlternateClusterSet => "static_destroyed_alternate_cluster_set",
            Self::Foliage => "foliage",
            Self::AlphaHeavyAggregate => "alpha_heavy_aggregate",
            Self::Transparent => "transparent",
            Self::NativeUi => "native_ui",
            Self::ArbitraryRuntimeFracture => "arbitrary_runtime_fracture",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VirtualGeometryAssetDecision {
    #[default]
    Candidate,
    UnsupportedSchemaVersion,
    ChecksumMismatch,
    EmptyHierarchy,
    NotBakedOffline,
    NonStaticOpaqueUnsupported,
    PoorAggregateGeometry,
    TransparentUnsupported,
    NativeUiUnsupported,
    RuntimeFractureUnsupported,
}

impl VirtualGeometryAssetDecision {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::UnsupportedSchemaVersion => "unsupported_schema_version",
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::EmptyHierarchy => "empty_hierarchy",
            Self::NotBakedOffline => "not_baked_offline",
            Self::NonStaticOpaqueUnsupported => "non_static_opaque_unsupported",
            Self::PoorAggregateGeometry => "poor_aggregate_geometry",
            Self::TransparentUnsupported => "transparent_unsupported",
            Self::NativeUiUnsupported => "native_ui_unsupported",
            Self::RuntimeFractureUnsupported => "runtime_fracture_unsupported",
        }
    }

    #[must_use]
    pub const fn accepted(self) -> bool {
        matches!(self, Self::Candidate)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualGeometryClusterCullHint {
    #[default]
    Visible,
    FrustumCulled,
    HzbOccluded,
}

impl VirtualGeometryClusterCullHint {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::FrustumCulled => "frustum_culled",
            Self::HzbOccluded => "hzb_occluded",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualGeometryBakeTool {
    #[default]
    FunRendererVirtualGeometryBake,
}

impl VirtualGeometryBakeTool {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FunRendererVirtualGeometryBake => "fun_renderer_virtual_geometry_bake",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualGeometryToolMetadata {
    pub tool: VirtualGeometryBakeTool,
    pub tool_version: u16,
    pub deterministic: bool,
    pub source_triangle_count: u32,
    pub source_index_count: u32,
    pub cluster_triangle_budget: u32,
    pub clusters_per_page: u32,
}

impl Default for VirtualGeometryToolMetadata {
    fn default() -> Self {
        Self {
            tool: VirtualGeometryBakeTool::FunRendererVirtualGeometryBake,
            tool_version: STATIC_VIRTUAL_GEOMETRY_TOOL_VERSION,
            deterministic: true,
            source_triangle_count: 0,
            source_index_count: 0,
            cluster_triangle_budget: 0,
            clusters_per_page: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VirtualGeometryBounds {
    pub center: [f32; 3],
    pub radius: f32,
}

impl VirtualGeometryBounds {
    #[must_use]
    pub const fn new(center: [f32; 3], radius: f32) -> Self {
        Self { center, radius }
    }

    #[must_use]
    pub fn distance_from(self, point: [f32; 3]) -> f32 {
        let dx = self.center[0] - point[0];
        let dy = self.center[1] - point[1];
        let dz = self.center[2] - point[2];
        (dx.mul_add(dx, dy.mul_add(dy, dz * dz))).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualGeometryRange {
    pub first: u32,
    pub count: u32,
}

impl VirtualGeometryRange {
    pub const EMPTY: Self = Self { first: 0, count: 0 };

    #[must_use]
    pub const fn new(first: u32, count: u32) -> Self {
        Self { first, count }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VirtualGeometryNormalCone {
    pub axis: [f32; 3],
    pub cutoff: f32,
}

impl VirtualGeometryNormalCone {
    #[must_use]
    pub const fn new(axis: [f32; 3], cutoff: f32) -> Self {
        Self { axis, cutoff }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualGeometryChildRange {
    pub first_child: u32,
    pub child_count: u32,
}

impl VirtualGeometryChildRange {
    pub const EMPTY: Self = Self {
        first_child: 0,
        child_count: 0,
    };

    #[must_use]
    pub const fn new(first_child: u32, child_count: u32) -> Self {
        Self {
            first_child,
            child_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualGeometryMaterialRange {
    pub material_id: VirtualGeometryMaterialId,
    pub material_signature: VirtualGeometryMaterialSignature,
    pub first_cluster: u32,
    pub cluster_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualGeometryPage {
    pub id: VirtualGeometryPageId,
    pub byte_offset: u64,
    pub compressed_bytes: u64,
    pub first_cluster: u32,
    pub cluster_count: u32,
    pub checksum: VirtualGeometryHash,
}

impl VirtualGeometryPage {
    #[must_use]
    pub fn logical_page_id(self, asset_id: VirtualGeometryAssetId) -> LogicalPageId {
        LogicalPageId::new(
            PageOwner::VirtualGeometry,
            virtual_geometry_page_value(asset_id, self.id),
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VirtualGeometryClusterHeader {
    pub id: VirtualGeometryClusterId,
    pub page_id: VirtualGeometryPageId,
    pub material_range_index: u32,
    pub material_id: VirtualGeometryMaterialId,
    pub material_signature: VirtualGeometryMaterialSignature,
    pub local_bounds: VirtualGeometryBounds,
    pub world_bounds: VirtualGeometryBounds,
    pub triangle_range: VirtualGeometryRange,
    pub index_range: VirtualGeometryRange,
    pub normal_cone: VirtualGeometryNormalCone,
    pub parent_cluster_id: Option<VirtualGeometryClusterId>,
    pub child_range: VirtualGeometryChildRange,
    pub triangle_count: u32,
    pub bounds_radius: f32,
    pub cluster_error: f32,
    pub screen_error: f32,
    pub screen_area: f32,
    pub distance_meters: f32,
    pub hzb_occluder: bool,
    pub material_id_count: u8,
    pub cull_hint: VirtualGeometryClusterCullHint,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualGeometryHierarchyNode {
    pub id: VirtualGeometryHierarchyNodeId,
    pub cluster_id: VirtualGeometryClusterId,
    pub parent: Option<VirtualGeometryHierarchyNodeId>,
    pub child_range: VirtualGeometryChildRange,
    pub children: Vec<VirtualGeometryHierarchyNodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct VirtualGeometryFallbackMesh {
    pub mesh_id: u64,
    pub bounds: VirtualGeometryBounds,
    pub triangle_range: VirtualGeometryRange,
    pub index_range: VirtualGeometryRange,
    pub checksum: VirtualGeometryHash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualGeometryDebugMetadata {
    pub source_triangle_count: u32,
    pub source_index_count: u32,
    pub cluster_count: u32,
    pub page_count: u32,
    pub hierarchy_node_count: u32,
    pub fallback_mesh_available: bool,
    pub deterministic_output: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct StaticVirtualGeometryAssetHeader {
    pub schema_version: u16,
    pub asset_id: VirtualGeometryAssetId,
    pub kind: VirtualGeometryAssetKind,
    pub root_node: VirtualGeometryHierarchyNodeId,
    pub page_count: u32,
    pub cluster_count: u32,
    pub hierarchy_node_count: u32,
    pub material_range_count: u32,
    pub checksum: VirtualGeometryHash,
    pub tool_metadata: VirtualGeometryToolMetadata,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaticVirtualGeometryAsset {
    pub header: StaticVirtualGeometryAssetHeader,
    pub material_ranges: Vec<VirtualGeometryMaterialRange>,
    pub pages: Vec<VirtualGeometryPage>,
    pub clusters: Vec<VirtualGeometryClusterHeader>,
    pub hierarchy: Vec<VirtualGeometryHierarchyNode>,
    pub fallback_mesh: VirtualGeometryFallbackMesh,
    pub debug_metadata: VirtualGeometryDebugMetadata,
}

impl StaticVirtualGeometryAsset {
    #[must_use]
    pub fn page(&self, page_id: VirtualGeometryPageId) -> Option<&VirtualGeometryPage> {
        self.pages.iter().find(|page| page.id == page_id)
    }

    #[must_use]
    pub fn cluster(
        &self,
        cluster_id: VirtualGeometryClusterId,
    ) -> Option<&VirtualGeometryClusterHeader> {
        self.clusters
            .iter()
            .find(|cluster| cluster.id == cluster_id)
    }

    #[must_use]
    pub fn node(
        &self,
        node_id: VirtualGeometryHierarchyNodeId,
    ) -> Option<&VirtualGeometryHierarchyNode> {
        self.hierarchy.iter().find(|node| node.id == node_id)
    }

    #[must_use]
    pub fn checksum_valid(&self) -> bool {
        compute_static_virtual_geometry_checksum(self) == self.header.checksum
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StaticVirtualGeometryBakeConfig {
    pub asset_id: u64,
    pub material_id: u32,
    pub fallback_mesh_id: u64,
    pub triangle_count: u32,
    pub index_count: u32,
    pub cluster_triangle_budget: u32,
    pub clusters_per_page: u32,
    pub root_page_compressed_bytes: u64,
    pub child_page_compressed_bytes: u64,
    pub bounds_radius: f32,
    pub screen_error: f32,
}

impl Default for StaticVirtualGeometryBakeConfig {
    fn default() -> Self {
        Self {
            asset_id: 1,
            material_id: 7,
            fallback_mesh_id: 1,
            triangle_count: 8_192,
            index_count: 24_576,
            cluster_triangle_budget: 512,
            clusters_per_page: 4,
            root_page_compressed_bytes: 128 * 1024,
            child_page_compressed_bytes: 256 * 1024,
            bounds_radius: 16.0,
            screen_error: 0.08,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaticVirtualGeometryBakeError {
    TriangleCountRequired,
    IndexCountRequired,
}

impl StaticVirtualGeometryBakeError {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TriangleCountRequired => "triangle_count_required",
            Self::IndexCountRequired => "index_count_required",
        }
    }
}

pub fn bake_static_virtual_geometry(
    config: &StaticVirtualGeometryBakeConfig,
) -> Result<StaticVirtualGeometryAsset, StaticVirtualGeometryBakeError> {
    if config.triangle_count == 0 {
        return Err(StaticVirtualGeometryBakeError::TriangleCountRequired);
    }
    if config.index_count == 0 {
        return Err(StaticVirtualGeometryBakeError::IndexCountRequired);
    }

    let asset_id = VirtualGeometryAssetId(config.asset_id);
    let material_id = VirtualGeometryMaterialId(config.material_id);
    let material_signature = VirtualGeometryMaterialSignature(stable_pair_hash(
        u64::from(config.material_id),
        config.asset_id,
    ));
    let cluster_triangle_budget = config.cluster_triangle_budget.max(1);
    let child_count = config
        .triangle_count
        .div_ceil(cluster_triangle_budget)
        .max(1);
    let clusters_per_page = config.clusters_per_page.max(1);
    let child_page_count = child_count.div_ceil(clusters_per_page);
    let root_cluster_id = VirtualGeometryClusterId(1);
    let root_node_id = VirtualGeometryHierarchyNodeId(1);
    let root_page_id = VirtualGeometryPageId(1);

    let mut pages = Vec::with_capacity(child_page_count.saturating_add(1) as usize);
    let mut page_offset = 0_u64;
    let root_page = page_with_checksum(
        root_page_id,
        page_offset,
        config.root_page_compressed_bytes,
        0,
        1,
    );
    page_offset = page_offset.saturating_add(root_page.compressed_bytes);
    pages.push(root_page);
    for page_index in 0..child_page_count {
        let first_child_cluster = 1 + page_index * clusters_per_page;
        let remaining = child_count.saturating_sub(page_index * clusters_per_page);
        let page = page_with_checksum(
            VirtualGeometryPageId(u64::from(page_index) + 2),
            page_offset,
            config.child_page_compressed_bytes,
            first_child_cluster,
            remaining.min(clusters_per_page),
        );
        page_offset = page_offset.saturating_add(page.compressed_bytes);
        pages.push(page);
    }

    let material_ranges = vec![VirtualGeometryMaterialRange {
        material_id,
        material_signature,
        first_cluster: 0,
        cluster_count: child_count.saturating_add(1),
    }];

    let mut clusters = Vec::with_capacity(child_count.saturating_add(1) as usize);
    clusters.push(VirtualGeometryClusterHeader {
        id: root_cluster_id,
        page_id: root_page_id,
        material_range_index: 0,
        material_id,
        material_signature,
        local_bounds: VirtualGeometryBounds::new([0.0, 0.0, 0.0], config.bounds_radius),
        world_bounds: VirtualGeometryBounds::new([0.0, 0.0, 0.0], config.bounds_radius),
        triangle_range: VirtualGeometryRange::new(0, config.triangle_count),
        index_range: VirtualGeometryRange::new(0, config.index_count),
        normal_cone: VirtualGeometryNormalCone::new([0.0, 1.0, 0.0], 0.0),
        parent_cluster_id: None,
        child_range: VirtualGeometryChildRange::new(1, child_count),
        triangle_count: config.triangle_count,
        bounds_radius: config.bounds_radius,
        cluster_error: config.screen_error,
        screen_error: config.screen_error,
        screen_area: 1.0,
        distance_meters: 0.0,
        hzb_occluder: true,
        material_id_count: 1,
        cull_hint: VirtualGeometryClusterCullHint::Visible,
    });

    let child_node_ids = (0..child_count)
        .map(|index| VirtualGeometryHierarchyNodeId(index + 2))
        .collect::<Vec<_>>();
    let mut hierarchy = Vec::with_capacity(child_count.saturating_add(1) as usize);
    hierarchy.push(VirtualGeometryHierarchyNode {
        id: root_node_id,
        cluster_id: root_cluster_id,
        parent: None,
        child_range: VirtualGeometryChildRange::new(0, child_count),
        children: child_node_ids,
    });

    for child_index in 0..child_count {
        let cluster_id = VirtualGeometryClusterId(u64::from(child_index) + 2);
        let page_id = VirtualGeometryPageId(u64::from(child_index / clusters_per_page) + 2);
        let first_triangle = child_index.saturating_mul(cluster_triangle_budget);
        let remaining_triangles = config.triangle_count.saturating_sub(first_triangle);
        let triangle_count = remaining_triangles.min(cluster_triangle_budget);
        let first_index = first_triangle.saturating_mul(3);
        let index_count = triangle_count
            .saturating_mul(3)
            .min(config.index_count.saturating_sub(first_index));
        let radius_scale = 1.0 / (child_count as f32).sqrt().max(1.0);
        let child_radius = (config.bounds_radius * radius_scale).max(0.001);
        let x_offset = child_index as f32 - child_count as f32 * 0.5;
        let distance = x_offset.abs();

        clusters.push(VirtualGeometryClusterHeader {
            id: cluster_id,
            page_id,
            material_range_index: 0,
            material_id,
            material_signature,
            local_bounds: VirtualGeometryBounds::new([x_offset, 0.0, 0.0], child_radius),
            world_bounds: VirtualGeometryBounds::new([x_offset, 0.0, 0.0], child_radius),
            triangle_range: VirtualGeometryRange::new(first_triangle, triangle_count),
            index_range: VirtualGeometryRange::new(first_index, index_count),
            normal_cone: VirtualGeometryNormalCone::new([0.0, 1.0, 0.0], 0.25),
            parent_cluster_id: Some(root_cluster_id),
            child_range: VirtualGeometryChildRange::EMPTY,
            triangle_count,
            bounds_radius: child_radius,
            cluster_error: config.screen_error / 16.0,
            screen_error: config.screen_error / 16.0,
            screen_area: 1.0 / child_count as f32,
            distance_meters: distance,
            hzb_occluder: false,
            material_id_count: 1,
            cull_hint: VirtualGeometryClusterCullHint::Visible,
        });
        hierarchy.push(VirtualGeometryHierarchyNode {
            id: VirtualGeometryHierarchyNodeId(child_index + 2),
            cluster_id,
            parent: Some(root_node_id),
            child_range: VirtualGeometryChildRange::EMPTY,
            children: Vec::new(),
        });
    }

    let fallback_mesh = VirtualGeometryFallbackMesh {
        mesh_id: config.fallback_mesh_id,
        bounds: VirtualGeometryBounds::new([0.0, 0.0, 0.0], config.bounds_radius),
        triangle_range: VirtualGeometryRange::new(0, config.triangle_count),
        index_range: VirtualGeometryRange::new(0, config.index_count),
        checksum: VirtualGeometryHash(stable_pair_hash(config.fallback_mesh_id, config.asset_id)),
    };

    let debug_metadata = VirtualGeometryDebugMetadata {
        source_triangle_count: config.triangle_count,
        source_index_count: config.index_count,
        cluster_count: saturating_u32(clusters.len()),
        page_count: saturating_u32(pages.len()),
        hierarchy_node_count: saturating_u32(hierarchy.len()),
        fallback_mesh_available: true,
        deterministic_output: true,
    };
    let mut asset = StaticVirtualGeometryAsset {
        header: StaticVirtualGeometryAssetHeader {
            schema_version: STATIC_VIRTUAL_GEOMETRY_SCHEMA_VERSION,
            asset_id,
            kind: VirtualGeometryAssetKind::StaticOpaqueDense,
            root_node: root_node_id,
            page_count: saturating_u32(pages.len()),
            cluster_count: saturating_u32(clusters.len()),
            hierarchy_node_count: saturating_u32(hierarchy.len()),
            material_range_count: saturating_u32(material_ranges.len()),
            checksum: VirtualGeometryHash(0),
            tool_metadata: VirtualGeometryToolMetadata {
                source_triangle_count: config.triangle_count,
                source_index_count: config.index_count,
                cluster_triangle_budget,
                clusters_per_page,
                ..Default::default()
            },
        },
        material_ranges,
        pages,
        clusters,
        hierarchy,
        fallback_mesh,
        debug_metadata,
    };
    asset.header.checksum = compute_static_virtual_geometry_checksum(&asset);
    Ok(asset)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VirtualGeometryMeshShaderMode {
    #[default]
    Disabled,
    Optional,
}

impl VirtualGeometryMeshShaderMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Optional => "optional",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum VirtualGeometryDrawPath {
    #[default]
    ComputeIndirect,
    OptionalMeshShader,
    FallbackMesh,
}

impl VirtualGeometryDrawPath {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ComputeIndirect => "compute_indirect",
            Self::OptionalMeshShader => "optional_mesh_shader",
            Self::FallbackMesh => "fallback_mesh",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticVirtualGeometryRuntimePolicy {
    pub screen_space_error_threshold_pixels: f32,
    pub affordable_child_page_bytes: u64,
    pub max_material_ranges_per_cluster: u8,
    pub conservative_missing_page_fallback: bool,
    pub mesh_shader_mode: VirtualGeometryMeshShaderMode,
}

impl Default for StaticVirtualGeometryRuntimePolicy {
    fn default() -> Self {
        Self {
            screen_space_error_threshold_pixels: 1.5,
            affordable_child_page_bytes: 2 * 1024 * 1024,
            max_material_ranges_per_cluster: 4,
            conservative_missing_page_fallback: true,
            mesh_shader_mode: VirtualGeometryMeshShaderMode::Disabled,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StaticVirtualGeometryViewContext {
    pub frame_index: u64,
    pub camera_position: [f32; 3],
    pub view_radius_meters: f32,
    pub viewport_area_pixels: u32,
    pub projected_bounds_radius_pixels: f32,
    pub object_visible: bool,
    pub frustum_culling_enabled: bool,
    pub hzb_available: bool,
    pub hzb_occlusion_enabled: bool,
    pub compute_indirect_enabled: bool,
    pub mesh_shader_supported: bool,
    pub gameplay_salience: u16,
    pub editor_focus: u16,
    pub temporal_instability: u16,
    pub motion_magnitude: u16,
    pub luminance_contrast_importance: u16,
    pub shadow_receiver_demand: u16,
}

impl Default for StaticVirtualGeometryViewContext {
    fn default() -> Self {
        Self {
            frame_index: 0,
            camera_position: [0.0, 0.0, 0.0],
            view_radius_meters: 256.0,
            viewport_area_pixels: 1920 * 1080,
            projected_bounds_radius_pixels: 512.0,
            object_visible: true,
            frustum_culling_enabled: true,
            hzb_available: true,
            hzb_occlusion_enabled: true,
            compute_indirect_enabled: true,
            mesh_shader_supported: false,
            gameplay_salience: 96,
            editor_focus: 0,
            temporal_instability: 32,
            motion_magnitude: 16,
            luminance_contrast_importance: 96,
            shadow_receiver_demand: 64,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualGeometrySelectionReason {
    ScreenErrorSatisfied,
    Leaf,
    MissingRootPageFallbackMesh,
    MissingChildPageFallbackCluster,
    MaterialRangeSplitAvoided,
}

impl VirtualGeometrySelectionReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScreenErrorSatisfied => "screen_error_satisfied",
            Self::Leaf => "leaf",
            Self::MissingRootPageFallbackMesh => "missing_root_page_fallback_mesh",
            Self::MissingChildPageFallbackCluster => "missing_child_page_fallback_cluster",
            Self::MaterialRangeSplitAvoided => "material_range_split_avoided",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualGeometryPageRequestReason {
    VisibleCluster,
    SplitScreenError,
    NearCamera,
    Occluder,
}

impl VirtualGeometryPageRequestReason {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VisibleCluster => "visible_cluster",
            Self::SplitScreenError => "split_screen_error",
            Self::NearCamera => "near_camera",
            Self::Occluder => "occluder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualGeometrySelectedCluster {
    pub cluster_id: VirtualGeometryClusterId,
    pub page_id: VirtualGeometryPageId,
    pub screen_space_error_pixels: f32,
    pub selection_reason: VirtualGeometrySelectionReason,
    pub fallback_parent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualGeometryPageRequestRecord {
    pub page_id: VirtualGeometryPageId,
    pub logical_id: LogicalPageId,
    pub compressed_bytes: u64,
    pub priority: PagePriorityScore,
    pub reason: VirtualGeometryPageRequestReason,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualGeometryDrawPacket {
    pub draw_path: VirtualGeometryDrawPath,
    pub page_id: Option<VirtualGeometryPageId>,
    pub material_signature: VirtualGeometryMaterialSignature,
    pub cluster_count: u32,
    pub triangle_count: u32,
    pub cluster_ids: Vec<VirtualGeometryClusterId>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct StaticVirtualGeometryMetrics {
    pub input_clusters: u32,
    pub resident_clusters: u32,
    pub visible_clusters: u32,
    pub drawn_clusters: u32,
    pub culled_clusters: u32,
    pub missing_fallback_clusters: u32,
    pub geometry_page_faults: u32,
    pub bytes_streamed: u64,
    pub hzb_timing_us: u32,
    pub mesh_shader_timing_us: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StaticVirtualGeometryFrameSelection {
    pub schema_version: u16,
    pub prototype_name: &'static str,
    pub asset_id: VirtualGeometryAssetId,
    pub frame_index: u64,
    pub asset_decision: VirtualGeometryAssetDecision,
    pub selected_clusters: Vec<VirtualGeometrySelectedCluster>,
    pub visible_clusters: Vec<VirtualGeometrySelectedCluster>,
    pub requested_pages: Vec<VirtualGeometryPageRequestRecord>,
    pub missing_pages: Vec<VirtualGeometryPageId>,
    pub fallback_clusters: Vec<VirtualGeometryClusterId>,
    pub fallback_mesh_used: bool,
    pub draw_packets: Vec<VirtualGeometryDrawPacket>,
    pub scheduler_report: PageRequestSubmissionReport,
    pub metrics: StaticVirtualGeometryMetrics,
}

impl StaticVirtualGeometryFrameSelection {
    #[must_use]
    pub fn empty(
        asset: &StaticVirtualGeometryAsset,
        context: StaticVirtualGeometryViewContext,
    ) -> Self {
        Self {
            schema_version: STATIC_VIRTUAL_GEOMETRY_SCHEMA_VERSION,
            prototype_name: STATIC_VIRTUAL_GEOMETRY_PROTOTYPE_NAME,
            asset_id: asset.header.asset_id,
            frame_index: context.frame_index,
            asset_decision: evaluate_static_virtual_geometry_asset(asset),
            selected_clusters: Vec::new(),
            visible_clusters: Vec::new(),
            requested_pages: Vec::new(),
            missing_pages: Vec::new(),
            fallback_clusters: Vec::new(),
            fallback_mesh_used: false,
            draw_packets: Vec::new(),
            scheduler_report: PageRequestSubmissionReport::default(),
            metrics: StaticVirtualGeometryMetrics {
                input_clusters: saturating_u32(asset.clusters.len()),
                hzb_timing_us: u32::from(context.hzb_available && context.hzb_occlusion_enabled),
                mesh_shader_timing_us: u32::from(context.mesh_shader_supported),
                ..Default::default()
            },
        }
    }

    #[must_use]
    pub fn debug_artifact(&self) -> StaticVirtualGeometryDebugArtifact {
        use core::fmt::Write as _;

        let mut content = String::new();
        let _ = writeln!(
            content,
            "schema_version={} frame_index={} asset_id={} decision={}",
            self.schema_version,
            self.frame_index,
            self.asset_id.0,
            self.asset_decision.as_str()
        );
        let _ = writeln!(
            content,
            "input_clusters={} resident_clusters={} visible_clusters={} drawn_clusters={} culled_clusters={}",
            self.metrics.input_clusters,
            self.metrics.resident_clusters,
            self.metrics.visible_clusters,
            self.metrics.drawn_clusters,
            self.metrics.culled_clusters
        );
        let _ = writeln!(
            content,
            "missing_fallback_clusters={} geometry_page_faults={} bytes_streamed={}",
            self.metrics.missing_fallback_clusters,
            self.metrics.geometry_page_faults,
            self.metrics.bytes_streamed
        );
        let _ = writeln!(
            content,
            "hzb_timing_us={} mesh_shader_timing_us={} fallback_mesh_used={}",
            self.metrics.hzb_timing_us, self.metrics.mesh_shader_timing_us, self.fallback_mesh_used
        );
        let _ = writeln!(
            content,
            "scheduler accepted={} already_requested={} newly_requested={}",
            self.scheduler_report.accepted,
            self.scheduler_report.already_requested,
            self.scheduler_report.newly_requested
        );
        let _ = writeln!(content, "draw_packets={}", self.draw_packets.len());
        for packet in &self.draw_packets {
            let page = packet.page_id.map_or(0, |page_id| page_id.0);
            let _ = writeln!(
                content,
                "packet path={} page={} clusters={} triangles={}",
                packet.draw_path.as_str(),
                page,
                packet.cluster_count,
                packet.triangle_count
            );
        }

        StaticVirtualGeometryDebugArtifact {
            schema_version: STATIC_VIRTUAL_GEOMETRY_SCHEMA_VERSION,
            asset_id: self.asset_id,
            frame_index: self.frame_index,
            content,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticVirtualGeometryDebugArtifact {
    pub schema_version: u16,
    pub asset_id: VirtualGeometryAssetId,
    pub frame_index: u64,
    pub content: String,
}

#[must_use]
pub fn evaluate_static_virtual_geometry_asset(
    asset: &StaticVirtualGeometryAsset,
) -> VirtualGeometryAssetDecision {
    if asset.header.schema_version != STATIC_VIRTUAL_GEOMETRY_SCHEMA_VERSION {
        return VirtualGeometryAssetDecision::UnsupportedSchemaVersion;
    }
    if !asset.header.tool_metadata.deterministic {
        return VirtualGeometryAssetDecision::NotBakedOffline;
    }
    if !asset.checksum_valid() {
        return VirtualGeometryAssetDecision::ChecksumMismatch;
    }
    if asset.hierarchy.is_empty() || asset.clusters.is_empty() || asset.pages.is_empty() {
        return VirtualGeometryAssetDecision::EmptyHierarchy;
    }
    match asset.header.kind {
        VirtualGeometryAssetKind::StaticOpaqueDense
        | VirtualGeometryAssetKind::StaticDestroyedAlternateClusterSet => {
            VirtualGeometryAssetDecision::Candidate
        }
        VirtualGeometryAssetKind::Foliage | VirtualGeometryAssetKind::AlphaHeavyAggregate => {
            VirtualGeometryAssetDecision::PoorAggregateGeometry
        }
        VirtualGeometryAssetKind::Transparent => {
            VirtualGeometryAssetDecision::TransparentUnsupported
        }
        VirtualGeometryAssetKind::NativeUi => VirtualGeometryAssetDecision::NativeUiUnsupported,
        VirtualGeometryAssetKind::ArbitraryRuntimeFracture => {
            VirtualGeometryAssetDecision::RuntimeFractureUnsupported
        }
    }
}

pub fn select_static_virtual_geometry_frame(
    asset: &StaticVirtualGeometryAsset,
    scheduler: &mut PageScheduler,
    context: StaticVirtualGeometryViewContext,
    policy: StaticVirtualGeometryRuntimePolicy,
) -> StaticVirtualGeometryFrameSelection {
    let mut selection = StaticVirtualGeometryFrameSelection::empty(asset, context);
    if !context.object_visible {
        return selection;
    }

    let asset_decision = evaluate_static_virtual_geometry_asset(asset);
    selection.asset_decision = asset_decision;
    if !asset_decision.accepted() {
        return selection;
    }

    let mut request_table =
        BTreeMap::<VirtualGeometryPageId, VirtualGeometryPageRequestRecord>::new();
    let mut stack = vec![asset.header.root_node];
    while let Some(node_id) = stack.pop() {
        let Some(node) = asset.node(node_id) else {
            continue;
        };
        let Some(cluster) = asset.cluster(node.cluster_id).copied() else {
            continue;
        };

        if !is_virtual_geometry_page_resident(asset, scheduler, cluster.page_id) {
            record_virtual_geometry_page_request(
                asset,
                context,
                cluster,
                VirtualGeometryPageRequestReason::VisibleCluster,
                &mut request_table,
            );
            selection.missing_pages.push(cluster.page_id);
            selection.metrics.geometry_page_faults =
                selection.metrics.geometry_page_faults.saturating_add(1);
            if node.parent.is_none() {
                selection.fallback_mesh_used = true;
                selection
                    .draw_packets
                    .push(build_fallback_mesh_packet(asset, context, policy));
                selection.metrics.missing_fallback_clusters = selection
                    .metrics
                    .missing_fallback_clusters
                    .saturating_add(1);
            }
            continue;
        }

        selection.metrics.resident_clusters = selection.metrics.resident_clusters.saturating_add(1);
        let screen_error = virtual_geometry_screen_space_error(cluster, context);
        let should_split =
            screen_error > policy.screen_space_error_threshold_pixels && !node.children.is_empty();
        if should_split {
            if child_material_range_count_too_high(
                asset,
                node,
                policy.max_material_ranges_per_cluster,
            ) {
                push_selected_cluster(
                    &mut selection,
                    cluster,
                    screen_error,
                    VirtualGeometrySelectionReason::MaterialRangeSplitAvoided,
                    false,
                );
                continue;
            }

            let missing_child_pages = missing_child_pages(asset, scheduler, node);
            if missing_child_pages.is_empty() {
                for child in node.children.iter().rev() {
                    stack.push(*child);
                }
                continue;
            }

            let missing_bytes = missing_child_pages
                .iter()
                .filter_map(|page_id| asset.page(*page_id))
                .map(|page| page.compressed_bytes)
                .sum::<u64>();
            if missing_bytes <= policy.affordable_child_page_bytes {
                for page_id in &missing_child_pages {
                    if let Some(page_cluster) = first_cluster_for_page(asset, *page_id) {
                        record_virtual_geometry_page_request(
                            asset,
                            context,
                            page_cluster,
                            VirtualGeometryPageRequestReason::SplitScreenError,
                            &mut request_table,
                        );
                    }
                }
            }

            push_selected_cluster(
                &mut selection,
                cluster,
                screen_error,
                VirtualGeometrySelectionReason::MissingChildPageFallbackCluster,
                true,
            );
            selection.fallback_clusters.push(cluster.id);
            selection.metrics.missing_fallback_clusters = selection
                .metrics
                .missing_fallback_clusters
                .saturating_add(1);
            selection.metrics.geometry_page_faults = selection
                .metrics
                .geometry_page_faults
                .saturating_add(saturating_u32(missing_child_pages.len()));
            selection.missing_pages.extend(missing_child_pages);
            continue;
        }

        let reason = if node.children.is_empty() {
            VirtualGeometrySelectionReason::Leaf
        } else {
            VirtualGeometrySelectionReason::ScreenErrorSatisfied
        };
        push_selected_cluster(&mut selection, cluster, screen_error, reason, false);
    }

    selection.requested_pages = request_table.values().copied().collect();
    selection.metrics.bytes_streamed = selection
        .requested_pages
        .iter()
        .map(|request| request.compressed_bytes)
        .sum();
    let requests = selection
        .requested_pages
        .iter()
        .map(|request| PageRequest {
            logical_id: request.logical_id,
            estimated_upload_bytes: request.compressed_bytes,
            priority_inputs: request.priority.inputs,
        })
        .collect::<Vec<_>>();
    selection.scheduler_report = scheduler.submit_owner_requests(&PageOwnerFrameRequests::new(
        PageOwner::VirtualGeometry,
        requests,
    ));

    selection.visible_clusters =
        cull_static_virtual_geometry_clusters(asset, &selection.selected_clusters, context, policy);
    selection.metrics.visible_clusters = saturating_u32(selection.visible_clusters.len());
    selection.metrics.culled_clusters = saturating_u32(
        selection
            .selected_clusters
            .len()
            .saturating_sub(selection.visible_clusters.len()),
    );
    selection
        .draw_packets
        .extend(build_virtual_geometry_draw_packets(
            asset,
            &selection.visible_clusters,
            context,
            policy,
        ));
    selection.metrics.drawn_clusters = selection
        .draw_packets
        .iter()
        .filter(|packet| packet.draw_path != VirtualGeometryDrawPath::FallbackMesh)
        .map(|packet| packet.cluster_count)
        .sum();
    selection
}

#[must_use]
pub fn virtual_geometry_page_value(
    asset_id: VirtualGeometryAssetId,
    page_id: VirtualGeometryPageId,
) -> u64 {
    stable_pair_hash(asset_id.0, page_id.0)
}

#[must_use]
pub fn virtual_geometry_screen_space_error(
    cluster: VirtualGeometryClusterHeader,
    context: StaticVirtualGeometryViewContext,
) -> f32 {
    if !cluster.bounds_radius.is_finite() || cluster.bounds_radius <= 0.0 {
        return context.projected_bounds_radius_pixels.max(0.0);
    }
    let baked_error = if cluster.screen_error.is_finite() && cluster.screen_error > 0.0 {
        cluster.screen_error
    } else {
        cluster.cluster_error
    };
    let normalized_error = (baked_error / cluster.bounds_radius).max(0.0);
    context.projected_bounds_radius_pixels.max(0.0) * normalized_error
}

fn push_selected_cluster(
    selection: &mut StaticVirtualGeometryFrameSelection,
    cluster: VirtualGeometryClusterHeader,
    screen_error: f32,
    selection_reason: VirtualGeometrySelectionReason,
    fallback_parent: bool,
) {
    selection
        .selected_clusters
        .push(VirtualGeometrySelectedCluster {
            cluster_id: cluster.id,
            page_id: cluster.page_id,
            screen_space_error_pixels: screen_error,
            selection_reason,
            fallback_parent,
        });
}

fn record_virtual_geometry_page_request(
    asset: &StaticVirtualGeometryAsset,
    context: StaticVirtualGeometryViewContext,
    cluster: VirtualGeometryClusterHeader,
    reason: VirtualGeometryPageRequestReason,
    request_table: &mut BTreeMap<VirtualGeometryPageId, VirtualGeometryPageRequestRecord>,
) {
    let Some(page) = asset.page(cluster.page_id) else {
        return;
    };
    let inputs = virtual_geometry_page_priority_inputs(context, cluster, reason);
    let priority = PagePriorityScore::compute(inputs, PagePriorityWeights::default());
    let record = VirtualGeometryPageRequestRecord {
        page_id: page.id,
        logical_id: page.logical_page_id(asset.header.asset_id),
        compressed_bytes: page.compressed_bytes,
        priority,
        reason,
    };
    match request_table.get(&record.page_id) {
        Some(existing) if existing.priority.value >= record.priority.value => {}
        _ => {
            request_table.insert(record.page_id, record);
        }
    }
}

fn virtual_geometry_page_priority_inputs(
    context: StaticVirtualGeometryViewContext,
    cluster: VirtualGeometryClusterHeader,
    reason: VirtualGeometryPageRequestReason,
) -> PagePriorityInputs {
    let screen_error = virtual_geometry_screen_space_error(cluster, context);
    PagePriorityInputs {
        projected_area: score_f32(cluster.screen_area.max(screen_error / 256.0), 0.0, 1.0, 255),
        camera_proximity: score_inverse_distance(
            cluster
                .world_bounds
                .distance_from(context.camera_position)
                .min(cluster.distance_meters.max(0.0)),
        ),
        visibility_confidence: match reason {
            VirtualGeometryPageRequestReason::VisibleCluster => 224,
            VirtualGeometryPageRequestReason::SplitScreenError => 240,
            VirtualGeometryPageRequestReason::NearCamera => 216,
            VirtualGeometryPageRequestReason::Occluder => 232,
        },
        temporal_instability: context.temporal_instability,
        motion_magnitude: context.motion_magnitude,
        luminance_contrast_importance: context.luminance_contrast_importance,
        shadow_receiver_demand: context.shadow_receiver_demand
            + u16::from(cluster.hzb_occluder) * 32,
        gameplay_salience: context.gameplay_salience,
        editor_focus: context.editor_focus,
    }
}

fn is_virtual_geometry_page_resident(
    asset: &StaticVirtualGeometryAsset,
    scheduler: &PageScheduler,
    page_id: VirtualGeometryPageId,
) -> bool {
    asset
        .page(page_id)
        .and_then(|page| scheduler.page(page.logical_page_id(asset.header.asset_id)))
        .is_some_and(|record| record.residency_state.is_resident())
}

fn missing_child_pages(
    asset: &StaticVirtualGeometryAsset,
    scheduler: &PageScheduler,
    node: &VirtualGeometryHierarchyNode,
) -> Vec<VirtualGeometryPageId> {
    let mut missing = BTreeMap::<VirtualGeometryPageId, ()>::new();
    for child_id in &node.children {
        if let Some(child_cluster) = asset
            .node(*child_id)
            .and_then(|child| asset.cluster(child.cluster_id))
            && !is_virtual_geometry_page_resident(asset, scheduler, child_cluster.page_id)
        {
            missing.insert(child_cluster.page_id, ());
        }
    }
    missing.into_keys().collect()
}

fn first_cluster_for_page(
    asset: &StaticVirtualGeometryAsset,
    page_id: VirtualGeometryPageId,
) -> Option<VirtualGeometryClusterHeader> {
    let page = asset.page(page_id)?;
    asset
        .clusters
        .get(page.first_cluster as usize)
        .copied()
        .filter(|cluster| cluster.page_id == page_id)
}

fn child_material_range_count_too_high(
    asset: &StaticVirtualGeometryAsset,
    node: &VirtualGeometryHierarchyNode,
    max_material_ranges_per_cluster: u8,
) -> bool {
    node.children.iter().any(|child_id| {
        asset
            .node(*child_id)
            .and_then(|child| asset.cluster(child.cluster_id))
            .is_some_and(|cluster| cluster.material_id_count > max_material_ranges_per_cluster)
    })
}

fn cull_static_virtual_geometry_clusters(
    asset: &StaticVirtualGeometryAsset,
    selected_clusters: &[VirtualGeometrySelectedCluster],
    context: StaticVirtualGeometryViewContext,
    policy: StaticVirtualGeometryRuntimePolicy,
) -> Vec<VirtualGeometrySelectedCluster> {
    let mut visible = Vec::with_capacity(selected_clusters.len());
    for selected in selected_clusters {
        let Some(cluster) = asset.cluster(selected.cluster_id) else {
            continue;
        };
        let frustum_rejects = context.frustum_culling_enabled
            && (matches!(
                cluster.cull_hint,
                VirtualGeometryClusterCullHint::FrustumCulled
            ) || cluster.world_bounds.distance_from(context.camera_position)
                > context
                    .view_radius_meters
                    .saturating_add_f32(cluster.world_bounds.radius));
        let hzb_rejects = context.hzb_available
            && context.hzb_occlusion_enabled
            && matches!(
                cluster.cull_hint,
                VirtualGeometryClusterCullHint::HzbOccluded
            );
        let cull_rejects = frustum_rejects || hzb_rejects;
        if cull_rejects && policy.conservative_missing_page_fallback && selected.fallback_parent {
            visible.push(*selected);
            continue;
        }
        if !cull_rejects {
            visible.push(*selected);
        }
    }
    visible
}

fn build_virtual_geometry_draw_packets(
    asset: &StaticVirtualGeometryAsset,
    visible_clusters: &[VirtualGeometrySelectedCluster],
    context: StaticVirtualGeometryViewContext,
    policy: StaticVirtualGeometryRuntimePolicy,
) -> Vec<VirtualGeometryDrawPacket> {
    let draw_path = draw_path_for_context(context, policy);
    let mut builders = BTreeMap::<
        (
            VirtualGeometryPageId,
            VirtualGeometryMaterialSignature,
            VirtualGeometryDrawPath,
        ),
        VirtualGeometryPacketBuilder,
    >::new();
    for selected in visible_clusters {
        let Some(cluster) = asset.cluster(selected.cluster_id) else {
            continue;
        };
        let key = (cluster.page_id, cluster.material_signature, draw_path);
        builders
            .entry(key)
            .or_insert_with(|| VirtualGeometryPacketBuilder {
                draw_path,
                page_id: Some(cluster.page_id),
                material_signature: cluster.material_signature,
                triangle_count: 0,
                cluster_ids: Vec::new(),
            })
            .push(cluster);
    }
    builders.into_values().map(Into::into).collect()
}

fn build_fallback_mesh_packet(
    asset: &StaticVirtualGeometryAsset,
    context: StaticVirtualGeometryViewContext,
    policy: StaticVirtualGeometryRuntimePolicy,
) -> VirtualGeometryDrawPacket {
    let material_signature = asset
        .material_ranges
        .first()
        .map_or(VirtualGeometryMaterialSignature(0), |range| {
            range.material_signature
        });
    VirtualGeometryDrawPacket {
        draw_path: if context.compute_indirect_enabled {
            draw_path_for_context(context, policy)
        } else {
            VirtualGeometryDrawPath::FallbackMesh
        },
        page_id: None,
        material_signature,
        cluster_count: 1,
        triangle_count: asset.fallback_mesh.triangle_range.count,
        cluster_ids: Vec::new(),
    }
}

fn draw_path_for_context(
    context: StaticVirtualGeometryViewContext,
    policy: StaticVirtualGeometryRuntimePolicy,
) -> VirtualGeometryDrawPath {
    if matches!(
        policy.mesh_shader_mode,
        VirtualGeometryMeshShaderMode::Optional
    ) && context.mesh_shader_supported
    {
        return VirtualGeometryDrawPath::OptionalMeshShader;
    }
    if context.compute_indirect_enabled {
        VirtualGeometryDrawPath::ComputeIndirect
    } else {
        VirtualGeometryDrawPath::FallbackMesh
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct VirtualGeometryPacketBuilder {
    draw_path: VirtualGeometryDrawPath,
    page_id: Option<VirtualGeometryPageId>,
    material_signature: VirtualGeometryMaterialSignature,
    triangle_count: u32,
    cluster_ids: Vec<VirtualGeometryClusterId>,
}

impl VirtualGeometryPacketBuilder {
    fn push(&mut self, cluster: &VirtualGeometryClusterHeader) {
        self.triangle_count = self.triangle_count.saturating_add(cluster.triangle_count);
        self.cluster_ids.push(cluster.id);
    }
}

impl From<VirtualGeometryPacketBuilder> for VirtualGeometryDrawPacket {
    fn from(builder: VirtualGeometryPacketBuilder) -> Self {
        Self {
            draw_path: builder.draw_path,
            page_id: builder.page_id,
            material_signature: builder.material_signature,
            cluster_count: saturating_u32(builder.cluster_ids.len()),
            triangle_count: builder.triangle_count,
            cluster_ids: builder.cluster_ids,
        }
    }
}

fn page_with_checksum(
    id: VirtualGeometryPageId,
    byte_offset: u64,
    compressed_bytes: u64,
    first_cluster: u32,
    cluster_count: u32,
) -> VirtualGeometryPage {
    let checksum = VirtualGeometryHash(
        stable_hash_words(&[
            id.0,
            byte_offset,
            compressed_bytes,
            u64::from(first_cluster),
            u64::from(cluster_count),
        ])
        .0,
    );
    VirtualGeometryPage {
        id,
        byte_offset,
        compressed_bytes,
        first_cluster,
        cluster_count,
        checksum,
    }
}

fn compute_static_virtual_geometry_checksum(
    asset: &StaticVirtualGeometryAsset,
) -> VirtualGeometryHash {
    let mut state = StableHasher::new();
    state.write_u16(asset.header.schema_version);
    state.write_u64(asset.header.asset_id.0);
    state.write_u64(asset.header.kind as u64);
    state.write_u32(asset.header.root_node.0);
    for page in &asset.pages {
        state.write_u64(page.id.0);
        state.write_u64(page.byte_offset);
        state.write_u64(page.compressed_bytes);
        state.write_u32(page.first_cluster);
        state.write_u32(page.cluster_count);
        state.write_u64(page.checksum.0);
    }
    for range in &asset.material_ranges {
        state.write_u32(range.material_id.0);
        state.write_u64(range.material_signature.0);
        state.write_u32(range.first_cluster);
        state.write_u32(range.cluster_count);
    }
    for cluster in &asset.clusters {
        state.write_u64(cluster.id.0);
        state.write_u64(cluster.page_id.0);
        state.write_u32(cluster.material_range_index);
        state.write_u32(cluster.material_id.0);
        state.write_u64(cluster.material_signature.0);
        write_bounds(&mut state, cluster.local_bounds);
        write_bounds(&mut state, cluster.world_bounds);
        state.write_u32(cluster.triangle_range.first);
        state.write_u32(cluster.triangle_range.count);
        state.write_u32(cluster.index_range.first);
        state.write_u32(cluster.index_range.count);
        state.write_u64(cluster.parent_cluster_id.map_or(0, |id| id.0));
        state.write_u32(cluster.child_range.first_child);
        state.write_u32(cluster.child_range.child_count);
        state.write_u32(cluster.triangle_count);
        state.write_u32(cluster.bounds_radius.to_bits());
        state.write_u32(cluster.cluster_error.to_bits());
        state.write_u32(cluster.screen_error.to_bits());
        state.write_u32(cluster.screen_area.to_bits());
        state.write_u32(cluster.distance_meters.to_bits());
        state.write_u8(u8::from(cluster.hzb_occluder));
        state.write_u8(cluster.material_id_count);
        state.write_u64(cluster.cull_hint as u64);
    }
    for node in &asset.hierarchy {
        state.write_u32(node.id.0);
        state.write_u64(node.cluster_id.0);
        state.write_u32(node.parent.map_or(0, |id| id.0));
        state.write_u32(node.child_range.first_child);
        state.write_u32(node.child_range.child_count);
        for child in &node.children {
            state.write_u32(child.0);
        }
    }
    state.write_u64(asset.fallback_mesh.mesh_id);
    write_bounds(&mut state, asset.fallback_mesh.bounds);
    state.write_u32(asset.fallback_mesh.triangle_range.first);
    state.write_u32(asset.fallback_mesh.triangle_range.count);
    state.write_u32(asset.fallback_mesh.index_range.first);
    state.write_u32(asset.fallback_mesh.index_range.count);
    state.finish()
}

fn write_bounds(state: &mut StableHasher, bounds: VirtualGeometryBounds) {
    state.write_u32(bounds.center[0].to_bits());
    state.write_u32(bounds.center[1].to_bits());
    state.write_u32(bounds.center[2].to_bits());
    state.write_u32(bounds.radius.to_bits());
}

fn stable_pair_hash(left: u64, right: u64) -> u64 {
    stable_hash_words(&[left, right]).0
}

fn stable_hash_words(words: &[u64]) -> VirtualGeometryHash {
    let mut state = StableHasher::new();
    for word in words {
        state.write_u64(*word);
    }
    state.finish()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct StableHasher {
    state: u64,
}

impl StableHasher {
    const OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;

    const fn new() -> Self {
        Self {
            state: Self::OFFSET,
        }
    }

    fn write_u8(&mut self, value: u8) {
        self.state ^= u64::from(value);
        self.state = self.state.wrapping_mul(Self::PRIME);
    }

    fn write_u16(&mut self, value: u16) {
        for byte in value.to_le_bytes() {
            self.write_u8(byte);
        }
    }

    fn write_u32(&mut self, value: u32) {
        for byte in value.to_le_bytes() {
            self.write_u8(byte);
        }
    }

    fn write_u64(&mut self, value: u64) {
        for byte in value.to_le_bytes() {
            self.write_u8(byte);
        }
    }

    const fn finish(self) -> VirtualGeometryHash {
        VirtualGeometryHash(self.state)
    }
}

fn score_f32(value: f32, min: f32, max: f32, scale: u16) -> u16 {
    if !value.is_finite() || max <= min {
        return 0;
    }
    let normalized = ((value - min) / (max - min)).clamp(0.0, 1.0);
    (normalized * f32::from(scale)).round() as u16
}

fn score_inverse_distance(distance_meters: f32) -> u16 {
    if !distance_meters.is_finite() {
        return 0;
    }
    let clamped = distance_meters.clamp(0.0, 512.0);
    ((1.0 - clamped / 512.0) * 255.0).round() as u16
}

fn saturating_u32(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

trait SaturatingF32 {
    fn saturating_add_f32(self, rhs: f32) -> f32;
}

impl SaturatingF32 for f32 {
    fn saturating_add_f32(self, rhs: f32) -> f32 {
        let sum = self + rhs;
        if sum.is_finite() { sum } else { f32::MAX }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PageSchedulerConfig, PageUploadState};

    fn resident_scheduler(asset: &StaticVirtualGeometryAsset) -> PageScheduler {
        let mut scheduler = PageScheduler::new(PageSchedulerConfig {
            physical_slot_count: 128,
            max_upload_bytes_per_frame: 16 * 1024 * 1024,
            fault_storm_threshold: 8,
            ..PageSchedulerConfig::default()
        });
        scheduler.begin_frame(1);
        let requests = asset
            .pages
            .iter()
            .map(|page| PageRequest {
                logical_id: page.logical_page_id(asset.header.asset_id),
                estimated_upload_bytes: page.compressed_bytes,
                priority_inputs: PagePriorityInputs::editor_critical(),
            })
            .collect::<Vec<_>>();
        scheduler.submit_owner_requests(&PageOwnerFrameRequests::new(
            PageOwner::VirtualGeometry,
            requests,
        ));
        scheduler.schedule_uploads();
        for page in &asset.pages {
            scheduler.complete_upload(page.logical_page_id(asset.header.asset_id));
        }
        scheduler
    }

    #[test]
    fn asset_tool_emits_deterministic_static_funvg_format() {
        let config = StaticVirtualGeometryBakeConfig {
            triangle_count: 4_096,
            cluster_triangle_budget: 256,
            clusters_per_page: 2,
            ..Default::default()
        };
        let first = bake_static_virtual_geometry(&config).expect("bake should succeed");
        let second = bake_static_virtual_geometry(&config).expect("bake should succeed");
        let first_payload = serde_json::to_vec_pretty(&first).expect("asset serializes");
        let second_payload = serde_json::to_vec_pretty(&second).expect("asset serializes");

        assert_eq!(first_payload, second_payload);
        assert_eq!(
            first.header.schema_version,
            STATIC_VIRTUAL_GEOMETRY_SCHEMA_VERSION
        );
        assert!(first.checksum_valid());
        assert_eq!(first.header.page_count as usize, first.pages.len());
        assert_eq!(first.header.cluster_count as usize, first.clusters.len());
        assert_eq!(first.material_ranges.len(), 1);
        assert!(first.fallback_mesh.triangle_range.count > 0);
    }

    #[test]
    fn runtime_selects_visible_resident_clusters_into_compute_indirect_packets() {
        let asset = bake_static_virtual_geometry(&StaticVirtualGeometryBakeConfig {
            triangle_count: 2_048,
            cluster_triangle_budget: 256,
            clusters_per_page: 2,
            ..Default::default()
        })
        .expect("bake should succeed");
        let mut scheduler = resident_scheduler(&asset);

        let selection = select_static_virtual_geometry_frame(
            &asset,
            &mut scheduler,
            StaticVirtualGeometryViewContext::default(),
            StaticVirtualGeometryRuntimePolicy::default(),
        );

        assert_eq!(
            selection.asset_decision,
            VirtualGeometryAssetDecision::Candidate
        );
        assert!(!selection.draw_packets.is_empty());
        assert!(
            selection
                .draw_packets
                .iter()
                .all(|packet| packet.draw_path == VirtualGeometryDrawPath::ComputeIndirect)
        );
        assert_eq!(selection.metrics.geometry_page_faults, 0);
        assert!(selection.metrics.visible_clusters > 0);
    }

    #[test]
    fn missing_child_pages_request_scheduler_and_fallback_to_parent_cluster() {
        let asset = bake_static_virtual_geometry(&StaticVirtualGeometryBakeConfig {
            triangle_count: 2_048,
            cluster_triangle_budget: 256,
            clusters_per_page: 2,
            ..Default::default()
        })
        .expect("bake should succeed");
        let mut scheduler = PageScheduler::new(PageSchedulerConfig {
            physical_slot_count: 16,
            max_upload_bytes_per_frame: 4 * 1024 * 1024,
            fault_storm_threshold: 2,
            ..PageSchedulerConfig::default()
        });
        scheduler.begin_frame(10);
        let root = asset.pages[0];
        scheduler.submit_owner_requests(&PageOwnerFrameRequests::new(
            PageOwner::VirtualGeometry,
            vec![PageRequest {
                logical_id: root.logical_page_id(asset.header.asset_id),
                estimated_upload_bytes: root.compressed_bytes,
                priority_inputs: PagePriorityInputs::editor_critical(),
            }],
        ));
        scheduler.schedule_uploads();
        assert!(scheduler.complete_upload(root.logical_page_id(asset.header.asset_id)));

        let selection = select_static_virtual_geometry_frame(
            &asset,
            &mut scheduler,
            StaticVirtualGeometryViewContext {
                projected_bounds_radius_pixels: 1024.0,
                ..Default::default()
            },
            StaticVirtualGeometryRuntimePolicy {
                affordable_child_page_bytes: 64 * 1024 * 1024,
                ..Default::default()
            },
        );

        assert!(!selection.requested_pages.is_empty());
        assert!(!selection.fallback_clusters.is_empty());
        assert!(selection.metrics.geometry_page_faults > 0);
        assert!(selection.metrics.bytes_streamed > 0);
        assert_eq!(
            scheduler
                .page(selection.requested_pages[0].logical_id)
                .map(|record| record.upload_state),
            Some(PageUploadState::Queued)
        );
    }

    #[test]
    fn missing_root_page_uses_fallback_mesh_without_requiring_mesh_shaders() {
        let asset = bake_static_virtual_geometry(&StaticVirtualGeometryBakeConfig::default())
            .expect("bake should succeed");
        let mut scheduler = PageScheduler::default();
        scheduler.begin_frame(42);

        let selection = select_static_virtual_geometry_frame(
            &asset,
            &mut scheduler,
            StaticVirtualGeometryViewContext {
                compute_indirect_enabled: false,
                mesh_shader_supported: false,
                ..Default::default()
            },
            StaticVirtualGeometryRuntimePolicy {
                mesh_shader_mode: VirtualGeometryMeshShaderMode::Optional,
                ..Default::default()
            },
        );

        assert!(selection.fallback_mesh_used);
        assert_eq!(selection.draw_packets.len(), 1);
        assert_eq!(
            selection.draw_packets[0].draw_path,
            VirtualGeometryDrawPath::FallbackMesh
        );
        assert_eq!(selection.metrics.missing_fallback_clusters, 1);
    }

    #[test]
    fn hzb_and_frustum_culling_compact_visible_clusters() {
        let mut asset = bake_static_virtual_geometry(&StaticVirtualGeometryBakeConfig {
            triangle_count: 1_024,
            cluster_triangle_budget: 256,
            clusters_per_page: 1,
            ..Default::default()
        })
        .expect("bake should succeed");
        asset.clusters[2].cull_hint = VirtualGeometryClusterCullHint::HzbOccluded;
        asset.header.checksum = compute_static_virtual_geometry_checksum(&asset);
        let mut scheduler = resident_scheduler(&asset);

        let selection = select_static_virtual_geometry_frame(
            &asset,
            &mut scheduler,
            StaticVirtualGeometryViewContext::default(),
            StaticVirtualGeometryRuntimePolicy::default(),
        );

        assert!(selection.metrics.culled_clusters > 0);
        assert!(selection.metrics.visible_clusters < selection.selected_clusters.len() as u32);
        assert!(
            selection
                .draw_packets
                .iter()
                .flat_map(|packet| packet.cluster_ids.iter())
                .all(|cluster_id| *cluster_id != asset.clusters[2].id)
        );
    }

    #[test]
    fn mesh_shader_path_is_optional_fast_path_only() {
        let asset = bake_static_virtual_geometry(&StaticVirtualGeometryBakeConfig {
            triangle_count: 1_024,
            cluster_triangle_budget: 256,
            clusters_per_page: 2,
            ..Default::default()
        })
        .expect("bake should succeed");
        let mut scheduler = resident_scheduler(&asset);

        let supported = select_static_virtual_geometry_frame(
            &asset,
            &mut scheduler,
            StaticVirtualGeometryViewContext {
                mesh_shader_supported: true,
                ..Default::default()
            },
            StaticVirtualGeometryRuntimePolicy {
                mesh_shader_mode: VirtualGeometryMeshShaderMode::Optional,
                ..Default::default()
            },
        );
        assert!(
            supported
                .draw_packets
                .iter()
                .all(|packet| packet.draw_path == VirtualGeometryDrawPath::OptionalMeshShader)
        );

        let mut scheduler = resident_scheduler(&asset);
        let unsupported = select_static_virtual_geometry_frame(
            &asset,
            &mut scheduler,
            StaticVirtualGeometryViewContext {
                mesh_shader_supported: false,
                ..Default::default()
            },
            StaticVirtualGeometryRuntimePolicy {
                mesh_shader_mode: VirtualGeometryMeshShaderMode::Optional,
                ..Default::default()
            },
        );
        assert!(
            unsupported
                .draw_packets
                .iter()
                .all(|packet| packet.draw_path == VirtualGeometryDrawPath::ComputeIndirect)
        );
    }

    #[test]
    fn large_static_scene_benchmark_artifact_records_page_faults() {
        let asset = bake_static_virtual_geometry(&StaticVirtualGeometryBakeConfig {
            asset_id: 99,
            triangle_count: 262_144,
            index_count: 786_432,
            cluster_triangle_budget: 512,
            clusters_per_page: 4,
            child_page_compressed_bytes: 192 * 1024,
            ..Default::default()
        })
        .expect("bake should succeed");
        let mut scheduler = PageScheduler::new(PageSchedulerConfig {
            physical_slot_count: 64,
            max_upload_bytes_per_frame: 8 * 1024 * 1024,
            fault_storm_threshold: 4,
            ..PageSchedulerConfig::default()
        });
        scheduler.begin_frame(7);
        let root = asset.pages[0];
        scheduler.submit_owner_requests(&PageOwnerFrameRequests::new(
            PageOwner::VirtualGeometry,
            vec![PageRequest {
                logical_id: root.logical_page_id(asset.header.asset_id),
                estimated_upload_bytes: root.compressed_bytes,
                priority_inputs: PagePriorityInputs::editor_critical(),
            }],
        ));
        scheduler.schedule_uploads();
        scheduler.complete_upload(root.logical_page_id(asset.header.asset_id));

        let selection = select_static_virtual_geometry_frame(
            &asset,
            &mut scheduler,
            StaticVirtualGeometryViewContext {
                projected_bounds_radius_pixels: 2048.0,
                gameplay_salience: 220,
                editor_focus: 160,
                ..Default::default()
            },
            StaticVirtualGeometryRuntimePolicy {
                affordable_child_page_bytes: 64 * 1024 * 1024,
                ..Default::default()
            },
        );
        let upload_report = scheduler.schedule_uploads();
        let artifact = selection.debug_artifact();
        if let Ok(path) = std::env::var(STATIC_VIRTUAL_GEOMETRY_BENCHMARK_ARTIFACT_ENV) {
            if let Some(parent) = std::path::Path::new(&path).parent()
                && !parent.as_os_str().is_empty()
            {
                std::fs::create_dir_all(parent).expect("artifact parent should be created");
            }
            std::fs::write(path, &artifact.content).expect("artifact should be written");
        }

        assert!(selection.metrics.input_clusters > 500);
        assert!(selection.metrics.geometry_page_faults > 0);
        assert!(selection.metrics.bytes_streamed > 0);
        assert!(upload_report.scheduled_pages > 0);
        assert!(artifact.content.contains("geometry_page_faults="));
        assert!(artifact.content.contains("bytes_streamed="));
    }
}
