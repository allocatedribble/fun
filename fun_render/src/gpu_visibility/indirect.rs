use std::collections::BTreeMap;

use crate::{
    FunDrawPacketBuildOptions, FunDrawPacketReport, FunDrawPacketSource, FunGpuDrawIndirectArgs,
    build_draw_packets_from_compacted_ids,
};

use super::types::{
    GpuDrawBucket, GpuObjectRecord, GpuVisibilityObjectId, draw_bucket_key_for_object,
};

#[derive(Debug, Clone, PartialEq)]
pub struct GpuIndirectBuildOutput {
    pub draw_packet_report: FunDrawPacketReport,
    pub draw_buckets: Vec<GpuDrawBucket>,
    pub indirect_args: Vec<FunGpuDrawIndirectArgs>,
    pub cpu_submit_count: u32,
    pub visible_instance_count: u32,
}

pub fn build_indirect_buckets_for_static_opaque(
    objects: &[GpuObjectRecord],
    visible_object_ids: &[GpuVisibilityObjectId],
    compacted_visible_instance_ids: &[u32],
    options: FunDrawPacketBuildOptions,
) -> GpuIndirectBuildOutput {
    let objects_by_id = objects
        .iter()
        .filter(|object| object.is_static_opaque_world())
        .map(|object| (object.object_id(), *object))
        .collect::<BTreeMap<_, _>>();

    let mut sources = Vec::new();
    for object_id in visible_object_ids {
        let Some(object) = objects_by_id.get(object_id).copied() else {
            continue;
        };
        let bucket_key = draw_bucket_key_for_object(object);
        let meshlet_count = object.mesh_cluster_range().count.max(1);
        for instance_id in object.instance_range().start..object.instance_range().end() {
            sources.push(FunDrawPacketSource {
                instance_id,
                bucket_key,
                meshlet_count,
            });
        }
    }

    let draw_packet_report =
        build_draw_packets_from_compacted_ids(&sources, compacted_visible_instance_ids, options);
    let indirect_args = draw_packet_report
        .packets
        .iter()
        .map(|packet| FunGpuDrawIndirectArgs {
            vertex_count_per_instance: 0,
            instance_count: packet.visible_instances,
            first_vertex: 0,
            first_instance: packet.compact_visible_offset,
        })
        .collect::<Vec<_>>();
    let draw_buckets = draw_packet_report
        .buckets
        .iter()
        .map(|bucket| {
            let object = objects_by_id
                .values()
                .find(|object| draw_bucket_key_for_object(**object) == bucket.key)
                .copied()
                .unwrap_or_default();
            GpuDrawBucket {
                bucket_key: bucket.key,
                material_bucket: object.material_bucket(),
                mesh_range: object.mesh_cluster_range(),
                indirect_arg_offset: bucket
                    .indirect_args_range
                    .map(|range| range.first_arg_index)
                    .unwrap_or(u32::MAX),
                visible_range: crate::InstanceRange::new(
                    bucket.compact_visible_offset,
                    bucket.compact_visible_count,
                ),
            }
        })
        .collect::<Vec<_>>();
    let cpu_submit_count = draw_packet_report
        .direct_draw_count
        .saturating_add(draw_packet_report.indirect_draw_count)
        .saturating_add(draw_packet_report.multi_draw_count);

    GpuIndirectBuildOutput {
        visible_instance_count: draw_packet_report.visible_instance_count,
        draw_packet_report,
        draw_buckets,
        indirect_args,
        cpu_submit_count,
    }
}
