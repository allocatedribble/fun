pub use fun_renderer::{
    StaticVirtualGeometryAsset, StaticVirtualGeometryBakeConfig as VirtualGeometryBakeConfig,
    StaticVirtualGeometryBakeError as VirtualGeometryBakeError, bake_static_virtual_geometry,
};

pub fn bake_virtual_geometry_lite(
    config: &VirtualGeometryBakeConfig,
) -> Result<StaticVirtualGeometryAsset, VirtualGeometryBakeError> {
    bake_static_virtual_geometry(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fun_renderer::{
        PageOwner, PageOwnerFrameRequests, PagePriorityInputs, PageRequest, PageScheduler,
        PageSchedulerConfig, StaticVirtualGeometryRuntimePolicy, StaticVirtualGeometryViewContext,
        VirtualGeometryAssetDecision, VirtualGeometryDrawPath,
        select_static_virtual_geometry_frame,
    };

    #[test]
    fn bake_tool_outputs_static_opaque_funvg_schema() {
        let asset = bake_virtual_geometry_lite(&VirtualGeometryBakeConfig::default())
            .expect("sample bake should be valid");

        assert_eq!(
            fun_renderer::evaluate_static_virtual_geometry_asset(&asset),
            VirtualGeometryAssetDecision::Candidate
        );
        assert!(asset.header.tool_metadata.deterministic);
        assert!(asset.checksum_valid());
        assert!(asset.clusters.len() > 1);
        assert_eq!(
            asset.clusters[0].child_range.child_count as usize,
            asset.clusters.len() - 1
        );
    }

    #[test]
    fn bake_tool_outputs_deterministic_json() {
        let config = VirtualGeometryBakeConfig {
            triangle_count: 1_024,
            cluster_triangle_budget: 256,
            clusters_per_page: 2,
            ..Default::default()
        };
        let first = bake_virtual_geometry_lite(&config).expect("bake should be valid");
        let second = bake_virtual_geometry_lite(&config).expect("bake should be valid");

        assert_eq!(
            serde_json::to_vec_pretty(&first).expect("first asset should serialize"),
            serde_json::to_vec_pretty(&second).expect("second asset should serialize")
        );
    }

    #[test]
    fn baked_asset_uses_shared_page_scheduler_and_indirect_packets() {
        let asset = bake_virtual_geometry_lite(&VirtualGeometryBakeConfig {
            triangle_count: 1_024,
            cluster_triangle_budget: 256,
            clusters_per_page: 2,
            ..Default::default()
        })
        .expect("bake should be valid");
        let mut scheduler = PageScheduler::new(PageSchedulerConfig {
            physical_slot_count: 16,
            max_upload_bytes_per_frame: 8 * 1024 * 1024,
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
            assert!(scheduler.complete_upload(page.logical_page_id(asset.header.asset_id)));
        }

        let selection = select_static_virtual_geometry_frame(
            &asset,
            &mut scheduler,
            StaticVirtualGeometryViewContext::default(),
            StaticVirtualGeometryRuntimePolicy::default(),
        );

        assert!(!selection.draw_packets.is_empty());
        assert!(
            selection
                .draw_packets
                .iter()
                .all(|packet| packet.draw_path == VirtualGeometryDrawPath::ComputeIndirect)
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
