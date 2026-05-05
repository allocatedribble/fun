use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::Resource;

use crate::{
    core::FunRenderPhaseKind,
    indirect_draw::{
        FunDepthPrepassMode, FunDrawBucketKey, FunDrawBudgetLane, FunMaterialSignature,
        FunMeshletFormat, FunSkeletalMode, FunTextureTableCompatibility, FunTransparencyMode,
    },
    material_pipeline::DENSE_MESHLET_CLUSTER_PIPELINE,
    signature::FunRenderPath,
};

pub const FUN_VG_PROTOTYPE_NAME: &str = "FunVG";
pub const FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION: u16 = 2;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct VirtualGeometryAssetId(pub u64);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct VirtualGeometryPageId(pub u64);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct VirtualGeometryClusterId(pub u64);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct VirtualGeometryHierarchyNodeId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum VirtualGeometryRuntimeStage {
    LoadAssetMetadata,
    SelectCandidateRoots,
    TraverseScreenSpaceError,
    RequestMissingPages,
    UseFallbackParentClusters,
    CullSelectedClusters,
    CompactVisibleClusters,
    EmitIndirectDrawPackets,
    SubmitIndirectRasterBuckets,
}

impl VirtualGeometryRuntimeStage {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LoadAssetMetadata => "load_asset_metadata",
            Self::SelectCandidateRoots => "select_candidate_roots",
            Self::TraverseScreenSpaceError => "traverse_screen_space_error",
            Self::RequestMissingPages => "request_missing_pages",
            Self::UseFallbackParentClusters => "use_fallback_parent_clusters",
            Self::CullSelectedClusters => "cull_selected_clusters",
            Self::CompactVisibleClusters => "compact_visible_clusters",
            Self::EmitIndirectDrawPackets => "emit_indirect_draw_packets",
            Self::SubmitIndirectRasterBuckets => "submit_indirect_raster_buckets",
        }
    }
}

pub const FUN_VG_RUNTIME_PIPELINE: &[VirtualGeometryRuntimeStage] = &[
    VirtualGeometryRuntimeStage::LoadAssetMetadata,
    VirtualGeometryRuntimeStage::SelectCandidateRoots,
    VirtualGeometryRuntimeStage::TraverseScreenSpaceError,
    VirtualGeometryRuntimeStage::RequestMissingPages,
    VirtualGeometryRuntimeStage::UseFallbackParentClusters,
    VirtualGeometryRuntimeStage::CullSelectedClusters,
    VirtualGeometryRuntimeStage::CompactVisibleClusters,
    VirtualGeometryRuntimeStage::EmitIndirectDrawPackets,
    VirtualGeometryRuntimeStage::SubmitIndirectRasterBuckets,
];

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualGeometryAssetKind {
    #[default]
    StaticOpaqueDense,
    StaticDestroyedAlternateClusterSet,
    Foliage,
    AlphaHeavyAggregate,
    Transparent,
    CefUi,
    ArbitraryRuntimeFracture,
}

impl VirtualGeometryAssetKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StaticOpaqueDense => "static_opaque_dense",
            Self::StaticDestroyedAlternateClusterSet => "static_destroyed_alternate_cluster_set",
            Self::Foliage => "foliage",
            Self::AlphaHeavyAggregate => "alpha_heavy_aggregate",
            Self::Transparent => "transparent",
            Self::CefUi => "cef_ui",
            Self::ArbitraryRuntimeFracture => "arbitrary_runtime_fracture",
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VirtualGeometryAssetDecision {
    #[default]
    Candidate,
    NotBakedOffline,
    NonStaticOpaqueUnsupported,
    PoorAggregateGeometry,
    TransparentUnsupported,
    CefUiUnsupported,
    RuntimeFractureUnsupported,
}

impl VirtualGeometryAssetDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Candidate => "candidate",
            Self::NotBakedOffline => "not_baked_offline",
            Self::NonStaticOpaqueUnsupported => "non_static_opaque_unsupported",
            Self::PoorAggregateGeometry => "poor_aggregate_geometry",
            Self::TransparentUnsupported => "transparent_unsupported",
            Self::CefUiUnsupported => "cef_ui_unsupported",
            Self::RuntimeFractureUnsupported => "runtime_fracture_unsupported",
        }
    }

    pub const fn accepted(self) -> bool {
        matches!(self, Self::Candidate)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualGeometryClusterCullHint {
    #[default]
    Visible,
    FrustumCulled,
    Occluded,
}

impl VirtualGeometryClusterCullHint {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "visible",
            Self::FrustumCulled => "frustum_culled",
            Self::Occluded => "occluded",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryBounds {
    pub center: [f32; 3],
    pub radius: f32,
}

impl VirtualGeometryBounds {
    pub const fn new(center: [f32; 3], radius: f32) -> Self {
        Self { center, radius }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryRange {
    pub first: u32,
    pub count: u32,
}

impl VirtualGeometryRange {
    pub const EMPTY: Self = Self { first: 0, count: 0 };

    pub const fn new(first: u32, count: u32) -> Self {
        Self { first, count }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryNormalCone {
    pub axis: [f32; 3],
    pub cutoff: f32,
}

impl VirtualGeometryNormalCone {
    pub const fn new(axis: [f32; 3], cutoff: f32) -> Self {
        Self { axis, cutoff }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryChildRange {
    pub first_child: u32,
    pub child_count: u32,
}

impl VirtualGeometryChildRange {
    pub const EMPTY: Self = Self {
        first_child: 0,
        child_count: 0,
    };

    pub const fn new(first_child: u32, child_count: u32) -> Self {
        Self {
            first_child,
            child_count,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VirtualGeometryDrawPath {
    IndirectRaster,
}

impl VirtualGeometryDrawPath {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::IndirectRaster => "indirect_raster",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryLitePolicy {
    pub static_opaque_only: bool,
    pub baked_offline: bool,
    pub gpu_culling_required: bool,
    pub indirect_raster_first: bool,
    pub software_visibility_buffer_enabled: bool,
    pub mesh_shaders_enabled: bool,
    pub bindless_mega_system_enabled: bool,
}

impl VirtualGeometryLitePolicy {
    pub const DEFAULT: Self = Self {
        static_opaque_only: true,
        baked_offline: true,
        gpu_culling_required: true,
        indirect_raster_first: true,
        software_visibility_buffer_enabled: false,
        mesh_shaders_enabled: false,
        bindless_mega_system_enabled: false,
    };
}

impl Default for VirtualGeometryLitePolicy {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryStaticCell {
    pub cell_id: u64,
    pub asset_id: VirtualGeometryAssetId,
    pub world_bounds: VirtualGeometryBounds,
    pub visible: bool,
}

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryAsset {
    pub schema_version: u16,
    pub asset_id: VirtualGeometryAssetId,
    pub root_node: VirtualGeometryHierarchyNodeId,
    pub kind: VirtualGeometryAssetKind,
    pub lite_policy: VirtualGeometryLitePolicy,
    pub pages: Vec<VirtualGeometryPage>,
    pub clusters: Vec<VirtualGeometryCluster>,
    pub hierarchy_nodes: Vec<VirtualGeometryHierarchyNode>,
    pub alternate_cluster_sets: u8,
    pub material_id_count: u16,
    pub triangle_count: u32,
}

impl VirtualGeometryAsset {
    pub fn page(&self, page_id: VirtualGeometryPageId) -> Option<&VirtualGeometryPage> {
        self.pages.iter().find(|page| page.id == page_id)
    }

    pub fn cluster(&self, cluster_id: VirtualGeometryClusterId) -> Option<&VirtualGeometryCluster> {
        self.clusters
            .iter()
            .find(|cluster| cluster.id == cluster_id)
    }

    pub fn node(
        &self,
        node_id: VirtualGeometryHierarchyNodeId,
    ) -> Option<&VirtualGeometryHierarchyNode> {
        self.hierarchy_nodes.iter().find(|node| node.id == node_id)
    }
}

pub const fn evaluate_virtual_geometry_asset(
    asset: &VirtualGeometryAsset,
) -> VirtualGeometryAssetDecision {
    if !asset.lite_policy.baked_offline {
        return VirtualGeometryAssetDecision::NotBakedOffline;
    }
    if !asset.lite_policy.static_opaque_only {
        return VirtualGeometryAssetDecision::NonStaticOpaqueUnsupported;
    }
    match asset.kind {
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
        VirtualGeometryAssetKind::CefUi => VirtualGeometryAssetDecision::CefUiUnsupported,
        VirtualGeometryAssetKind::ArbitraryRuntimeFracture => {
            VirtualGeometryAssetDecision::RuntimeFractureUnsupported
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryPage {
    pub id: VirtualGeometryPageId,
    pub compressed_bytes: u64,
    pub first_cluster: u32,
    pub cluster_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryCluster {
    pub id: VirtualGeometryClusterId,
    pub page_id: VirtualGeometryPageId,
    pub material_id: u32,
    pub material_signature: FunMaterialSignature,
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
    pub occluder: bool,
    pub material_id_count: u8,
    pub cull_hint: VirtualGeometryClusterCullHint,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryHierarchyNode {
    pub id: VirtualGeometryHierarchyNodeId,
    pub cluster_id: VirtualGeometryClusterId,
    pub parent: Option<VirtualGeometryHierarchyNodeId>,
    pub child_range: VirtualGeometryChildRange,
    pub children: Vec<VirtualGeometryHierarchyNodeId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualGeometryPageBudget {
    pub max_page_requests_per_frame: u32,
    pub max_upload_bytes_per_frame: u64,
}

impl VirtualGeometryPageBudget {
    pub const fn for_lane(lane: FunDrawBudgetLane) -> Self {
        match lane {
            FunDrawBudgetLane::PresentationFloor => Self {
                max_page_requests_per_frame: 2,
                max_upload_bytes_per_frame: 512 * 1024,
            },
            FunDrawBudgetLane::FullRuntime => Self {
                max_page_requests_per_frame: 4,
                max_upload_bytes_per_frame: 2 * 1024 * 1024,
            },
            FunDrawBudgetLane::StreamingSpike => Self {
                max_page_requests_per_frame: 8,
                max_upload_bytes_per_frame: 6 * 1024 * 1024,
            },
            FunDrawBudgetLane::SolariCloudHeavy => Self {
                max_page_requests_per_frame: 3,
                max_upload_bytes_per_frame: 1024 * 1024,
            },
            FunDrawBudgetLane::Competitive5v5 => Self {
                max_page_requests_per_frame: 4,
                max_upload_bytes_per_frame: 2 * 1024 * 1024,
            },
            FunDrawBudgetLane::LargeBattle => Self {
                max_page_requests_per_frame: 10,
                max_upload_bytes_per_frame: 8 * 1024 * 1024,
            },
        }
    }
}

impl Default for VirtualGeometryPageBudget {
    fn default() -> Self {
        Self::for_lane(FunDrawBudgetLane::FullRuntime)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VirtualGeometryPageBudgetUsage {
    pub requested_pages: u32,
    pub requested_bytes: u64,
    pub deferred_pages: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualGeometryPageRequestReason {
    VisibleCluster,
    SplitScreenError,
    NearCamera,
    Occluder,
}

impl VirtualGeometryPageRequestReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::VisibleCluster => "visible_cluster",
            Self::SplitScreenError => "split_screen_error",
            Self::NearCamera => "near_camera",
            Self::Occluder => "occluder",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualGeometryPageRequest {
    pub page_id: VirtualGeometryPageId,
    pub compressed_bytes: u64,
    pub priority_score: i32,
    pub reason: VirtualGeometryPageRequestReason,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualGeometryPageRequestDecision {
    AlreadyResident,
    AlreadyRequested,
    Requested,
    DeferredBudget,
}

impl VirtualGeometryPageRequestDecision {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AlreadyResident => "already_resident",
            Self::AlreadyRequested => "already_requested",
            Self::Requested => "requested",
            Self::DeferredBudget => "deferred_budget",
        }
    }
}

#[derive(Debug, Clone, Default, Resource)]
pub struct VirtualGeometryResidency {
    pub resident_pages: BTreeSet<VirtualGeometryPageId>,
    pub requested_pages: BTreeMap<VirtualGeometryPageId, VirtualGeometryPageRequest>,
}

impl VirtualGeometryResidency {
    pub fn with_resident_pages<const N: usize>(pages: [VirtualGeometryPageId; N]) -> Self {
        Self {
            resident_pages: pages.into_iter().collect(),
            requested_pages: BTreeMap::new(),
        }
    }

    pub fn is_page_resident(&self, page_id: VirtualGeometryPageId) -> bool {
        self.resident_pages.contains(&page_id)
    }

    pub fn request_page(
        &mut self,
        request: VirtualGeometryPageRequest,
        budget: VirtualGeometryPageBudget,
        usage: &mut VirtualGeometryPageBudgetUsage,
    ) -> VirtualGeometryPageRequestDecision {
        if self.resident_pages.contains(&request.page_id) {
            return VirtualGeometryPageRequestDecision::AlreadyResident;
        }
        if let Some(existing) = self.requested_pages.get_mut(&request.page_id) {
            if request.priority_score > existing.priority_score {
                *existing = request;
            }
            return VirtualGeometryPageRequestDecision::AlreadyRequested;
        }
        if usage.requested_pages >= budget.max_page_requests_per_frame
            || usage
                .requested_bytes
                .saturating_add(request.compressed_bytes)
                > budget.max_upload_bytes_per_frame
        {
            usage.deferred_pages = usage.deferred_pages.saturating_add(1);
            return VirtualGeometryPageRequestDecision::DeferredBudget;
        }

        usage.requested_pages = usage.requested_pages.saturating_add(1);
        usage.requested_bytes = usage
            .requested_bytes
            .saturating_add(request.compressed_bytes);
        self.requested_pages.insert(request.page_id, request);
        VirtualGeometryPageRequestDecision::Requested
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualGeometrySelectionPolicy {
    pub screen_space_error_threshold_pixels: f32,
    pub affordable_child_page_bytes: u64,
    pub max_material_ids_per_cluster: u8,
    pub conservative_culling: bool,
}

impl Default for VirtualGeometrySelectionPolicy {
    fn default() -> Self {
        Self {
            screen_space_error_threshold_pixels: 1.5,
            affordable_child_page_bytes: 2 * 1024 * 1024,
            max_material_ids_per_cluster: 4,
            conservative_culling: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VirtualGeometryViewContext {
    pub frame_index: u64,
    pub lane: FunDrawBudgetLane,
    pub object_visible: bool,
    pub projected_bounds_radius_pixels: f32,
    pub camera_distance_meters: f32,
    pub gpu_culling_enabled: bool,
    pub page_budget: VirtualGeometryPageBudget,
}

impl Default for VirtualGeometryViewContext {
    fn default() -> Self {
        Self {
            frame_index: 0,
            lane: FunDrawBudgetLane::FullRuntime,
            object_visible: true,
            projected_bounds_radius_pixels: 512.0,
            camera_distance_meters: 32.0,
            gpu_culling_enabled: true,
            page_budget: VirtualGeometryPageBudget::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VirtualGeometrySelectionReason {
    ScreenErrorSatisfied,
    Leaf,
    MissingChildPageFallback,
    MaterialSplitAvoided,
    MissingRootPage,
}

impl VirtualGeometrySelectionReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ScreenErrorSatisfied => "screen_error_satisfied",
            Self::Leaf => "leaf",
            Self::MissingChildPageFallback => "missing_child_page_fallback",
            Self::MaterialSplitAvoided => "material_split_avoided",
            Self::MissingRootPage => "missing_root_page",
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VirtualGeometryDrawPacket {
    pub draw_path: VirtualGeometryDrawPath,
    pub page_id: VirtualGeometryPageId,
    pub material_signature: FunMaterialSignature,
    pub bucket_key: FunDrawBucketKey,
    pub cluster_count: u32,
    pub triangle_count: u32,
    pub cluster_ids: Vec<VirtualGeometryClusterId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VirtualGeometryViewSelection {
    pub schema_version: u16,
    pub prototype_name: &'static str,
    pub asset_id: VirtualGeometryAssetId,
    pub frame_index: u64,
    pub asset_decision: VirtualGeometryAssetDecision,
    pub selected_clusters: Vec<VirtualGeometrySelectedCluster>,
    pub visible_clusters: Vec<VirtualGeometrySelectedCluster>,
    pub requested_pages: Vec<VirtualGeometryPageRequest>,
    pub missing_pages: Vec<VirtualGeometryPageId>,
    pub fallback_clusters: Vec<VirtualGeometryClusterId>,
    pub culled_clusters: u32,
    pub compacted_cluster_count: u32,
    pub draw_packets: Vec<VirtualGeometryDrawPacket>,
    pub upload_bytes_requested: u64,
    pub deferred_page_requests: u32,
    pub material_split_avoided: u32,
}

impl VirtualGeometryViewSelection {
    pub fn empty(asset: &VirtualGeometryAsset, context: VirtualGeometryViewContext) -> Self {
        Self {
            schema_version: FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION,
            prototype_name: FUN_VG_PROTOTYPE_NAME,
            asset_id: asset.asset_id,
            frame_index: context.frame_index,
            asset_decision: evaluate_virtual_geometry_asset(asset),
            selected_clusters: Vec::new(),
            visible_clusters: Vec::new(),
            requested_pages: Vec::new(),
            missing_pages: Vec::new(),
            fallback_clusters: Vec::new(),
            culled_clusters: 0,
            compacted_cluster_count: 0,
            draw_packets: Vec::new(),
            upload_bytes_requested: 0,
            deferred_page_requests: 0,
            material_split_avoided: 0,
        }
    }
}

pub fn virtual_geometry_screen_space_error(
    cluster: VirtualGeometryCluster,
    context: VirtualGeometryViewContext,
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

pub fn select_virtual_geometry_for_static_cell(
    cell: VirtualGeometryStaticCell,
    asset: &VirtualGeometryAsset,
    residency: &mut VirtualGeometryResidency,
    mut context: VirtualGeometryViewContext,
    policy: VirtualGeometrySelectionPolicy,
) -> VirtualGeometryViewSelection {
    if cell.asset_id != asset.asset_id {
        context.object_visible = false;
    } else {
        context.object_visible &= cell.visible;
    }
    select_virtual_geometry_view(asset, residency, context, policy)
}

pub fn select_virtual_geometry_view(
    asset: &VirtualGeometryAsset,
    residency: &mut VirtualGeometryResidency,
    context: VirtualGeometryViewContext,
    policy: VirtualGeometrySelectionPolicy,
) -> VirtualGeometryViewSelection {
    let mut selection = VirtualGeometryViewSelection::empty(asset, context);
    if !context.object_visible {
        return selection;
    }

    let asset_decision = evaluate_virtual_geometry_asset(asset);
    selection.asset_decision = asset_decision;
    if !asset_decision.accepted() {
        return selection;
    }

    let mut budget_usage = VirtualGeometryPageBudgetUsage::default();
    let mut stack = vec![asset.root_node];
    while let Some(node_id) = stack.pop() {
        let Some(node) = asset.node(node_id) else {
            continue;
        };
        let Some(cluster) = asset.cluster(node.cluster_id).copied() else {
            continue;
        };

        if !residency.is_page_resident(cluster.page_id) {
            request_cluster_page(
                asset,
                residency,
                context,
                cluster,
                VirtualGeometryPageRequestReason::VisibleCluster,
                &mut budget_usage,
                &mut selection,
            );
            selection.missing_pages.push(cluster.page_id);
            selection
                .selected_clusters
                .push(VirtualGeometrySelectedCluster {
                    cluster_id: cluster.id,
                    page_id: cluster.page_id,
                    screen_space_error_pixels: virtual_geometry_screen_space_error(
                        cluster, context,
                    ),
                    selection_reason: VirtualGeometrySelectionReason::MissingRootPage,
                    fallback_parent: true,
                });
            continue;
        }

        let screen_error = virtual_geometry_screen_space_error(cluster, context);
        let should_split =
            screen_error > policy.screen_space_error_threshold_pixels && !node.children.is_empty();
        if should_split {
            if child_material_count_too_high(asset, node, policy.max_material_ids_per_cluster) {
                selection.material_split_avoided =
                    selection.material_split_avoided.saturating_add(1);
                push_selected_cluster(
                    &mut selection,
                    cluster,
                    screen_error,
                    VirtualGeometrySelectionReason::MaterialSplitAvoided,
                    false,
                );
                continue;
            }

            let missing_child_pages = missing_child_pages(asset, residency, node);
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
                    if let Some(page) = asset.page(*page_id) {
                        let request = VirtualGeometryPageRequest {
                            page_id: *page_id,
                            compressed_bytes: page.compressed_bytes,
                            priority_score: page_priority_score(
                                context,
                                cluster,
                                screen_error,
                                VirtualGeometryPageRequestReason::SplitScreenError,
                            ),
                            reason: VirtualGeometryPageRequestReason::SplitScreenError,
                        };
                        record_page_request(
                            residency,
                            context.page_budget,
                            request,
                            &mut budget_usage,
                            &mut selection,
                        );
                    }
                }
            } else {
                selection.deferred_page_requests = selection
                    .deferred_page_requests
                    .saturating_add(saturating_u32(missing_child_pages.len()));
            }

            push_selected_cluster(
                &mut selection,
                cluster,
                screen_error,
                VirtualGeometrySelectionReason::MissingChildPageFallback,
                true,
            );
            selection.fallback_clusters.push(cluster.id);
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

    selection.visible_clusters = cull_selected_clusters(asset, &selection, context, policy);
    selection.culled_clusters = saturating_u32(
        selection
            .selected_clusters
            .len()
            .saturating_sub(selection.visible_clusters.len()),
    );
    selection.compacted_cluster_count = saturating_u32(selection.visible_clusters.len());
    selection.draw_packets =
        build_virtual_geometry_draw_packets(asset, &selection.visible_clusters);
    selection
}

fn push_selected_cluster(
    selection: &mut VirtualGeometryViewSelection,
    cluster: VirtualGeometryCluster,
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

fn request_cluster_page(
    asset: &VirtualGeometryAsset,
    residency: &mut VirtualGeometryResidency,
    context: VirtualGeometryViewContext,
    cluster: VirtualGeometryCluster,
    reason: VirtualGeometryPageRequestReason,
    budget_usage: &mut VirtualGeometryPageBudgetUsage,
    selection: &mut VirtualGeometryViewSelection,
) {
    let Some(page) = asset.page(cluster.page_id) else {
        return;
    };
    let request = VirtualGeometryPageRequest {
        page_id: page.id,
        compressed_bytes: page.compressed_bytes,
        priority_score: page_priority_score(
            context,
            cluster,
            virtual_geometry_screen_space_error(cluster, context),
            reason,
        ),
        reason,
    };
    record_page_request(
        residency,
        context.page_budget,
        request,
        budget_usage,
        selection,
    );
}

fn record_page_request(
    residency: &mut VirtualGeometryResidency,
    budget: VirtualGeometryPageBudget,
    request: VirtualGeometryPageRequest,
    budget_usage: &mut VirtualGeometryPageBudgetUsage,
    selection: &mut VirtualGeometryViewSelection,
) {
    match residency.request_page(request, budget, budget_usage) {
        VirtualGeometryPageRequestDecision::Requested => {
            selection.upload_bytes_requested = selection
                .upload_bytes_requested
                .saturating_add(request.compressed_bytes);
            selection.requested_pages.push(request);
        }
        VirtualGeometryPageRequestDecision::DeferredBudget => {
            selection.deferred_page_requests = selection.deferred_page_requests.saturating_add(1);
        }
        VirtualGeometryPageRequestDecision::AlreadyResident
        | VirtualGeometryPageRequestDecision::AlreadyRequested => {}
    }
}

fn page_priority_score(
    context: VirtualGeometryViewContext,
    cluster: VirtualGeometryCluster,
    screen_error: f32,
    reason: VirtualGeometryPageRequestReason,
) -> i32 {
    let visible = 4_000;
    let screen_error_score = score_f32(screen_error, 0.0, 256.0, 4_000);
    let near_score = score_inverse_distance(
        cluster
            .distance_meters
            .min(context.camera_distance_meters)
            .max(0.0),
    );
    let occluder_score = i32::from(cluster.occluder) * 1_500;
    let reason_score = match reason {
        VirtualGeometryPageRequestReason::VisibleCluster => 1_000,
        VirtualGeometryPageRequestReason::SplitScreenError => 2_000,
        VirtualGeometryPageRequestReason::NearCamera => 1_500,
        VirtualGeometryPageRequestReason::Occluder => 1_800,
    };
    visible + screen_error_score + near_score + occluder_score + reason_score
}

fn child_material_count_too_high(
    asset: &VirtualGeometryAsset,
    node: &VirtualGeometryHierarchyNode,
    max_material_ids_per_cluster: u8,
) -> bool {
    node.children.iter().any(|child_id| {
        asset
            .node(*child_id)
            .and_then(|child| asset.cluster(child.cluster_id))
            .is_some_and(|cluster| cluster.material_id_count > max_material_ids_per_cluster)
    })
}

fn missing_child_pages(
    asset: &VirtualGeometryAsset,
    residency: &VirtualGeometryResidency,
    node: &VirtualGeometryHierarchyNode,
) -> Vec<VirtualGeometryPageId> {
    let mut missing = BTreeSet::new();
    for child_id in &node.children {
        if let Some(child_cluster) = asset
            .node(*child_id)
            .and_then(|child| asset.cluster(child.cluster_id))
            && !residency.is_page_resident(child_cluster.page_id)
        {
            missing.insert(child_cluster.page_id);
        }
    }
    missing.into_iter().collect()
}

fn cull_selected_clusters(
    asset: &VirtualGeometryAsset,
    selection: &VirtualGeometryViewSelection,
    context: VirtualGeometryViewContext,
    policy: VirtualGeometrySelectionPolicy,
) -> Vec<VirtualGeometrySelectedCluster> {
    let mut visible = Vec::new();
    for selected in &selection.selected_clusters {
        let Some(cluster) = asset.cluster(selected.cluster_id) else {
            continue;
        };
        let cull_rejects = context.gpu_culling_enabled
            && matches!(
                cluster.cull_hint,
                VirtualGeometryClusterCullHint::FrustumCulled
                    | VirtualGeometryClusterCullHint::Occluded
            );
        if cull_rejects && policy.conservative_culling && selected.fallback_parent {
            visible.push(*selected);
            continue;
        }
        if !cull_rejects {
            visible.push(*selected);
        }
    }
    visible
}

pub fn build_virtual_geometry_draw_packets(
    asset: &VirtualGeometryAsset,
    visible_clusters: &[VirtualGeometrySelectedCluster],
) -> Vec<VirtualGeometryDrawPacket> {
    let mut builders = BTreeMap::<
        (VirtualGeometryPageId, FunMaterialSignature),
        VirtualGeometryPacketBuilder,
    >::new();
    for selected in visible_clusters {
        let Some(cluster) = asset.cluster(selected.cluster_id) else {
            continue;
        };
        let key = (cluster.page_id, cluster.material_signature);
        let builder = builders.entry(key).or_insert(VirtualGeometryPacketBuilder {
            page_id: cluster.page_id,
            material_signature: cluster.material_signature,
            triangle_count: 0,
            cluster_ids: Vec::new(),
        });
        builder.triangle_count = builder
            .triangle_count
            .saturating_add(cluster.triangle_count);
        builder.cluster_ids.push(cluster.id);
    }

    builders
        .into_values()
        .map(|builder| VirtualGeometryDrawPacket {
            draw_path: VirtualGeometryDrawPath::IndirectRaster,
            page_id: builder.page_id,
            material_signature: builder.material_signature,
            bucket_key: virtual_geometry_draw_bucket_key(builder.material_signature),
            cluster_count: saturating_u32(builder.cluster_ids.len()),
            triangle_count: builder.triangle_count,
            cluster_ids: builder.cluster_ids,
        })
        .collect()
}

struct VirtualGeometryPacketBuilder {
    page_id: VirtualGeometryPageId,
    material_signature: FunMaterialSignature,
    triangle_count: u32,
    cluster_ids: Vec<VirtualGeometryClusterId>,
}

pub const fn virtual_geometry_draw_bucket_key(
    material_signature: FunMaterialSignature,
) -> FunDrawBucketKey {
    FunDrawBucketKey {
        phase: FunRenderPhaseKind::MainOpaque,
        material_signature,
        shader_pipeline_signature: DENSE_MESHLET_CLUSTER_PIPELINE,
        meshlet_format: FunMeshletFormat::ClusterMesh,
        texture_table: FunTextureTableCompatibility::BindlessCompatible,
        depth_prepass: FunDepthPrepassMode::DepthOnly,
        transparency: FunTransparencyMode::Opaque,
        skeletal: FunSkeletalMode::Static,
        render_path: FunRenderPath::GpuCulledIndirect,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum VirtualGeometryVisibilityBufferExtensionStatus {
    #[default]
    DeferredSeparateBranch,
    CandidateAfterHardwarePathFails,
}

impl VirtualGeometryVisibilityBufferExtensionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DeferredSeparateBranch => "deferred_separate_branch",
            Self::CandidateAfterHardwarePathFails => "candidate_after_hardware_path_fails",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualGeometryVisibilityBufferExtensionInput {
    pub hardware_raster_reduces_draw_calls_enough: bool,
    pub material_resolve_stable: bool,
    pub gpu_time_wins_dense_scenes: bool,
    pub compute_culling_and_indirect_ready: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualGeometryVisibilityBufferExtensionPlan {
    pub status: VirtualGeometryVisibilityBufferExtensionStatus,
    pub reason: &'static str,
}

pub const fn virtual_geometry_visibility_buffer_extension_plan(
    input: VirtualGeometryVisibilityBufferExtensionInput,
) -> VirtualGeometryVisibilityBufferExtensionPlan {
    if !input.compute_culling_and_indirect_ready {
        return VirtualGeometryVisibilityBufferExtensionPlan {
            status: VirtualGeometryVisibilityBufferExtensionStatus::DeferredSeparateBranch,
            reason: "compute_culling_and_indirect_draw_path_first",
        };
    }
    if input.hardware_raster_reduces_draw_calls_enough
        || !input.material_resolve_stable
        || !input.gpu_time_wins_dense_scenes
    {
        return VirtualGeometryVisibilityBufferExtensionPlan {
            status: VirtualGeometryVisibilityBufferExtensionStatus::DeferredSeparateBranch,
            reason: "hardware_raster_or_material_resolve_not_proven_insufficient",
        };
    }
    VirtualGeometryVisibilityBufferExtensionPlan {
        status: VirtualGeometryVisibilityBufferExtensionStatus::CandidateAfterHardwarePathFails,
        reason: "separate_branch_after_hardware_path_measurement",
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VirtualGeometryBenchmarkSample {
    pub standard_dense_draw_calls: u32,
    pub funvg_draw_calls: u32,
    pub standard_cpu_submission_ns: u64,
    pub funvg_cpu_submission_ns: u64,
    pub p95_net_effect_ns: i64,
    pub upload_bytes: u64,
    pub upload_budget_bytes: u64,
    pub missing_pages: u32,
    pub fallback_cluster_count: u32,
    pub cef_or_present_pacing_artifact_detected: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VirtualGeometryAcceptanceReport {
    pub fewer_draw_calls: bool,
    pub lower_cpu_submission: bool,
    pub stable_p95: bool,
    pub bounded_upload_spikes: bool,
    pub no_cef_or_present_pacing_artifact: bool,
    pub explainable_fallback_behavior: bool,
    pub accepted: bool,
    pub keep_opt_in: bool,
}

impl VirtualGeometryAcceptanceReport {
    pub const fn from_benchmark_sample(sample: VirtualGeometryBenchmarkSample) -> Self {
        let fewer_draw_calls = sample.funvg_draw_calls < sample.standard_dense_draw_calls;
        let lower_cpu_submission =
            sample.funvg_cpu_submission_ns < sample.standard_cpu_submission_ns;
        let stable_p95 = sample.p95_net_effect_ns <= 0;
        let bounded_upload_spikes = sample.upload_bytes <= sample.upload_budget_bytes;
        let no_cef_or_present_pacing_artifact = !sample.cef_or_present_pacing_artifact_detected;
        let explainable_fallback_behavior =
            sample.missing_pages == 0 || sample.fallback_cluster_count >= sample.missing_pages;
        let accepted = fewer_draw_calls
            && lower_cpu_submission
            && stable_p95
            && bounded_upload_spikes
            && no_cef_or_present_pacing_artifact
            && explainable_fallback_behavior;
        Self {
            fewer_draw_calls,
            lower_cpu_submission,
            stable_p95,
            bounded_upload_spikes,
            no_cef_or_present_pacing_artifact,
            explainable_fallback_behavior,
            accepted,
            keep_opt_in: !accepted,
        }
    }
}

fn score_f32(value: f32, min: f32, max: f32, scale: i32) -> i32 {
    if !value.is_finite() || max <= min {
        return 0;
    }
    let normalized = ((value.clamp(min, max) - min) / (max - min)).clamp(0.0, 1.0);
    (normalized * scale as f32) as i32
}

fn score_inverse_distance(distance_meters: f32) -> i32 {
    if !distance_meters.is_finite() {
        return 0;
    }
    let distance = distance_meters.max(0.0);
    ((1.0 / (1.0 + distance / 50.0)) * 1_500.0) as i32
}

fn saturating_u32(value: usize) -> u32 {
    value.min(u32::MAX as usize) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    const ASSET_ID: VirtualGeometryAssetId = VirtualGeometryAssetId(1);
    const ROOT_PAGE: VirtualGeometryPageId = VirtualGeometryPageId(10);
    const CHILD_PAGE_A: VirtualGeometryPageId = VirtualGeometryPageId(20);
    const CHILD_PAGE_B: VirtualGeometryPageId = VirtualGeometryPageId(30);
    const ROOT_CLUSTER: VirtualGeometryClusterId = VirtualGeometryClusterId(100);
    const CHILD_CLUSTER_A: VirtualGeometryClusterId = VirtualGeometryClusterId(200);
    const CHILD_CLUSTER_B: VirtualGeometryClusterId = VirtualGeometryClusterId(300);
    const ROOT_NODE: VirtualGeometryHierarchyNodeId = VirtualGeometryHierarchyNodeId(1);
    const CHILD_NODE_A: VirtualGeometryHierarchyNodeId = VirtualGeometryHierarchyNodeId(2);
    const CHILD_NODE_B: VirtualGeometryHierarchyNodeId = VirtualGeometryHierarchyNodeId(3);

    fn cluster(
        id: VirtualGeometryClusterId,
        page_id: VirtualGeometryPageId,
        material_id: u32,
        material_signature: FunMaterialSignature,
        triangle_range: VirtualGeometryRange,
        bounds_radius: f32,
        cluster_error: f32,
        parent_cluster_id: Option<VirtualGeometryClusterId>,
        child_range: VirtualGeometryChildRange,
        occluder: bool,
    ) -> VirtualGeometryCluster {
        VirtualGeometryCluster {
            id,
            page_id,
            material_id,
            material_signature,
            local_bounds: VirtualGeometryBounds::new([0.0, 0.0, 0.0], bounds_radius),
            world_bounds: VirtualGeometryBounds::new([0.0, 0.0, 0.0], bounds_radius),
            triangle_range,
            index_range: VirtualGeometryRange::new(
                triangle_range.first * 3,
                triangle_range.count * 3,
            ),
            normal_cone: VirtualGeometryNormalCone::new([0.0, 1.0, 0.0], 0.25),
            parent_cluster_id,
            child_range,
            triangle_count: triangle_range.count,
            bounds_radius,
            cluster_error,
            screen_error: cluster_error,
            screen_area: 0.5,
            distance_meters: 24.0,
            occluder,
            material_id_count: 1,
            cull_hint: VirtualGeometryClusterCullHint::Visible,
        }
    }

    fn test_asset() -> VirtualGeometryAsset {
        VirtualGeometryAsset {
            schema_version: FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION,
            asset_id: ASSET_ID,
            root_node: ROOT_NODE,
            kind: VirtualGeometryAssetKind::StaticOpaqueDense,
            lite_policy: VirtualGeometryLitePolicy::default(),
            pages: vec![
                VirtualGeometryPage {
                    id: ROOT_PAGE,
                    compressed_bytes: 128 * 1024,
                    first_cluster: 0,
                    cluster_count: 1,
                },
                VirtualGeometryPage {
                    id: CHILD_PAGE_A,
                    compressed_bytes: 256 * 1024,
                    first_cluster: 1,
                    cluster_count: 1,
                },
                VirtualGeometryPage {
                    id: CHILD_PAGE_B,
                    compressed_bytes: 256 * 1024,
                    first_cluster: 2,
                    cluster_count: 1,
                },
            ],
            clusters: vec![
                cluster(
                    ROOT_CLUSTER,
                    ROOT_PAGE,
                    7,
                    FunMaterialSignature(7),
                    VirtualGeometryRange::new(0, 2_048),
                    8.0,
                    0.08,
                    None,
                    VirtualGeometryChildRange::new(0, 2),
                    true,
                ),
                cluster(
                    CHILD_CLUSTER_A,
                    CHILD_PAGE_A,
                    7,
                    FunMaterialSignature(7),
                    VirtualGeometryRange::new(2_048, 1_024),
                    4.0,
                    0.005,
                    Some(ROOT_CLUSTER),
                    VirtualGeometryChildRange::EMPTY,
                    true,
                ),
                cluster(
                    CHILD_CLUSTER_B,
                    CHILD_PAGE_B,
                    8,
                    FunMaterialSignature(8),
                    VirtualGeometryRange::new(3_072, 1_024),
                    4.0,
                    0.005,
                    Some(ROOT_CLUSTER),
                    VirtualGeometryChildRange::EMPTY,
                    false,
                ),
            ],
            hierarchy_nodes: vec![
                VirtualGeometryHierarchyNode {
                    id: ROOT_NODE,
                    cluster_id: ROOT_CLUSTER,
                    parent: None,
                    child_range: VirtualGeometryChildRange::new(0, 2),
                    children: vec![CHILD_NODE_A, CHILD_NODE_B],
                },
                VirtualGeometryHierarchyNode {
                    id: CHILD_NODE_A,
                    cluster_id: CHILD_CLUSTER_A,
                    parent: Some(ROOT_NODE),
                    child_range: VirtualGeometryChildRange::EMPTY,
                    children: Vec::new(),
                },
                VirtualGeometryHierarchyNode {
                    id: CHILD_NODE_B,
                    cluster_id: CHILD_CLUSTER_B,
                    parent: Some(ROOT_NODE),
                    child_range: VirtualGeometryChildRange::EMPTY,
                    children: Vec::new(),
                },
            ],
            alternate_cluster_sets: 0,
            material_id_count: 2,
            triangle_count: 4_096,
        }
    }

    fn context() -> VirtualGeometryViewContext {
        VirtualGeometryViewContext {
            frame_index: 42,
            lane: FunDrawBudgetLane::StreamingSpike,
            projected_bounds_radius_pixels: 512.0,
            camera_distance_meters: 24.0,
            page_budget: VirtualGeometryPageBudget {
                max_page_requests_per_frame: 8,
                max_upload_bytes_per_frame: 2 * 1024 * 1024,
            },
            ..Default::default()
        }
    }

    #[test]
    fn runtime_pipeline_order_is_stable() {
        let stages = FUN_VG_RUNTIME_PIPELINE
            .iter()
            .map(|stage| stage.as_str())
            .collect::<Vec<_>>();

        assert_eq!(
            stages,
            [
                "load_asset_metadata",
                "select_candidate_roots",
                "traverse_screen_space_error",
                "request_missing_pages",
                "use_fallback_parent_clusters",
                "cull_selected_clusters",
                "compact_visible_clusters",
                "emit_indirect_draw_packets",
                "submit_indirect_raster_buckets"
            ]
        );
    }

    #[test]
    fn screen_space_error_splits_when_child_pages_are_resident() {
        let asset = test_asset();
        let mut residency =
            VirtualGeometryResidency::with_resident_pages([ROOT_PAGE, CHILD_PAGE_A, CHILD_PAGE_B]);

        let selection = select_virtual_geometry_view(
            &asset,
            &mut residency,
            context(),
            VirtualGeometrySelectionPolicy::default(),
        );

        assert_eq!(selection.selected_clusters.len(), 2);
        assert_eq!(selection.fallback_clusters.len(), 0);
        assert_eq!(
            selection
                .selected_clusters
                .iter()
                .map(|cluster| cluster.cluster_id)
                .collect::<Vec<_>>(),
            [CHILD_CLUSTER_A, CHILD_CLUSTER_B]
        );
    }

    #[test]
    fn funvg_lite_policy_is_static_opaque_offline_and_indirect_raster() {
        let asset = test_asset();

        assert_eq!(asset.schema_version, FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION);
        assert!(asset.lite_policy.static_opaque_only);
        assert!(asset.lite_policy.baked_offline);
        assert!(asset.lite_policy.gpu_culling_required);
        assert!(asset.lite_policy.indirect_raster_first);
        assert!(!asset.lite_policy.software_visibility_buffer_enabled);
        assert!(!asset.lite_policy.mesh_shaders_enabled);
        assert!(!asset.lite_policy.bindless_mega_system_enabled);
    }

    #[test]
    fn static_cell_visibility_gates_cluster_selection() {
        let asset = test_asset();
        let mut residency =
            VirtualGeometryResidency::with_resident_pages([ROOT_PAGE, CHILD_PAGE_A, CHILD_PAGE_B]);
        let hidden_cell = VirtualGeometryStaticCell {
            cell_id: 77,
            asset_id: ASSET_ID,
            world_bounds: VirtualGeometryBounds::new([0.0, 0.0, 0.0], 16.0),
            visible: false,
        };

        let selection = select_virtual_geometry_for_static_cell(
            hidden_cell,
            &asset,
            &mut residency,
            context(),
            VirtualGeometrySelectionPolicy::default(),
        );

        assert!(selection.selected_clusters.is_empty());
        assert!(selection.draw_packets.is_empty());
    }

    #[test]
    fn missing_child_page_requests_page_and_draws_parent_fallback() {
        let asset = test_asset();
        let mut residency = VirtualGeometryResidency::with_resident_pages([ROOT_PAGE]);

        let selection = select_virtual_geometry_view(
            &asset,
            &mut residency,
            context(),
            VirtualGeometrySelectionPolicy::default(),
        );

        assert_eq!(selection.selected_clusters.len(), 1);
        assert_eq!(selection.selected_clusters[0].cluster_id, ROOT_CLUSTER);
        assert_eq!(selection.fallback_clusters, [ROOT_CLUSTER]);
        assert_eq!(selection.requested_pages.len(), 2);
        assert_eq!(selection.missing_pages, [CHILD_PAGE_A, CHILD_PAGE_B]);
        assert_eq!(selection.draw_packets.len(), 1);
        assert_eq!(selection.draw_packets[0].cluster_count, 1);
    }

    #[test]
    fn residency_throttles_page_requests_per_frame() {
        let asset = test_asset();
        let mut residency = VirtualGeometryResidency::with_resident_pages([ROOT_PAGE]);
        let selection = select_virtual_geometry_view(
            &asset,
            &mut residency,
            VirtualGeometryViewContext {
                page_budget: VirtualGeometryPageBudget {
                    max_page_requests_per_frame: 1,
                    max_upload_bytes_per_frame: 2 * 1024 * 1024,
                },
                ..context()
            },
            VirtualGeometrySelectionPolicy::default(),
        );

        assert_eq!(selection.requested_pages.len(), 1);
        assert_eq!(selection.deferred_page_requests, 1);
        assert_eq!(selection.upload_bytes_requested, 256 * 1024);
    }

    #[test]
    fn draw_packets_group_by_material_and_page() {
        let asset = test_asset();
        let visible = [
            VirtualGeometrySelectedCluster {
                cluster_id: CHILD_CLUSTER_A,
                page_id: CHILD_PAGE_A,
                screen_space_error_pixels: 0.5,
                selection_reason: VirtualGeometrySelectionReason::Leaf,
                fallback_parent: false,
            },
            VirtualGeometrySelectedCluster {
                cluster_id: CHILD_CLUSTER_B,
                page_id: CHILD_PAGE_B,
                screen_space_error_pixels: 0.5,
                selection_reason: VirtualGeometrySelectionReason::Leaf,
                fallback_parent: false,
            },
        ];

        let packets = build_virtual_geometry_draw_packets(&asset, &visible);

        assert_eq!(packets.len(), 2);
        assert_eq!(
            packets[0].draw_path,
            VirtualGeometryDrawPath::IndirectRaster
        );
        assert_eq!(packets[0].page_id, CHILD_PAGE_A);
        assert_eq!(packets[0].material_signature, FunMaterialSignature(7));
        assert_eq!(
            packets[0].bucket_key.meshlet_format,
            FunMeshletFormat::ClusterMesh
        );
        assert_eq!(packets[1].material_signature, FunMaterialSignature(8));
    }

    #[test]
    fn culling_removes_occluded_clusters_before_packets() {
        let mut asset = test_asset();
        asset.clusters[1].cull_hint = VirtualGeometryClusterCullHint::Occluded;
        let mut residency =
            VirtualGeometryResidency::with_resident_pages([ROOT_PAGE, CHILD_PAGE_A, CHILD_PAGE_B]);

        let selection = select_virtual_geometry_view(
            &asset,
            &mut residency,
            context(),
            VirtualGeometrySelectionPolicy::default(),
        );

        assert_eq!(selection.selected_clusters.len(), 2);
        assert_eq!(selection.visible_clusters.len(), 1);
        assert_eq!(selection.culled_clusters, 1);
        assert_eq!(selection.draw_packets.len(), 1);
        assert_eq!(selection.draw_packets[0].cluster_ids, [CHILD_CLUSTER_B]);
    }

    #[test]
    fn material_diversity_can_prevent_cluster_splitting() {
        let mut asset = test_asset();
        asset.clusters[1].material_id_count = 9;
        let mut residency =
            VirtualGeometryResidency::with_resident_pages([ROOT_PAGE, CHILD_PAGE_A, CHILD_PAGE_B]);

        let selection = select_virtual_geometry_view(
            &asset,
            &mut residency,
            context(),
            VirtualGeometrySelectionPolicy::default(),
        );

        assert_eq!(selection.selected_clusters[0].cluster_id, ROOT_CLUSTER);
        assert_eq!(selection.material_split_avoided, 1);
        assert!(selection.draw_packets.len() == 1);
    }

    #[test]
    fn foliage_and_alpha_heavy_assets_are_poor_funvg_candidates() {
        let mut foliage = test_asset();
        foliage.kind = VirtualGeometryAssetKind::Foliage;
        let mut alpha = test_asset();
        alpha.kind = VirtualGeometryAssetKind::AlphaHeavyAggregate;

        assert_eq!(
            evaluate_virtual_geometry_asset(&foliage),
            VirtualGeometryAssetDecision::PoorAggregateGeometry
        );
        assert_eq!(
            evaluate_virtual_geometry_asset(&alpha),
            VirtualGeometryAssetDecision::PoorAggregateGeometry
        );
    }

    #[test]
    fn funvg_lite_rejects_runtime_or_non_static_opaque_assets() {
        let mut runtime_asset = test_asset();
        runtime_asset.lite_policy.baked_offline = false;
        let mut non_static_asset = test_asset();
        non_static_asset.lite_policy.static_opaque_only = false;

        assert_eq!(
            evaluate_virtual_geometry_asset(&runtime_asset),
            VirtualGeometryAssetDecision::NotBakedOffline
        );
        assert_eq!(
            evaluate_virtual_geometry_asset(&non_static_asset),
            VirtualGeometryAssetDecision::NonStaticOpaqueUnsupported
        );
    }

    #[test]
    fn static_destroyed_chunks_can_use_alternate_cluster_sets() {
        let mut asset = test_asset();
        asset.kind = VirtualGeometryAssetKind::StaticDestroyedAlternateClusterSet;
        asset.alternate_cluster_sets = 3;

        assert_eq!(
            evaluate_virtual_geometry_asset(&asset),
            VirtualGeometryAssetDecision::Candidate
        );
        assert_eq!(asset.alternate_cluster_sets, 3);
    }

    #[test]
    fn arbitrary_runtime_fracture_uses_different_path_until_proven() {
        let mut asset = test_asset();
        asset.kind = VirtualGeometryAssetKind::ArbitraryRuntimeFracture;

        assert_eq!(
            evaluate_virtual_geometry_asset(&asset),
            VirtualGeometryAssetDecision::RuntimeFractureUnsupported
        );
    }

    #[test]
    fn software_visibility_buffer_extension_stays_separate_branch() {
        let not_ready = virtual_geometry_visibility_buffer_extension_plan(
            VirtualGeometryVisibilityBufferExtensionInput {
                hardware_raster_reduces_draw_calls_enough: false,
                material_resolve_stable: true,
                gpu_time_wins_dense_scenes: true,
                compute_culling_and_indirect_ready: false,
            },
        );
        let candidate = virtual_geometry_visibility_buffer_extension_plan(
            VirtualGeometryVisibilityBufferExtensionInput {
                hardware_raster_reduces_draw_calls_enough: false,
                material_resolve_stable: true,
                gpu_time_wins_dense_scenes: true,
                compute_culling_and_indirect_ready: true,
            },
        );

        assert_eq!(
            not_ready.status,
            VirtualGeometryVisibilityBufferExtensionStatus::DeferredSeparateBranch
        );
        assert_eq!(
            candidate.status,
            VirtualGeometryVisibilityBufferExtensionStatus::CandidateAfterHardwarePathFails
        );
    }

    #[test]
    fn acceptance_requires_draw_cpu_p95_upload_and_artifact_proof() {
        let report = VirtualGeometryAcceptanceReport::from_benchmark_sample(
            VirtualGeometryBenchmarkSample {
                standard_dense_draw_calls: 600,
                funvg_draw_calls: 80,
                standard_cpu_submission_ns: 2_500_000,
                funvg_cpu_submission_ns: 500_000,
                p95_net_effect_ns: -700_000,
                upload_bytes: 1024 * 1024,
                upload_budget_bytes: 2 * 1024 * 1024,
                missing_pages: 2,
                fallback_cluster_count: 2,
                cef_or_present_pacing_artifact_detected: false,
            },
        );

        assert!(report.fewer_draw_calls);
        assert!(report.lower_cpu_submission);
        assert!(report.stable_p95);
        assert!(report.bounded_upload_spikes);
        assert!(report.no_cef_or_present_pacing_artifact);
        assert!(report.explainable_fallback_behavior);
        assert!(report.accepted);
        assert!(!report.keep_opt_in);
    }

    #[test]
    fn acceptance_rejects_hidden_cef_or_present_pacing_artifact() {
        let report = VirtualGeometryAcceptanceReport::from_benchmark_sample(
            VirtualGeometryBenchmarkSample {
                standard_dense_draw_calls: 600,
                funvg_draw_calls: 80,
                standard_cpu_submission_ns: 2_500_000,
                funvg_cpu_submission_ns: 500_000,
                p95_net_effect_ns: -700_000,
                upload_bytes: 1024 * 1024,
                upload_budget_bytes: 2 * 1024 * 1024,
                missing_pages: 0,
                fallback_cluster_count: 0,
                cef_or_present_pacing_artifact_detected: true,
            },
        );

        assert!(!report.no_cef_or_present_pacing_artifact);
        assert!(!report.accepted);
        assert!(report.keep_opt_in);
    }
}
