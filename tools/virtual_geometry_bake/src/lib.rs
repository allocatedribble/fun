use fun_render::{
    FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION, FunMaterialSignature, VirtualGeometryAsset,
    VirtualGeometryAssetId, VirtualGeometryAssetKind, VirtualGeometryBounds,
    VirtualGeometryChildRange, VirtualGeometryCluster, VirtualGeometryClusterCullHint,
    VirtualGeometryClusterId, VirtualGeometryHierarchyNode, VirtualGeometryHierarchyNodeId,
    VirtualGeometryLitePolicy, VirtualGeometryNormalCone, VirtualGeometryPage,
    VirtualGeometryPageId, VirtualGeometryRange,
};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VirtualGeometryBakeConfig {
    pub asset_id: u64,
    pub material_id: u32,
    pub triangle_count: u32,
    pub index_count: u32,
    pub cluster_triangle_budget: u32,
    pub clusters_per_page: u32,
    pub root_page_compressed_bytes: u64,
    pub child_page_compressed_bytes: u64,
    pub bounds_radius: f32,
    pub screen_error: f32,
}

impl Default for VirtualGeometryBakeConfig {
    fn default() -> Self {
        Self {
            asset_id: 1,
            material_id: 7,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VirtualGeometryBakeError {
    TriangleCountRequired,
    IndexCountRequired,
}

pub fn bake_virtual_geometry_lite(
    config: &VirtualGeometryBakeConfig,
) -> Result<VirtualGeometryAsset, VirtualGeometryBakeError> {
    if config.triangle_count == 0 {
        return Err(VirtualGeometryBakeError::TriangleCountRequired);
    }
    if config.index_count == 0 {
        return Err(VirtualGeometryBakeError::IndexCountRequired);
    }

    let cluster_triangle_budget = config.cluster_triangle_budget.max(1);
    let child_count = config
        .triangle_count
        .div_ceil(cluster_triangle_budget)
        .max(1);
    let clusters_per_page = config.clusters_per_page.max(1);
    let page_count = child_count.div_ceil(clusters_per_page);
    let root_cluster_id = VirtualGeometryClusterId(1);
    let root_node_id = VirtualGeometryHierarchyNodeId(1);
    let root_page_id = VirtualGeometryPageId(1);

    let mut pages = Vec::with_capacity(page_count.saturating_add(1) as usize);
    pages.push(VirtualGeometryPage {
        id: root_page_id,
        compressed_bytes: config.root_page_compressed_bytes,
        first_cluster: 0,
        cluster_count: 1,
    });
    for page_index in 0..page_count {
        let first_child_cluster = 1 + page_index * clusters_per_page;
        let remaining = child_count.saturating_sub(page_index * clusters_per_page);
        pages.push(VirtualGeometryPage {
            id: VirtualGeometryPageId(u64::from(page_index) + 2),
            compressed_bytes: config.child_page_compressed_bytes,
            first_cluster: first_child_cluster,
            cluster_count: remaining.min(clusters_per_page),
        });
    }

    let material_signature = FunMaterialSignature(u64::from(config.material_id));
    let mut clusters = Vec::with_capacity(child_count.saturating_add(1) as usize);
    clusters.push(VirtualGeometryCluster {
        id: root_cluster_id,
        page_id: root_page_id,
        material_id: config.material_id,
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
        occluder: true,
        material_id_count: 1,
        cull_hint: VirtualGeometryClusterCullHint::Visible,
    });

    let mut hierarchy_nodes = Vec::with_capacity(child_count.saturating_add(1) as usize);
    let child_node_ids = (0..child_count)
        .map(|index| VirtualGeometryHierarchyNodeId(index + 2))
        .collect::<Vec<_>>();
    hierarchy_nodes.push(VirtualGeometryHierarchyNode {
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
        let x_offset = child_index as f32 - (child_count as f32 * 0.5);

        clusters.push(VirtualGeometryCluster {
            id: cluster_id,
            page_id,
            material_id: config.material_id,
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
            distance_meters: 0.0,
            occluder: false,
            material_id_count: 1,
            cull_hint: VirtualGeometryClusterCullHint::Visible,
        });
        hierarchy_nodes.push(VirtualGeometryHierarchyNode {
            id: VirtualGeometryHierarchyNodeId(child_index + 2),
            cluster_id,
            parent: Some(root_node_id),
            child_range: VirtualGeometryChildRange::EMPTY,
            children: Vec::new(),
        });
    }

    Ok(VirtualGeometryAsset {
        schema_version: FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION,
        asset_id: VirtualGeometryAssetId(config.asset_id),
        root_node: root_node_id,
        kind: VirtualGeometryAssetKind::StaticOpaqueDense,
        lite_policy: VirtualGeometryLitePolicy::default(),
        pages,
        clusters,
        hierarchy_nodes,
        alternate_cluster_sets: 0,
        material_id_count: 1,
        triangle_count: config.triangle_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_render::{
        VirtualGeometryDrawPath, VirtualGeometryResidency, VirtualGeometryViewContext,
    };

    #[test]
    fn bake_tool_outputs_static_opaque_funvg_lite_schema() {
        let asset = bake_virtual_geometry_lite(&VirtualGeometryBakeConfig::default())
            .expect("sample bake should be valid");

        assert_eq!(asset.schema_version, FUN_VIRTUAL_GEOMETRY_SCHEMA_VERSION);
        assert_eq!(asset.kind, VirtualGeometryAssetKind::StaticOpaqueDense);
        assert!(asset.lite_policy.static_opaque_only);
        assert!(asset.lite_policy.baked_offline);
        assert!(!asset.lite_policy.software_visibility_buffer_enabled);
        assert!(asset.clusters.len() > 1);
        assert_eq!(
            asset.clusters[0].child_range.child_count as usize,
            asset.clusters.len() - 1
        );
    }

    #[test]
    fn baked_asset_uses_page_residency_and_indirect_raster_packets() {
        let asset = bake_virtual_geometry_lite(&VirtualGeometryBakeConfig {
            triangle_count: 1_024,
            cluster_triangle_budget: 256,
            clusters_per_page: 2,
            ..Default::default()
        })
        .expect("bake should be valid");
        let resident_pages = asset.pages.iter().map(|page| page.id).collect::<Vec<_>>();
        let mut residency = VirtualGeometryResidency::with_resident_pages([
            resident_pages[0],
            resident_pages[1],
            resident_pages[2],
        ]);

        let selection = fun_render::select_virtual_geometry_view(
            &asset,
            &mut residency,
            VirtualGeometryViewContext::default(),
            fun_render::VirtualGeometrySelectionPolicy::default(),
        );

        assert!(!selection.draw_packets.is_empty());
        assert!(
            selection
                .draw_packets
                .iter()
                .all(|packet| packet.draw_path == VirtualGeometryDrawPath::IndirectRaster)
        );
    }

    #[test]
    fn bake_rejects_empty_geometry() {
        assert_eq!(
            bake_virtual_geometry_lite(&VirtualGeometryBakeConfig {
                triangle_count: 0,
                ..Default::default()
            }),
            Err(VirtualGeometryBakeError::TriangleCountRequired)
        );
    }
}
